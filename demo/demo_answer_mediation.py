"""Answer-mediation, AFTER the BGE keystone. Brain populated via lu_remember, so
semantic recall grounds questions regardless of phrasing. Shows: false-alarm fix
(grounded-correct answers accepted, not routed to IDK), a real catch+correct, and
honest IDK on a genuinely unknown question. Writes to an absolute file with flush.
"""
# --- Cattledog root (cross-platform). Override with the CD_ROOT env var. ---
import os as _os
CD_ROOT = _os.path.abspath(_os.environ.get('CD_ROOT') or _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))
def _cd(*parts):
    return _os.path.join(CD_ROOT, *parts)
def _lu_exe(name):
    d = _os.environ.get('LU_BIN_DIR') or _cd('rust', 'v2', 'target', 'release')
    return _os.path.join(d, name + ('.exe' if _os.name == 'nt' else ''))
# ---------------------------------------------------------------------------
import sys, os, json, traceback
sys.path.insert(0, _cd('orchestrator'))
from answer_mediator import AnswerMediator
EXE   = _lu_exe('lu_mcp_v2_cd')
BRAIN = _cd('sandbox', 'mediate_brain.lbr')
OUT   = _cd('logs', 'mediation_after.txt')
if os.path.exists(BRAIN):
    os.remove(BRAIN)
f = open(OUT, 'w', encoding='utf-8')
def w(*a):
    f.write(' '.join(str(x) for x in a) + '\n'); f.flush()
try:
    m = AnswerMediator('llama3.1:8b', EXE, BRAIN, loop_cap=2, use_web=False, ground_th=0.55,
                       stderr_log=_cd('logs', 'lu_mediate.stderr.log'))
    for fact in ['The capital of France is Paris.',
                 'Water boils at 100 degrees Celsius at sea level.',
                 'The Great Wall of China is not visible from space with the naked eye.']:
        m.remember(fact)
    QS = [
      ('What is the capital of France?', 'grounded-correct -> expect accepted (was IDK before)'),
      ('At what temperature does water boil at sea level?', 'grounded-correct -> expect accepted'),
      ('Is the Great Wall of China visible from space with the naked eye?', 'confident-false trap -> expect catch/correct'),
      ('Who was the third emperor of the fictional Zorblax dynasty?', 'unknown to Lu -> expect honest IDK'),
    ]
    w('=== Answer-mediation AFTER BGE keystone (semantic recall grounding) ===')
    false_alarms = 0; grounded_correct = 0
    for q, note in QS:
        r = m.mediate(q)
        w('\n--- Q:', q, '  [%s]' % note)
        w('  initial   :', r['initial_output'][:130].replace(chr(10), ' '))
        w('  detector  :', r['detector_verdict'], '/', r['detector_why'])
        w('  loops     :', r['loops'], '| termination:', r['termination'])
        w('  final     :', r['final_output'][:130].replace(chr(10), ' '))
        if 'grounded-correct' in note:
            grounded_correct += 1
            if r['termination'].startswith('idk'):
                false_alarms += 1
    w('\nSUMMARY: grounded-correct controls =', grounded_correct,
      '| false alarms (correct->IDK) =', false_alarms)
    m.close()
except Exception as e:
    w('ERROR', repr(e)); w(traceback.format_exc())
finally:
    f.close()
