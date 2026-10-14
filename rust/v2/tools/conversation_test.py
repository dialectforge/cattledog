# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

"""Multi-turn conversation test with Lu — tests recall, narration, thinking, curiosity, session memory, and web research."""
import json, subprocess, queue, threading, time, sys, io

if sys.stdout.encoding and sys.stdout.encoding.lower() not in ("utf-8", "utf8"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

MCP_EXE = r"<AI_FACTORY_ROOT>\v2\target\release\lu_mcp_v2.exe"
BRAIN   = r"<AI_FACTORY_ROOT>\v2\brain\lu_brain.lbr"

class McpClient:
    def __init__(self, exe, brain):
        self.proc = subprocess.Popen(
            [exe, brain], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, bufsize=0,
        )
        self._q    = queue.Queue()
        self._rid  = 0
        self._lock = threading.Lock()
        self._err_lines: list[str] = []
        self._err_lock  = threading.Lock()
        threading.Thread(target=self._read,     daemon=True).start()
        threading.Thread(target=self._read_err, daemon=True).start()
        time.sleep(1.0)

    def _read(self):
        for raw in self.proc.stdout:
            line = raw.decode("utf-8", errors="replace").strip()
            if line:
                try: self._q.put(json.loads(line))
                except: pass

    def _read_err(self):
        for raw in self.proc.stderr:
            line = raw.decode("utf-8", errors="replace").rstrip()
            if line:
                with self._err_lock:
                    self._err_lines.append(line)

    def drain_stderr(self) -> list[str]:
        with self._err_lock:
            lines = list(self._err_lines)
            self._err_lines.clear()
        return lines

    def call(self, method, params, timeout=120.0):
        with self._lock:
            self._rid += 1; rid = self._rid
        req = {"jsonrpc": "2.0", "id": rid, "method": method, "params": params}
        self.proc.stdin.write((json.dumps(req) + "\n").encode("utf-8"))
        self.proc.stdin.flush()
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                resp = self._q.get(timeout=5)
                if resp.get("id") == rid:
                    return resp
                self._q.put(resp)
            except queue.Empty:
                pass
        return {"error": {"message": "timeout"}}

    def tool(self, name, args, timeout=120.0):
        return self.call("tools/call", {"name": name, "arguments": args}, timeout)

    def close(self):
        try: self.proc.stdin.close()
        except: pass
        try: self.proc.wait(timeout=10)
        except: self.proc.kill()


def get_text(resp) -> str:
    if "result" not in resp:
        return f"[error] {resp.get('error', {}).get('message', '?')}"
    r = resp["result"]
    if isinstance(r, dict) and "content" in r:
        return " ".join(i.get("text", "") for i in r["content"] if i.get("type") == "text")
    return str(r)


def section(label: str, width: int = 70):
    print(f"\n{'=' * width}", flush=True)
    print(label, flush=True)
    print('=' * width, flush=True)


# ---------------------------------------------------------------------------
# Kill any stale lu_mcp_v2.exe
# ---------------------------------------------------------------------------
import subprocess as _sp
try:
    _sp.run(["taskkill", "/F", "/IM", "lu_mcp_v2.exe"],
            capture_output=True, timeout=10)
    print("[init] Killed existing lu_mcp_v2.exe (if any).", flush=True)
    time.sleep(2)
except Exception as e:
    print(f"[init] taskkill note: {e}", flush=True)


# ---------------------------------------------------------------------------
# Start server and wait for brain to be ready
# ---------------------------------------------------------------------------
section("[1/3] Starting MCP server and loading brain (~42s)...")
client = McpClient(MCP_EXE, BRAIN)

brain_ready = False
for attempt in range(60):
    r = client.tool("lu_status", {}, timeout=15)
    t = get_text(r)
    still_loading = "still loading" in t.lower() or "retry" in t.lower()
    is_ready = (not still_loading) and (
        "nodes" in t.lower() or "memory" in t.lower() or
        "ready" in t.lower() or len(t) > 80
    )
    if is_ready:
        print(f"[3/3] Brain ready!  {t[:200]}", flush=True)
        brain_ready = True
        break
    print(f"  Loading... attempt {attempt + 1}: {t[:120]}", flush=True)
    time.sleep(3)

if not brain_ready:
    print("[WARNING] Brain may not be fully loaded — proceeding anyway.", flush=True)

startup_err = client.drain_stderr()
if startup_err:
    print("\n[STARTUP STDERR]", flush=True)
    for line in startup_err:
        print(f"  {line}", flush=True)


# ---------------------------------------------------------------------------
# 6 fixed conversation turns — send all regardless of Lu's responses
# ---------------------------------------------------------------------------
TURNS = [
    # 1. Open question
    "How does music affect the brain? I'm curious about what actually happens neurologically when we listen to a song.",
    # 2. Go deeper on emotional response
    "You mentioned the brain — can you go deeper on the emotional side? What's happening neurologically when a piece of music gives you chills or makes you tear up?",
    # 3. Opinion on music therapy
    "What do you think about using music as therapy — like for people with depression or Alzheimer's? Does the science actually support that?",
    # 4. Reasoning question — universal vs learned
    "Do you think there's a universal quality to music that all humans respond to regardless of culture, or is our emotional response to music entirely learned?",
    # 5. If Lu offered to look something up — accept it
    "Yes, please look into that — I'd love to know what the latest research says.",
    # 6. Synthesis / teaching children
    "So pulling everything together from what you've told me — if music really does wire the brain in these ways, what would that mean for how we should teach music to children?",
]

section("CONVERSATION")
responses: list[tuple[str, str]] = []

for i, query in enumerate(TURNS, 1):
    print(f"\n--- Turn {i} ---", flush=True)
    print(f"\nME: {query}", flush=True)

    r    = client.tool("lu_conversation", {"message": query}, timeout=120)
    text = get_text(r)

    print(f"\nLU: {text}", flush=True)

    err_lines = client.drain_stderr()
    if err_lines:
        print(f"\n[DEBUG stderr — turn {i}]", flush=True)
        for line in err_lines[-40:]:
            print(f"  {line}", flush=True)

    responses.append((query, text))
    time.sleep(2)


# ---------------------------------------------------------------------------
# Status check — wonder queue
# ---------------------------------------------------------------------------
section("STATUS CHECK — wonder queue & curiosity")
r_status    = client.tool("lu_status", {}, timeout=30)
status_text = get_text(r_status)
print(f"\nSTATUS:\n{status_text}", flush=True)

err_lines = client.drain_stderr()
if err_lines:
    print("\n[DEBUG stderr — status check]", flush=True)
    for line in err_lines[-20:]:
        print(f"  {line}", flush=True)


# ---------------------------------------------------------------------------
# Analysis
# ---------------------------------------------------------------------------
section("ANALYSIS")
all_lu_text = " ".join(r for _, r in responses).lower()

print("\n--- Coherence / topic keywords ---", flush=True)
music_words = ["music", "brain", "neural", "dopamine", "emotion", "therapy",
               "alzheimer", "culture", "chills", "children", "learn", "cortex",
               "memory", "rhythm", "melody", "mood"]
hits = [w for w in music_words if w in all_lu_text]
print(f"  Keywords found across all responses: {hits}", flush=True)

print("\n--- Session context carry-over ---", flush=True)
if len(responses) >= 4:
    later = responses[3][1].lower()
    carried = [t for t in ["emotion", "dopamine", "neural", "brain", "therapy", "music"]
               if t in later]
    print(f"  Turn-4 references earlier topics: {carried}", flush=True)

print("\n--- Wonder queue / curiosity ---", flush=True)
wonder_indicators = ["wonder", "curious", "queue", "pending", "enqueued", "0 pending", "1 pending"]
found_wonder = [w for w in wonder_indicators if w in status_text.lower()]
print(f"  Wonder indicators in status: {found_wonder}", flush=True)
if any(w in status_text.lower() for w in ["pending", "wonder", "curious", "queue"]):
    print("  => Curiosity system appears active.", flush=True)
else:
    print("  => No wonder queue entries detected.", flush=True)

print("\n--- Web research triggered ---", flush=True)
research_indicators = ["search", "found", "retrieved", "web", "source", "article",
                       "according to", "research shows", "studies show", "study"]
research_hits = [w for w in research_indicators if w in all_lu_text]
print(f"  Research indicators in responses: {research_hits}", flush=True)

print("\n--- Thinking layer / opinion turns ---", flush=True)
for i, (q, resp) in enumerate(responses, 1):
    if "do you think" in q.lower() or "what do you think" in q.lower():
        hedges = ["i think", "in my view", "i believe", "it seems", "perhaps",
                  "it depends", "complex", "interesting", "uncertain"]
        found = [h for h in hedges if h in resp.lower()]
        print(f"  Turn {i} (opinion question) markers: {found}", flush=True)

client.close()
print("\n[DONE]", flush=True)
