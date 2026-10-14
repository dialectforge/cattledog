# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

"""Chat with Lu - debug version that captures stderr for recall pipeline tracing."""
import json, subprocess, queue, threading, time, sys, io

if sys.stdout.encoding and sys.stdout.encoding.lower() not in ("utf-8", "utf8"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

MCP_EXE = r"<AI_FACTORY_ROOT>\v2\target\release\lu_mcp_v2.exe"
BRAIN   = r"<AI_FACTORY_ROOT>\v2\brain\lu_brain.lbr"
STDERR_LOG = r"<AI_FACTORY_ROOT>\v2\tools\chat_debug_stderr.txt"

class McpClient:
    def __init__(self, exe, brain):
        self.proc = subprocess.Popen(
            [exe, brain], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, bufsize=0,
        )
        self._q = queue.Queue()
        self._rid = 0
        self._lock = threading.Lock()
        self._err_file = open(STDERR_LOG, "w", encoding="utf-8", errors="replace")
        threading.Thread(target=self._read, daemon=True).start()
        threading.Thread(target=self._drain_err, daemon=True).start()
        time.sleep(1.0)
    def _read(self):
        for raw in self.proc.stdout:
            line = raw.decode("utf-8", errors="replace").strip()
            if line:
                try: self._q.put(json.loads(line))
                except: pass
    def _drain_err(self):
        for raw in self.proc.stderr:
            line = raw.decode("utf-8", errors="replace")
            self._err_file.write(line)
            self._err_file.flush()
    def call(self, method, params, timeout=60.0):
        with self._lock:
            self._rid += 1; rid = self._rid
        req = {"jsonrpc":"2.0","id":rid,"method":method,"params":params}
        self.proc.stdin.write((json.dumps(req)+"\n").encode("utf-8"))
        self.proc.stdin.flush()
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                resp = self._q.get(timeout=5)
                if resp.get("id") == rid: return resp
                self._q.put(resp)
            except queue.Empty: pass
        return {"error":{"message":"timeout"}}
    def tool(self, name, args, timeout=90.0):
        return self.call("tools/call",{"name":name,"arguments":args},timeout)
    def close(self):
        try: self.proc.stdin.close()
        except: pass
        try: self.proc.wait(timeout=10)
        except: self.proc.kill()
        try: self._err_file.close()
        except: pass

def get_text(resp):
    if "result" not in resp:
        return f"[error] {resp.get('error',{}).get('message','?')}"
    r = resp["result"]
    if isinstance(r, dict) and "content" in r:
        return " ".join(i.get("text","") for i in r["content"] if i.get("type")=="text")
    return str(r)

print("[1/3] Starting MCP server...", flush=True)
client = McpClient(MCP_EXE, BRAIN)
print("[2/3] Waiting for brain to load...", flush=True)
for attempt in range(120):
    r = client.tool("lu_status", {}, timeout=15)
    t = get_text(r)
    if "nodes" in t.lower():
        print(f"[3/3] Brain ready! {t[:120]}", flush=True)
        break
    print(f"  Loading... ({attempt+1})", flush=True)
    time.sleep(3)

QUERIES = [
    "How does the brain process language?",
    "Second language adult vs kid?",
    "What is quantum computing?",
]

out = open(r"<AI_FACTORY_ROOT>\v2\tools\chat_output6.txt", "w", encoding="utf-8")
for q in QUERIES:
    sep = "=" * 60
    print(f"\n{sep}", flush=True)
    out.write(f"\n{sep}\nQ: {q}\n")
    out.flush()
    print(f"Q: {q}", flush=True)
    r = client.tool("lu_conversation", {"message": q}, timeout=90)
    text = get_text(r)
    print(f"A: {text}", flush=True)
    out.write(f"A: {text}\n")
    out.flush()

out.close()
client.close()
print(f"\nDone. Debug stderr saved to {STDERR_LOG}", flush=True)
