#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
r"""
Lu Ai - PDF Textbook Training for Lu

Extracts text from a PDF, uses Ollama to break it into clean
atomic knowledge sentences, then feeds them to Lu via MCP bridge.

Usage:
    python train_pdf.py "C:\path\to\book.pdf"
    python train_pdf.py "C:\path\to\book.pdf" --start-page 15
    python train_pdf.py "C:\path\to\book.pdf" --model qwen3:8b
    python train_pdf.py "C:\path\to\book.pdf" --dry-run
"""

import sys
import os
import json
import time
import argparse
import urllib.request
import urllib.error
import re

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from train import MCPBridge, DEFAULT_MODEL

OLLAMA_URL = "http://localhost:11434/api/generate"


# --- PDF Extraction ---

def extract_pages(pdf_path, start_page=0, end_page=None):
    """Extract text from PDF pages using PyMuPDF (fitz)."""
    import fitz  # pymupdf
    doc = fitz.open(pdf_path)
    total = len(doc)
    if end_page is None:
        end_page = total
    end_page = min(end_page, total)

    print(f"[pdf] Opened: {os.path.basename(pdf_path)}")
    print(f"[pdf] Total pages: {total}")
    print(f"[pdf] Reading pages {start_page+1} to {end_page}")

    pages = []
    for i in range(start_page, end_page):
        page = doc[i]
        text = page.get_text("text").strip()
        if len(text) > 50:
            pages.append({"page": i + 1, "text": text})

    doc.close()
    print(f"[pdf] Extracted {len(pages)} pages with content")
    return pages


# --- Front Matter Detection (fixed: less aggressive) ---

# Only truly unambiguous front-matter phrases
FRONT_MATTER_PHRASES = [
    "copyright", "isbn", "all rights reserved",
    "table of contents", "acknowledgment",
    "preface", "dedication", "about the author",
    "licensed under", "creative commons attribution",
]

def is_front_matter(text, page_num):
    """Detect front matter pages. Only flag in first 30 pages."""
    if page_num > 30:
        return False
    lower = text.lower()
    hits = sum(1 for w in FRONT_MATTER_PHRASES if w in lower)
    return hits >= 3

def is_back_matter(text, page_num, total_pages):
    """Detect back matter like index, glossary. Only in last 10%."""
    if page_num < total_pages * 0.9:
        return False
    lower = text[:300].lower()
    lines = lower.split('\n')
    # Index pages have lots of short lines with page numbers
    if lower.startswith("index") or lower.startswith("glossary"):
        return True
    return False


# --- Ollama Chunking ---

CHUNK_SYSTEM = """You are a knowledge extractor for an AI training system.
Given a page of text from a textbook, extract the KEY FACTS as atomic sentences.
Output ONLY valid JSON. No markdown, no explanation, no code blocks.
Your JSON must have this exact structure:
{"items": [{"content": "one clear factual sentence", "associations": ["a related fact from the same page"]}]}

RULES:
- Each "content" must be ONE complete, standalone sentence (10-30 words)
- Skip headers, page numbers, figure captions, and exercise instructions
- Skip sentences that only make sense in context ("As shown above...", "See figure 3.1")
- Focus on FACTS, DEFINITIONS, and HOW-TO knowledge
- Each fact must be understandable WITHOUT reading the rest of the page
- associations should be genuinely related content from the same page
- Generate 5-15 items per page depending on how much real content there is
- If the page has no useful educational content, return {"items": []}"""


def sanitize_text_for_json(text):
    """Remove characters that break JSON/pipes."""
    # Replace problematic unicode
    text = text.replace('\x00', '')
    text = text.replace('\r\n', '\n').replace('\r', '\n')
    # Replace non-ASCII that might break cp1252 pipes
    text = text.encode('ascii', errors='replace').decode('ascii')
    return text


