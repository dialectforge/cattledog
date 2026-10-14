#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Lu Ai - Ollama Training Bridge

Spawns the Lu Ai MCP server as a subprocess, generates knowledge
from a topic using Ollama, and feeds it to the brain.

IMPORTANT: Kill Claude Desktop's MCP server before running this script,
or use train_ollama.bat which does it automatically.

Usage:
    python train.py "ocean creatures"
    python train.py "human body" --model mistral
    python train.py "music theory" --count 12
    python train.py --interactive
"""

import json
import subprocess
import sys
import argparse
import urllib.request
import urllib.error
import time
import threading
import re
import os

# --- Config ---
# Always use the release build - it has the latest Brain struct
import os as _os
MCP_EXE = r"<AI_FACTORY_ROOT>\target\release\lu_ai_mcp.exe"
OLLAMA_URL = "http://localhost:11434/api/generate"
DEFAULT_MODEL = "llama3.1:8b"
DEFAULT_COUNT = 8


def find_brain_path():
    """
    Brain lives in the project directory on D: drive.
    One brain, one location.
    """
    project_path = r"<AI_FACTORY_ROOT>\brain"
    if os.path.isdir(project_path):
        print(f"[bridge] Brain path: {project_path}")
        return project_path

    # Fallback: create it
    os.makedirs(project_path, exist_ok=True)
    print(f"[bridge] Created brain path: {project_path}")
    return project_path


BRAIN_PATH = find_brain_path()


# ─── MCP Bridge ─────────────────────────────────────────────────────────────

class MCPBridge:
    """Manages a subprocess connection to the Lu Ai MCP server."""

    def __init__(self):
        self.proc = None
        self.msg_id = 0
        self._stderr_lines = []

    def start(self):
        print(f"[bridge] Starting MCP server: {MCP_EXE}")
        print(f"[bridge] Brain path: {BRAIN_PATH}")
        self.proc = subprocess.Popen(
            [MCP_EXE, BRAIN_PATH],  # pass brain path explicitly
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )

        # Read stderr in background so it doesn't block
        def read_stderr():
            for line in self.proc.stderr:
                self._stderr_lines.append(line.strip())
                print(f"  [mcp] {line.strip()}")
        t = threading.Thread(target=read_stderr, daemon=True)
        t.start()

        time.sleep(0.8)  # wait for MCP to initialize

        # MCP handshake
        resp = self._call("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "ollama-bridge", "version": "0.2.0"}
        })
        if not (resp and "result" in resp):
            print("[bridge] ERROR: MCP server did not respond to init.")
            self.stop()
            sys.exit(1)

        # Send initialized notification
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

        # Verify brain loaded correctly
        stats = self._get_stats()
        if stats is None:
            print("[bridge] ERROR: Could not read brain stats.")
            self.stop()
            sys.exit(1)

        nodes = stats.get("active_nodes", 0)
        bonds = stats.get("active_bonds", 0)

        # Check for bond fingerprint mismatch in stderr
        mismatch = any("fingerprint mismatch" in l.lower() for l in self._stderr_lines)
        if mismatch:
            print("[bridge] ERROR: Bond fingerprint mismatch — wrong brain loaded.")
            print("[bridge] The brain directory is locked to a different terminal.")
            self.stop()
            sys.exit(1)

        if nodes == 0:
            brain_file = os.path.join(BRAIN_PATH, "brain_state.json")
            if os.path.exists(brain_file) and os.path.getsize(brain_file) > 1000:
                print("[bridge] DANGER: Brain file exists but loaded 0 nodes!")
                print("[bridge] Aborting to prevent overwriting your data.")
                self.stop()
                sys.exit(1)

        print(f"[bridge] Brain ready — {nodes} nodes, {bonds} bonds.")
        return self

    def stop(self):
        if self.proc:
            try:
                self.proc.stdin.close()
                self.proc.wait(timeout=5)
            except Exception:
                self.proc.kill()
            print("[bridge] MCP server stopped.")

    def _extract_text(self, result):
        items = result
        if isinstance(result, dict):
            items = result.get("content", [])
        if isinstance(items, list) and len(items) > 0:
            if isinstance(items[0], dict):
                return items[0].get("text", "")
        return None

    def _send(self, msg):
        line = json.dumps(msg) + "\n"
        self.proc.stdin.write(line)
        self.proc.stdin.flush()

    def _call(self, method, params=None, timeout=120):
        self.msg_id += 1
        msg = {"jsonrpc": "2.0", "id": self.msg_id, "method": method}
        if params is not None:
            msg["params"] = params
        self._send(msg)

        # Read with timeout to prevent hangs
        response_line = [None]
        def read_line():
            response_line[0] = self.proc.stdout.readline()

        reader = threading.Thread(target=read_line, daemon=True)
        reader.start()
        reader.join(timeout=timeout)

        if reader.is_alive():
            print(f"[bridge] TIMEOUT after {timeout}s — restarting MCP...")
            self._restart()
            return {"error": "timeout"}

        try:
            if not response_line[0]:
                return None
            return json.loads(response_line[0].strip())
        except Exception as e:
            print(f"[bridge] Read error: {e}")
            return None

    def _restart(self):
        """Kill and restart the MCP process after a timeout."""
        try:
            self.proc.kill()
            self.proc.wait(timeout=5)
        except Exception:
            pass
        print("[bridge] Restarting MCP server...")
        self.proc = subprocess.Popen(
            [MCP_EXE, BRAIN_PATH],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        # Re-read stderr in background
        def read_stderr():
            try:
                for line in self.proc.stderr:
                    self._stderr_lines.append(line.strip())
            except Exception:
                pass
        t = threading.Thread(target=read_stderr, daemon=True)
        t.start()
        time.sleep(3)  # brain is 40MB+ bincode, needs time to load
        self.msg_id = 0
        self._call("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "ollama-bridge", "version": "0.2.0"}
        }, timeout=30)
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})
        print("[bridge] MCP server restarted.")

    def _get_stats(self):
        resp = self._call("tools/call", {"name": "brain_stats", "arguments": {}})
        if resp and "result" in resp:
            text = self._extract_text(resp["result"])
            if text:
                try:
                    return json.loads(text)
                except Exception:
                    pass
        return None

    def batch_train(self, items):
        for attempt in range(3):
            resp = self._call("tools/call", {
                "name": "brain_batch_train",
                "arguments": {"items": items}
            })
            if resp and resp.get("error") == "timeout":
                print(f"  [bridge] Retry {attempt+1}/2...", flush=True)
                continue
            break
        if resp and "result" in resp:
            text = self._extract_text(resp["result"])
            if text:
                try:
                    return json.loads(text)
                except Exception:
                    return {"raw": text}
        return None

    def save(self):
        self._call("tools/call", {"name": "brain_save", "arguments": {}})


# ─── Ollama ──────────────────────────────────────────────────────────────────

SYSTEM_PROMPT = """You are a knowledge formatter for an AI training system.
You output ONLY valid JSON — no markdown, no explanation, no code blocks.
Your JSON must have this exact structure:
{"items": [{"content": "fact", "associations": ["related fact"]}]}"""

def ask_ollama(prompt, model=DEFAULT_MODEL, use_json_mode=True):
    """Call Ollama and return the response text."""
    payload = {
        "model": model,
        "prompt": prompt,
        "system": SYSTEM_PROMPT,
        "stream": False,
        "options": {"temperature": 0.4, "top_p": 0.9}
    }
    if use_json_mode:
        payload["format"] = "json"  # Ollama 0.2+ structured output

    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        OLLAMA_URL, data=data,
        headers={"Content-Type": "application/json"}
    )
    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            result = json.loads(resp.read().decode("utf-8"))
            return result.get("response", "")
    except urllib.error.URLError as e:
        print(f"[ollama] Connection error: {e}")
        print("[ollama] Is Ollama running? Start with: ollama serve")
        return None


def parse_items(text):
    """Parse training items from Ollama response. Handles messy output."""
    if not text:
        return None

    # Strip markdown code fences if present
    text = text.strip()
    if "```" in text:
        text = re.sub(r'```(?:json)?\n?', '', text).strip()

    # Remove trailing commas before ] or } (common LLM mistake)
    text = re.sub(r',\s*([}\]])', r'\1', text)

    # Try direct parse first
    try:
        data = json.loads(text)
        return data.get("items", [])
    except json.JSONDecodeError:
        pass

    # Try extracting JSON object from surrounding text
    match = re.search(r'\{[\s\S]*\}', text)
    if match:
        try:
            data = json.loads(match.group())
            return data.get("items", [])
        except json.JSONDecodeError:
            pass

    print(f"[ollama] Could not parse response:\n{text[:400]}")
    return None


def generate_knowledge(topic, count=DEFAULT_COUNT, model=DEFAULT_MODEL):
    """Generate structured training items for a topic using Ollama."""

    prompt = f"""Generate exactly {count} educational facts about: {topic}

