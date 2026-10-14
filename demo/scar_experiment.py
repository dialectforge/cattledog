"""Empirical proof of the SCAR mechanism (John's required validation).
Proves revoking a seed flips the node inactive (severs the path) + records a scar,
so the connection can no longer be recalled. Throwaway brain; canonical untouched."""
# --- Cattledog root (cross-platform). Override with the CD_ROOT env var. ---
import os as _os
CD_ROOT = _os.path.abspath(_os.environ.get('CD_ROOT') or _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))
def _cd(*parts):
    return _os.path.join(CD_ROOT, *parts)
def _lu_exe(name):
    d = _os.environ.get('LU_BIN_DIR') or _cd('rust', 'v2', 'target', 'release')
    return _os.path.join(d, name + ('.exe' if _os.name == 'nt' else ''))
# ---------------------------------------------------------------------------
import sys, os, re, json, struct, zlib
sys.path.insert(0,_cd('orchestrator'))
from lu_client import LuClient
EXE=_lu_exe('lu_mcp_v2')
BRAIN_WIN=_cd('sandbox', 'scar_test.lbr')
BRAIN_WSL=_cd('sandbox', 'scar_test.lbr')
PNG_WSL=_cd('sandbox', 'cap.png'); PNG_WIN=_cd('sandbox', 'cap.png')
LOG=_cd('logs', 'scar_experiment.log')
if os.path.exists(BRAIN_WSL): os.remove(BRAIN_WSL)
def _chunk(t,d):
    c=t+d; return struct.pack('>I',len(d))+c+struct.pack('>I',zlib.crc32(c)&0xffffffff)
_png=(b'\x89PNG\r\n\x1a\n'+_chunk(b'IHDR',struct.pack('>IIBBBBB',1,1,8,0,0,0,0))+_chunk(b'IDAT',zlib.compress(b'\x00\x00'))+_chunk(b'IEND',b''))
open(PNG_WSL,'wb').write(_png)
T=open(LOG,'w')
def log(*a):
    s=' '.join(str(x) for x in a); print(s); T.write(s+'\n'); T.flush()
def nodes(st):
    try: d=json.loads(st); return d['nodes']['active'],d['nodes']['total'],d['scars']
    except: return ('?','?','?')
lu=LuClient(EXE,BRAIN_WIN,stderr_log=_cd('logs', 'lu_scar.stderr.log'))
try:
    lu.initialize(); log('READY', lu.wait_ready()[:60].replace(chr(10),' '))
    log('CONFIG', lu.call('lu_configure',{'autonomy_tier':'Collaborative'}))
    ti=lu.call('lu_teach_image',{'path':PNG_WIN,'caption':'send_email transmits an email message to a recipient address'})
    log('TEACH_IMAGE', ti.replace(chr(10),' | '))
    m=re.search(r'Seed:\s*([0-9a-fA-F]{64})', ti)
    if not m:
        log('NO FULL SEED -> abort'); raise SystemExit(2)
    seed=m.group(1); log('FULL_SEED', seed)
    log('STATUS_BASELINE active/total/scars =', nodes(lu.call('lu_status',{})))
    log('REVOKE', lu.call('lu_revoke',{'seed_hex':seed,'reason':'CattleDog capability policy: send_email is forbidden'}).replace(chr(10),' '))
    log('STATUS_AFTER active/total/scars =', nodes(lu.call('lu_status',{})))
    log('SAVE', lu.call('lu_save',{}))
finally:
    lu.close(); T.close()