def chunk_page_with_ollama(page_text, page_num, model=DEFAULT_MODEL):
    """Send a page to Ollama and get back atomic knowledge items."""
    if len(page_text) > 4000:
        page_text = page_text[:4000]

    prompt = f"""Extract key facts from this textbook page (page {page_num}).

PAGE TEXT:
{page_text}

Output ONLY the JSON with extracted facts. If the page has no real educational content (just headers, exercises, or formatting), return {{"items": []}}"""

    payload = {
        "model": model,
        "prompt": prompt,
        "system": CHUNK_SYSTEM,
        "stream": False,
        "format": "json",
        "options": {"temperature": 0.2, "top_p": 0.9, "num_ctx": 4096}
    }

    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        OLLAMA_URL, data=data,
        headers={"Content-Type": "application/json"}
    )
    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            result = json.loads(resp.read().decode("utf-8"))
            text = result.get("response", "")
            if not text:
                return []
            text = text.strip()
            if "```" in text:
                text = re.sub(r'```(?:json)?\n?', '', text).strip()
            text = re.sub(r',\s*([}\]])', r'\1', text)
            try:
                parsed = json.loads(text)
                items = parsed.get("items", [])
                # Sanitize content to prevent pipe encoding errors
                for item in items:
                    if "content" in item:
                        item["content"] = sanitize_text_for_json(item["content"])
                    if "associations" in item:
                        item["associations"] = [sanitize_text_for_json(a) for a in item["associations"]]
                valid = [i for i in items if isinstance(i, dict)
                         and "content" in i and len(i["content"]) > 15]
                return valid
            except json.JSONDecodeError:
                match = re.search(r'\{[\s\S]*\}', text)
                if match:
                    try:
                        parsed = json.loads(match.group())
                        items = parsed.get("items", [])
                        valid = [i for i in items if isinstance(i, dict)
                                 and "content" in i and len(i["content"]) > 15]
                        return valid
                    except json.JSONDecodeError:
                        pass
                print(f"  [ollama] Parse error page {page_num}: {text[:100]}")
                return []
    except urllib.error.URLError as e:
        print(f"  [ollama] Error on page {page_num}: {e}")
        return []
    except Exception as e:
        print(f"  [ollama] Unexpected error on page {page_num}: {e}")
        return []


# --- Main ---

