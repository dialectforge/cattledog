# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

"""Diagnostic: check scar detection in propose()"""
import subprocess, json, sys, time

MCP = r"<AI_FACTORY_ROOT>\target\debug\ai_factory_mcp.exe"

proc = subprocess.Popen(
    [MCP],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    bufsize=0,
)

def send(method, params=None):
    msg = {"jsonrpc": "2.0", "id": 1, "method": method}
    if params:
        msg["params"] = params
    raw = json.dumps(msg) + "\n"
    proc.stdin.write(raw.encode())
    proc.stdin.flush()
    line = proc.stdout.readline().decode().strip()
    return json.loads(line) if line else None

# Initialize
send("initialize", {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {"name": "diag", "version": "1"}
})

# Wait for brain load
time.sleep(6)

# Call propose
resp = send("tools/call", {
    "name": "brain_propose",
    "arguments": {
        "content": "Dogs are loyal pets that have lived alongside humans for thousands of years as companions and helpers",
        "source": "scar_diag"
    }
})

# Now read stderr for our debug lines
proc.stdin.close()
time.sleep(1)
stderr_out = proc.stderr.read().decode()

print("=== STDERR (debug) ===")
for line in stderr_out.split("\n"):
    if "propose" in line.lower() or "scar" in line.lower():
        print(line)

print("\n=== RESULT ===")
if resp and "result" in resp:
    content = resp["result"].get("content", [{}])
    if content:
        text = content[0].get("text", "")
        try:
            data = json.loads(text)
            print(f"recommendation: {data.get('recommendation')}")
            print(f"scar_conflicts: {data.get('scar_conflicts')}")
        except:
            print(text[:500])

proc.terminate()
