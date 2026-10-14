# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

import json, subprocess, time, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

MCP = r"<AI_FACTORY_ROOT>\v2\target\release\lu_mcp_v2.exe"
OUT = r"<AI_FACTORY_ROOT>\v2\tools\chat_results.txt"

def rpc(method, params, mid):
    req = json.dumps({"jsonrpc":"2.0","id":mid,"method":method,"params":params})
    return f"Content-Length: {len(req)}\r\n\r\n{req}".encode()

proc = subprocess.Popen([MCP], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
print("Started MCP, sending init...", flush=True)
proc.stdin.write(rpc("initialize", {"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"c","version":"1"}}, 0))
proc.stdin.flush()

# Read responses by accumulating until we find our JSON
def read_response(proc, target_id, timeout=300):
    buf = b""
    deadline = time.time() + timeout
    while time.time() < deadline:
        byte = proc.stdout.read(1)
        if not byte:
            break
        buf += byte
        try:
            text = buf.decode("utf-8", errors="replace")
            for line in text.split("\n"):
                line = line.strip()
                if line.startswith("{") and f'"id":{target_id}' in line.replace(" ",""):
                    return json.loads(line)
        except:
            pass
    return None

print("Waiting for init response...", flush=True)
r = read_response(proc, 0, timeout=300)
print(f"Init: {'OK' if r else 'TIMEOUT'}", flush=True)

# Poll lu_status until brain ready
for attempt in range(120):
    proc.stdin.write(rpc("tools/call", {"name":"lu_status","arguments":{}}, 99))
    proc.stdin.flush()
    r = read_response(proc, 99, timeout=10)
    if r and "result" in r:
        c = r["result"].get("content",[])
        t = c[0].get("text","") if c else ""
        if "nodes" in t.lower():
            print(f"Brain ready: {t[:120]}", flush=True)
            break
    print(f"  Loading brain... ({attempt+1})", flush=True)
    time.sleep(3)

QUERIES = [
    "Hey Lu, what do you know about how the brain processes language? Like how do we actually understand words?",
    "That's interesting. So what happens when someone learns a second language as an adult vs a kid?",
    "Ok switching gears - can you explain what makes quantum computing different from regular computing? Don't just give me the textbook answer, help me actually understand it.",
    "So if quantum computers are so powerful, why can't I buy one at Best Buy?",
    "One more thing - what's your take on whether AI will ever be truly conscious? Not just smart, but actually aware?",
]

f = open(OUT, "w", encoding="utf-8")
for i, q in enumerate(QUERIES):
    print(f"\nME: {q}", flush=True)
    proc.stdin.write(rpc("tools/call", {"name":"brain_ask","arguments":{"query":q}}, i+1))
    proc.stdin.flush()
    r = read_response(proc, i+1, timeout=120)
    if r and "result" in r:
        c = r["result"].get("content",[])
        text = c[0].get("text","") if c else "(empty)"
    else:
        text = "(no response)"
    print(f"LU: {text}\n", flush=True)
    f.write(f"ME: {q}\n\nLU: {text}\n\n{'='*60}\n\n")
    f.flush()

f.close()
proc.terminate()
print("Done!", flush=True)
