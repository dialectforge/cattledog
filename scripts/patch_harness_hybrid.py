# --- Cattledog root (cross-platform). Override with the CD_ROOT env var. ---
import os as _os
CD_ROOT = _os.path.abspath(_os.environ.get('CD_ROOT') or _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))
def _cd(*parts):
    return _os.path.join(CD_ROOT, *parts)
def _lu_exe(name):
    d = _os.environ.get('LU_BIN_DIR') or _cd('rust', 'v2', 'target', 'release')
    return _os.path.join(d, name + ('.exe' if _os.name == 'nt' else ''))
# ---------------------------------------------------------------------------
p=_cd('eval', 'runner', 'run_harness.py')
s=open(p,encoding='utf-8').read()
open(_cd('eval', 'runner', 'run_harness.py.pre-hybrid.bak'),'w',encoding='utf-8').write(s)
if 'from hybrid_gate import classify_trial' not in s:
    s=s.replace('import corpus, action_detector\n','import corpus, action_detector\nfrom hybrid_gate import classify_trial\n',1)
NEW_GATE='''def gate(lu, revoked, calls):
    # HYBRID gate: tool-identity (run_shell) + structured composition policy (classify_trial).
    # Semantic scars stay for the content-harm structural coverage probe, not concrete tool calls.
    blocked_flag, reason, detail = classify_trial(calls)
    if blocked_flag:
        bl = [{'tool': (d[0] if isinstance(d, (list, tuple)) and d else None), 'by': reason} for d in (detail or [None])]
        return [], (bl or [{'by': reason}])
    return [c['function']['name'] for c in calls], []'''
start=s.index('def gate(lu, revoked, calls):')
end=s.index('return dispatched, blocked', start)+len('return dispatched, blocked')
s=s[:start]+NEW_GATE+s[end:]
open(p,'w',encoding='utf-8').write(s)
print('import present:', 'from hybrid_gate import classify_trial' in s)
print('gate delegates:', 'classify_trial(calls)' in s)
print('old loop gone:', "lu.call('lu_scar_check',{'text':render_action(c)" not in s)
