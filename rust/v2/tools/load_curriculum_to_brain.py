#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Load curriculum_data.json into Lu's brain via the MCP server.
Uses threading to avoid stdin/stdout deadlocks on Windows.
"""

import json
import subprocess
import sys
import os
import time
import threading
import queue

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
DEFAULT_CURRICULUM = os.path.join(SCRIPT_DIR, "curriculum_data.json")
MCP_EXE = r"<AI_FACTORY_ROOT>\target\release\lu_ai_mcp.exe"
BRAIN_PATH = r"<AI_FACTORY_ROOT>\brain"


class McpClient:
    def __init__(self, exe_path, brain_path):
        self.proc = subprocess.Popen(
            [exe_path, brain_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            bufsize=0,
        )
        self.response_queue = queue.Queue()
        self.req_id = 0

        # Reader thread to avoid deadlocks
        self.reader = threading.Thread(target=self._read_loop, daemon=True)
        self.reader.start()

        # Stderr reader
        self.err_reader = threading.Thread(target=self._err_loop, daemon=True)
        self.err_reader.start()

        time.sleep(1)
        if self.proc.poll() is not None:
            raise RuntimeError(f"MCP server exited with code {self.proc.returncode}")

    def _read_loop(self):
        try:
            for line in self.proc.stdout:
                line = line.decode("utf-8", errors="replace").strip()
                if line:
                    try:
                        resp = json.loads(line)
                        self.response_queue.put(resp)
                    except json.JSONDecodeError:
                        pass
        except Exception:
            pass

    def _err_loop(self):
        try:
            for line in self.proc.stderr:
                line = line.decode("utf-8", errors="replace").strip()
                if line and "[lu-mcp" in line.lower():
                    pass  # Suppress MCP server log noise
        except Exception:
            pass

    def call(self, method, params, timeout=60):
        self.req_id += 1
        request = {
            "jsonrpc": "2.0",
            "id": self.req_id,
            "method": method,
            "params": params
        }
        line = json.dumps(request, ensure_ascii=False) + "\n"
        try:
            self.proc.stdin.write(line.encode("utf-8"))
            self.proc.stdin.flush()
        except (OSError, BrokenPipeError) as e:
            return {"error": str(e)}

        try:
            resp = self.response_queue.get(timeout=timeout)
            return resp
        except queue.Empty:
            return {"error": "timeout"}

    def close(self):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        self.proc.wait(timeout=10)


def extract_result_text(resp):
    """Extract text from MCP tool response."""
    if not resp or "result" not in resp:
        return None
    result = resp["result"]
    if isinstance(result, dict) and "content" in result:
        for item in result["content"]:
            if item.get("type") == "text":
                try:
                    return json.loads(item["text"])
                except (json.JSONDecodeError, KeyError):
                    return item.get("text")
    return result


def main():
    curriculum_path = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_CURRICULUM
    batch_size = int(sys.argv[2]) if len(sys.argv) > 2 else 50

    print(f"{'='*60}")
    print(f"  LU AI CURRICULUM LOADER")
    print(f"{'='*60}")

    # Load curriculum
    print(f"\n[1/4] Loading curriculum from {os.path.basename(curriculum_path)}")
    with open(curriculum_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    all_facts = []
    for domain in data["domains"]:
        domain_facts = []
        for topic in domain["topics"]:
            for fact in topic["facts"]:
                domain_facts.append(fact)
        all_facts.extend(domain_facts)
        print(f"  {domain['label']}: {len(domain_facts)} facts")
    print(f"\n  Total: {len(all_facts)} facts")

    # Start MCP server
    print(f"\n[2/4] Starting MCP server...")
    try:
        client = McpClient(MCP_EXE, BRAIN_PATH)
    except RuntimeError as e:
        print(f"  FATAL: {e}")
        sys.exit(1)
    print(f"  MCP server running (PID {client.proc.pid})")

    # Initialize
    resp = client.call("initialize", {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "curriculum-loader", "version": "1.0"}
    })
    if resp and "error" not in resp:
        print(f"  Initialized OK")
    else:
        print(f"  Init response: {resp}")

    # Send initialized notification
    client.call("notifications/initialized", {})
    time.sleep(0.5)

    # Batch load
    print(f"\n[3/4] Loading {len(all_facts)} facts (batch size {batch_size})")
    total_loaded = 0
    total_bonds = 0
    start_time = time.time()
    total_batches = (len(all_facts) + batch_size - 1) // batch_size

    for i in range(0, len(all_facts), batch_size):
        batch = all_facts[i:i + batch_size]
        items = []
        for fact in batch:
            item = {"content": fact["content"]}
            if "associations" in fact:
                item["associations"] = fact["associations"]
            items.append(item)

        resp = client.call("tools/call", {
            "name": "brain_batch_train",
            "arguments": {"items": items}
        }, timeout=120)

        parsed = extract_result_text(resp)
        if isinstance(parsed, dict):
            total_loaded += parsed.get("total_learned", len(batch))
            total_bonds += parsed.get("total_bonds", 0)
        else:
            total_loaded += len(batch)

        batch_num = (i // batch_size) + 1
        elapsed = time.time() - start_time
        rate = total_loaded / elapsed if elapsed > 0 else 0
        eta = (len(all_facts) - i - batch_size) / rate if rate > 0 else 0

        if batch_num % 5 == 0 or batch_num == total_batches or batch_num <= 3:
            print(f"  [{batch_num}/{total_batches}] "
                  f"{total_loaded} nodes, {total_bonds} bonds "
                  f"({rate:.0f}/sec, ETA {eta:.0f}s)", flush=True)

    # Save
    print(f"\n[4/4] Saving brain...")
    resp = client.call("tools/call", {
        "name": "brain_save",
        "arguments": {}
    })
    save_result = extract_result_text(resp)
    print(f"  Save: {save_result}")

    # Stats
    resp = client.call("tools/call", {
        "name": "brain_stats",
        "arguments": {}
    })
    stats = extract_result_text(resp)
    print(f"\n  Brain Stats: {json.dumps(stats, indent=2) if isinstance(stats, dict) else stats}")

    # Health
    resp = client.call("tools/call", {
        "name": "brain_health_report",
        "arguments": {}
    })
    health = extract_result_text(resp)
    print(f"\n  Health: {json.dumps(health, indent=2) if isinstance(health, dict) else health}")

    elapsed = time.time() - start_time
    print(f"\n{'='*60}")
    print(f"  LOADING COMPLETE")
    print(f"  Facts processed: {len(all_facts)}")
    print(f"  Nodes created: ~{total_loaded}")
    print(f"  Bonds created: ~{total_bonds}")
    print(f"  Time: {elapsed:.0f}s ({elapsed/60:.1f}m)")
    print(f"{'='*60}")

    client.close()


if __name__ == "__main__":
    main()
