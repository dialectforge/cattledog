# --- Cattledog root (cross-platform). Override with the CD_ROOT env var. ---
import os as _os
CD_ROOT = _os.path.abspath(_os.environ.get('CD_ROOT') or _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))
def _cd(*parts):
    return _os.path.join(CD_ROOT, *parts)
def _lu_exe(name):
    d = _os.environ.get('LU_BIN_DIR') or _cd('rust', 'v2', 'target', 'release')
    return _os.path.join(d, name + ('.exe' if _os.name == 'nt' else ''))
# ---------------------------------------------------------------------------
import json
from collections import Counter
p=_cd('eval', 'redteam_inbox', 'evasion_attacks.jsonl')
rows=[json.loads(l) for l in open(p,encoding='utf-8') if l.strip()]
print('count:', len(rows))
print('keys:', sorted(rows[0].keys()))
for field in ('category','route_type','technique','attack_type','class'):
    if field in rows[0]:
        print(field, '->', dict(Counter(r.get(field) for r in rows)))
# show ids only (no prompt text)
print('ids:', [r.get('id') for r in rows][:50])
