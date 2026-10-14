# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

import json
path = r'<USER_HOME>\AppData\Local\ai_factory\brain\brain_state.json'
with open(path) as f:
    d = json.load(f)
nodes = d.get('network', {}).get('nodes', {})
active = [n for n in nodes.values() if not n.get('revoked', False)]
print('Nodes on disk:', len(active))

# Also check a fingerprint node that only exists in our brain
labels = [n['label'] for n in nodes.values() if not n.get('revoked', False)]
stalactite = [l for l in labels if 'stalactite' in l.lower()]
print('Stalactite node:', stalactite if stalactite else 'NOT FOUND - this is the Ollama brain')
