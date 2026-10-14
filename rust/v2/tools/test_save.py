# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

"""Quick test: spawn lu_mcp_v2, initialise, call lu_save, print result."""
import subprocess, json, time, sys

EXE   = r"<AI_FACTORY_ROOT>\v2\target\release\lu_mcp_v2.exe"
BRAIN = r"<AI_FACTORY_ROOT>\v2\brain\lu_brain.lbr"

proc = subprocess.Popen(
    [EXE, BRAIN],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
)
time.sleep(3)

def send(method, params, rid):
    msg = json.dumps({"jsonrpc": "2.0", "id": rid, "method": method, "params": params}) + "\n"
    proc.stdin.write(msg.encode())
    proc.stdin.flush()
    line = proc.stdout.readline().decode().strip()
    return json.loads(line) if line else None

print("Init:", send("initialize", {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {"name": "test", "version": "1.0"},
}, 1))

time.sleep(2)

print("Save:", send("tools/call", {"name": "lu_save", "arguments": {}}, 2))

proc.stdin.close()
proc.wait(timeout=10)
