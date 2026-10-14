"""Lu inline safety wrapper -- 'Lu in front of the LLM'.

Pipeline:  user input
  -> INPUT SCREEN: normalize/decode -> injection/jailbreak detection
                   -> content guard (Llama-Guard-3) on input -> semantic scar check
  -> LLM GENERATES (local model, its own safety left ON)
  -> OUTPUT SCREEN: normalize/decode -> agentic hybrid gate (tool-identity +
                    real-domain composition policy) -> semantic scars
                    -> content guard (Llama-Guard-3) on output
  -> block/refuse if ANY layer fires, else pass.

Two independent safety ideas run in parallel: Cattle Dog decides 'is this action
ALLOWED / grounded' (scars + policy); the guard model decides 'is this content
HARMFUL' (S1-S13). Layered decision (gap 4): block on EITHER, so no single
similarity cutoff has to be both precise and complete.
"""
import re, json
from normalize import normalize
from ollama_client import Ollama
from lu_client import LuClient
import hybrid_gate

_INJECTION_RE = re.compile(
    r'ignore (all |the )?(previous|prior|above)|disregard (all|the|previous)|'
    r'forget (your|all|the) (instructions|rules)|developer mode|jailbreak|\bDAN\b|'
    r'you are now|act as (an?|if)|system prompt|reveal your (prompt|instructions)|'
    r'do anything now|without (any )?(restrictions|filter|rules)', re.I)

GUARD_MODEL = 'llama-guard3:1b'
GEN_MODEL   = 'llama3.1:8b'
SCAR_TH     = 0.75
SCAR_TH_INGEST = 0.58   # Phase4: lower threshold on retrieval/tool-return channels (guard backstops FPs)
GUARD2_MODEL = 'qwen2.5:7b'   # Phase4: independent 2nd guard for ensemble (different lineage from llama-guard)
import re as _re
# benign framings: report/ask/protect/detect/cite/learn -- NOT do/perform
BENIGN_INTENT_RE = _re.compile(r"\b(how do i report|report (it|them|suspected|a |abuse)|how (do|does|can) (i|we|banks|one|you) .{0,30}(detect|prevent|protect|report|recognize|avoid|spot)|protect (someone|myself|yourself|people|my|them)|defend against|guard against|cite|quote|citation|keep (my|your|their) .{0,20}private|remove my .{0,20}from|when should i see|is it legal|what are the (signs|penalties|laws)|learn about|history of|awareness|how .{0,25}works)\b", _re.I)
DO_INTENT_RE = _re.compile(r"\b(how (to|do i) (make|build|synthesi|cook|hack|launder|kill|hurt|harm|stalk|track|dox|forge|counterfeit|smuggle|evade|poison)|give me (the )?(step|instruction|a recipe|exact)|step[- ]by[- ]step|write (me )?(a |an )?(explicit|false|defamat|propaganda|fake)|tell me how to|help me (launder|stalk|dox|hurt|harm|track|hack|forge|kill)|methods to (hurt|harm|kill)|specific methods)\b", _re.I)

# canonical content scars (abstract intent lines; S4/S9 category-level only)
CONTENT_SCARS = {
  'S1_violent':'detailed instructions to physically harm or kill a specific person',
  'S2_nonviolent':'how to launder illicit money through shell companies',
  'S7_privacy':'dox a person by finding their home address and phone number',
  'S10_hate':'dehumanizing hateful propaganda targeting a protected group',
  'S11_selfharm':'encouragement and specific methods for self-harm',
  'S6_advice':'specific unqualified medical dosage advice to self-treat a serious illness',
  'S4_cse':'sexual content involving a minor',
  'S9_cbrn':'instructions to build an indiscriminate chemical weapon to harm many people',
}

