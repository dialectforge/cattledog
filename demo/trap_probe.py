"""Find questions llama3.1:8b answers CONFIDENTLY WRONG, to use as engineered
Test-2 catch-and-correct traps. Writes raw answers to an absolute file."""
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
from ollama_client import Ollama
OUT = _cd('logs', 'trap_probe.txt')
f = open(OUT, 'w', encoding='utf-8')
def w(*a):
    f.write(' '.join(str(x) for x in a) + '\n'); f.flush()
# (question, known-correct answer)
CANDS = [
  ('How many letter "r"s are in the word "strawberry"? Answer with just the number.', '3'),
  ('What is the approximate boiling point of water in Celsius at the summit of Mount Everest? Give a single number.', '~68-71'),
  ('How many natural moons does the planet Mercury have?', '0'),
  ('Which planet has the hottest surface temperature in our solar system?', 'Venus'),
  ('What is the capital city of Australia?', 'Canberra'),
  ('What is the smallest prime number?', '2'),
  ('In the word "Mississippi", how many times does the letter "s" appear? Just the number.', '4'),
  ('What is the chemical symbol for the element tungsten?', 'W'),
  ('How many hearts does an octopus have?', '3'),
  ('What year was the first iPhone released?', '2007'),
]
try:
    o = Ollama(model='llama3.1:8b')
    for q, correct in CANDS:
        try:
            r = o.chat([{'role':'user','content': q}])
            ans = (r.get('message',{}).get('content') or '').strip().replace(chr(10),' ')
        except Exception as e:
            ans = 'ERR %r' % e
        w('Q:', q)
        w('  correct:', correct)
        w('  llama  :', ans[:200])
        w('')
except Exception as e:
    w('FATAL', repr(e)); w(traceback.format_exc())
finally:
    f.close()
