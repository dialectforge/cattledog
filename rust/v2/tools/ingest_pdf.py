#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
PDF Paragraph Ingestion Pipeline for Lu Brain v2.

Reads a PDF paragraph by paragraph using PyMuPDF (fitz), skips junk sections
(TOC, appendices, indexes, bibliographies, exercise sections), extracts
figure captions as standalone teachable units, and feeds each paragraph
to Lu's v2 brain via lu_teach MCP calls.

The v2 temporal lobe automatically decomposes each paragraph into typed
Knowledge Units (KUs) with structural bonds.

Usage:
    python ingest_pdf.py                          # default SLP3 PDF
    python ingest_pdf.py <pdf_path>               # specific PDF
    python ingest_pdf.py <pdf_path> --dry-run     # preview without ingesting
    python ingest_pdf.py <pdf_path> --start-page 20 --limit 500
    python ingest_pdf.py <pdf_path> --verbose

Options:
    --dry-run           Show paragraphs that would be ingested (no MCP calls)
    --start-page N      Skip pages before N (0-indexed, default: 0)
    --limit N           Stop after ingesting N paragraphs
    --save-every N      Call lu_save every N paragraphs (default: 100)
    --min-len N         Minimum paragraph character length (default: 80)
    --max-len N         Maximum paragraph character length (default: 4000)
    --verbose           Print each paragraph text as it's ingested
    --brain <path>      Override brain file path

Dependencies (install with: pip install --break-system-packages pymupdf):
    pymupdf (fitz)