def main():
    parser = argparse.ArgumentParser(description="PDF Textbook Training for Lu")
    parser.add_argument("pdf", help="Path to PDF file")
    parser.add_argument("--start-page", type=int, default=0,
                        help="First page to process (0-indexed, default: 0)")
    parser.add_argument("--end-page", type=int, default=None,
                        help="Last page to process (default: all)")
    parser.add_argument("--model", "-m", default=DEFAULT_MODEL,
                        help=f"Ollama model (default: {DEFAULT_MODEL})")
    parser.add_argument("--dry-run", action="store_true",
                        help="Extract and chunk but don't feed to brain")
    parser.add_argument("--batch-size", type=int, default=5,
                        help="Pages to process before saving (default: 5)")
    args = parser.parse_args()

    if not os.path.exists(args.pdf):
        print(f"ERROR: File not found: {args.pdf}")
        sys.exit(1)

    print(f"{'='*55}")
    print(f"  Lu Ai -- PDF Textbook Training")
    print(f"  PDF:   {os.path.basename(args.pdf)}")
    print(f"  Model: {args.model}")
    print(f"  Mode:  {'DRY RUN' if args.dry_run else 'LIVE'}")
    print(f"{'='*55}\n")

    # Step 1: Extract pages
    pages = extract_pages(args.pdf, args.start_page, args.end_page)
    if not pages:
        print("ERROR: No pages with content found.")
        sys.exit(1)

    total_pdf_pages = pages[-1]["page"] if pages else 0

    # Step 2: Filter out front/back matter
    content_pages = []
    skipped_front = 0
    skipped_back = 0
    for p in pages:
        if is_front_matter(p["text"], p["page"]):
            skipped_front += 1
            print(f"  [skip] Page {p['page']} -- front matter")
            continue
        if is_back_matter(p["text"], p["page"], total_pdf_pages):
            skipped_back += 1
            print(f"  [skip] Page {p['page']} -- back matter")
            continue
        content_pages.append(p)

    print(f"\n[filter] {len(content_pages)} content pages")
    print(f"[filter] Skipped {skipped_front} front, {skipped_back} back\n")

    if not content_pages:
        print("ERROR: No content pages after filtering.")
        sys.exit(1)

    # Step 3: Start MCP bridge (unless dry run)
    bridge = None
    if not args.dry_run:
        bridge = MCPBridge()
        bridge.start()
        stats = bridge._get_stats()
        start_nodes = stats.get("active_nodes", 0) if stats else 0
        print(f"[brain] Starting at {start_nodes} nodes\n")

    # Step 4: Process pages through Ollama, then feed to brain
    total_items = 0
    total_learned = 0
    total_bonds = 0
    failed_pages = 0
    mcp_restarts = 0
    start_time = time.time()

    for idx, page in enumerate(content_pages):
        pnum = page["page"]
        elapsed = time.time() - start_time
        rate = total_items / max(elapsed, 1) * 60

        print(f"[{idx+1}/{len(content_pages)}] Page {pnum}  "
              f"({total_items} items, {rate:.0f}/min)", flush=True)

        items = chunk_page_with_ollama(page["text"], pnum, args.model)
        if not items:
            print(f"  [ollama] No facts extracted")
            failed_pages += 1
            continue

        total_items += len(items)
        print(f"  [ollama] {len(items)} facts extracted")

        if args.dry_run:
            for item in items[:3]:
                print(f"    -> {item['content'][:80]}")
            if len(items) > 3:
                print(f"    ... and {len(items)-3} more")
            continue

        # Feed to brain — with pipe crash recovery
        try:
            result = bridge.batch_train(items)
            if result and "total_learned" in result:
                learned = result["total_learned"]
                bonds = result.get("total_bonds", 0)
                total_learned += learned
                total_bonds += bonds
                print(f"  [brain] +{learned} nodes, +{bonds} bonds")
            elif result:
                print(f"  [brain] {str(result)[:80]}")
            else:
                print(f"  [brain] No response")
        except (OSError, BrokenPipeError) as e:
            print(f"  [brain] PIPE ERROR: {e}")
            print(f"  [brain] Restarting MCP bridge...")
            mcp_restarts += 1
            try:
                bridge.stop()
            except Exception:
                pass
            time.sleep(2)
            bridge = MCPBridge()
            bridge.start()
            # Retry this batch
            try:
                result = bridge.batch_train(items)
                if result and "total_learned" in result:
                    learned = result["total_learned"]
                    total_learned += learned
                    print(f"  [brain] Recovered: +{learned} nodes")
            except Exception as e2:
                print(f"  [brain] Retry also failed: {e2}")
                failed_pages += 1

        # Save periodically
        if (idx + 1) % args.batch_size == 0 and bridge:
            try:
                bridge.save()
                cs = bridge._get_stats()
                cn = cs.get("active_nodes", 0) if cs else 0
                print(f"  [save] {cn} nodes | {time.time()-start_time:.0f}s\n")
            except (OSError, BrokenPipeError):
                print(f"  [save] Pipe error during save, restarting...")
                mcp_restarts += 1
                try:
                    bridge.stop()
                except Exception:
                    pass
                time.sleep(2)
                bridge = MCPBridge()
                bridge.start()

    # Step 5: Final save and summary
    elapsed = time.time() - start_time

    if bridge:
        try:
            bridge.save()
            final_stats = bridge._get_stats()
            final_nodes = final_stats.get("active_nodes", 0) if final_stats else 0
            bridge.stop()
        except Exception:
            final_nodes = start_nodes + total_learned
    else:
        final_nodes = 0
        start_nodes = 0

    print(f"\n{'='*55}")
    print(f"  PDF Training Complete!")
    print(f"  Time:    {elapsed:.0f}s ({elapsed/60:.1f} min)")
    print(f"  Pages:   {len(content_pages)} processed, {failed_pages} failed")
    print(f"  Items:   {total_items} extracted by Ollama")
    if not args.dry_run:
        print(f"  Learned: {total_learned} nodes, {total_bonds} bonds")
        print(f"  Brain:   {start_nodes} -> {final_nodes} (+{final_nodes-start_nodes})")
        if mcp_restarts > 0:
            print(f"  MCP restarts: {mcp_restarts}")
    print(f"{'='*55}")


if __name__ == "__main__":
    main()
