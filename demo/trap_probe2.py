# --- Cattledog root (cross-platform). Override with the CD_ROOT env var. ---
import os as _os
CD_ROOT = _os.path.abspath(_os.environ.get('CD_ROOT') or _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))
def _cd(*parts):
    return _os.path.join(CD_ROOT, *parts)
def _lu_exe(name):
    d = _os.environ.get('LU_BIN_DIR') or _cd('rust', 'v2', 'target', 'release')
    return _os.path.join(d, name + ('.exe' if _os.name == 'nt' else ''))
# ---------------------------------------------------------------------------
import sys, os, traceback
sys.path.insert(0, _cd('orchestrator'))
from ollama_client import Ollama
OUT = _cd('logs', 'trap_probe2.txt')
f = open(OUT, 'w', encoding='utf-8')
def w(*a):
    f.write(' '.join(str(x) for x in a) + '\n'); f.flush()
CANDS = [
  ('Which is larger: 9.11 or 9.9? Answer with just the number.', '9.9'),
  ('Is 9.11 greater than 9.9? Answer yes or no.', 'no'),
  ('What is 47 multiplied by 89? Answer with just the number.', '4183'),
  ('How many times does the letter "t" appear in the word "tattletale"? Just the number.', '4'),
  ('How many letter "e"s are in the word "beekeeper"? Just the number.', '5'),
  ('What is 128 multiplied by 256? Answer with just the number.', '32768'),
  ('How many letter "a"s are in the word "banana"? Just the number.', '3'),
]
try:
    o = Ollama(model='llama3.1:8b')
    for q, correct in CANDS:
        r = o.chat([{'role':'user','content': q}])
        ans = (r.get('message',{}).get('content') or '').strip().replace(chr(10),' ')
        w('Q:', q)
        w('  correct:', correct, '| llama:', ans[:120])
        w('')
except Exception as e:
    w('FATAL', repr(e)); w(traceback.format_exc())
finally:
    f.close()
