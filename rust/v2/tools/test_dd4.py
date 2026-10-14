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
    ("ConvLab/dailydialog", {}),
    ("OpenRL/daily_dialog", {}),
    ("yangwang825/daily_dialog_plus_plus", {}),
    ("kmyoo/dailydialog-tiny", {}),
]

for name, kwargs in candidates:
    print(f"Trying '{name}'...")
    try:
        ds = load_dataset(name, split="train", streaming=True, **kwargs)
        ex = next(iter(ds))
        print(f"  SUCCESS! Keys: {list(ex.keys())}")
        print(f"  Sample: {str(ex)[:400]}")
        print()
    except Exception as e:
        print(f"  FAILED: {e}\n")
