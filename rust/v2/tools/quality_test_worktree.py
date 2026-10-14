# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

"""
Quality test for Lu Brain after HuggingFace training.
Sends 40 prompts via MCP and grades each response.
"""
import json, subprocess, queue, threading, time, sys, io, re

if sys.stdout.encoding and sys.stdout.encoding.lower() not in ("utf-8", "utf8"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

MCP_EXE = r"<AI_FACTORY_ROOT>\v2\.claude\worktrees\hopeful-ride\target\release\lu_ai_mcp.exe"
BRAIN_PATH = r"<AI_FACTORY_ROOT>\brain"

# 40 test prompts across categories
PROMPTS = [
    # === Previously identified gaps ===
    "What is the Big Bang?",
    "What is the speed of light?",
    "What is the ozone layer?",
    "What causes earthquakes?",
    "What is a black hole?",
    "What is gravity?",
    "What is quantum mechanics?",
    "How does the moon affect tides?",
    # === Science (should be strong now) ===
    "What is photosynthesis?",
    "What is DNA?",
    "How does natural selection work?",
    "What is the greenhouse effect?",
    "What is an atom?",
    "How do vaccines work?",
    "What is the water cycle?",
    "What is evolution?",
    # === Programming (new knowledge) ===
    "What is a function in programming?",
    "What is recursion?",
    "What is an API?",
    "What is object-oriented programming?",
    "What is a database?",
    "How does a for loop work?",
    "What is machine learning?",
    "What is an algorithm?",
    # === General knowledge ===
    "What is climate change?",
    "How does electricity work?",
    "What is the internet?",
    "What causes thunder and lightning?",
    "What is the solar system?",
    "How do airplanes fly?",
    "What is a volcano?",
    "What is the immune system?",
    # === Harder / synthesis ===
    "What is the difference between a virus and bacteria?",
    "How does the brain process memory?",
    "What is nuclear fusion?",
    "Why is the sky blue?",
    "What is relativity?",
    "How do computers store data?",
    "What is plate tectonics?",
    "What is entropy?",
]


class McpClient:
    def __init__(self, exe, brain):
        self.proc = subprocess.Popen(
            [exe, brain], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, bufsize=0,
        )
        self._q = queue.Queue()
        self._rid = 0
        self._lock = threading.Lock()
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
        for _ in self.proc.stderr: pass

    def call(self, method, params, timeout=60.0):
        with self._lock:
            self._rid += 1
            rid = self._rid
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

    def tool(self, name, args, timeout=30.0):
        return self.call("tools/call", {"name":name,"arguments":args}, timeout)

    def close(self):
        try: self.proc.stdin.close()
        except: pass
        try: self.proc.wait(timeout=10)
        except: self.proc.kill()


def get_text(resp):
    if "result" not in resp:
        return f"[error] {resp.get('error',{}).get('message','?')}"
    r = resp["result"]
    if isinstance(r, dict) and "content" in r:
        raw = " ".join(i.get("text","") for i in r["content"] if i.get("type")=="text")
        # brain_ask returns JSON with a 'response' field  extract it for cleaner display/grading
        try:
            import json as _j
            parsed = _j.loads(raw)
            if "response" in parsed:
                return parsed["response"]
        except Exception:
            pass
        return raw
    return str(r)


def grade(prompt, response):
    """Simple auto-grading: check response quality."""
    r = response.lower()
    score = 0
    # Length check (at least a sentence)
    if len(response) > 50: score += 1
    if len(response) > 150: score += 1
    # Not an error or rejection
    if "[error]" not in r and "i don't" not in r and "no relevant" not in r:
        score += 1
    # Contains some relevant content (not just filler)
    words = prompt.lower().split()
    content_words = [w for w in words if len(w) > 3 and w not in
        ("what","how","does","the","that","this","with","from","about","work")]
    matches = sum(1 for w in content_words if w in r)
    if matches >= 1: score += 1
    if matches >= 2: score += 1
    return score  # 0-5


def main():
    print("=" * 70)
    print("  LU BRAIN v2 â QUALITY TEST (40 prompts)")
    print("=" * 70)
    print(f"  Loading brain from {BRAIN_PATH}...")
    print(f"  (This includes keyword index rebuild over all nodes)\n")

    client = McpClient(MCP_EXE, BRAIN_PATH)
    # Initialize
    client.call("initialize", {
        "protocolVersion":"2024-11-05","capabilities":{},
        "clientInfo":{"name":"quality-test","version":"1.0"}
    })
    client.call("tools/call", {"name":"lu_configure","arguments":{"autonomy_tier":"Autonomous"}})

    # Wait for brain to finish loading (keyword index rebuild over 1.7M+ nodes)
    print("  Waiting for brain to finish loading...")
    for attempt in range(60):
        resp = client.tool("lu_status", {}, timeout=10.0)
        status_text = get_text(resp)
        if "still loading" not in status_text.lower():
            print(f"  Brain ready! ({status_text[:120]})")
            break
        if attempt % 6 == 0:
            print(f"  Still loading... ({attempt * 5}s elapsed)")
        time.sleep(5)
    else:
        print("  WARNING: Brain may still be loading after 5 minutes, proceeding anyway...")

    print(f"\n  Running {len(PROMPTS)} prompts...\n")

    results = []
    total_score = 0
    for i, prompt in enumerate(PROMPTS):
        resp = client.tool("brain_ask", {"query": prompt}, timeout=30.0)
        text = get_text(resp)
        score = grade(prompt, text)
        total_score += score
        results.append((prompt, text, score))

        # Truncate display
        display = text[:200].replace("\n"," ")
        if len(text) > 200: display += "..."
        star = "*" * score
        print(f"  [{i+1:2d}/40] ({score}/5) {star:5s} | {prompt}")
        print(f"          {display}\n")

    # Summary
    max_score = len(PROMPTS) * 5
    pct = (total_score / max_score) * 100
    print("=" * 70)
    print(f"  RESULTS: {total_score}/{max_score} ({pct:.1f}%)")
    print("=" * 70)

    # Category breakdowns
    categories = [
        ("Knowledge Gaps (prev failures)", 0, 8),
        ("Science", 8, 16),
        ("Programming", 16, 24),
        ("General Knowledge", 24, 32),
        ("Harder / Synthesis", 32, 40),
    ]
    for name, start, end in categories:
        cat_score = sum(r[2] for r in results[start:end])
        cat_max = (end - start) * 5
        cat_pct = (cat_score / cat_max) * 100
        print(f"  {name:35s}: {cat_score}/{cat_max} ({cat_pct:.1f}%)")

    # Flag failures
    failures = [r for r in results if r[2] <= 1]
    if failures:
        print(f"\n  FAILURES (score <= 1):")
        for prompt, text, score in failures:
            print(f"    - {prompt}: {text[:100]}")

    print("=" * 70)
    client.close()


if __name__ == "__main__":
    main()
