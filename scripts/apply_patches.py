# --- Cattledog root (cross-platform). Override with the CD_ROOT env var. ---
import os as _os
CD_ROOT = _os.path.abspath(_os.environ.get('CD_ROOT') or _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))
def _cd(*parts):
    return _os.path.join(CD_ROOT, *parts)
def _lu_exe(name):
    d = _os.environ.get('LU_BIN_DIR') or _cd('rust', 'v2', 'target', 'release')
    return _os.path.join(d, name + ('.exe' if _os.name == 'nt' else ''))
# ---------------------------------------------------------------------------
import sys
BR=_cd('rust', 'v2', 'crates', 'brain', 'src', 'lib.rs')
MC=_cd('rust', 'v2', 'crates', 'mcp_server', 'src', 'lib.rs')
ib=open(_cd('scripts', 'insert_brain.txt'),encoding='utf-8').read()
im=open(_cd('scripts', 'insert_mcp.txt'),encoding='utf-8').read()
s=open(BR,encoding='utf-8').read()
if 'semantic_scar_check' in s:
    print('brain: already patched')
else:
    a='    pub fn revoke(&mut self, seed: &Seed, reason: &str) {'
    assert s.count(a)==1, ('brain anchor count', s.count(a))
    s=s.replace(a, ib+a, 1)
    open(BR,'w',encoding='utf-8').write(s)
    print('brain: patched')
s=open(MC,encoding='utf-8').read()
if 'lu_scar_check' in s:
    print('mcp: already patched')
else:
    a='            "lu_status" => {'
    assert s.count(a)==1, ('mcp anchor count', s.count(a))
    s=s.replace(a, im+a, 1)
    open(MC,'w',encoding='utf-8').write(s)
    print('mcp: patched')
print('done')
