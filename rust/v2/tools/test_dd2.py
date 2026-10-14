# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

import sys, os
sys.stdout = open(sys.stdout.fileno(), mode='w', encoding='utf-8', errors='replace', buffering=1)
sys.stderr = open(sys.stderr.fileno(), mode='w', encoding='utf-8', errors='replace', buffering=1)
try:
    import platform as _p
    _p._wmi = None
except: pass

from datasets import load_dataset

candidates = [
    ("li2017dailydialog/daily_dialog", {}),
    ("daily_dialog", {}),
    ("benjaminbeilharz/daily_dialog", {}),
    ("liyucheng/daily_dialog", {}),
    ("daily_dialog", {"trust_remote_code": True}),
]

for name, kwargs in candidates:
    print(f"Trying '{name}' {kwargs}...")
    try:
        ds = load_dataset(name, split="train", streaming=True, **kwargs)
        ex = next(iter(ds))
        print(f"  SUCCESS! Keys: {list(ex.keys())}")
        print(f"  Sample: {str(ex)[:300]}")
        break
    except Exception as e:
        print(f"  FAILED: {e}")
