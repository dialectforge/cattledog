# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

"""Quick probe to check field schemas of candidate programming datasets."""
from datasets import load_dataset

candidates = [
    ("sahil2801/CodeAlpaca-20k", None, "train"),
    ("iamtarun/python_code_instructions_18k_alpaca", None, "train"),
    ("m-a-p/Code-Feedback", None, "train"),
    ("HuggingFaceH4/no_robots", None, "train"),
    ("nickrosh/Evol-Instruct-Code-80k-v1", None, "train"),
]

for hf_path, config, split in candidates:
    print(f"\n{'='*60}")
    print(f"  {hf_path}  config={config}  split={split}")
    print(f"{'='*60}")
    try:
        ds = load_dataset(hf_path, config, split=split, streaming=True)
        it = iter(ds)
        ex = next(it)
        print(f"  Fields: {list(ex.keys())}")
        for k, v in ex.items():
            val_str = str(v)[:200]
            print(f"    {k}: {type(v).__name__} = {val_str}")
    except Exception as e:
        print(f"  ERROR: {e}")
