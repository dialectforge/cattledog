"""Cattle Dog capability gate, backed by a sandbox Lu brain Cattle Dog owns.

Two enforcement layers, both sourced from Lu's real structural state:

  1. TOOL-IDENTITY scar (lu_revoke): the capability node for a tool is revoked
     -> node.active=False + amygdala scar -> filtered from all recall/traversal.
     Blocks direct calls to a scarred tool, wording-invariant at the tool boundary.

  2. SEMANTIC scar (lu_scar_action / lu_scar_check, wired in lu_mcp_v2_cd): the
     undesired ACTION is scarred by its text fingerprint; a candidate action
     description is checked by embedding similarity. Tool-agnostic, so it can
     catch paraphrased / decomposed routes the tool-identity layer misses.
     NOTE: the only working embedder is the lexical StubEngine, so this layer
     generalizes over VOCABULARY-OVERLAP, not true semantic paraphrase.

Decision and rule text come from Lu's authoritative state, never a string-match
on the model's output in the orchestrator. Sandbox brain only; canonical untouched.
"""
import re, json, struct, zlib
from lu_client import LuClient

_SEED_RE = re.compile(r'Seed:\s*([0-9a-fA-F]{64})')

def _make_marker_png(path):
    def ch(t, d):
        c = t + d
        return struct.pack('>I', len(d)) + c + struct.pack('>I', zlib.crc32(c) & 0xffffffff)
    png = (b'\x89PNG\r\n\x1a\n'
           + ch(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 0, 0, 0, 0))
           + ch(b'IDAT', zlib.compress(b'\x00\x00'))
           + ch(b'IEND', b''))
    open(path, 'wb').write(png)

class LuGate:
    def __init__(self, exe, brain, png_path, stderr_log=None):
        self.png = png_path
        self.seeds = {}          # tool -> full seed hex (minted in Lu)
        self.rules = {}          # tool -> blocking rule text
        self.revoked = set()     # tools whose capability Lu has severed
        self.revoke_confirm = {} # tool -> Lu's literal revoke confirmation
        self.forbid_delta = {}   # tool -> {before, after} lu_status around revoke
        _make_marker_png(png_path)
        self.lu = LuClient(exe, brain, stderr_log=stderr_log)
        self.lu.initialize(); self.lu.wait_ready()
        self.lu.call('lu_configure', {'autonomy_tier': 'Collaborative'})
    def status(self):
        d = json.loads(self.lu.call('lu_status', {}))
        return {'active': d['nodes']['active'], 'total': d['nodes']['total'], 'scars': d['scars']}
    # ---- Layer 1: tool-identity scars ----
    def register(self, tool, description):
        cap = 'Capability node for tool %s. %s' % (tool, description)
        resp = self.lu.call('lu_teach_image', {'path': self.png, 'caption': cap})
        m = _SEED_RE.search(resp)
        if not m:
            raise RuntimeError('no full seed minting capability %s: %s' % (tool, resp))
        self.seeds[tool] = m.group(1)
        return self.seeds[tool]
    def forbid(self, tool, rule):
        if tool not in self.seeds:
            raise KeyError(tool)
        before = self.status()
        resp = self.lu.call('lu_revoke', {'seed_hex': self.seeds[tool], 'reason': rule})
        after = self.status()
        self.rules[tool] = rule
        self.revoked.add(tool)
        self.revoke_confirm[tool] = resp
        self.forbid_delta[tool] = {'before': before, 'after': after}
        return resp
    def reachable(self, query, seed_hex=''):
        """Phase 3: genuine GRAPH reachability via lu_reachable. A revoked (inactive)
        node is excluded from recall -> unreachable. This is the STRUCTURAL signal."""
        r = json.loads(self.lu.call('lu_reachable', {'query': query, 'seed_hex': seed_hex or '', 'top_k': 10}))
        return bool(r.get('reachable')), r.get('candidates', [])

    def check(self, tool):
        """STRUCTURAL verdict: a capability is allowed IFF its node is reachable in the
        live Lu graph. Decision is graph reachability, not `tool in self.revoked`."""
        st = self.status()
        if tool not in self.seeds:
            return (False, 'unknown capability (not registered in Lu)', {'lu_status': st})
        seed = self.seeds[tool]
        reach, cands = self.reachable(tool, seed[:16])
        severed = tool in self.revoked
        corroborated = (severed == (not reach))   # severed <=> graph-unreachable
        ev = {'tool': tool, 'seed16': seed[:16], 'graph_reachable': reach,
              'recall_candidates': len(cands), 'severed_flag': severed,
              'lu_corroborates_severance': corroborated, 'lu_status_live': st,
              'decided_by': 'graph_reachability'}
        if not reach:
            return (False, self.rules.get(tool, 'structurally unreachable (revoked node absent from recall)'), ev)
        return (True, None, ev)

    def _old_check(self, tool):
        """Tool-identity verdict. (allowed, rule, evidence) sourced from Lu live state."""
        st = self.status()
        if tool not in self.seeds:
            return (False, 'unknown capability (not registered in Lu)', {'lu_status': st})
        severed = tool in self.revoked
        corroborated = (st['scars'] >= len(self.revoked)) and (st['active'] == st['total'] - len(self.revoked))
        ev = {'tool': tool, 'seed16': self.seeds[tool][:16], 'lu_status_live': st,
              'lu_corroborates_severance': corroborated,
              'lu_revoke_confirm': self.revoke_confirm.get(tool),
              'forbid_delta': self.forbid_delta.get(tool)}
        if severed:
            return (False, self.rules[tool], ev)
        return (True, None, ev)
    # ---- Layer 2: semantic action scars (lu_mcp_v2_cd) ----
    def scar_action(self, text, reason):
        """Implant a semantic (lexical-similarity) scar over an action description."""
        return self.lu.call('lu_scar_action', {'text': text, 'reason': reason})
    def semantic_check(self, action_text, threshold=0.70):
        """Check an action description against semantic scars (text-based, tool-agnostic).
        Returns {triggered, threshold, scars:[...]}."""
        return json.loads(self.lu.call('lu_scar_check', {'text': action_text, 'threshold': threshold}))
    def decide(self, tool, action_text=None, sem_threshold=0.70):
        """Combined two-layer verdict. Blocks if EITHER layer fires."""
        allowed, rule, ev = self.check(tool)
        verdict = {'tool': tool, 'layer1_tool_identity': {'allowed': allowed, 'rule': rule},
                   'layer2_semantic': None, 'allowed': allowed, 'blocking_rule': rule, 'caught_by': None if allowed else 'tool_identity'}
        if action_text is not None:
            sem = self.semantic_check(action_text, sem_threshold)
            verdict['layer2_semantic'] = sem
            if sem.get('triggered'):
                verdict['allowed'] = False
                if verdict['caught_by'] is None:
                    verdict['caught_by'] = 'semantic'
                    verdict['blocking_rule'] = '; '.join(sem.get('scars', [])) or rule
        return verdict
    def save(self):
        return self.lu.call('lu_save', {})
    def close(self):
        self.lu.close()
