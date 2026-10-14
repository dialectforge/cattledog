"""Stand up the Cattle Dog sandbox brain (owned, separate from canonical Lu).
Creates/persists .\sandbox\\cattledog.lbr with an owner marker."""
# --- Cattledog root (cross-platform). Override with the CD_ROOT env var. ---
import os as _os
CD_ROOT = _os.path.abspath(_os.environ.get('CD_ROOT') or _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))
def _cd(*parts):
    return _os.path.join(CD_ROOT, *parts)
def _lu_exe(name):
    d = _os.environ.get('LU_BIN_DIR') or _cd('rust', 'v2', 'target', 'release')
    return _os.path.join(d, name + ('.exe' if _os.name == 'nt' else ''))
# ---------------------------------------------------------------------------
import os, sys
sys.path.insert(0, _cd('orchestrator'))
from lu_client import LuClient
EXE   = os.environ.get('LU_EXE', _lu_exe('lu_mcp_v2'))
BRAIN = os.environ.get('CD_BRAIN', _cd('sandbox', 'cattledog.lbr'))
lu = LuClient(EXE, BRAIN, stderr_log=_cd('logs', 'lu_sandbox.stderr.log'))
try:
    lu.initialize()
    print('READY  :', lu.wait_ready()[:160].replace(chr(10),' '))
    print('CONFIG :', lu.call('lu_configure', {'autonomy_tier':'Assisted'})[:160])
    print('TEACH  :', lu.call('lu_teach', {'ku_type':'Definition',
        'content':'Cattle Dog sandbox brain: owned by the Cattle Dog POC, not the canonical Lu.'})[:160])
    print('SAVE   :', lu.call('lu_save', {})[:160])
    print('STATUS :', lu.call('lu_status', {})[:300].replace(chr(10),' '))
finally:
    lu.close()
