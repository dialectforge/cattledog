#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Lu Ai - Master PDF Training Runner

Trains Lu on multiple PDF textbooks sequentially.
Saves progress after each book. Writes a progress log file
so you can check status any time.

Usage:
    python train_all_books.py
    python train_all_books.py --resume-from 2   (skip first N books)
"""

import sys
import os
import time
import argparse
import subprocess

PROGRESS_FILE = r"<AI_FACTORY_ROOT>\tools\training_progress.txt"
TRAIN_SCRIPT = r"<AI_FACTORY_ROOT>\tools\train_pdf.py"
MODEL = "mistral:7b"

# Each book: (name, path, start_page, end_page)
# start_page=0 means "from the beginning" (front matter filter handles skipping)
BOOKS = [
    ("Workplace Software and Skills",
     r"<USER_HOME>\OneDrive\Desktop\Workplace_Software_and_Skills_-_WEB_IlfJtcP.pdf",
     39, None),  # Pages 17-39 already trained in first run

    ("Foundations of Information Systems",
     r"<USER_HOME>\OneDrive\Desktop\Foundations_of_Information_Systems_-_WEB_oNlbGYl.pdf",
     0, None),

    ("Introduction to Python Programming",
     r"<USER_HOME>\OneDrive\Desktop\Introduction_to_Python_Programming_-_WEB.pdf",
     0, None),

    ("Introduction to Computer Science",
     r"<USER_HOME>\OneDrive\Desktop\Introduction_To_Computer_Science_-_WEB.pdf",
     0, None),

    ("Principles of Data Science",
     r"<USER_HOME>\OneDrive\Desktop\Principles-of-Data-Science-WEB.pdf",
     0, None),
]


def log(msg):
    """Write to both console and progress file."""
    timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
    line = f"[{timestamp}] {msg}"
    print(line, flush=True)
    with open(PROGRESS_FILE, "a", encoding="utf-8") as f:
        f.write(line + "\n")


def train_book(name, path, start_page, end_page):
    """Run train_pdf.py for one book as a subprocess."""
    if not os.path.exists(path):
        log(f"ERROR: File not found: {path}")
        return False

    log(f"STARTING: {name}")
    log(f"  Path: {path}")
    log(f"  Pages: {start_page} to {'end' if end_page is None else end_page}")

    cmd = [
        sys.executable, "-u", TRAIN_SCRIPT,
        path,
        "--model", MODEL,
        "--batch-size", "10",
        "--start-page", str(start_page),
    ]
    if end_page is not None:
        cmd.extend(["--end-page", str(end_page)])

    # Run and capture output line by line
    output_file = os.path.join(
        r"<AI_FACTORY_ROOT>\tools",
        f"book_output_{name.replace(' ', '_')[:30]}.txt"
    )

    start = time.time()
    try:
        with open(output_file, "w", encoding="utf-8") as out:
            proc = subprocess.Popen(
                cmd, stdout=out, stderr=subprocess.STDOUT,
                text=True, bufsize=1
            )
            proc.wait()  # Wait for completion
            elapsed = time.time() - start
            
            if proc.returncode == 0:
                log(f"COMPLETED: {name} in {elapsed/60:.1f} minutes")
                return True
            else:
                log(f"FAILED: {name} (exit code {proc.returncode}) after {elapsed/60:.1f} min")
                log(f"  Check output: {output_file}")
                return False
    except Exception as e:
        elapsed = time.time() - start
        log(f"ERROR: {name} crashed after {elapsed/60:.1f} min: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(description="Train all PDF textbooks")
    parser.add_argument("--resume-from", type=int, default=0,
                        help="Skip first N books (0-indexed)")
    args = parser.parse_args()

    # Clear progress file
    if args.resume_from == 0:
        with open(PROGRESS_FILE, "w", encoding="utf-8") as f:
            f.write("")

    log("=" * 55)
    log("  LU AI — MULTI-BOOK PDF TRAINING")
    log(f"  Books: {len(BOOKS)}")
    log(f"  Model: {MODEL}")
    log(f"  Resume from: book #{args.resume_from}")
    log("=" * 55)

    total_start = time.time()
    completed = 0
    failed = 0

    for i, (name, path, start_page, end_page) in enumerate(BOOKS):
        if i < args.resume_from:
            log(f"SKIPPING [{i+1}/{len(BOOKS)}]: {name}")
            continue

        log(f"\n{'—'*55}")
        log(f"BOOK [{i+1}/{len(BOOKS)}]: {name}")
        log(f"{'—'*55}")

        success = train_book(name, path, start_page, end_page)
        if success:
            completed += 1
        else:
            failed += 1
            log(f"  Continuing to next book despite failure...")

    total_elapsed = time.time() - total_start
    log(f"\n{'='*55}")
    log(f"  ALL TRAINING COMPLETE")
    log(f"  Time:      {total_elapsed/60:.1f} min ({total_elapsed/3600:.1f} hours)")
    log(f"  Completed: {completed}/{len(BOOKS)}")
    log(f"  Failed:    {failed}/{len(BOOKS)}")
    log(f"  Progress:  {PROGRESS_FILE}")
    log(f"{'='*55}")


if __name__ == "__main__":
    main()
