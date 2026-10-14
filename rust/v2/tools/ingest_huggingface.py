#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
HuggingFace Dataset Ingestion Pipeline for Lu Brain v2.

Streams text from HuggingFace's finepdfs_100BT dataset and feeds it into
Lu's brain via the existing MCP pipeline. Reuses McpClient and paragraph
filtering from ingest_pdf.py.

Dataset: HuggingFaceFW/finepdfs_100BT
  - ~30 million documents, ~100 billion tokens
  - Fields: text, id, url, token_count, language, dataset
  - License: ODC-BY

Usage:
    python ingest_huggingface.py                          # test run, 1000 docs
    python ingest_huggingface.py --limit 5000             # 5000 docs
    python ingest_huggingface.py --dry-run                # preview only
    python ingest_huggingface.py --resume                 # skip already-ingested docs
    python ingest_huggingface.py --min-edu-score 0.8 --min-tokens 500

Options:
    --limit N           Cap at N documents (default: 1000)
    --min-tokens N      Skip docs shorter than N tokens (default: 200)
    --min-edu-score N   Filter by educational quality score (default: 0.7)
    --dry-run           Preview paragraphs without ingesting
    --resume            Skip document IDs already in the training log
    --batch-size N      Save brain every N paragraphs (default: 100)
    --brain <path>      Override brain file path
    --log <path>        Override training log path
    --verbose           Print each paragraph and MCP response

Dependencies:
    pip install datasets --break-system-packages

