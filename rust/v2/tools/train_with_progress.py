#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Training wrapper that feeds progress updates to the dashboard.

Wraps ingest_pdf.py core logic and writes JSON progress to a shared file
so the HTTP server can serve it to the browser dashboard.

Usage (called by start_training.py):
    python train_with_progress.py --progress-file <path> <pdf1> [pdf2 ...]
"""

import argparse
import io
import json
import os
import re
import sys
import time
from pathlib import Path

# Force UTF-8 output on Windows
if sys.stdout.encoding and sys.stdout.encoding.lower() not in ("utf-8", "utf8"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
if sys.stderr.encoding and sys.stderr.encoding.lower() not in ("utf-8", "utf8"):
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace")

# Add tools dir to path so we can import ingest_pdf
sys.path.insert(0, str(Path(__file__).parent))
import ingest_pdf as ingester

MCP_EXE   = r"<AI_FACTORY_ROOT>\v2\target\release\lu_mcp_v2.exe"
DEFAULT_BRAIN = r"<AI_FACTORY_ROOT>\v2\brain\lu_brain.lbr"


# ── Progress I/O ──────────────────────────────────────────────────────────────

def write_progress(progress_file: str, state: dict):
    """Atomically write progress JSON (write-to-tmp then rename).
    Retries on Windows PermissionError caused by HTTP server reading the file."""
    tmp = progress_file + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(state, f)
    for attempt in range(10):
        try:
            os.replace(tmp, progress_file)
            return
        except PermissionError:
            time.sleep(0.05)
    # Last attempt — let it raise if still failing
    os.replace(tmp, progress_file)


# ── Book training ─────────────────────────────────────────────────────────────

def train_book(pdf_path: str, book_idx: int, state: dict,
               progress_file: str, client, args) -> None:
    """Train on a single PDF, writing progress updates after every paragraph."""
    book_name = os.path.basename(pdf_path)
    print(f"\n{'='*65}")
    print(f"  BOOK [{book_idx + 1}]: {book_name}")
    print(f"{'='*65}")

    state["books"][book_idx]["status"] = "training"
    state["current_book"] = book_idx
    write_progress(progress_file, state)

    total_paragraphs = 0
    total_ku = 0
    errors = 0
    last_save = 0
    start_time = time.time()

    for para in ingester.iter_pdf_paragraphs(pdf_path, verbose=args.verbose):
        text = para["text"]
        page = para["page"]

        # Length filter — skip too short
        if len(text) < args.min_len:
            continue

        # Long paragraph — split into sentence chunks
        if len(text) > args.max_len:
            sentences = re.split(r'(?<=[.!?])\s+', text)
            chunks: list[str] = []
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

            for chunk in chunks:
                if len(chunk) < args.min_len:
                    continue
                ku = ingester.ingest_paragraph(client, chunk, verbose=args.verbose)
                total_ku += ku
                if ku == 0:
                    errors += 1
                total_paragraphs += 1
        else:
            ku = ingester.ingest_paragraph(client, text, verbose=args.verbose)
            total_ku += ku
            if ku == 0:
                errors += 1
            total_paragraphs += 1

        # Progress update every paragraph
        elapsed = time.time() - start_time
        rate = total_paragraphs / elapsed if elapsed > 0 else 0.0
        state["books"][book_idx].update({
            "paragraphs": total_paragraphs,
            "ku_count":   total_ku,
            "errors":     errors,
            "rate":       round(rate, 2),
            "page":       page,
        })
        write_progress(progress_file, state)

        # Console progress every 50 paragraphs
        if total_paragraphs % 50 == 0:
            print(f"  [{total_paragraphs} para] +{total_ku} KU | "
                  f"{rate:.1f}/s | page {page}", flush=True)

        # Periodic brain save
        if total_paragraphs - last_save >= args.save_every:
            save_result = ingester.save_brain(client)
            last_save = total_paragraphs
            print(f"  [SAVE @ {total_paragraphs}] {save_result}", flush=True)

        if args.limit and total_paragraphs >= args.limit:
            print(f"  Reached --limit {args.limit}, stopping.", flush=True)
            break

    # Final save for this book
    save_result = ingester.save_brain(client)
    elapsed = time.time() - start_time
    print(f"\n  Final save: {save_result}")
    print(f"  Done: {total_paragraphs} paragraphs, {total_ku} KUs in {elapsed:.0f}s "
          f"({elapsed / 60:.1f}m)", flush=True)

    state["books"][book_idx]["status"] = "done"
    write_progress(progress_file, state)


# ── Main ──────────────────────────────────────────────────────────────────────

def parse_args():
    p = argparse.ArgumentParser(description="Train Lu Brain v2 with dashboard progress")
    p.add_argument("pdfs", nargs="+", help="PDF files to train on")
    p.add_argument("--progress-file", required=True, help="Path to JSON progress file")
    p.add_argument("--brain", default=DEFAULT_BRAIN, help="Brain .lbr file path")
    p.add_argument("--min-len", type=int, default=80, metavar="N",
                   help="Minimum paragraph char length (default 80)")
    p.add_argument("--max-len", type=int, default=4000, metavar="N",
                   help="Maximum paragraph char length (default 4000)")
    p.add_argument("--save-every", type=int, default=100, metavar="N",
                   help="Save brain every N paragraphs (default 100)")
    p.add_argument("--limit", type=int, default=0, metavar="N",
                   help="Stop each book after N paragraphs (0 = unlimited)")
    p.add_argument("--verbose", action="store_true",
                   help="Print each paragraph and MCP response")
    return p.parse_args()


def main():
    args = parse_args()
    progress_file = args.progress_file

    # Load the progress state written by start_training.py
    with open(progress_file, "r", encoding="utf-8") as f:
        state = json.load(f)
    state["status"] = "training"
    write_progress(progress_file, state)

    # ── Start MCP client ─────────────────────────────────────────────────────
    print("[1/3] Starting MCP server...")
    if not os.path.isfile(MCP_EXE):
        msg = f"MCP exe not found: {MCP_EXE}"
        print(f"FATAL: {msg}")
        state["status"] = "error"
        state["error"] = msg
        write_progress(progress_file, state)
        sys.exit(1)

    try:
        client = ingester.McpClient(MCP_EXE, args.brain)
    except RuntimeError as e:
        print(f"FATAL: {e}")
        state["status"] = "error"
        state["error"] = str(e)
        write_progress(progress_file, state)
        sys.exit(1)

    print(f"  MCP server running (PID {client.proc.pid})")

    # ── Initialize MCP session ───────────────────────────────────────────────
    print("[2/3] Initializing MCP session...")
    resp = client.send("initialize", {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "train-with-progress", "version": "1.0"},
    })
    if resp.get("error"):
        print(f"  WARNING: initialize error: {resp['error']}")
    else:
        print("  Session initialized OK")

    client.notify("notifications/initialized", {})
    time.sleep(0.3)

    resp = client.tool_call("lu_configure", {"autonomy_tier": "Autonomous"})
    print(f"  Configure: {ingester.extract_result_text(resp)}")

    # ── Train each book ──────────────────────────────────────────────────────
    print(f"\n[3/3] Training {len(args.pdfs)} PDF(s)...")
    for idx, pdf_path in enumerate(args.pdfs):
        if not os.path.isfile(pdf_path):
            print(f"  SKIP: not found: {pdf_path}")
            state["books"][idx]["status"] = "error"
            write_progress(progress_file, state)
            continue
        train_book(pdf_path, idx, state, progress_file, client, args)

    client.close()

    state["status"] = "complete"
    state["finished_at"] = time.time()
    write_progress(progress_file, state)

    total_para = sum(b.get("paragraphs", 0) for b in state["books"])
    total_ku   = sum(b.get("ku_count",   0) for b in state["books"])
    print(f"\n{'='*65}")
    print(f"  ALL TRAINING COMPLETE")
    print(f"  Total paragraphs: {total_para:,}")
    print(f"  Total KUs learned: {total_ku:,}")
    print(f"{'='*65}")


if __name__ == "__main__":
    main()
