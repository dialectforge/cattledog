# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

import os

log = r"<AI_FACTORY_ROOT>\v2\tools\overnight_log.txt"
if os.path.isfile(log):
    with open(log, "r", encoding="utf-8", errors="replace") as f:
        lines = f.readlines()
    # Show DONE lines, SAVE lines, and last 5 lines
    print("=== Completed datasets ===")
    for l in lines:
        if "DONE" in l and "ingested=" in l:
            print(f"  {l.strip()}")
    print(f"\n=== Last 5 lines ===")
    for l in lines[-5:]:
        print(f"  {l.strip()}")
    print(f"\n  Total log lines: {len(lines)}")
else:
    print("No overnight_log.txt found")
