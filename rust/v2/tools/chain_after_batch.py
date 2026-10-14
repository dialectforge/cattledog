# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

"""
Monitor the current batch ingestion and launch run_originals.bat when it finishes.
Detects completion by watching for the lu_mcp_v2 process to disappear after the
batch script's python processes finish.
"""
import subprocess, time, os

TOOLS_DIR = r"<AI_FACTORY_ROOT>\v2\tools"

def count_ingestion_processes():
    """Count python.exe processes doing ingestion (connected to lu_mcp)."""
    result = subprocess.run(["tasklist"], capture_output=True, text=True)
    lines = result.stdout.splitlines()
    py = sum(1 for l in lines if "python.exe" in l.lower())
    mcp = sum(1 for l in lines if "lu_mcp_v2.exe" in l.lower())
    return py, mcp

print("Waiting for current batch to finish...")
print("(Checking every 30 seconds for lu_mcp_v2 to exit)")

# Wait for lu_mcp to appear first (in case batch just started a new dataset)
time.sleep(10)

while True:
    py, mcp = count_ingestion_processes()
    # The Desktop Commander lu_mcp (small, ~5MB) is always running.
    # The ingestion lu_mcp is the big one (~3-4GB).
    # When batch finishes, there will be 0 or 1 small lu_mcp left.
    print(f"  [{time.strftime('%H:%M:%S')}] python={py}, lu_mcp={mcp}")
    
    # If no big MCP process and fewer pythons, batch is done
    if mcp <= 1:
        print("Batch appears finished (no large lu_mcp_v2 running).")
        break
    
    time.sleep(30)

print("\nLaunching original datasets training...")
os.system(f'start /B cmd /c "{TOOLS_DIR}\\run_originals.bat" > "{TOOLS_DIR}\\originals_runner_log.txt" 2>&1')
print("Originals batch launched! Check originals_runner_log.txt for progress.")
