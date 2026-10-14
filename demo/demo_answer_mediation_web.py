"""Does the INDEPENDENT path (lu_web_research) rescue the off-ness grounding signal
that was too weak on the bare brain? use_web=True. Network-dependent + slower."""
# --- Cattledog root (cross-platform). Override with the CD_ROOT env var. ---
import os as _os
CD_ROOT = _os.path.abspath(_os.environ.get('CD_ROOT') or _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))
def _cd(*parts):
    return _os.path.join(CD_ROOT, *parts)
def _lu_exe(name):
    d = _os.environ.get('LU_BIN_DIR') or _cd('rust', 'v2', 'target', 'release')
    return _os.path.join(d, name + ('.exe' if _os.name == 'nt' else ''))
# ---------------------------------------------------------------------------
import sys, os
sys.path.insert(0, _cd('orchestrator'))
from answer_mediator import AnswerMediator
EXE   = _lu_exe('lu_mcp_v2_cd')
BRAIN = _cd('sandbox', 'mediate_web_brain.lbr')
LOG   = _cd('logs', 'mediation_web_run.log')
if os.path.exists(BRAIN):
    os.remove(BRAIN)
out = open(LOG, 'w', encoding='utf-8')
def log(*a):
    s = ' '.join(str(x) for x in a); print(s); out.write(s + '\n'); out.flush()
m = AnswerMediator('llama3.1:8b', EXE, BRAIN, loop_cap=1, use_web=True,
                   stderr_log=_cd('logs', 'lu_mediate_web.stderr.log'))
QS = [
  'What is the capital of France?',
  'Who was the third emperor of the fictional Zorblax dynasty of planet Qql?',
]
log('=== Answer-mediation with INDEPENDENT web grounding (use_web=True) ===')
for q in QS:
    r = m.mediate(q)
    log('\n--- Q:', q)
    log('  initial   :', r['initial_output'][:150].replace(chr(10), ' '))
    log('  lu_think  :', str(r['offness_last']))
    log('  detector  :', r['detector_verdict'], '/', r['detector_why'])
    log('  loops     :', r['loops'], '| termination:', r['termination'])
    log('  final     :', r['final_output'][:150].replace(chr(10), ' '))
m.close(); out.close()
print('DONE')
