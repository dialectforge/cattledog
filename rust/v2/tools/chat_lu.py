# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

"""Chat with Lu via MCP - send queries and print responses."""
import json, subprocess, threading, queue, time, sys, io

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

MCP_EXE = r"<AI_FACTORY_ROOT>\v2\target\release\lu_mcp_v2.exe"

def start_mcp():
    proc = subprocess.Popen(
        [MCP_EXE],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return proc

def read_stdout(proc, out_q):
    while True:
        line = proc.stdout.readline()
        if not line:
            break
        out_q.put(line.decode("utf-8", errors="replace").strip())

def send_rpc(proc, method, params, msg_id):
    req = json.dumps({"jsonrpc": "2.0", "id": msg_id, "method": method, "params": params})
    raw = f"Content-Length: {len(req)}\r\n\r\n{req}"
    proc.stdin.write(raw.encode())
    proc.stdin.flush()

def wait_response(out_q, msg_id, timeout=300):
    deadline = time.time() + timeout
    buf = ""
    while time.time() < deadline:
        try:
            line = out_q.get(timeout=2)
            buf += line + "\n"
            if f'"id":{msg_id}' in line or f'"id": {msg_id}' in line:
                try:
                    obj = json.loads(line)
                    return obj
                except:
                    pass
            # Try parsing accumulated buffer for JSON
            for part in buf.split("\n"):
                part = part.strip()
                if part.startswith("{") and (f'"id":{msg_id}' in part or f'"id": {msg_id}' in part):
                    try:
                        return json.loads(part)
                    except:
                        pass
        except queue.Empty:
            continue
    return None

QUERIES = [
    "Hey Lu, what do you know about how the brain processes language? Like how do we actually understand words?",
    "That's interesting. So what happens when someone learns a second language as an adult vs a kid?",
    "Ok switching gears - can you explain what makes quantum computing different from regular computing? Don't just give me the textbook answer, help me actually understand it.",
    "So if quantum computers are so powerful, why can't I buy one at Best Buy?",
    "One more thing - what's your take on whether AI will ever be truly conscious? Not just smart, but actually aware?",
]

print("Starting MCP server...", flush=True)
proc = start_mcp()
out_q = queue.Queue()
t = threading.Thread(target=read_stdout, args=(proc, out_q), daemon=True)
t.start()

# Wait for brain to load
print("Waiting for brain to load...", flush=True)
send_rpc(proc, "initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "chat", "version": "1.0"}}, 0)
resp = wait_response(out_q, 0, timeout=300)
if resp:
    print("MCP initialized.", flush=True)

# Wait for brain ready via lu_status
for attempt in range(60):
    send_rpc(proc, "tools/call", {"name": "lu_status", "arguments": {}}, 99)
    r = wait_response(out_q, 99, timeout=30)
    if r and "result" in r:
        content = r["result"].get("content", [])
        text = content[0].get("text", "") if content else ""
        if "ready" in text.lower() or "nodes" in text.lower():
            print(f"Brain ready! {text[:100]}", flush=True)
            break
    print(f"  Waiting for brain... attempt {attempt+1}", flush=True)
    time.sleep(5)

results = []
for i, q in enumerate(QUERIES):
    print(f"\n{'='*60}", flush=True)
    print(f"ME: {q}", flush=True)
    print(f"{'='*60}", flush=True)
    
    send_rpc(proc, "tools/call", {"name": "brain_ask", "arguments": {"query": q}}, i+1)
    r = wait_response(out_q, i+1, timeout=120)
    
    if r and "result" in r:
        content = r["result"].get("content", [])
        text = content[0].get("text", "") if content else "(no response)"
    elif r and "error" in r:
        text = f"ERROR: {r['error']}"
    else:
        text = "(timeout - no response)"
    
    print(f"\nLU: {text}\n", flush=True)
    results.append({"query": q, "response": text})

# Save results
with open(r"<AI_FACTORY_ROOT>\v2\tools\chat_results.txt", "w", encoding="utf-8") as f:
    for r in results:
        f.write(f"ME: {r['query']}\n\n")
        f.write(f"LU: {r['response']}\n\n")
        f.write("="*60 + "\n\n")

print("\nDone! Results saved to chat_results.txt", flush=True)
proc.terminate()
