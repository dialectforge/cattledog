#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Probe script: verify all 14 new datasets added to ingest_hf_conversations.py
can actually be streamed from HuggingFace.

Streams exactly 1 example from each dataset and prints the field names + a
short preview.  Fails gracefully — a dataset that errors shows as FAIL rather
than aborting the whole run.

Usage:
    python probe_new_datasets.py
"""

import io
import sys
import time
import traceback

if sys.stdout.encoding and sys.stdout.encoding.lower() not in ("utf-8", "utf8"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
if sys.stderr.encoding and sys.stderr.encoding.lower() not in ("utf-8", "utf8"):
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace")

try:
    from datasets import load_dataset
except ImportError:
    print("ERROR: 'datasets' library not installed.  Run: pip install datasets")
    sys.exit(1)

# ── New datasets to probe ─────────────────────────────────────────────────────
# Each entry: (display_name, hf_path, config_or_None, split)
NEW_DATASETS = [
    ("gsm8k",             "openai/gsm8k",                         "main",            "train"),
    ("arc-challenge",     "allenai/ai2_arc",                      "ARC-Challenge",   "train"),
    ("commonsense-qa",    "tau/commonsense_qa",                    None,              "train"),
    ("triviaqa",          "trivia_qa",                            "rc.nocontext",    "train"),
    ("hotpotqa",          "hotpot_qa",                            "fullwiki",        "train"),
    ("mmlu",              "cais/mmlu",                            "all",             "test"),
    ("boolq",             "google/boolq",                         None,              "train"),
    ("dolly-15k",         "databricks/databricks-dolly-15k",      None,              "train"),
    ("hellaswag",         "Rowan/hellaswag",                      None,              "train"),
    ("winogrande",        "winogrande",                           "winogrande_xl",   "train"),
    ("wikitext",          "wikitext",                             "wikitext-103-v1", "train"),
    ("squad",             "rajpurkar/squad",                      None,              "train"),
    ("orca-math",         "microsoft/orca-math-word-problems-200k", None,              "train"),
    ("natural-questions", "google-research-datasets/nq_open",     None,              "train"),
    # ── Batch 2: conversational / social ────────────────────────────────────
    ("oasst1",            "OpenAssistant/oasst1",                 None,              "train"),
    ("prosocial-dialog",  "allenai/prosocial-dialog",             None,              "train"),
    ("persona-chat",      "AlekseyKorshuk/persona-chat",          None,              "train"),
    ("hh-rlhf",           "Anthropic/hh-rlhf",                    None,              "train"),
    # ── Batch 2: practical / how-to ──────────────────────────────────────────
    ("dolly-15k",         "databricks/databricks-dolly-15k",      None,              "train"),
    # ── Batch 2: CS depth / technical ────────────────────────────────────────
    ("stackexchange",     "sentence-transformers/stackexchange-duplicates", "post-post-pair", "train"),
    ("evol-codealpaca",   "theblackcat102/evol-codealpaca-v1",    None,              "train"),
    # ── Batch 2: encyclopedic / news ─────────────────────────────────────────
    ("goodwiki",          "euirim/goodwiki",                      None,              "train"),
    ("ag-news",           "fancyzhx/ag_news",                     None,              "train"),
]


def probe_one(display_name, hf_path, config, split):
    """
    Try to stream 1 example from the dataset.
    Returns (ok: bool, message: str, fields: list[str]).
    """
    load_kwargs = dict(split=split, streaming=True, trust_remote_code=True)
    if config:
        load_kwargs["name"] = config

    try:
        ds = load_dataset(hf_path, **load_kwargs)
        example = next(iter(ds))
        fields = list(example.keys())
        # Build a short preview of the first few field values
        preview_parts = []
        for key in fields[:4]:
            val = example[key]
            if isinstance(val, str):
                snippet = val[:60].replace("\n", " ")
                preview_parts.append(f"{key}={snippet!r}")
            elif isinstance(val, list):
                preview_parts.append(f"{key}=[len={len(val)}]")
            elif isinstance(val, dict):
                preview_parts.append(f"{key}={{keys={list(val.keys())[:3]}}}")
            else:
                preview_parts.append(f"{key}={str(val)[:40]!r}")
        preview = "  |  ".join(preview_parts)
        return True, preview, fields
    except Exception:
        tb = traceback.format_exc().strip().splitlines()[-1]
        return False, tb, []


def main():
    print("=" * 72)
    print("  PROBE: New HuggingFace datasets (stream 1 example each)")
    print("=" * 72)
    print()

    results = []
    for display_name, hf_path, config, split in NEW_DATASETS:
        cfg_str = f"/{config}" if config else ""
        print(f"  [{display_name}]  {hf_path}{cfg_str}  split={split}")
        t0 = time.time()
        ok, msg, fields = probe_one(display_name, hf_path, config, split)
        elapsed = time.time() - t0
        status = "OK  " if ok else "FAIL"
        field_str = f"  fields={fields}" if ok else ""
        print(f"    {status}  ({elapsed:.1f}s){field_str}")
        if ok:
            print(f"    preview: {msg}")
        else:
            print(f"    error:   {msg}")
        print()
        results.append((display_name, ok, elapsed))

    # Summary
    print("=" * 72)
    passed = sum(1 for _, ok, _ in results if ok)
    failed = len(results) - passed
    print(f"  RESULTS: {passed}/{len(results)} passed"
          + (f"  ({failed} FAILED)" if failed else "  — all good!"))
    print()
    if failed:
        print("  FAILED datasets:")
        for name, ok, _ in results:
            if not ok:
                print(f"    - {name}")
    print("=" * 72)
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
