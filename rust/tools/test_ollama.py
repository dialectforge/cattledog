# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

import urllib.request
import json
import time
import sys

model = sys.argv[1] if len(sys.argv) > 1 else "mistral:7b"
print(f"Testing {model}...", flush=True)
start = time.time()

payload = {
    "model": model,
    "prompt": 'Extract facts from this text: "The sun is a star. It provides light and heat to Earth." Output ONLY JSON: {"items": [{"content": "fact", "associations": ["related"]}]}',
    "stream": False,
    "format": "json",
    "options": {"num_ctx": 2048, "temperature": 0.2}
}

data = json.dumps(payload).encode("utf-8")
req = urllib.request.Request(
    "http://localhost:11434/api/generate",
    data=data,
    headers={"Content-Type": "application/json"}
)

try:
    with urllib.request.urlopen(req, timeout=300) as resp:
        result = json.loads(resp.read().decode("utf-8"))
        elapsed = time.time() - start
        response = result.get("response", "NO RESPONSE")
        print(f"Done in {elapsed:.1f}s", flush=True)
        print(f"Response: {response[:500]}", flush=True)
except Exception as e:
    elapsed = time.time() - start
    print(f"Error after {elapsed:.1f}s: {e}", flush=True)
