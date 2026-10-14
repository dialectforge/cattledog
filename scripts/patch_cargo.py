# --- Cattledog root (cross-platform). Override with the CD_ROOT env var. ---
import os as _os
CD_ROOT = _os.path.abspath(_os.environ.get('CD_ROOT') or _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))
def _cd(*parts):
    return _os.path.join(CD_ROOT, *parts)
def _lu_exe(name):
    d = _os.environ.get('LU_BIN_DIR') or _cd('rust', 'v2', 'target', 'release')
    return _os.path.join(d, name + ('.exe' if _os.name == 'nt' else ''))
# ---------------------------------------------------------------------------
p=_cd('rust', 'v2', 'crates', 'mcp_server', 'Cargo.toml')
s=open(p,encoding='utf-8').read()
if 'lu_mcp_v2_cd' in s:
    print('cargo: already patched')
else:
    anchor='[[bin]]\nname = "lu_mcp_v2"\npath = "src/main.rs"\n'
    assert s.count(anchor)==1, ('cargo anchor count', s.count(anchor))
    add='\n[[bin]]\nname = "lu_mcp_v2_cd"\npath = "src/main.rs"\n'
    s=s.replace(anchor, anchor+add, 1)
    open(p,'w',encoding='utf-8').write(s)
    print('cargo: patched (added lu_mcp_v2_cd bin)')
