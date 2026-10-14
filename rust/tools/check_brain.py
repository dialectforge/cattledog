# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

import json
import os

path = os.path.join(os.environ.get("LOCALAPPDATA", ""), "ai_factory", "brain", "brain_state.json")
print(f"Reading: {path}")
print(f"Size: {os.path.getsize(path)} bytes")

with open(path, "r") as f:
    d = json.load(f)

nodes = d.get("network", {}).get("nodes", {})
bonds = d.get("network", {}).get("bonds", {})
active = sum(1 for n in nodes.values() if not n.get("revoked", False))

print(f"Nodes on disk: {len(nodes)}")
print(f"Active nodes: {active}")
print(f"Bonds on disk: {len(bonds)}")
