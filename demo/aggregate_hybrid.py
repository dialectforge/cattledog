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
p=_cd('eval', 'runner', 'results_hybrid.jsonl')
rows=[json.loads(l) for l in open(p,encoding='utf-8') if l.strip()]
meta=[r for r in rows if r.get('section')=='meta']
summ=[r for r in rows if r.get('section')=='summary']
trials=[r for r in rows if r.get('section')=='agentic_trial' and r.get('status')=='valid']
valid=len(trials)
def mis(t): return t.get('full_stack_misbehaved')=='Y'
neutral=sum(1 for t in trials if not mis(t))
leak=sum(1 for t in trials if mis(t))
print('=== HARNESS-PATH (action_detector ground truth) ===')
print('valid agentic trials:', valid)
print('neutralized: %d/%d = %.1f%%'%(neutral,valid,100.0*neutral/max(valid,1)))
print('leak (action still occurred): %d/%d = %.1f%%'%(leak,valid,100.0*leak/max(valid,1)))
print('\nleak by category:')
for c,k in sorted(Counter(t['category'] for t in trials if mis(t)).items()): print('  %-42s %d'%(c,k))
print('\nleak by route_type:')
for c,k in sorted(Counter(t['route_type'] for t in trials if mis(t)).items()): print('  %-16s %d'%(c,k))
# end-state breakdown of leaks (which action_detector end-state occurred)
es=Counter()
for t in trials:
    if mis(t):
        for k,v in (t.get('full_actions_occurred') or {}).items():
            if v: es[k]+=1
print('\nleaked end-states (action_detector):')
for c,k in sorted(es.items()): print('  %-26s %d'%(c,k))
# EXFIL/DESTROY-specific leak (the seam the hybrid targets)
exfil_cats=[t for t in trials if mis(t) and (t.get('full_actions_occurred',{}).get('exfiltrate_secret_offbox') or t.get('full_actions_occurred',{}).get('destroy_data') or t.get('full_actions_occurred',{}).get('arbitrary_code_exec'))]
print('\nEXFIL/DESTROY/EXEC leaks (the targeted seam): %d'%len(exfil_cats))
for t in exfil_cats: print('  ',t['id'],t['category'],t['route_type'])
if summ: print('\nharness summary:', json.dumps(summ[-1]))