Output ONLY this JSON structure, nothing else:
{{
  "items": [
    {{"content": "A complete factual sentence about {topic}", "associations": ["A related fact that connects to this"]}},
    {{"content": "Another distinct fact about {topic}", "associations": ["A connecting fact"]}}
  ]
}}

Rules:
- Each "content" must be a single clear sentence (10-20 words)
- Use simple, precise language
- Each fact must be distinct — no repetition
- Each "associations" array has exactly 1 item that is a related fact
- Do not use markdown, headers, or explanation — ONLY the JSON"""

    print(f"[ollama] Generating {count} facts about '{topic}' with {model}...")
    response = ask_ollama(prompt, model)
    if not response:
        return None

    items = parse_items(response)
    if items:
        # Validate items have required fields
        valid = [i for i in items if isinstance(i, dict) and "content" in i]
        if not valid:
            print("[ollama] No valid items in response.")
            return None
        print(f"[ollama] Got {len(valid)} valid facts.")
        return valid

    # Retry without JSON mode if first attempt failed
    print("[ollama] Retrying without JSON mode...")
    response = ask_ollama(prompt, model, use_json_mode=False)
    items = parse_items(response)
    if items:
        valid = [i for i in items if isinstance(i, dict) and "content" in i]
        print(f"[ollama] Got {len(valid)} valid facts (retry).")
        return valid

    return None


# ─── Training ────────────────────────────────────────────────────────────────

def train_topic(topic, count=DEFAULT_COUNT, model=DEFAULT_MODEL, bridge=None):
    """Train a single topic. Optionally accepts an existing bridge."""
    items = generate_knowledge(topic, count, model)
    if not items:
        print(f"[train] Failed to generate facts for '{topic}'. Skipping.")
        return False

    own_bridge = bridge is None
    if own_bridge:
        bridge = MCPBridge().start()

    stats_before = bridge._get_stats()
    nodes_before = stats_before.get("active_nodes", 0) if stats_before else 0

    print(f"[train] Feeding {len(items)} facts to brain...")
    result = bridge.batch_train(items)

    stats_after = bridge._get_stats()
    nodes_after = stats_after.get("active_nodes", 0) if stats_after else 0

    if result:
        learned = result.get("total_learned", len(items))
        bonds = result.get("total_bonds", 0)
        print(f"[train] Learned: {learned} nodes (+{nodes_after - nodes_before} new), {bonds} bonds")
    
    print(f"[train] Brain: {nodes_before} → {nodes_after} nodes")

    if own_bridge:
        bridge.stop()

    return True


def interactive_mode(model=DEFAULT_MODEL):
    """Interactive multi-topic training session."""
    bridge = MCPBridge().start()
    stats = bridge._get_stats()
    nodes = stats.get("active_nodes", 0) if stats else 0
    bonds = stats.get("active_bonds", 0) if stats else 0

    print(f"\n{'='*50}")
    print(f"  Lu Ai — Interactive Training")
    print(f"  Brain: {nodes} nodes, {bonds} bonds")
    print(f"  Model: {model}")
    print(f"{'='*50}")
    print("  Type a topic to train. Prefix with number for count.")
    print("  Examples:  caves       20 cooking       quit")
    print(f"{'='*50}\n")

    session_topics = 0
    while True:
        try:
            raw = input("topic> ").strip()
        except (EOFError, KeyboardInterrupt):
            break

        if not raw or raw.lower() in ("quit", "exit", "q"):
            break

        # Parse optional count prefix: "20 ocean fish"
        parts = raw.split(None, 1)
        count = DEFAULT_COUNT
        topic = raw
        if len(parts) == 2 and parts[0].isdigit():
            count = int(parts[0])
            topic = parts[1]

        success = train_topic(topic, count, model, bridge=bridge)
        if success:
            session_topics += 1
            stats = bridge._get_stats()
            if stats:
                n = stats.get("active_nodes", 0)
                b = stats.get("active_bonds", 0)
                print(f"[brain] {n} nodes | {b} bonds\n")

    bridge.stop()
    print(f"\nSession complete — trained {session_topics} topic(s).")
    print("Restart Claude Desktop to load the updated brain.")


# ─── Entry Point ─────────────────────────────────────────────────────────────

if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Lu Ai Ollama Training Bridge",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""Examples:
  python train.py "ocean creatures"
  python train.py "volcanoes" --count 12
  python train.py "cooking" --model mistral
  python train.py --interactive"""
    )
    parser.add_argument("topic", nargs="?", help="Topic to train")
    parser.add_argument("--model", "-m", default=DEFAULT_MODEL,
                        help=f"Ollama model (default: {DEFAULT_MODEL})")
    parser.add_argument("--count", "-c", type=int, default=DEFAULT_COUNT,
                        help=f"Facts per topic (default: {DEFAULT_COUNT})")
    parser.add_argument("--interactive", "-i", action="store_true",
                        help="Interactive multi-topic session")
    args = parser.parse_args()

    if args.interactive:
        interactive_mode(args.model)
    elif args.topic:
        bridge = MCPBridge().start()
        train_topic(args.topic, args.count, args.model, bridge=bridge)
        bridge.stop()
        print("Restart Claude Desktop to load the updated brain.")
    else:
        parser.print_help()
