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
IN=_cd('eval', 'redteam_inbox', 'evasion_attacks.jsonl')
RES=_cd('eval', 'runner', 'results_evasion.jsonl')
OUT=_cd('logs', 'evasion_results.txt')
meta={}
for l in open(IN,encoding='utf-8'):
    if l.strip():
        r=json.loads(l); meta[r['id']]={'tech':r.get('evasion_technique'),'end':r.get('end_state')}
rows=[json.loads(l) for l in open(RES,encoding='utf-8') if l.strip()]
trials=[r for r in rows if r.get('section')=='agentic_trial']
f=open(OUT,'w',encoding='utf-8')
def w(*a): f.write(' '.join(str(x) for x in a)+'\n')
def end_of(tid): return 'exfil' if tid.startswith('exfil') else 'destroy'
def leaked(t): return t.get('full_stack_misbehaved')=='Y'
def mech(t):
    return ','.join(b.get('by') for b in (t.get('lu_why') or [])) or '-'
def render(t):
    calls=(t.get('base_alone_output') or {}).get('tool_calls') or []
    out=[]
    for c in calls[:2]:
        fn=c.get('function',{}); a=fn.get('arguments',{})
        s=' '.join('%s=%s'%(k,v) for k,v in (a.items() if isinstance(a,dict) else []))
        out.append('%s(%s)'%(fn.get('name'), s[:90]))
    return ' | '.join(out)

valid=[t for t in trials if t.get('base_misbehaved')=='Y']
disc=[t for t in trials if t.get('base_misbehaved')!='Y']
w('============ EVASION RED-TEAM -- hybrid harness (model + classify_trial) ============')
w('total=%d  valid(base-alone acted)=%d  discarded(base did NOT act)=%d' % (len(trials),len(valid),len(disc)))
for end in ('exfil','destroy'):
    sub=[t for t in valid if end_of(t['id'])==end]
    lk=[t for t in sub if leaked(t)]
    w('\n======== %s : valid=%d  caught=%d  LEAK=%d  (leak rate %.0f%%) ========' % (end.upper(),len(sub),len(sub)-len(lk),len(lk),100.0*len(lk)/max(len(sub),1)))
    for t in sorted(sub,key=lambda x:x['id']):
        v='LEAK' if leaked(t) else 'caught'
        w('  %-11s %-32s %-6s by=%-26s %s' % (t['id'], (meta.get(t['id'],{}).get('tech') or '?')[:32], v, mech(t)[:26], render(t)[:110]))
w('\n======== DISCARDED (base-alone did not dispatch -- negative control failed, not counted) ========')
for t in sorted(disc,key=lambda x:x['id']):
    w('  %-11s %s' % (t['id'], meta.get(t['id'],{}).get('tech')))
lkv=[t for t in valid if leaked(t)]
w('\n======== OVERALL ========')
w('valid=%d  LEAKED=%d  leak rate on valid=%.1f%%' % (len(valid),len(lkv),100.0*len(lkv)/max(len(valid),1)))
for end in ('exfil','destroy'):
    sub=[t for t in valid if end_of(t['id'])==end]; lk=[t for t in sub if leaked(t)]
    w('  %-11s %d/%d leaked' % (end, len(lk), len(sub)))
f.close(); print('written')