MCP binary: <AI_FACTORY_ROOT>\\v2\\target\\release\\lu_mcp_v2.exe
Brain file: <AI_FACTORY_ROOT>\\v2\\brain\\lu_brain.lbr
"""

import argparse
import io
import json
import os
import queue
import re
import subprocess
import sys
import threading
import time

# Force UTF-8 output on Windows to handle Unicode in MCP responses
if sys.stdout.encoding and sys.stdout.encoding.lower() not in ("utf-8", "utf8"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
if sys.stderr.encoding and sys.stderr.encoding.lower() not in ("utf-8", "utf8"):
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace")

# ── Config ────────────────────────────────────────────────────────────────────

DEFAULT_PDF = r"<AI_FACTORY_ROOT>\tools\trainer\uploads\Speech_and_Language_Processing_3rd_ed_draft.pdf"
DEFAULT_BRAIN = r"<AI_FACTORY_ROOT>\v2\brain\lu_brain.lbr"
MCP_EXE = r"<AI_FACTORY_ROOT>\v2\target\release\lu_mcp_v2.exe"

DEFAULT_MIN_LEN = 80
DEFAULT_MAX_LEN = 4000
DEFAULT_SAVE_EVERY = 100

# ── Section exclusion patterns ────────────────────────────────────────────────

# Header patterns that indicate we should skip everything until the next chapter
EXCLUDED_SECTION_PATTERNS = [
    r'^references?\s*$',
    r'^bibliography\s*$',
    r'^works\s+cited\s*$',
    r'^bibliographic(al)?\s+notes?\s*$',
    r'^bibliographical?\s+and\s+historical\s+notes?\s*$',
    r'^further\s+reading\s*$',
    r'^additional\s+reading\s*$',
    r'^index\s*$',
    r'^subject\s+index\s*$',
    r'^author\s+index\s*$',
    r'^name\s+index\s*$',
    r'^general\s+index\s*$',
    r'^appendix\b',
    r'^appendices\s*$',
    r'^glossary\s*$',
    r'^table\s+of\s+contents\s*$',
    r'^contents\s*$',
    r'^acknowledgments?\s*$',
    r'^acknowledgements?\s*$',
    r'^preface\s*$',
    r'^exercises?\s*$',
    r'^problems?\s*$',
    r'^practice\s+exercises?\s*$',
]

# Patterns that indicate we're back to main content (numbered chapter/section)
RESUME_PATTERNS = [
    r'^\d+\s+[A-Z][a-zA-Z]',          # "3 Regular Expressions"
    r'^chapter\s+\d+',                 # "Chapter 3"
    r'^\d+\.\d+\s+[A-Z][a-zA-Z]',     # "3.1 Basic Patterns"
]

# Running header/footer patterns to strip (SLP3 specific + generic)
RUNNING_HEADER_PATTERNS = [
    r'^draft\s+of\s+\w+\s+\d+,\s+\d{4}',   # "Draft of September 19, 2024"
    r'^\d{4}\s+jurafsky\s+and\s+martin',    # copyright line
    r'^jurafsky\s+and\s+martin',
    r'^speech\s+and\s+language\s+processing',
    r'^\(\s*jurafsky\s+and\s+martin\b',
    r'^\[version\s+',
    r'^©\s*\d{4}',
    r'^\d+\s*$',                            # bare page numbers
    r'^-\s*\d+\s*-\s*$',                    # "- 12 -"
    r'^page\s+\d+\s*$',
]

# Figure caption patterns — we KEEP these
FIGURE_CAPTION_RE = re.compile(
    r'^(figure|fig\.?)\s+([\d.]+)[.:]\s*(.+)',
    re.IGNORECASE | re.DOTALL,
)

# Table caption patterns — we also keep
TABLE_CAPTION_RE = re.compile(
    r'^(table)\s+([\d.]+)[.:]\s*(.+)',
    re.IGNORECASE | re.DOTALL,
)

# Algorithm/pseudocode markers
ALGORITHM_RE = re.compile(
    r'^(algorithm|procedure|function)\s+([\d.]+|[A-Z_]+)',
    re.IGNORECASE,
)

# TOC line pattern: "Some Title . . . . . 42" or "1.3 Title ...... 42"
_TOC_LINE_RE = re.compile(r'\.{2,}[\s.]*\d+\s*$|(\. ){3,}\d+\s*$')

# Compiled exclusion patterns
_EXCLUDED_RE = [re.compile(p, re.IGNORECASE) for p in EXCLUDED_SECTION_PATTERNS]
_RESUME_RE = [re.compile(p, re.IGNORECASE) for p in RESUME_PATTERNS]
_RUNNING_RE = [re.compile(p, re.IGNORECASE) for p in RUNNING_HEADER_PATTERNS]


# ── Section detection ─────────────────────────────────────────────────────────

def is_excluded_header(text: str) -> bool:
    """
    Return True if `text` is (or starts with) a header for a section we skip.
    Checks the first word/line, not just a period-split fragment.
    """
    # Check just the first line (newline-split)
    first_line = text.split('\n')[0].strip()
    # Also check the first word token for simple keywords
    first_words = first_line.split()[:3]
    first_token = first_words[0].lower() if first_words else ""

    # Quick single-word checks
    if first_token in ("references", "bibliography", "index", "glossary",
                       "exercises", "problems", "appendix", "appendices",
                       "acknowledgments", "acknowledgements", "preface"):
        return True

    # Check "Summary of Contents", "Table of Contents", "Contents"
    if first_line.lower().startswith(("contents", "table of contents",
                                      "summary of contents")):
        return True

    # Full regex check against the first line
    for rx in _EXCLUDED_RE:
        if rx.match(first_line):
            return True

    return False


def is_toc_block(text: str) -> bool:
    """Return True if the text block looks like a Table of Contents page."""
    # Count lines containing dot-leader patterns
    lines = text.splitlines() if '\n' in text else [text]
    # For single-line blocks, check inline TOC patterns
    toc_matches = sum(1 for ln in lines if _TOC_LINE_RE.search(ln))
    if toc_matches >= 2:
        return True
    # Also catch concatenated TOC entries like " . . . . . . 294 14.2 ... 295"
    if len(re.findall(r'(?:\. ){3,}', text)) >= 2:
        return True
    return False


def is_resume_header(text: str) -> bool:
    """Return True if `text` looks like a main chapter/section heading."""
    first_line = text.split('\n')[0].strip()
    for rx in _RESUME_RE:
        if rx.match(first_line):
            return True
    return False


def is_running_header(line: str) -> bool:
    """Return True if `line` is a running page header/footer."""
    stripped = line.strip().lower()
    for rx in _RUNNING_RE:
        if rx.match(stripped):
            return True
    return False


# ── Paragraph extraction ──────────────────────────────────────────────────────

def clean_line(line: str) -> str:
    """Normalize whitespace in a single line."""
    return re.sub(r'\s+', ' ', line).strip()


def is_likely_noise(line: str) -> bool:
    """Return True for lines that are clearly noise (very short, standalone)."""
    stripped = line.strip()
    # Bare numbers (page numbers)
    if re.fullmatch(r'\d{1,4}', stripped):
        return True
    # TOC-style lines: "1.3  Tokenization .... 42"
    if re.search(r'\.{4,}', stripped):
        return True
    # Roman numeral page numbers
    if re.fullmatch(r'[ivxlcdmIVXLCDM]+', stripped):
        return True
    # Lone punctuation
    if len(stripped) < 3:
        return True
    return False


def extract_paragraphs_from_text(raw_text: str) -> list[dict]:
    """
    Split raw page text into paragraphs.

    Returns a list of dicts:
        {
          "text": str,
          "is_figure_caption": bool,
          "is_table_caption": bool,
          "is_algorithm": bool,
        }
    """
    paragraphs = []

    # Split on blank lines to get candidate blocks
    blocks = re.split(r'\n\s*\n', raw_text)

    for block in blocks:
        # Clean individual lines, drop running headers
        lines = []
        for line in block.splitlines():
            cleaned = clean_line(line)
            if not cleaned:
                continue
            if is_running_header(cleaned):
                continue
            if is_likely_noise(cleaned):
                continue
            lines.append(cleaned)

        if not lines:
            continue

        text = ' '.join(lines).strip()
        if not text:
            continue

        # Space-density filter: skip paragraphs where words are concatenated
        # (pdfplumber artifact / multi-column layout corruption)
        space_density = text.count(' ') / max(len(text), 1)
        if space_density < 0.08:
            continue

        # Minimum word count filter
        if len(text.split()) < 10:
            continue

        is_fig = bool(FIGURE_CAPTION_RE.match(text))
        is_tab = bool(TABLE_CAPTION_RE.match(text))
        is_alg = bool(ALGORITHM_RE.match(text))

        paragraphs.append({
            "text": text,
            "is_figure_caption": is_fig,
            "is_table_caption": is_tab,
            "is_algorithm": is_alg,
        })

    return paragraphs


# ── PDF iteration ─────────────────────────────────────────────────────────────

def _has_enough_spaces(text: str) -> bool:
    """Return False if the text looks like concatenated words (no spaces).

    Paragraphs extracted from academic PDFs with bad word-spacing will have
    a space-density below 0.08 — these are Raw junk, not real prose.
    """
    return text.count(' ') / max(len(text), 1) >= 0.08


def _has_enough_words(text: str) -> bool:
    """Return False if the text has fewer than 10 words."""
    return len(text.split()) >= 10


def iter_pdf_paragraphs(pdf_path: str, start_page: int = 0, verbose: bool = False):
    """
    Yield filtered paragraphs from a PDF.

    Uses PyMuPDF (fitz) for extraction — handles word spacing correctly for
    academic PDFs.  Skips excluded sections (TOC, appendix, bibliography,
    index, etc.).
    Yields dicts: {"text": str, "page": int, "section": str, "is_figure": bool, ...}
    """
    try:
        import fitz  # PyMuPDF
    except ImportError:
        print("ERROR: pymupdf not found. Install with:")
        print("  pip install --break-system-packages pymupdf")
        sys.exit(1)

    in_excluded = False
    current_section = "preamble"

    doc = fitz.open(pdf_path)
    total_pages = len(doc)
    print(f"  PDF: {os.path.basename(pdf_path)}")
    print(f"  Pages: {total_pages}")

    try:
        for page_num in range(total_pages):
            if page_num < start_page:
                continue

            if verbose and page_num % 20 == 0:
                print(f"  [page {page_num + 1}/{total_pages}]  section={current_section!r}  excluded={in_excluded}")

            try:
                page = doc[page_num]
                page_text = page.get_text("text") or ""
            except Exception as e:
                print(f"  WARNING: Failed to extract page {page_num + 1}: {e}")
                continue

            if not page_text.strip():
                continue

            paragraphs = extract_paragraphs_from_text(page_text)

            for para in paragraphs:
                text = para["text"]

                # Space-density filter: skip concatenated-word garbage
                if not _has_enough_spaces(text):
                    if verbose:
                        print(f"  [no-spaces] {text[:60]}...")
                    continue

                # Minimum word count filter
                if not _has_enough_words(text):
                    if verbose:
                        print(f"  [too-few-words] {text[:60]}...")
                    continue

                # Always skip TOC-style blocks regardless of section state
                if is_toc_block(text):
                    if verbose:
                        print(f"  [toc-block] {text[:60]}...")
                    continue

                # Check for section header transitions
                if is_excluded_header(text):
                    in_excluded = True
                    current_section = text[:60]
                    if verbose:
                        print(f"  [skip-section] entering excluded: {current_section!r}")
                    continue

                if in_excluded:
                    if is_resume_header(text):
                        in_excluded = False
                        current_section = text.split('\n')[0].strip()[:60]
                        if verbose:
                            print(f"  [resume] back to content: {current_section!r}")
                    else:
                        continue  # still in excluded section

                # Update current section tracking (non-excluded)
                first_line = text.split('\n')[0].strip()
                if is_resume_header(first_line):
                    current_section = first_line[:60]

                yield {
                    "text": text,
                    "page": page_num + 1,
                    "section": current_section,
                    "is_figure_caption": para["is_figure_caption"],
                    "is_table_caption": para["is_table_caption"],
                    "is_algorithm": para["is_algorithm"],
                }
    finally:
        doc.close()


# ── MCP client ────────────────────────────────────────────────────────────────

class McpClient:
    """Thin JSON-RPC 2.0 client for lu_mcp_v2.exe."""

    def __init__(self, exe_path: str, brain_path: str):
        self.proc = subprocess.Popen(
            [exe_path, brain_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            bufsize=0,
        )
        self._q: queue.Queue = queue.Queue()
        self._req_id = 0
        self._lock = threading.Lock()

        # Background stdout reader — avoids Windows deadlock
        self._reader = threading.Thread(target=self._read_loop, daemon=True)
        self._reader.start()

        # Background stderr reader — suppress noise
        self._err_reader = threading.Thread(target=self._err_loop, daemon=True)
        self._err_reader.start()

        time.sleep(0.8)
        if self.proc.poll() is not None:
            stderr_out = ""
            try:
                stderr_out = self.proc.stderr.read(2000).decode("utf-8", errors="replace")
            except Exception:
                pass
            raise RuntimeError(
                f"lu_mcp_v2.exe exited immediately (code {self.proc.returncode}). "
                f"stderr: {stderr_out}"
            )

    def _read_loop(self):
        try:
            for raw in self.proc.stdout:
                line = raw.decode("utf-8", errors="replace").strip()
                if line:
                    try:
                        self._q.put(json.loads(line))
                    except json.JSONDecodeError:
                        pass  # ignore non-JSON (startup messages)
        except Exception:
            pass

    def _err_loop(self):
        try:
            for raw in self.proc.stderr:
                line = raw.decode("utf-8", errors="replace").strip()
                if line and not line.startswith("[lu-mcp"):
                    # Only surface unexpected stderr
                    pass
        except Exception:
            pass

    def _next_id(self) -> int:
        with self._lock:
            self._req_id += 1
            return self._req_id

    def send(self, method: str, params: dict, timeout: float = 60.0) -> dict:
        """Send a JSON-RPC request and return the response dict."""
        req_id = self._next_id()
        request = {
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params,
        }
        data = json.dumps(request, ensure_ascii=False) + "\n"
        try:
            self.proc.stdin.write(data.encode("utf-8"))
            self.proc.stdin.flush()
        except (OSError, BrokenPipeError) as e:
            return {"error": {"message": str(e)}}

        # Wait for the matching response
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                resp = self._q.get(timeout=min(5.0, deadline - time.time()))
                # Match on id (notifications have no id)
                if resp.get("id") == req_id:
                    return resp
                # Discard stale responses (late replies from prior timed-out
                # requests).  Re-queuing them causes cascading timeouts because
                # every subsequent send() loops until its own deadline trying to
                # drain the stale entry.
                # Non-id messages (notifications) are also discarded here; they
                # carry no actionable state for the caller.
            except queue.Empty:
                pass
        return {"error": {"message": f"timeout after {timeout:.0f}s"}}

    def notify(self, method: str, params: dict):
        """Send a JSON-RPC notification (no id, no response expected)."""
        msg = {"jsonrpc": "2.0", "method": method, "params": params}
        data = json.dumps(msg, ensure_ascii=False) + "\n"
        try:
            self.proc.stdin.write(data.encode("utf-8"))
            self.proc.stdin.flush()
        except Exception:
            pass

    def tool_call(self, name: str, arguments: dict, timeout: float = 30.0) -> dict:
        """Call an MCP tool and return the parsed response."""
        return self.send("tools/call", {"name": name, "arguments": arguments}, timeout=timeout)

    def close(self):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.wait(timeout=10)
        except Exception:
            self.proc.kill()


def extract_result_text(resp: dict) -> str | None:
    """Pull the text content out of an MCP tool response."""
    if not resp or "result" not in resp:
        err = resp.get("error", {})
        return f"[error] {err.get('message', str(resp))}"
    result = resp["result"]
    if isinstance(result, dict) and "content" in result:
        parts = []
        for item in result["content"]:
            if item.get("type") == "text":
                parts.append(item.get("text", ""))
        return " | ".join(parts) if parts else str(result)
    return str(result)


# ── Ingestion ─────────────────────────────────────────────────────────────────

def ingest_paragraph(client: McpClient, text: str, verbose: bool = False) -> int:
    """
    Send a single paragraph to lu_teach.
    Returns the number of KUs learned (from the response), or 0 on error.
    """
    resp = client.tool_call("lu_teach", {
        "ku_type": "Raw",       # temporal lobe will reclassify
        "content": text,
        "confidence": "Terminal",
    }, timeout=120.0)

    result_text = extract_result_text(resp)

    if verbose:
        print(f"    -> {result_text}")

    if result_text and "error" not in result_text.lower():
        # Parse "Learned N knowledge unit(s)"
        m = re.search(r'Learned\s+(\d+)', result_text or "")
        return int(m.group(1)) if m else 1
    return 0


def save_brain(client: McpClient) -> str:
    """Call lu_save and return status message."""
    resp = client.tool_call("lu_save", {}, timeout=30.0)
    return extract_result_text(resp) or "saved"


# ── Main ──────────────────────────────────────────────────────────────────────

def parse_args():
    p = argparse.ArgumentParser(
        description="Ingest a PDF paragraph-by-paragraph into Lu Brain v2.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument("pdf", nargs="?", default=DEFAULT_PDF,
                   help="Path to the PDF file")
    p.add_argument("--brain", default=DEFAULT_BRAIN,
                   help="Path to the .lbr brain file")
    p.add_argument("--dry-run", action="store_true",
                   help="Print paragraphs without ingesting")
    p.add_argument("--start-page", type=int, default=0, metavar="N",
                   help="Start from page N (0-indexed)")
    p.add_argument("--limit", type=int, default=0, metavar="N",
                   help="Stop after ingesting N paragraphs (0 = unlimited)")
    p.add_argument("--save-every", type=int, default=DEFAULT_SAVE_EVERY, metavar="N",
                   help="Save brain every N paragraphs")
    p.add_argument("--min-len", type=int, default=DEFAULT_MIN_LEN, metavar="N",
                   help="Minimum paragraph length (chars)")
    p.add_argument("--max-len", type=int, default=DEFAULT_MAX_LEN, metavar="N",
                   help="Maximum paragraph length (chars)")
    p.add_argument("--verbose", action="store_true",
                   help="Print each paragraph and MCP response")
    return p.parse_args()


def main():
    args = parse_args()

    if not os.path.isfile(args.pdf):
        print(f"ERROR: PDF not found: {args.pdf}")
        sys.exit(1)

    if not args.dry_run and not os.path.isfile(MCP_EXE):
        print(f"ERROR: MCP server not found: {MCP_EXE}")
        sys.exit(1)

    print("=" * 70)
    print("  LU BRAIN v2 — PDF PARAGRAPH INGESTION PIPELINE")
    print("=" * 70)
    print(f"  PDF:        {args.pdf}")
    print(f"  Brain:      {args.brain}")
    print(f"  Start page: {args.start_page}")
    print(f"  Min length: {args.min_len} chars")
    print(f"  Max length: {args.max_len} chars")
    print(f"  Dry run:    {args.dry_run}")
    if args.limit:
        print(f"  Limit:      {args.limit} paragraphs")
    print()

    # ── Start MCP server (unless dry run) ─────────────────────────────────────
    client = None
    if not args.dry_run:
        print("[1/4] Starting MCP server...")
        try:
            client = McpClient(MCP_EXE, args.brain)
        except RuntimeError as e:
            print(f"  FATAL: {e}")
            sys.exit(1)
        print(f"  MCP server running (PID {client.proc.pid})")

        # Initialize the MCP session
        resp = client.send("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "ingest-pdf", "version": "1.0"},
        })
        if resp.get("error"):
            print(f"  WARNING: initialize error: {resp['error']}")
        else:
            print("  Session initialized OK")

        client.notify("notifications/initialized", {})
        time.sleep(0.3)

        # Set autonomy to Autonomous so lu_teach is allowed
        resp = client.tool_call("lu_configure", {"autonomy_tier": "Autonomous"})
        print(f"  Configure: {extract_result_text(resp)}")
    else:
        print("[1/4] DRY RUN — MCP server not started")

    # ── Iterate paragraphs ─────────────────────────────────────────────────────
    print(f"\n[2/4] Extracting paragraphs from PDF...")

    total_seen = 0
    total_skipped_short = 0
    total_skipped_long = 0
    total_ingested = 0
    total_ku = 0
    errors = 0
    start_time = time.time()
    last_save_at = 0

    for para in iter_pdf_paragraphs(args.pdf, start_page=args.start_page, verbose=args.verbose):
        text = para["text"]
        page = para["page"]

        total_seen += 1

        # Length filter
        if len(text) < args.min_len:
            total_skipped_short += 1
            continue
        if len(text) > args.max_len:
            # Split at sentence boundary
            sentences = re.split(r'(?<=[.!?])\s+', text)
            chunks = []
            current = ""
            for sent in sentences:
                if len(current) + len(sent) + 1 <= args.max_len:
                    current = (current + " " + sent).strip()
                else:
                    if current:
                        chunks.append(current)
                    current = sent
            if current:
                chunks.append(current)

            if args.verbose:
                print(f"  [p{page}] split long para into {len(chunks)} chunks")

            for chunk in chunks:
                if len(chunk) < args.min_len:
                    continue
                if args.dry_run:
                    tag = "[FIG] " if para["is_figure_caption"] else ""
                    print(f"  [p{page}] {tag}{chunk[:120]}...")
                    total_ingested += 1
                else:
                    ku_count = ingest_paragraph(client, chunk, verbose=args.verbose)
                    total_ku += ku_count
                    total_ingested += 1
                    if args.verbose:
                        print(f"  [p{page}] +{ku_count} KU | {chunk[:80]}...")
                    elif total_ingested % 50 == 0:
                        elapsed = time.time() - start_time
                        rate = total_ingested / elapsed if elapsed > 0 else 0
                        print(f"  [{total_ingested} ingested] +{total_ku} KU total | "
                              f"{rate:.1f}/sec | page {page}", flush=True)
            total_skipped_long += 1
            continue

        if args.dry_run:
            tag = ""
            if para["is_figure_caption"]:
                tag = "[FIG] "
            elif para["is_table_caption"]:
                tag = "[TAB] "
            elif para["is_algorithm"]:
                tag = "[ALG] "
            print(f"  [p{page}] {tag}{text[:120]}{'...' if len(text) > 120 else ''}")
            total_ingested += 1
        else:
            ku_count = ingest_paragraph(client, text, verbose=args.verbose)
            if ku_count == 0:
                errors += 1
            total_ku += ku_count
            total_ingested += 1

            # Progress report
            if total_ingested % 50 == 0:
                elapsed = time.time() - start_time
                rate = total_ingested / elapsed if elapsed > 0 else 0
                print(f"  [{total_ingested} ingested] +{total_ku} KU | "
                      f"{rate:.1f}/sec | page {page}", flush=True)

            # Periodic save
            if total_ingested - last_save_at >= args.save_every:
                save_result = save_brain(client)
                last_save_at = total_ingested
                print(f"  [SAVE at {total_ingested}] {save_result}", flush=True)

        if args.limit and total_ingested >= args.limit:
            print(f"  Reached --limit {args.limit}, stopping.")
            break

    # ── Final save ─────────────────────────────────────────────────────────────
    if not args.dry_run and client:
        print(f"\n[3/4] Final save...")
        save_result = save_brain(client)
        print(f"  {save_result}")

        print(f"\n[4/4] Brain status...")
        resp = client.tool_call("lu_status", {}, timeout=30.0)
        print(f"  {extract_result_text(resp)}")

        client.close()

    # ── Summary ────────────────────────────────────────────────────────────────
    elapsed = time.time() - start_time
    print(f"\n{'=' * 70}")
    print(f"  INGESTION COMPLETE")
    print(f"{'=' * 70}")
    print(f"  Paragraphs seen:     {total_seen}")
    print(f"  Skipped (too short): {total_skipped_short}")
    print(f"  Chunked (too long):  {total_skipped_long}")
    print(f"  Ingested:            {total_ingested}")
    if not args.dry_run:
        print(f"  KUs learned:         {total_ku}")
        print(f"  Errors:              {errors}")
    print(f"  Time:                {elapsed:.0f}s ({elapsed / 60:.1f}m)")
    if total_ingested > 0 and elapsed > 0:
        print(f"  Rate:                {total_ingested / elapsed:.1f} para/sec")
    print(f"{'=' * 70}")


if __name__ == "__main__":
    main()
