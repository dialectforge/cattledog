"""Probe Lu's off-ness/grounding signals to design the answer-mediation detector.
No network calls here (lu_web_research probed separately). Operator-side."""
# --- Cattledog root (cross-platform). Override with the CD_ROOT env var. ---
import os as _os
CD_ROOT = _os.path.abspath(_os.environ.get('CD_ROOT') or _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))
def _cd(*parts):
    return _os.path.join(CD_ROOT, *parts)
def _lu_exe(name):
    d = _os.environ.get('LU_BIN_DIR') or _cd('rust', 'v2', 'target', 'release')
    return _os.path.join(d, name + ('.exe' if _os.name == 'nt' else ''))
# ---------------------------------------------------------------------------
import sys, os, json
sys.path.insert(0,_cd('orchestrator'))
from lu_client import LuClient
EXE=_lu_exe('lu_mcp_v2_cd')
BRAIN=_cd('sandbox', 'probe_sig.lbr')
if os.path.exists(_cd('sandbox', 'probe_sig.lbr')):
    os.remove(_cd('sandbox', 'probe_sig.lbr'))
lu=LuClient(EXE,BRAIN,stderr_log=_cd('logs', 'lu_probe.stderr.log'))
def short(s,n=240): return s[:n].replace(chr(10),' ')
try:
    lu.initialize(); lu.wait_ready()
    lu.call('lu_configure',{'autonomy_tier':'Collaborative'})
    print('TEACH', short(lu.call('lu_teach',{'ku_type':'Definition','content':'The capital of France is Paris.'})))
    print('RECALL_QUALITY(empty-ish):', short(lu.call('lu_recall_quality',{'last_n':5})))
    print('THINK known:', short(lu.call('lu_think',{'topic':'capital of France','depth':2})))
    print('THINK unknown:', short(lu.call('lu_think',{'topic':'the Zorblax dynasty of planet Qql','depth':2})))
    print('CONV known:', short(lu.call('lu_conversation',{'message':'What is the capital of France?'})))
    print('CONV unknown:', short(lu.call('lu_conversation',{'message':'Who was the third emperor of the Zorblax dynasty?'})))
    print('RECALL_QUALITY after queries:', short(lu.call('lu_recall_quality',{'last_n':5})))
finally:
    lu.close()
