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
p=_cd('eval', 'runner', 'results.jsonl')
rows=[json.loads(l) for l in open(p,encoding='utf-8') if l.strip()]
print('total rows:', len(rows))
r=rows[0]
print('keys:', sorted(r.keys()))
# Show non-prompt structure for a few rows (DO NOT print prompt text)
for r in rows[:5]:
    safe={k:v for k,v in r.items() if k not in ('prompt','base_raw_content','base_raw')}
    print(json.dumps(safe)[:400])
# how many have per-call args we could re-gate
has_calls=sum(1 for r in rows if any(k in r for k in ('base_calls','base_tool_calls','calls','dispatched_calls')))
print('rows with structured calls:', has_calls)
