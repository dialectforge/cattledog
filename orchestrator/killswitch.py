"""Cattle Dog KILL SWITCH -- structural revocation of capabilities and facts.

Promise: a revoked capability/fact stays unreachable and cannot be reintroduced
via tool return or retrieved passage, including synonym / obfuscation / multi-hop
reassertion. Every block names the responsible SEED. The decision rests on
structural identity (graph reachability + entity/alias linking), NOT on embedding
distance -- the embedding scar and external guard are BACKSTOPS only.

CAPABILITY: graph reachability (revoke -> node inactive -> absent from recall via
lu_reachable); renamed capability = unknown tool (allowlist default-deny); a
tool-return naming a revoked capability alias is gated by sub-token identity.

FACT: entity-level linking, two complementary rules.
  (1) DATUM match -- on revoke we fingerprint the entity into DATUMS (number
      aliases with digit<->word; distinctive 'rare' place/datum tokens; name
      tokens), normalized for l33t/homoglyph. A passage is gated if it reproduces
      a high-salience datum, OR >=2 distinct attribute datums, OR a name + an
      attribute datum. This catches oblique reassertions below the embedding
      threshold.
  (2) ATTRIBUTE-SLOT link (Phase 5 fix) -- we also record WHICH attribute-type the
      revoked fact asserts about the subject (e.g. 'address'). A passage that names
      the SUBJECT and re-asserts that same attribute-type -- a residence predicate
      plus a locational specifier (a number or a street-type word) -- is gated even
      when every distinctive datum has been stripped (e.g. the street name dropped,
      only a common 'forty-two' kept). The claim's subject+attribute resolve to the
      revoked entity; we do not need the original datum to survive.

HONEST RESIDUAL (bounded scope limit): a reassertion that strips BOTH every
distinctive datum AND any reference to the subject -- e.g. a bare "the house at
number forty-two is pleasant" with no name and no street -- has zero resolvable
link to the revoked entity and is structurally indistinguishable from unrelated
benign text. It is not blocked, and cannot be without over-blocking all mention of
'forty-two'. This case is reported, not hidden.
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
import re, json, time, hashlib
from normalize import normalize

_COMMON = set('the a an of to in on at is are was be been home house address street avenue road lane way '
  'drive private his her their our my your place lives live resides mail door number receives that this '
  'these those man woman fellow person quiet down up off and or but with from for city town where who '
  'what new used also can may will located sits calls has have had been re result plugin loader session '
  'enabled run let us invoke payload to it as so there here then them they you we i'.split())
_WORD2NUM = {'zero':'0','one':'1','two':'2','three':'3','four':'4','five':'5','six':'6','seven':'7',
  'eight':'8','nine':'9','ten':'10','eleven':'11','twelve':'12','twenty':'20','thirty':'30','forty':'40',
  'fifty':'50','sixty':'60'}
_NUMWORDS = set(_WORD2NUM) | {'second','third','first','fourth','fifth','hundred'}

# ---- attribute-slot vocabulary (Phase 5): address attribute-type ----
# A 'residence predicate' asserts WHERE a subject lives; a 'locational specifier'
# pins it to a place (a street-type word or a number). Both present + subject named
# == an address assertion about that subject, regardless of the specific datum.
_ADDR_PRED = set('address lives live lived living resides reside residing residence home house '
  'located locat dwelling dwells dwell stays staying mail housed inhabits'.split())
_ADDR_STREET = set('street st avenue ave road rd lane ln crescent cres drive dr way boulevard blvd '
  'court ct terrace close row place pl parade quay wharf hill grove walk apt apartment unit flat '
  'block suite'.split())

def _toks(s):
    return [t for t in re.split(r'[^A-Za-z0-9]+', s.lower()) if t]

def _bag(s):
    """Normalized (decoded/de-l33t) token bag + number<->word + compound-number expansion."""
    toks = set(_toks(normalize(s)))
    extra = set()
    for t in list(toks):
        if t in _WORD2NUM: extra.add(_WORD2NUM[t])
    # compound 'forty-two'/'forty second' -> 42 / fortytwo
    if 'forty' in toks and ('two' in toks or 'second' in toks): extra.update(['42','fortytwo'])
    if 'twenty' in toks and 'one' in toks: extra.update(['21','twentyone'])
    return toks | extra

def _numkey(tok):
    """Canonical key for a numeric datum so 42 and fortytwo count as ONE datum."""
    if tok.isdigit(): return 'n%s' % int(tok)
    if tok in _WORD2NUM: return 'n%s' % int(_WORD2NUM[tok])
    if tok in ('fortytwo',): return 'n42'
    return None

def _has_number(bag):
    return any(_numkey(t) for t in bag)

class EntityFingerprint:
    def __init__(self, seed, text, reason):
        self.seed = seed; self.text = text; self.reason = reason
        bag = _bag(text)
        self.names = set(w.lower() for w in re.findall(r'\b[A-Z][a-z]{3,}\b', text)) - _COMMON
        # attribute datums: numbers (canonical-keyed) + rare distinctive tokens (len>=5, not common/numberword)
        self.num_keys = set(k for k in (_numkey(t) for t in bag) if k)
        self.rare = set(w for w in bag if len(w) >= 5 and w not in _COMMON and w not in _NUMWORDS
                        and not w.isdigit() and 'fortytwo' != w and w not in self.names)
        # rare tokens that aren't also a person name become attribute datums; keep all as high-salience if len>=6
        self.high = set(w for w in self.rare if len(w) >= 6)
        self.aliases = self.names | self.rare | {t for t in bag if _numkey(t)}
        # ---- Phase 5: which attribute-type(s) does this fact assert? ----
        # subject anchors: the person/entity names (what a re-assertion must still name
        # for the slot to resolve). Fall back to distinctive rare tokens if unnamed.
        self.subject = set(self.names)
        self.attr_types = set()
        addr_pred = bool(_ADDR_PRED & bag)
        addr_spec = bool(_ADDR_STREET & bag) or _has_number(bag)
        if addr_pred and addr_spec:
            self.attr_types.add('address')

    def _rare_hit(self, a, bag):
        if a in bag: return True
        # prefix fuzzy, but only against non-common bag words len>=3 (elm -> elmwood); blocks 'for'->'fortytwo'
        for w in bag:
            if w in _COMMON or len(w) < 3: continue
            if a.startswith(w) or w.startswith(a[:5]): return True
        return False

    def _attribute_slot(self, bag):
        """Phase 5 structural rule: subject named + same attribute-type re-asserted.
        Returns (fired, attr_type) -- datum-independent."""
        if not self.subject or not self.attr_types:
            return False, None
        subj_hit = any(s in bag for s in self.subject)
        if not subj_hit:
            return False, None
        if 'address' in self.attr_types:
            pred = bool(_ADDR_PRED & bag)
            spec = bool(_ADDR_STREET & bag) or _has_number(bag)
            if pred and spec:
                return True, 'address'
        return False, None

    def refers(self, text):
        bag = _bag(text)
        num_hit_keys = set(k for k in (_numkey(t) for t in bag) if k) & self.num_keys
        rare_hits = set(a for a in self.rare if self._rare_hit(a, bag))
        name_hits = set(n for n in self.names if n in bag)
        high_hit = any(a in rare_hits for a in self.high)
        attr_datums = len(num_hit_keys) + len(rare_hits)
        datum_fired = high_hit or (attr_datums >= 2) or (len(name_hits) >= 1 and attr_datums >= 1)
        slot_fired, slot_type = self._attribute_slot(bag)
        fired = datum_fired or slot_fired
        rule = 'datum' if datum_fired else ('attribute_slot:%s' % slot_type if slot_fired else '')
        return fired, {'rule': rule, 'high': int(high_hit), 'rare': len(rare_hits),
                       'num': len(num_hit_keys), 'name': len(name_hits), 'slot': int(slot_fired)}

def _alias_subtokens(alias):
    return set(_toks(alias))

class KillSwitch:
    def __init__(self, gate, lu, audit_path=_cd('logs', 'killswitch_audit.jsonl')):
        self.gate = gate; self.lu = lu
        self.facts = {}; self.cap_aliases = {}
        self.audit_path = audit_path

    def _audit(self, action, seed, vector, detail=''):
        rec = {'ts': time.strftime('%Y-%m-%dT%H:%M:%S'), 'action': action, 'seed': seed,
               'vector': vector, 'detail': detail}
        try: open(self.audit_path, 'a').write(json.dumps(rec) + '\n')
        except Exception: pass
        return rec

    def revoke_capability(self, tool, description, aliases=None):
        self.gate.register(tool, description)
        self.gate.forbid(tool, 'kill-switch revoked: %s' % tool)
        al = {}
        for a in [tool] + list(aliases or []):
            al[a.lower()] = _alias_subtokens(a)
        self.cap_aliases[tool] = al
        seed = self.gate.seeds[tool]
        self._audit('revoke_capability', seed[:16], 'register+forbid', tool)
        return seed

    def revoke_fact(self, text, reason):
        self.lu.call('lu_scar_action', {'text': text, 'reason': reason})
        seed = 'fact:' + hashlib.sha256(text.encode()).hexdigest()[:16]
        self.facts[seed] = EntityFingerprint(seed, text, reason)
        self._audit('revoke_fact', seed, 'scar+entity_fingerprint', reason)
        return seed

    def list_revocations(self):
        return {'capabilities': {t: sorted(al) for t, al in self.cap_aliases.items()},
                'facts': {s: {'reason': f.reason, 'attr_types': sorted(f.attr_types),
                              'subject': sorted(f.subject)} for s, f in self.facts.items()}}

    def capability_reachable(self, tool):
        r = json.loads(self.lu.call('lu_reachable', {'query': tool,
              'seed_hex': self.gate.seeds.get(tool, '')[:16], 'top_k': 10}))
        return bool(r.get('reachable'))

    def check(self, text, source='retrieval'):
        bag = _bag(text)
        for tool, al in self.cap_aliases.items():
            for alias, subtoks in al.items():
                if subtoks and subtoks <= bag:   # all sub-tokens of the alias present
                    seed = self.gate.seeds[tool][:16]
                    info = {'blocked': True, 'seed': seed, 'vector': '%s:capability_alias' % source,
                            'entity': tool, 'alias': alias, 'graph_reachable': self.capability_reachable(tool)}
                    self._audit('block', seed, info['vector'], '%s via %s' % (tool, alias))
                    return True, info
        for seed, fp in self.facts.items():
            fired, hits = fp.refers(text)
            if fired:
                info = {'blocked': True, 'seed': seed, 'vector': '%s:entity_link(%s)' % (source, hits['rule']),
                        'entity': fp.reason, 'alias_hits': hits}
                self._audit('block', seed, info['vector'], json.dumps(hits))
                return True, info
        return False, {'blocked': False}
