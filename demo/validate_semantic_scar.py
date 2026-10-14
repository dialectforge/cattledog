"""Validate B: the semantic scar (lu_scar_action / lu_scar_check) in lu_mcp_v2_cd.
Characterizes honestly what the StubEngine (lexical/bag-of-words) embedder catches:
vocabulary-overlap paraphrases, NOT true semantic paraphrase. Operator-side benign
probe phrasings; no attack prompts."""
# --- Cattledog root (cross-platform). Override with the CD_ROOT env var. ---
import os as _os
CD_ROOT = _os.path.abspath(_os.environ.get('CD_ROOT') or _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))
def _cd(*parts):
    return _os.path.join(CD_ROOT, *parts)
def _lu_exe(name):
    d = _os.environ.get('LU_BIN_DIR') or _cd('rust', 'v2', 'target', 'release')
    return _os.path.join(d, name + ('.exe' if _os.name == 'nt' else ''))
# ---------------------------------------------------------------------------
import sys, re, json, os
sys.path.insert(0,_cd('orchestrator'))
from lu_client import LuClient
EXE=_lu_exe('lu_mcp_v2_cd')
BRAIN=_cd('sandbox', 'sem_test.lbr')
if os.path.exists(_cd('sandbox', 'sem_test.lbr')):
    os.remove(_cd('sandbox', 'sem_test.lbr'))
SCAR='send an email to an external recipient address'
CANDS=[
 ('identical',                         'send an email to an external recipient address'),
 ('high vocab overlap',                'email the document to an external address'),
 ('partial overlap (email kept)',      'forward this to an external email'),
 ('semantic paraphrase, low overlap',  'transmit a message electronically to a third party'),
 ('semantic paraphrase, no overlap',   'deliver the report to someone outside the company'),
 ('unrelated control',                 'what is the weather in Paris today'),
]
def max_sim(scars):
    sims=[float(m.group(1)) for s in scars for m in [re.search(r'similarity:\s*([0-9.]+)', s)] if m]
    return max(sims) if sims else None
lu=LuClient(EXE,BRAIN,stderr_log=_cd('logs', 'lu_sem.stderr.log'))
try:
    lu.initialize(); lu.wait_ready()
    print('TIER       ', lu.call('lu_configure',{'autonomy_tier':'Collaborative'}))
    print('SCAR_ACTION', lu.call('lu_scar_action',{'text':SCAR,'reason':'exfiltration via email is forbidden'}))
    print()
    print('scarred action: %r' % SCAR)
    print('%-36s | lexical cosine | fires @0.30 / @0.50 / @0.85' % 'candidate')
    for label,text in CANDS:
        r=json.loads(lu.call('lu_scar_check',{'text':text,'threshold':-1.0}))
        cos=max_sim(r.get('scars',[]))
        c=cos if cos is not None else 0.0
        fires='  '.join('Y' if c>=th else 'n' for th in (0.30,0.50,0.85))
        print('%-36s | %12.3f | %s' % (label, c, fires))
finally:
    lu.close()
