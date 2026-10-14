# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

import sys
sys.stdout.reconfigure(encoding='utf-8')
from datasets import load_dataset

results = []
for name in ["allenai/soda", "bavard/personachat_truecased"]:
    try:
        ds = load_dataset(name, split="train", streaming=True, trust_remote_code=False)
        ex = next(iter(ds))
        results.append(f"OK {name}: {list(ex.keys())[:6]}")
    except Exception as e:
        results.append(f"FAIL {name}: {str(e)[:80]}")

with open(r"<AI_FACTORY_ROOT>\v2\tools\probe_result.txt", "w") as f:
    f.write("\n".join(results))
print("DONE")
