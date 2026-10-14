# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

import json

paths = [
    r'<USER_HOME>\AppData\Local\ai_factory\brain\brain_state.json',
    r'<USER_HOME>\AppData\Local\Packages\<CLAUDE_PACKAGE>\LocalCache\Local\ai_factory\brain\brain_state.json',
]

for path in paths:
    try:
        with open(path) as f:
            d = json.load(f)
        nodes = d.get('network', {}).get('nodes', {})
        active = [n for n in nodes.values() if not n.get('revoked', False)]
        labels = [n['label'] for n in active]
        stalactite = [l for l in labels if 'stalactite' in l.lower()]
        print(f"\nPath: {path}")
        print(f"  Nodes: {len(active)}")
        print(f"  Stalactite: {'FOUND (our brain)' if stalactite else 'NOT FOUND (Ollama brain)'}")
    except Exception as e:
        print(f"\nPath: {path}")
        print(f"  Error: {e}")
