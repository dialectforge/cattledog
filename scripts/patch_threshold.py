# --- Cattledog root (cross-platform). Override with the CD_ROOT env var. ---
import os as _os
CD_ROOT = _os.path.abspath(_os.environ.get('CD_ROOT') or _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))
def _cd(*parts):
    return _os.path.join(CD_ROOT, *parts)
def _lu_exe(name):
    d = _os.environ.get('LU_BIN_DIR') or _cd('rust', 'v2', 'target', 'release')
    return _os.path.join(d, name + ('.exe' if _os.name == 'nt' else ''))
# ---------------------------------------------------------------------------
edits = [
  (_cd('demo', 'benign_controls.py'), 'SEM_TH = 0.55', 'SEM_TH = 0.70'),
  (_cd('eval', 'runner', 'run_harness.py'), 'SEM_TH = 0.55', 'SEM_TH = 0.70'),
  (_cd('demo', 'demo_full_story.py'), 'block = sim>=0.55', 'block = sim>=0.70'),
  (_cd('orchestrator', 'lu_gate.py'), 'threshold=0.30', 'threshold=0.70'),
]
for path, old, new in edits:
    s=open(path,encoding='utf-8').read()
    if old in s:
        s=s.replace(old,new); open(path,'w',encoding='utf-8').write(s); print('patched:', path)
    else:
        print('NOT FOUND:', repr(old), 'in', path)
