# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

import json, os

paths = {
    "STANDARD": r'<USER_HOME>\AppData\Local\ai_factory\brain',
    "SANDBOX": r'<USER_HOME>\AppData\Local\Packages\<CLAUDE_PACKAGE>\LocalCache\Local\ai_factory\brain',
}

for label, brain_dir in paths.items():
    brain_file = os.path.join(brain_dir, 'brain_state.json')
    fp_file = os.path.join(brain_dir, 'bond_fingerprint.hex')
    print(f"\n{'='*50}")
    print(f"  {label}: {brain_dir}")
    print(f"{'='*50}")
    
    if not os.path.exists(brain_file):
        print("  brain_state.json: NOT FOUND")
        continue
    
    size = os.path.getsize(brain_file)
    print(f"  File size: {size:,} bytes")
    
    with open(brain_file) as f:
        d = json.load(f)
    
    nodes = d.get('network', {}).get('nodes', {})
    total = len(nodes)
    revoked = sum(1 for n in nodes.values() if n.get('revoked', False))
    active = total - revoked
    print(f"  Total node keys: {total}")
    print(f"  Active: {active}, Revoked: {revoked}")
    
    bonds = d.get('network', {}).get('bonds', {})
    print(f"  Bonds: {len(bonds)}")
    
    bond = d.get('terminal_bond', {})
    ai_seed = bond.get('ai_seed', {}).get('bytes', [])
    if ai_seed:
        ai_hex = ''.join(f'{b:02x}' for b in ai_seed)
        print(f"  AI seed: {ai_hex[:16]}...")
    else:
        print(f"  AI seed: NOT FOUND")
    
    if os.path.exists(fp_file):
        print(f"  Fingerprint: {open(fp_file).read().strip()[:16]}...")
    else:
        print(f"  Fingerprint: MISSING")
    
    # Sample first node label
    if nodes:
        sample = list(nodes.values())[0]
        print(f"  Sample label: {sample.get('label', '?')[:80]}")
