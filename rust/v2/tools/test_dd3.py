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
from huggingface_hub import list_datasets

# Search for dailydialog variants
print("Searching for dailydialog datasets on HF Hub...")
try:
    results = list(list_datasets(search="dailydialog", limit=15))
    for r in results:
        print(f"  {r.id}  (likes={getattr(r, 'likes', '?')})")
except Exception as e:
    print(f"Search failed: {e}")

# Try some known alternatives
candidates = [
    "Nidhal-Baccouri/dailydialog",
    "silicone",  # has dailydialog as a config
]

for name in candidates:
    print(f"\nTrying '{name}'...")
    try:
        ds = load_dataset(name, split="train", streaming=True)
        ex = next(iter(ds))
        print(f"  SUCCESS! Keys: {list(ex.keys())}")
        print(f"  Sample: {str(ex)[:300]}")
    except Exception as e:
        print(f"  FAILED: {e}")

# Try silicone with dailydialog_act config
print(f"\nTrying 'silicone' config='dyda_da'...")
try:
    ds = load_dataset("silicone", "dyda_da", split="train", streaming=True)
    ex = next(iter(ds))
    print(f"  SUCCESS! Keys: {list(ex.keys())}")
    print(f"  Sample: {str(ex)[:300]}")
except Exception as e:
    print(f"  FAILED: {e}")
