# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

import json, os

paths = [
    r'<USER_HOME>\AppData\Local\ai_factory\brain',
    r'<USER_HOME>\AppData\Local\Packages\<CLAUDE_PACKAGE>\LocalCache\Local\ai_factory\brain',
]

for p in paths:
    print(f"\n--- {p} ---")
    brain_file = os.path.join(p, 'brain_state.json')
    fp_file = os.path.join(p, 'bond_fingerprint.hex')

    if not os.path.exists(brain_file):
        print("  No brain file")
        continue

    with open(brain_file) as f:
        d = json.load(f)
    nodes = d.get('network', {}).get('nodes', {})
    active = len([n for n in nodes.values() if not n.get('revoked', False)])
    print(f"  Nodes: {active}")

    if os.path.exists(fp_file):
        fp = open(fp_file).read().strip()
        print(f"  Fingerprint: {fp[:16]}...")
    else:
        print("  Fingerprint: MISSING")