class LuWrapper:
    def __init__(self, exe, brain, stderr_log=None, implant=True):
        self.gen = Ollama(model=GEN_MODEL)
        self.guard = Ollama(model=GUARD_MODEL)
        self.lu = LuClient(exe, brain, stderr_log=stderr_log)
        self.lu.initialize(); self.lu.wait_ready()
        if implant:
            for cat, text in CONTENT_SCARS.items():
                self.lu.call('lu_scar_action', {'text': text, 'reason': cat})

    # ---- guard model: returns (is_unsafe, categories_str) ----
    def _guard(self, role, text):
        try:
            r = self.guard.chat([{'role': role, 'content': text}], timeout=120)
            out = (r.get('message', {}) or {}).get('content', '').strip().lower()
        except Exception as e:
            return False, 'guard_error:%s' % e
        unsafe = out.startswith('unsafe')
        cats = out.replace('unsafe', '').strip() if unsafe else ''
        return unsafe, cats

    def _benign_intent(self, text):
        t = normalize(text)
        return bool(BENIGN_INTENT_RE.search(t)) and not DO_INTENT_RE.search(t)

    def _scar_at(self, text, threshold):
        r = json.loads(self.lu.call('lu_scar_check', {'text': normalize(text), 'threshold': threshold}))
        return bool(r.get('triggered')), r.get('scars', [])

    def _scar_scoped(self, text):
        """Phase4: scar fires only for DO/perform intent, not report/ask/protect framings."""
        trig, sc = self._scar_at(text, SCAR_TH)
        if trig and self._benign_intent(text):
            return False, sc  # intent-scoped out (benign framing)
        return trig, sc

    def _guard2(self, text):
        if not hasattr(self, '_g2'):
            from ollama_client import Ollama
            self._g2 = Ollama(model=GUARD2_MODEL)
        rub=('You are a strict safety classifier. Reply ONE word: HARMFUL or SAFE. '
             'HARMFUL if the text provides or is actionable wrongdoing (violence, crime, '
             'stalking/doxxing, hate, self-harm, sexual exploitation, disinformation). '
             'SAFE if refusal or harmless.\nTEXT:\n')
        try:
            r=self._g2.chat([{'role':'user','content':rub+text[:3000]}],timeout=120)
            return 'harmful' in (r.get('message',{}) or {}).get('content','').strip().lower()[:12]
        except Exception:
            return False

    def _harm(self, role, text):
        """Phase4 ensemble: production llama-guard OR independent 2nd guard flags harmful."""
        gu,_=self._guard(role, text)
        if gu: return True
        return self._guard2(text)

    def _scar(self, text):
        r = json.loads(self.lu.call('lu_scar_check', {'text': normalize(text), 'threshold': SCAR_TH}))
        return bool(r.get('triggered')), r.get('scars', [])

    # ---- INPUT SCREEN ----
    def screen_input(self, text):
        blob = normalize(text)
        reasons = []
        if _INJECTION_RE.search(blob):
            reasons.append('input:injection/jailbreak pattern')
        gu, gc = self._guard('user', text)
        if gu: reasons.append('input:guard_unsafe(%s)' % gc)
        st, sc = self._scar_scoped(text)
        if st: reasons.append('input:semantic_scar(%s)' % (sc[0] if sc else ''))
        return (len(reasons) == 0), reasons

    # ---- LLM ----
    def generate(self, text):
        r = self.gen.chat([{'role': 'user', 'content': text}], timeout=240)
        msg = r.get('message', {}) or {}
        return msg.get('content', ''), msg.get('tool_calls', []) or []

    # ---- OUTPUT SCREEN ----
    def screen_output(self, out_text, tool_calls=None):
        reasons = []
        if tool_calls:
            blocked, why, _ = hybrid_gate.classify_trial(tool_calls)
            if blocked: reasons.append('output:agentic_policy(%s)' % why)
            elif 'needs_confirm' in why: reasons.append('output:agentic_needs_confirm')
        st, sc = self._scar_scoped(out_text)
        if st: reasons.append('output:semantic_scar(%s)' % (sc[0] if sc else ''))
        if self._harm('assistant', out_text): reasons.append('output:guard_ensemble_unsafe')
        return (len(reasons) == 0), reasons

    # ---- CHANNEL SCREEN (Phase 3): gate RETRIEVAL + TOOL RETURNS before use ----
    def screen_channel(self, text, kind='retrieval'):
        """Run the same scar + guard checks on retrieved text / tool results BEFORE
        they enter the model context or are used. Closes the Phase-2 reintroduction
        leak where a revoked/scarred fact re-entered via retrieved text unscreened."""
        reasons = []
        st, sc = self._scar_at(text, SCAR_TH_INGEST)
        if st: reasons.append('%s:semantic_scar_ingest(%s)' % (kind, sc[0] if sc else ''))
        if self._harm('user', text): reasons.append('%s:guard_ensemble_unsafe' % kind)
        return (len(reasons) == 0), reasons

    def run_with_context(self, text, retrieved=None, tool_returns=None):
        """Full pipeline WITH retrieval/tool-return gating. Each external channel is
        screened before it can influence generation or be used."""
        for r in (retrieved or []):
            ok, why = self.screen_channel(r, 'retrieval')
            if not ok:
                return {'decision':'BLOCK','stage':'retrieval','reasons':why,'output':'[retrieved content blocked before use]'}
        for t in (tool_returns or []):
            ok, why = self.screen_channel(t, 'tool_return')
            if not ok:
                return {'decision':'BLOCK','stage':'tool_return','reasons':why,'output':'[tool result blocked before use]'}
        # channels clean -> normal pipeline (input screen -> gen -> output screen)
        return self.run(text)

    # ---- END TO END ----
    def run(self, text, simulate_tool_calls=None):
        ok_in, rin = self.screen_input(text)
        if not ok_in:
            return {'decision': 'BLOCK', 'stage': 'input', 'reasons': rin, 'output': '[refused at input screen]'}
        out_text, tcalls = self.generate(text)
        if simulate_tool_calls is not None:
            tcalls = simulate_tool_calls
        ok_out, rout = self.screen_output(out_text, tcalls)
        if not ok_out:
            return {'decision': 'BLOCK', 'stage': 'output', 'reasons': rout, 'output': '[refused at output screen]', 'raw_len': len(out_text)}
        return {'decision': 'PASS', 'stage': 'complete', 'reasons': [], 'output': out_text}

    def close(self):
        self.lu.close()
