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

# Try the new-style path
print("Trying 'daily_dialog'...")
try:
    ds = load_dataset("daily_dialog", split="train", streaming=True)
    ex = next(iter(ds))
    print("SUCCESS with 'daily_dialog'")
    print("Keys:", list(ex.keys()))
    print("Sample dialog:", str(ex.get("dialog", ex.get("dialogue", "N/A")))[:200])
except Exception as e:
    print(f"FAILED: {e}")

# Try another path
print("\nTrying 'roskoN/dailydialog'...")
try:
    ds2 = load_dataset("roskoN/dailydialog", split="train", streaming=True)
    ex2 = next(iter(ds2))
    print("SUCCESS with 'roskoN/dailydialog'")
    print("Keys:", list(ex2.keys()))
except Exception as e:
    print(f"FAILED: {e}")
