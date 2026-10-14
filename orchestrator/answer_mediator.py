"""A) Answer-mediation layer -- now grounded on REAL BGE semantic recall.

John's off-ness concept: 'Lu sees something off, checks, kicks back to the LLM.'
Grounding signal switched from lu_think (lexical, brittle) to lu_semantic_recall
(BGE-small ONNX dense retrieval) so questions ground regardless of phrasing.

Pipeline per question:
  base answers -> Lu retrieves the most relevant remembered fact (semantic) ->
  if grounded (similarity >= ground_th): reconcile the base answer against the fact
    (kick-back: show the model its output + Lu's fact; correct or keep) ->
  if ungrounded: optionally lu_web_research, else surface uncertainty (idk) ->
  hard loop cap routes to IDK on exhaustion (never accept-last).
Test-3 fields logged. Independent path = lu_web_research / Lu's remembered facts.
"""
import json, re
from ollama_client import Ollama
from lu_client import LuClient

def _extract_json(s):
    m = re.search(r'\{.*\}', s or '', re.DOTALL)
    if not m:
        return None
    try:
        return json.loads(m.group(0))
    except Exception:
        return None

class AnswerMediator:
    def __init__(self, base_model, lu_exe, lu_brain, loop_cap=2, use_web=False,
                 ground_th=0.55, stderr_log=None):
        self.base = Ollama(model=base_model)
        self.lu = LuClient(lu_exe, lu_brain, stderr_log=stderr_log)
        self.lu.initialize(); self.lu.wait_ready()
        self.lu.call('lu_configure', {'autonomy_tier': 'Collaborative'})
        self.cap = loop_cap
        self.use_web = use_web
        self.ground_th = ground_th

    def remember(self, text):
        return self.lu.call('lu_remember', {'text': text})

    def _ask_base(self, prompt):
        r = self.base.chat([{'role': 'user', 'content': prompt}])
        return (r.get('message', {}).get('content') or '').strip()

    def _lu_view(self, question):
        if self.use_web:
            try:
                self.lu.call('lu_web_research', {'query': question}, timeout=200)
            except Exception:
                pass
        r = json.loads(self.lu.call('lu_semantic_recall', {'query': question, 'top_k': 1, 'threshold': 0.0}))
        hits = r.get('hits', [])
        top = hits[0] if hits else None
        sim = top['similarity'] if top else 0.0
        return {'fact': (top or {}).get('text'), 'sim': sim, 'grounded': sim >= self.ground_th}

    def _reconcile(self, question, answer, fact):
        prompt = (
            'Question: %s\nYour previous answer: %s\n'
            'An INDEPENDENT reference states: %s\n\n'
            'If your answer conflicts with or is not supported by the reference, correct it. '
            'Reply ONLY as JSON: {"consistent": true|false, "answer": "<corrected or unchanged>"}.'
        ) % (question, answer, fact)
        raw = self._ask_base(prompt)
        j = _extract_json(raw) or {}
        return {'consistent': bool(j.get('consistent', False)), 'answer': j.get('answer') or answer}

    def _offness(self, question, answer):
        v = self._lu_view(question)
        if not v['grounded']:
            return True, {'verdict': 'off', 'why': 'lu_ungrounded(no_recall sim=%.2f)' % v['sim'],
                          'lu_view': v, 'reconcile': None}
        rec = self._reconcile(question, answer, v['fact'])
        off = not rec['consistent']
        return off, {'verdict': 'off' if off else 'not_off',
                     'why': 'conflict_with_grounded_fact' if off else 'consistent_with_grounded_fact(sim=%.2f)' % v['sim'],
                     'lu_view': v, 'reconcile': rec}

    def mediate(self, question):
        log = {'question': question}
        initial = self._ask_base(question)
        log['initial_output'] = initial
        off, ev = self._offness(question, initial)
        log['detector_verdict'] = ev['verdict']
        log['detector_why'] = ev['why']
        loops = 0
        final = initial
        termination = 'accepted_initial_not_off'
        while off and loops < self.cap:
            loops += 1
            if ev.get('reconcile') and ev['reconcile'].get('answer'):
                candidate = ev['reconcile']['answer']
            else:
                candidate = self._ask_base(
                    'Question: %s\nYour previous answer: %s\nAn independent reference could not '
                    'corroborate this. If you cannot verify it, reply exactly "UNSURE". '
                    'Otherwise give your best verified answer.' % (question, final))
            if candidate.strip().upper().startswith('UNSURE'):
                off = True
                break
            final = candidate
            off, ev = self._offness(question, final)
            if not off:
                termination = 'corrected'
                break
        if off:
            final = '[IDK] I cannot verify this confidently, so I will not assert an answer.'
            termination = 'idk_on_cap' if loops >= self.cap else 'idk_ungrounded'
        log.update({'loops': loops, 'final_output': final, 'termination': termination,
                    'offness_last': ev['why']})
        return log

    def close(self):
        self.lu.close()
