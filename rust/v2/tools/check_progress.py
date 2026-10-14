# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

import os, csv

log = r"<AI_FACTORY_ROOT>\v2\tools\hf_conversations_log.csv"
if os.path.isfile(log):
    with open(log, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        rows = list(reader)
    datasets = {}
    for r in rows:
        ds = r["dataset"]
        datasets[ds] = datasets.get(ds, 0) + 1
    print(f"Total ingested: {len(rows)}")
    for ds, count in sorted(datasets.items()):
        print(f"  {ds}: {count}")
else:
    print("No log file found")

# Check last few lines of ALL log files
for logname in ["science_facts_log.txt", "sciq_log.txt", "wiki_log.txt", "obqa_log.txt", "sciqa_log.txt"]:
    slog = os.path.join(r"<AI_FACTORY_ROOT>\v2\tools", logname)
    if os.path.isfile(slog):
        with open(slog, "r", encoding="utf-8") as f:
            lines = f.readlines()
        print(f"\nLast 3 lines of {logname}:")
        for line in lines[-3:]:
            print(f"  {line.rstrip()}")

# Check if python / lu_mcp still running
import subprocess
result = subprocess.run(["tasklist"], capture_output=True, text=True)
py_count = sum(1 for line in result.stdout.splitlines() if "python" in line.lower())
mcp_count = sum(1 for line in result.stdout.splitlines() if "lu_mcp" in line.lower())
print(f"\nRunning: {py_count} python, {mcp_count} lu_mcp_v2 processes")