MCP binary: <AI_FACTORY_ROOT>\\v2\\target\\release\\lu_mcp_v2.exe
Brain file: <AI_FACTORY_ROOT>\\v2\\brain\\lu_brain.lbr
"""

import argparse
import csv
import io
import os
import re
import signal
import sys
import time
from datetime import datetime

# Force UTF-8 output on Windows
if sys.stdout.encoding and sys.stdout.encoding.lower() not in ("utf-8", "utf8"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
if sys.stderr.encoding and sys.stderr.encoding.lower() not in ("utf-8", "utf8"):
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace")

# ── Paths ─────────────────────────────────────────────────────────────────────

_TOOLS_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _TOOLS_DIR)

DEFAULT_BRAIN = r"<AI_FACTORY_ROOT>\v2\brain\lu_brain.lbr"
MCP_EXE = r"<AI_FACTORY_ROOT>\v2\target\release\lu_mcp_v2.exe"
DEFAULT_LOG = r"<AI_FACTORY_ROOT>\v2\tools\hf_training_log.csv"

DATASET_NAME = "HuggingFaceFW/finepdfs_100BT"
DATASET_SPLIT = "train"

# ── Import from ingest_pdf.py ─────────────────────────────────────────────────

try:
    from ingest_pdf import (
        McpClient,
        extract_result_text,
        extract_paragraphs_from_text,
        save_brain,
        _has_enough_spaces,
        _has_enough_words,
        is_toc_block,
        is_excluded_header,
    )
except ImportError as e:
    print(f"ERROR: Could not import from ingest_pdf.py: {e}")
    print(f"  Expected ingest_pdf.py to be in: {_TOOLS_DIR}")
    sys.exit(1)

# ── Constants ─────────────────────────────────────────────────────────────────

DEFAULT_LIMIT = 1000
DEFAULT_MIN_TOKENS = 200
DEFAULT_MIN_EDU_SCORE = 0.7
DEFAULT_BATCH_SIZE = 100
MAX_PARA_LEN = 4000
MIN_PARA_LEN = 80

LOG_FIELDS = [
    "timestamp", "doc_id", "source_url", "language", "token_count",
    "paragraphs_seen", "paragraphs_ingested", "paragraphs_skipped",
    "ku_created", "brain_node_count",
]

# ── Graceful shutdown ─────────────────────────────────────────────────────────

_shutdown = False

def _handle_sigint(sig, frame):
    global _shutdown
    print("\n\n[SIGINT] Ctrl+C received — will finish current document and exit.", flush=True)
    _shutdown = True

signal.signal(signal.SIGINT, _handle_sigint)

# ── Training log ──────────────────────────────────────────────────────────────

def load_processed_ids(log_path: str) -> set:
    """Return set of doc IDs already recorded in the training log."""
    processed = set()
    if not os.path.exists(log_path):
        return processed
    try:
        with open(log_path, newline="", encoding="utf-8") as f:
            reader = csv.DictReader(f)
            for row in reader:
                if row.get("doc_id"):
                    processed.add(row["doc_id"])
    except Exception as e:
        print(f"  WARNING: Could not read training log: {e}")
    return processed


def append_log_row(log_path: str, row: dict):
    """Append one row to the CSV training log."""
    write_header = not os.path.exists(log_path)
    try:
        with open(log_path, "a", newline="", encoding="utf-8") as f:
            writer = csv.DictWriter(f, fieldnames=LOG_FIELDS)
            if write_header:
                writer.writeheader()
            writer.writerow(row)
    except Exception as e:
        print(f"  WARNING: Could not write training log: {e}")

# ── Dataset streaming ─────────────────────────────────────────────────────────

def _build_full_features():
    """
    Construct the explicit Features schema matching the actual finepdfs_100BT Parquet files.

    The dataset's Hub metadata specifies a narrow schema (9 fields), but the Parquet
    files contain 24 fields including fw_edu_scores, fw_edu_v2_scores, etc. Passing
    the full Features avoids datasets.CastError during streaming iteration.
    """
    from datasets import Features, Value, Sequence
    return Features({
        "text": Value("string"),
        "id": Value("string"),
        "dump": Value("string"),
        "url": Value("string"),
        "date": Value("string"),
        "file_path": Value("string"),
        "offset": Value("int64"),
        "token_count": Value("int64"),
        "language": Value("string"),
        "page_average_lid": Value("string"),
        "page_average_lid_score": Value("float64"),
        "full_doc_lid": Value("string"),
        "full_doc_lid_score": Value("float64"),
        "per_page_languages": Sequence(Value("string")),
        "is_truncated": Value("bool"),
        "extractor": Value("string"),
        "page_ends": Sequence(Value("int64")),
        "fw_edu_scores": Sequence(Value("float64")),
        "fw_edu_v2_scores": Sequence(Value("float64")),
        "dclm_scores": Sequence(Value("float64")),
        "ocr_quality_scores": Sequence(Value("float64")),
        "minhash_cluster_size": Value("int64"),
        "duplicate_count": Value("int64"),
        "dataset": Value("string"),
    })


def load_dataset_stream(min_tokens: int, min_edu_score: float):
    """Load finepdfs_100BT in streaming mode. Returns (dataset_iter, edu_field)."""
    try:
        from datasets import load_dataset
    except ImportError:
        print("ERROR: 'datasets' package not found.")
        print("  Install with: pip install datasets --break-system-packages")
        sys.exit(1)

    features = _build_full_features()
    # Prefer fw_edu_v2_scores (better quality signal) over fw_edu_scores
    edu_field = "fw_edu_v2_scores"
    print(f"  Educational score field: {edu_field}")
    print(f"  Loading {DATASET_NAME} in streaming mode...")
    try:
        ds = load_dataset(DATASET_NAME, split=DATASET_SPLIT, streaming=True, features=features)
    except Exception as e:
        print(f"ERROR: Failed to load dataset: {e}")
        sys.exit(1)

    return ds, edu_field


def passes_quality_filter(example: dict, min_tokens: int, min_edu_score: float, edu_field: str | None) -> bool:
    """Return True if this document passes quality thresholds."""
    token_count = example.get("token_count", 0) or 0
    if token_count < min_tokens:
        return False

    if edu_field and min_edu_score > 0:
        score = example.get(edu_field)
        # fw_edu_scores may be a list or a float
        if isinstance(score, list):
            score = score[0] if score else None
        if score is not None and float(score) < min_edu_score:
            return False

    return True

# ── Paragraph ingestion ───────────────────────────────────────────────────────

def sanitize_text(text: str) -> str:
    """
    Remove null bytes and control characters that cause Windows EINVAL on
    stdin pipes. Keeps printable ASCII, Unicode letters/symbols, newlines,
    tabs, and carriage returns.
    """
    # Remove null bytes (primary cause of [Errno 22] on Windows pipes)
    text = text.replace("\x00", "")
    # Remove other ASCII control chars except \t \n \r
    text = re.sub(r"[\x01-\x08\x0b\x0c\x0e-\x1f\x7f]", "", text)
    return text.strip()


def ingest_paragraph_with_source(
    client: McpClient,
    text: str,
    source: str,
    verbose: bool = False,
) -> int:
    """
    Send a single paragraph to lu_teach with source metadata.
    Returns the number of KUs learned, or 0 on error.
    """
    text = sanitize_text(text)
    if not text:
        return 0
    resp = client.tool_call("lu_teach", {
        "ku_type": "Raw",
        "content": text,
        "confidence": "Terminal",
        "source": source,
    }, timeout=120.0)

    result_text = extract_result_text(resp)

    if verbose:
        print(f"    -> {result_text}")

    if result_text and "error" not in result_text.lower():
        m = re.search(r'Learned\s+(\d+)', result_text or "")
        return int(m.group(1)) if m else 1
    return 0


def get_brain_node_count(client: McpClient) -> int:
    """Query lu_status and parse node count. Returns 0 if unavailable."""
    try:
        resp = client.tool_call("lu_status", {}, timeout=15.0)
        text = extract_result_text(resp) or ""
        m = re.search(r'(\d[\d,]*)\s+(?:nodes?|knowledge units?|KUs?)', text, re.IGNORECASE)
        if m:
            return int(m.group(1).replace(",", ""))
    except Exception:
        pass
    return 0


def split_long_text(text: str, max_len: int = MAX_PARA_LEN) -> list[str]:
    """Split text at sentence boundaries to stay under max_len."""
    if len(text) <= max_len:
        return [text]
    sentences = re.split(r'(?<=[.!?])\s+', text)
    chunks = []
    current = ""
    for sent in sentences:
        if len(current) + len(sent) + 1 <= max_len:
            current = (current + " " + sent).strip()
        else:
            if current:
                chunks.append(current)
            current = sent
    if current:
        chunks.append(current)
    return chunks if chunks else [text[:max_len]]


def process_document(
    example: dict,
    source_tag: str,
    client: McpClient | None,
    dry_run: bool,
    verbose: bool,
) -> tuple[int, int, int]:
    """
    Process one document: extract paragraphs and ingest each one.

    Returns (paragraphs_seen, paragraphs_ingested, ku_created).
    """
    raw_text = example.get("text") or ""
    if not raw_text.strip():
        return 0, 0, 0

    paragraphs = extract_paragraphs_from_text(raw_text)

    seen = 0
    ingested = 0
    ku_total = 0

    for para in paragraphs:
        text = para["text"]
        seen += 1

        # Basic quality checks (already done in extract_paragraphs_from_text,
        # but double-check after potential re-assembly)
        if len(text) < MIN_PARA_LEN:
            continue
        if not _has_enough_spaces(text):
            continue
        if not _has_enough_words(text):
            continue
        if is_toc_block(text):
            continue
        if is_excluded_header(text):
            continue

        # Split if too long
        chunks = split_long_text(text)

        for chunk in chunks:
            if len(chunk) < MIN_PARA_LEN:
                continue

            if dry_run:
                if verbose:
                    print(f"    [DRY] {chunk[:100]}{'...' if len(chunk) > 100 else ''}")
                ingested += 1
            else:
                ku = ingest_paragraph_with_source(client, chunk, source_tag, verbose=verbose)
                ku_total += ku
                ingested += 1

    return seen, ingested, ku_total

# ── Argument parsing ──────────────────────────────────────────────────────────

def parse_args():
    p = argparse.ArgumentParser(
        description="Stream HuggingFace finepdfs_100BT into Lu Brain v2.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument("--brain", default=DEFAULT_BRAIN,
                   help="Path to the .lbr brain file")
    p.add_argument("--log", default=DEFAULT_LOG,
                   help="Path to the CSV training log")
    p.add_argument("--limit", type=int, default=DEFAULT_LIMIT, metavar="N",
                   help=f"Cap at N documents (default: {DEFAULT_LIMIT})")
    p.add_argument("--min-tokens", type=int, default=DEFAULT_MIN_TOKENS, metavar="N",
                   help=f"Skip docs with fewer than N tokens (default: {DEFAULT_MIN_TOKENS})")
    p.add_argument("--min-edu-score", type=float, default=DEFAULT_MIN_EDU_SCORE, metavar="N",
                   help=f"Minimum educational quality score (default: {DEFAULT_MIN_EDU_SCORE})")
    p.add_argument("--batch-size", type=int, default=DEFAULT_BATCH_SIZE, metavar="N",
                   help=f"Save brain every N paragraphs (default: {DEFAULT_BATCH_SIZE})")
    p.add_argument("--dry-run", action="store_true",
                   help="Preview paragraphs without ingesting")
    p.add_argument("--resume", action="store_true",
                   help="Skip document IDs already in the training log")
    p.add_argument("--verbose", action="store_true",
                   help="Print each paragraph and MCP response")
    return p.parse_args()

# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    args = parse_args()

    print("=" * 70)
    print("  LU BRAIN v2 — HUGGINGFACE INGESTION PIPELINE")
    print("=" * 70)
    print(f"  Dataset:       {DATASET_NAME}")
    print(f"  Brain:         {args.brain}")
    print(f"  Log:           {args.log}")
    print(f"  Limit:         {args.limit} documents")
    print(f"  Min tokens:    {args.min_tokens}")
    print(f"  Min edu score: {args.min_edu_score}")
    print(f"  Batch size:    {args.batch_size} paragraphs")
    print(f"  Dry run:       {args.dry_run}")
    print(f"  Resume:        {args.resume}")
    print()

    # ── Load resume set ───────────────────────────────────────────────────────
    processed_ids: set = set()
    if args.resume:
        processed_ids = load_processed_ids(args.log)
        print(f"[RESUME] {len(processed_ids)} previously ingested documents will be skipped.")
        print()

    # ── Load dataset FIRST (takes ~60-90s; MCP would timeout if started before) ─
    print(f"[1/4] Connecting to HuggingFace dataset...")
    print(f"  (Loading dataset before starting MCP to avoid idle timeout)\n")
    ds, edu_field = load_dataset_stream(args.min_tokens, args.min_edu_score)
    print()

    # ── Start MCP server (after dataset load, so idle timeout can't fire) ─────
    client = None
    if not args.dry_run:
        if not os.path.isfile(MCP_EXE):
            print(f"ERROR: MCP server not found: {MCP_EXE}")
            sys.exit(1)

        print("[2/4] Starting MCP server...")
        try:
            client = McpClient(MCP_EXE, args.brain)
        except RuntimeError as e:
            print(f"  FATAL: {e}")
            sys.exit(1)
        print(f"  MCP server running (PID {client.proc.pid})")

        resp = client.send("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "ingest-huggingface", "version": "1.0"},
        })
        if resp.get("error"):
            print(f"  WARNING: initialize error: {resp['error']}")
        else:
            print("  Session initialized OK")

        client.notify("notifications/initialized", {})
        time.sleep(0.3)

        resp = client.tool_call("lu_configure", {"autonomy_tier": "Autonomous"})
        print(f"  Configure: {extract_result_text(resp)}")
    else:
        print("[2/4] DRY RUN — MCP server not started")
    print()

    # ── Streaming ingestion loop ──────────────────────────────────────────────
    print("[3/4] Streaming and ingesting documents...\n")

    total_docs_seen = 0
    total_docs_filtered = 0
    total_docs_skipped_resume = 0
    total_docs_ingested = 0
    total_para_seen = 0
    total_para_ingested = 0
    total_para_skipped = 0
    total_ku = 0
    para_since_save = 0
    start_time = time.time()

    try:
        for example in ds:
            if _shutdown:
                print("\n[SHUTDOWN] Stopping at user request.")
                break

            if args.limit and total_docs_ingested >= args.limit:
                print(f"\n  Reached --limit {args.limit} documents.")
                break

            total_docs_seen += 1

            # Quality filter
            if not passes_quality_filter(example, args.min_tokens, args.min_edu_score, edu_field):
                total_docs_filtered += 1
                continue

            doc_id = str(example.get("id") or f"idx_{total_docs_seen}")
            source_url = example.get("url") or ""
            language = example.get("language") or "unknown"
            token_count = example.get("token_count") or 0

            # Resume check
            if args.resume and doc_id in processed_ids:
                total_docs_skipped_resume += 1
                continue

            source_tag = f"huggingface:finepdfs_100BT:{doc_id}"

            # Process document
            p_seen, p_ingested, ku = process_document(
                example, source_tag, client, args.dry_run, args.verbose
            )

            total_para_seen += p_seen
            total_para_ingested += p_ingested
            total_para_skipped += (p_seen - p_ingested)
            total_ku += ku
            total_docs_ingested += 1
            para_since_save += p_ingested

            # Periodic brain save
            if not args.dry_run and client and para_since_save >= args.batch_size:
                save_result = save_brain(client)
                para_since_save = 0
                node_count = get_brain_node_count(client)
                print(f"  [SAVE] {save_result} | nodes={node_count}", flush=True)
            else:
                node_count = 0

            # Write log row
            append_log_row(args.log, {
                "timestamp": datetime.utcnow().isoformat(),
                "doc_id": doc_id,
                "source_url": source_url,
                "language": language,
                "token_count": token_count,
                "paragraphs_seen": p_seen,
                "paragraphs_ingested": p_ingested,
                "paragraphs_skipped": p_seen - p_ingested,
                "ku_created": ku,
                "brain_node_count": node_count,
            })

            # Progress report every 10 documents
            if total_docs_ingested % 10 == 0:
                elapsed = time.time() - start_time
                doc_rate = total_docs_ingested / elapsed if elapsed > 0 else 0
                para_rate = total_para_ingested / elapsed if elapsed > 0 else 0
                print(
                    f"  [doc {total_docs_ingested:>5}] "
                    f"seen={total_docs_seen} filtered={total_docs_filtered} "
                    f"| para_in={total_para_ingested} KU={total_ku} "
                    f"| {doc_rate:.1f} doc/s {para_rate:.1f} para/s",
                    flush=True,
                )

    except KeyboardInterrupt:
        print("\n[KeyboardInterrupt] Stopping...")

    # ── Final save ────────────────────────────────────────────────────────────
    if not args.dry_run and client:
        print(f"\n[4/4] Final save...")
        save_result = save_brain(client)
        print(f"  {save_result}")

        node_count = get_brain_node_count(client)
        print(f"  Brain nodes: {node_count}")

        resp = client.tool_call("lu_status", {}, timeout=30.0)
        print(f"  Status: {extract_result_text(resp)}")

        client.close()
    else:
        print(f"\n[4/4] No save needed (dry run or no client).")

    # ── Summary ───────────────────────────────────────────────────────────────
    elapsed = time.time() - start_time
    print(f"\n{'=' * 70}")
    print(f"  INGESTION COMPLETE")
    print(f"{'=' * 70}")
    print(f"  Documents seen:        {total_docs_seen}")
    print(f"  Documents filtered:    {total_docs_filtered}  (quality/token threshold)")
    print(f"  Documents skipped:     {total_docs_skipped_resume}  (resume)")
    print(f"  Documents ingested:    {total_docs_ingested}")
    print(f"  Paragraphs seen:       {total_para_seen}")
    print(f"  Paragraphs ingested:   {total_para_ingested}")
    print(f"  Paragraphs skipped:    {total_para_skipped}")
    if not args.dry_run:
        print(f"  KUs created:           {total_ku}")
    print(f"  Time:                  {elapsed:.0f}s ({elapsed / 60:.1f}m)")
    if total_docs_ingested > 0 and elapsed > 0:
        print(f"  Rate:                  {total_docs_ingested / elapsed:.2f} docs/sec")
    print(f"  Training log:          {args.log}")
    print(f"{'=' * 70}")


if __name__ == "__main__":
    main()
