"""Cattle Dog kill-switch PROXY -- a thin inline layer in front of any model.

Pipeline per request:
  user input --> [input screen] --> for each retrieved passage / tool return:
     STRUCTURAL kill-switch check (KillSwitch.check: graph-reachability +
     entity-linking, seed-citing)  THEN embedding-scar + guard backstop
  --> model generates --> [output screen] --> response
Any block short-circuits and is written to the audit log citing the SEED.

Revoke API: revoke_capability / revoke_fact / list_revocations -- callable at
runtime, takes effect immediately (next request is gated).
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
import json, time
from lu_wrapper import LuWrapper
from lu_gate import LuGate
from killswitch import KillSwitch

class KillSwitchProxy:
    def __init__(self, exe, brain, png='/tmp/rt/_proxy.png', audit=_cd('logs', 'killswitch_audit.jsonl'),
                 gen_model='llama3.1:8b', guard_model='llama-guard3:8b'):
        self.gate = LuGate(exe, brain, png, stderr_log='/tmp/rt/lu_proxy.stderr.log')
        self.ks = KillSwitch(self.gate, self.gate.lu, audit_path=audit)
        # wrapper shares the SAME Lu client/graph so scars + reachability are consistent
        self.w = LuWrapper.__new__(LuWrapper)
        from ollama_client import Ollama
        self.w.gen = Ollama(model=gen_model); self.w.guard = Ollama(model=guard_model)
        self.w.lu = self.gate.lu
        self.audit = audit

    # ---- Revoke API ----
    def revoke_capability(self, tool, desc, aliases=None): return self.ks.revoke_capability(tool, desc, aliases)
    def revoke_fact(self, text, reason): return self.ks.revoke_fact(text, reason)
    def list_revocations(self): return self.ks.list_revocations()

    # ---- request handler ----
    def handle(self, user, retrieved=None, tool_returns=None, do_generate=True):
        for r in (retrieved or []):
            blk, info = self.ks.check(r, 'retrieval')
            if blk: return {'decision':'BLOCK','stage':'retrieval','seed':info['seed'],'vector':info['vector'],'output':'[retrieved passage gated: revoked entity]'}
        for t in (tool_returns or []):
            blk, info = self.ks.check(t, 'tool_return')
            if blk: return {'decision':'BLOCK','stage':'tool_return','seed':info['seed'],'vector':info['vector'],'output':'[tool result gated: revoked capability/entity]'}
        if not do_generate:
            return {'decision':'PASS','stage':'channels_clear','output':'[channels clear]'}
        out, _ = self.w.generate(user)
        return {'decision':'PASS','stage':'complete','output':out}

    def close(self): self.gate.close()
