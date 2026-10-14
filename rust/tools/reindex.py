#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Lu Ai — Brain Reindex

Triggers a full word-level bond discovery pass across all nodes.
This connects new knowledge to old knowledge by finding shared words.

Algorithm (runs inside the Rust brain for speed):
1. Build inverted index: word → list of nodes containing that word
2. Skip stop words and high-frequency words (>5% of nodes)
3. Create bonds between nodes that share meaningful words
4. Already-existing bonds get strengthened instead of duplicated

Usage:
    python reindex.py
    python reindex.py --freq 3.0        # stricter: skip words in >3% of nodes
    python reindex.py --max-per-word 100 # allow more bonds per shared word

Run AFTER training new knowledge to wire it into the existing graph.
Close Claude Desktop first to avoid MCP conflicts.
"""

import sys
import os
import time
import subprocess
import argparse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from train import MCPBridge


def kill_existing_mcp():
    try:
        result = subprocess.run(
            ["taskkill", "/F", "/IM", "lu_ai_mcp.exe"],
            capture_output=True, text=True
        )
        if result.returncode == 0:
            print("[reindex] Killed existing MCP process(es)")
            time.sleep(1)
    except Exception:
        pass

def main():
    parser = argparse.ArgumentParser(description="Reindex Lu's brain — wire new knowledge to old")
    parser.add_argument("--freq", type=float, default=5.0,
                        help="Skip words in more than this %% of nodes (default 5.0)")
    parser.add_argument("--max-per-word", type=int, default=50,
                        help="Max bond pairs per word bucket (default 50)")
    args = parser.parse_args()

    kill_existing_mcp()

    bridge = MCPBridge()
    bridge.start()

    # Get stats before
    stats_before = bridge._get_stats()
    nodes_before = stats_before.get("active_nodes", 0) if stats_before else 0
    bonds_before = stats_before.get("active_bonds", 0) if stats_before else 0

    print(f"Brain before reindex: {nodes_before} nodes, {bonds_before} bonds")
    print(f"Settings: max_freq_pct={args.freq}%, max_per_word={args.max_per_word}")
    print(f"{'='*50}")
    print(f"Reindexing... (this scans all nodes once)", flush=True)

    start_time = time.time()

    # Call the reindex tool via MCP tools/call
    resp = bridge._call("tools/call", {
        "name": "brain_reindex",
        "arguments": {
            "max_freq_pct": args.freq,
            "max_per_word": args.max_per_word,
        }
    })

    # Extract the result from MCP response envelope
    result = None
    if resp and "result" in resp:
        content = resp["result"].get("content", [])
        if content and len(content) > 0:
            text = content[0].get("text", "")
            try:
                import json as _json
                result = _json.loads(text)
            except Exception:
                result = {"raw": text}

    elapsed = time.time() - start_time

    if result:
        created = result.get("bonds_created", 0)
        strengthened = result.get("bonds_strengthened", 0)
        words = result.get("words_processed", 0)
        skipped = result.get("words_skipped_high_freq", 0)

        print(f"\nReindex complete in {elapsed:.1f}s")
        print(f"  Words processed: {words}")
        print(f"  Words skipped (too common): {skipped}")
        print(f"  New bonds created: {created}")
        print(f"  Existing bonds strengthened: {strengthened}")
    else:
        print(f"\nReindex returned no result after {elapsed:.1f}s")

    # Save
    bridge.save()
    print(f"  [saved]")

    # Get stats after
    stats_after = bridge._get_stats()
    nodes_after = stats_after.get("active_nodes", 0) if stats_after else 0
    bonds_after = stats_after.get("active_bonds", 0) if stats_after else 0

    print(f"\n{'='*50}")
    print(f"Brain after reindex: {nodes_after} nodes, {bonds_after} bonds")
    print(f"Bond change: {bonds_before} -> {bonds_after} (+{bonds_after - bonds_before})")
    print(f"{'='*50}")

    bridge.stop()


if __name__ == "__main__":
    main()
