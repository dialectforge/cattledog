#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
smart_extractor.py -- Cat.1 retrain rescue extractor v2 (iter 4).

Iter-4 changes vs iter-3 (Phase B subject-anchoring fix):
  * L5 numerical-attribute patterns now require the matched sentence to
    contain the article subject (title or a derived alias). Fixes the
    Phase A bug where 'Abraham Lincoln' picked up `BornOn February 12, 1809`
    (correct) AND `BornOn December 21, 1850` (wrong third party). Same
    fix applied to L3 lead-paragraph typed patterns since they share the
    failure mode (compound lead sentences with secondary subjects).
  * Helpers added: _split_sentences, _build_subject_aliases,
    _sentence_anchors_subject.
  * _isa_lead first regex (title-prefix) is unchanged -- it already anchors
    by construction. The fallback regex now runs only on subject-anchored
    sentences.

Iter-3 changes vs iter-2:
  * NEW L4: body proper-noun co-mention walker. Scans the body (excluding
    See also / References / External links / Bibliography tails) for
    capitalised noun phrases (1-4 words) and emits a RelatedTo bond from
    title to each phrase mentioned >=2 times. Weight scales with frequency.
    This is the highest-leverage layer because the HF wikimedia/wikipedia
    dump has stripped wikitext: there are no [[X]] internal links available
    as a side-channel, so we recover the same signal from text co-mention.
  * NEW L5: typed numerical attribute extractor. Scans body for explicit
    "founded in YYYY", "population of N", "area of N km^2", "altitude N m",
    "died on D", "born on D" patterns and emits typed bonds.
  * Per-article cap on L4: top 80 most-frequent body proper-nouns to prevent
    runaway hubs on long articles.
  * Stopword filter for L4: aggressive sentence-start drop list + month/day
    names + numeric-only + single-letter.

Iter-2 layers (L1 / L2 / L2b / L3-*) preserved unchanged in iter-3, with
L3 patterns now subject-anchored in iter-4.
"""

import re
from collections import Counter
from typing import List, Dict, Optional, Tuple

_HEADING_RX = re.compile(r"^([A-Z][A-Za-z0-9 ,\-/&'()]{2,79})\s*$")
_PUNCT_TAIL = re.compile(r"[\s\.,;:!?\-]+$")

_DROP_HEADINGS = {
    "see also", "references", "external links", "notes", "bibliography",
    "further reading", "sources", "footnotes", "citations", "gallery",
    "contents", "index",
}

_STOP_OBJECTS = {
    "", "the", "a", "an", "of", "and", "or", "but", "to", "with", "in", "on",
    "at", "by", "for", "from", "this", "that", "these", "those", "it", "its",
    "is", "was", "are", "were", "be", "been", "being",
}

_STOP_ISA_NOUNS = {
    "name", "romanization", "transliteration", "variant", "form", "version",
    "alternate", "alternative", "abbreviation", "acronym", "way", "thing",
    "matter", "manner", "pronunciation", "spelling", "young", "youngest",
    "oldest", "first", "second", "third", "latter", "former", "only", "such",
    "least", "most", "more", "less", "better", "worse", "best", "worst",
    "earlier", "later", "final", "initial", "original", "main", "primary",
    "secondary", "latest", "earliest", "standard", "normal", "typical",
    "common", "rare", "strange", "odd", "even", "same", "other", "another",
    "each", "type", "kind", "form", "species", "variety", "category", "set",
    "group", "class", "family", "order", "phylum", "genus", "piece",
}

# L4 sentence-start stopwords. Single-word capitalised tokens that almost
# always come from sentence starts, not entity references.
_L4_SINGLE_WORD_STOP = {
    # Sentence-start function words
    "the", "a", "an", "this", "that", "these", "those", "it", "its",
    "he", "she", "his", "her", "hers", "him", "i", "we", "they", "them",
    "their", "theirs", "you", "your", "yours", "my", "mine", "our", "ours",
    "in", "on", "at", "by", "for", "from", "to", "with", "of", "and", "or",
    "but", "so", "yet", "nor", "if", "when", "while", "as", "because",
    "although", "though", "whereas", "whilst", "during", "before",
    "after", "until", "since", "however", "therefore", "thus", "moreover",
    "furthermore", "nevertheless", "still", "yet", "meanwhile",
    "many", "much", "several", "some", "all", "both", "each", "every",
    "other", "another", "such", "any", "none", "few", "most", "more",
    "less", "least", "best", "worst", "first", "second", "third",
    "yes", "no", "maybe", "perhaps", "often", "sometimes", "always",
    "never", "there", "here", "now", "then", "today", "tomorrow",
    "yesterday", "once", "twice", "also", "only", "even", "just",
    # Spelled-out numbers (sentence-start: "One reason...", "Two of the...")
    "one", "two", "three", "four", "five", "six", "seven", "eight",
    "nine", "ten", "eleven", "twelve", "hundred", "thousand", "million",
    # Sentence-start hedges & adverbs
    "according", "besides", "thereafter", "thereby", "thereof", "thereupon",
    "unlike", "like", "upon", "throughout", "about", "around",
    "despite", "essentially", "basically", "finally",
    "firstly", "secondly", "thirdly", "originally", "eventually",
    "recently", "currently", "previously", "subsequently",
    # Auxiliaries
    "is", "was", "are", "were", "be", "been", "being", "has", "have",
    "had", "do", "does", "did", "will", "would", "could", "should",
    "can", "may", "might", "must", "shall",
    # Months
    "january", "february", "march", "april", "may", "june", "july",
    "august", "september", "october", "november", "december",
    # Days of week
    "monday", "tuesday", "wednesday", "thursday", "friday", "saturday",
    "sunday",
    # Common section/heading-like single words
    "see", "references", "notes", "sources", "bibliography", "contents",
    "introduction", "summary", "overview", "chapter", "section",
    "part", "page", "figure", "table", "image", "file",
    # Pronouns / titles handled separately
}

# L4 generic adjective stoplist (avoid "Greek", "American", etc. as standalone
# entities -- they're descriptors, not entities). Multi-word phrases that
# include them are still allowed.
_L4_GENERIC_ADJECTIVES = {
    "american", "british", "french", "german", "italian", "spanish",
    "russian", "chinese", "japanese", "korean", "indian", "european",
    "asian", "african", "australian", "canadian", "mexican", "brazilian",
    "greek", "roman", "latin", "arabic", "persian", "jewish", "christian",
    "muslim", "buddhist", "hindu", "islamic", "orthodox", "catholic",
    "protestant", "democratic", "republican", "socialist", "communist",
    "northern", "southern", "eastern", "western", "central", "upper",
    "lower", "new", "old", "modern", "ancient", "medieval", "early",
    "late", "middle", "native", "native",
}

def _clean(s):
    if not s: return ""
    s = s.strip()
    s = _PUNCT_TAIL.sub("", s)
    s = re.sub(r"\s+", " ", s)
    return s

def _ok_term(s, min_len=2, max_len=80):
    if not s: return False
    if len(s) < min_len or len(s) > max_len: return False
    if s.lower() in _STOP_OBJECTS: return False
    alpha = sum(1 for c in s if c.isalpha())
    if alpha < max(2, len(s) // 2): return False
    return True

def _split_paragraphs(text):
    return [p.strip() for p in text.split("\n\n") if p.strip()]

def _first_n_paragraphs(text, n=3):
    return _split_paragraphs(text)[:n]

_PERSON_NAME_RX = re.compile(r"^[A-Z][a-z]+(?:\s+[A-Z][a-z\.]+){1,2}$")
_CATEGORY_HINTS = re.compile(
    r"\b(?:s|people|writers|musicians|artists|scientists|families|works|albums|"
    r"songs|films|books|species|cuisine|history|culture|states|cities|towns|"
    r"villages|countries|presidents|composers|philosophers|kings|queens|wars|"
    r"battles|treaties|empires|kingdoms|republics|theorists|navies|forces|"
    r"academics|staff|writers|theory|theories|systems|methods|laws|rules|"
    r"religions|languages|alphabets|equations|systems|articles|inscriptions|"
    r"saints|gods|members|games|plays|essays)\b",
    re.IGNORECASE,
)

def _looks_like_person_name(s):
    if _PERSON_NAME_RX.match(s) and not _CATEGORY_HINTS.search(s):
        return True
    return False


# ============================================================
# Subject-anchoring helpers (iter-4)
# ============================================================
# Sentence boundary heuristic: split on .!?  followed by whitespace, OR newlines.
# Less aggressive than requiring [A-Z] follow-on -- works on "Mr. Smith was born
# March 14, 1879" by splitting at "Mr." and at the period after the date; both
# halves get the anchor check independently.
_SENT_SPLIT_RX = re.compile(r"(?<=[.!?])\s+|\n+")


def _split_sentences(text):
    """Crude sentence splitter. Returns list of non-empty sentence strings."""
    if not text: return []
    return [s.strip() for s in _SENT_SPLIT_RX.split(text) if s.strip()]


def _build_subject_aliases(title):
    """Return set of acceptable strings for matching the article subject in
    a sentence. Includes the full title plus each capitalised word in the
    title that is at least 4 chars long. For 'Abraham Lincoln' this returns
    {'Abraham Lincoln', 'Abraham', 'Lincoln'}; for short titles like 'ABC'
    only the title itself is returned."""
    aliases = set()
    if not title: return aliases
    aliases.add(title)
    for w in re.findall(r"[A-Z][A-Za-z]+", title):
        if len(w) >= 4:
            aliases.add(w)
    return aliases


def _sentence_anchors_subject(sentence, aliases):
    """True if the sentence contains any alias of the article subject.
    Substring match (case-sensitive) -- aliases are already capitalised."""
    if not aliases: return False
    for alias in aliases:
        if alias and alias in sentence:
            return True
    return False


# ============================================================
# L1 -- See also section walker (RelatedTo)
# ============================================================
def extract_see_also(title, text, source_url):
    bonds = []
    if not text or not title: return bonds
    m = re.search(r"\nSee also\s*\n(.*?)(?=\n[A-Z][A-Za-z][^\n]{0,60}\s*\n|$)",
                  text, re.DOTALL)
    if not m: return bonds
    block = m.group(1)
    for raw in block.split("\n"):
        item = _clean(raw.lstrip(" *\t•·–—‐-"))
        if not _ok_term(item, min_len=3, max_len=80): continue
        if item.lower() in _DROP_HEADINGS: continue
        if item.startswith("http") or item.startswith("www."): continue
        bonds.append({
            "subject": title, "relation": "RelatedTo", "object": item,
            "weight": 0.55, "confidence": "Unverified",
            "layer": "L1", "source_url": source_url,
        })
    return bonds


# ============================================================
# L2 -- Section heading walker (HasAspect)
# ============================================================
def extract_section_headings(title, text, source_url):
    bonds = []
    if not text or not title: return bonds
    lines = text.split("\n")
    for i, line in enumerate(lines):
        line = line.rstrip()
        if not _HEADING_RX.match(line): continue
        head = _clean(line)
        if not _ok_term(head, min_len=4, max_len=79): continue
        if head.lower() in _DROP_HEADINGS: continue
        prev_empty = (i == 0) or (not lines[i-1].strip())
        if not prev_empty: continue
        if head.lower() == title.lower(): continue
        bonds.append({
            "subject": title, "relation": "HasAspect", "object": head,
            "weight": 0.5, "confidence": "Unverified",
            "layer": "L2", "source_url": source_url,
        })
    return bonds


# ============================================================
# L2b -- Category-tail walker (IsA)
# ============================================================
def extract_category_tail(title, text, source_url):
    bonds = []
    if not text or not title: return bonds
    anchor_match = None
    for marker in ("\nExternal links\n", "\nReferences\n"):
        idx = text.rfind(marker)
        if idx > 0 and (anchor_match is None or idx > anchor_match):
            anchor_match = idx
    tail = text[anchor_match:] if anchor_match else text[-2500:]
    for line in tail.split("\n"):
        line = line.strip()
        if not line: continue
        if not _HEADING_RX.match(line): continue
        if line.endswith((".", "!", "?", ":", ";")): continue
        cat = _clean(line)
        if not _ok_term(cat, min_len=4, max_len=70): continue
        if cat.lower() in _DROP_HEADINGS: continue
        if cat.lower() == title.lower(): continue
        if re.match(r"^\d{1,4}$", cat): continue
        if _looks_like_person_name(cat): continue
        bonds.append({
            "subject": title, "relation": "IsA", "object": cat,
            "weight": 0.5, "confidence": "Unverified",
            "layer": "L2b", "source_url": source_url,
        })
    return bonds


# ============================================================
# L3 -- Lead-paragraph sentence patterns (typed bonds)
# ============================================================
_RX_LOCATED = re.compile(r"\b(?:located|situated|based|headquartered)\s+(?:in|at|near)\s+(the\s+)?([A-Z][a-zA-Z]{1,30}(?:\s+(?:[A-Z][a-zA-Z]{1,30}|of|the|de)){0,3})(?=[.;\n]|,\s+(?:which|who|where|that|and))")
_RX_IN_PLACE = re.compile(r"\bin\s+the\s+(?:city|town|village|state|country|province|region|county|district|kingdom|empire|republic)\s+of\s+([A-Z][a-zA-Z]{1,30}(?:\s+(?:[A-Z][a-zA-Z]{1,30}|of|the|de)){0,3})(?=[.;\n,])")
_RX_PART_OF = re.compile(r"\b(?:part|member|component)\s+of\s+(?:the\s+)?([A-Z][a-zA-Z]{1,30}(?:\s+(?:[A-Z][a-zA-Z]{1,30}|of|the|de)){0,3})(?=[.;\n,])")
_RX_FOUNDED_BY = re.compile(r"\b(?:founded|established|created|invented|developed)\s+by\s+([A-Z][a-z\.\-]+(?:\s+(?:[A-Z][a-z\.\-]+|de|van|von|of|the|den)){0,3})(?=[.;\n,]|\s+in\s+\d)")
_RX_FOUNDED_IN = re.compile(r"\b(?:founded|established|created|built|formed)\s+in\s+(\d{3,4}(?:\s*BC)?)\b")
_RX_DISCOVERED_BY = re.compile(r"\bdiscovered\s+by\s+([A-Z][a-z\.\-]+(?:\s+(?:[A-Z][a-z\.\-]+|de|van|von|of|the|den)){0,3})(?=[.;\n,]|\s+in\s+\d)")
_RX_NAMED_AFTER = re.compile(r"\bnamed\s+(?:after|for|in honor of|in honour of)\s+([A-Z][a-z\.\-]+(?:\s+(?:[A-Z][a-z\.\-]+|de|van|von|of|the|den)){0,3})(?=[.;\n,])")
_RX_MEASURED_IN = re.compile(r"\b(?:measured|expressed|recorded)\s+in\s+([a-z][a-z\- ]{2,40}?)(?=[.;\n,])")
_RX_BORN_IN = re.compile(r"\bborn\s+(?:on\s+\w+\s+\d+,?\s+)?(?:in\s+)?([A-Z][A-Za-z\-, ]{2,80}?|\d{3,4})(?=[.;\n,]|\s+in\s+\d)")
_RX_DIED_IN = re.compile(r"\bdied\s+(?:on\s+\w+\s+\d+,?\s+)?(?:in\s+)?([A-Z][A-Za-z\-, ]{2,80}?|\d{3,4})(?=[.;\n,]|\s+in\s+\d)")
_RX_WRITTEN_BY = re.compile(r"\b(?:written|composed|directed|produced|painted|sculpted|designed)\s+by\s+([A-Z][a-z\.\-]+(?:\s+(?:[A-Z][a-z\.\-]+|de|van|von|of|the|den)){0,3})(?=[.;\n,]|\s+in\s+\d)")
_RX_COMPOSED_OF = re.compile(r"\b(?:consists|composed|made|comprised|formed)\s+of\s+([a-z][a-zA-Z \-,]{2,80}?)(?=[.;\n])")
_RX_KNOWN_FOR = re.compile(r"\b(?:known|famous|noted|celebrated)\s+for\s+([a-z][a-zA-Z \-,]{2,80}?)(?=[.;\n])")
_RX_USED_FOR = re.compile(r"\b(?:used|employed)\s+(?:as|for|in)\s+([a-z][a-zA-Z \-,]{2,80}?)(?=[.;\n])")
_RX_TYPE_OF = re.compile(r"\b(?:a|an)\s+(?:type|kind|form|species|variety|category)\s+of\s+([a-z][a-zA-Z \-]{2,60}?)(?=[.;\n,])")

_L3_PATTERNS = [
    (_RX_LOCATED, "LocatedIn", 0.6, "L3-loc", 2),
    (_RX_IN_PLACE, "LocatedIn", 0.6, "L3-loc", 1),
    (_RX_PART_OF, "PartOf", 0.55, "L3-part", 1),
    (_RX_FOUNDED_BY, "FoundedBy", 0.6, "L3-foundby", 1),
    (_RX_FOUNDED_IN, "FoundedIn", 0.6, "L3-foundin", 1),
    (_RX_DISCOVERED_BY, "DiscoveredBy", 0.6, "L3-discby", 1),
    (_RX_NAMED_AFTER, "NamedAfter", 0.6, "L3-named", 1),
    (_RX_MEASURED_IN, "MeasuredIn", 0.5, "L3-meas", 1),
    (_RX_BORN_IN, "BornIn", 0.55, "L3-born", 1),
    (_RX_DIED_IN, "DiedIn", 0.55, "L3-died", 1),
    (_RX_WRITTEN_BY, "WrittenBy", 0.6, "L3-writby", 1),
    (_RX_COMPOSED_OF, "ComposedOf", 0.5, "L3-comp", 1),
    (_RX_KNOWN_FOR, "KnownFor", 0.5, "L3-known", 1),
    (_RX_USED_FOR, "UsedFor", 0.5, "L3-used", 1),
    (_RX_TYPE_OF, "IsA", 0.55, "L3-type", 1),
]

_TRAILING_JUNK = re.compile(r"\s+(?:and|or|but|which|who|where|that|in|on|of|at|by|for|from|to|with|the)\s*$", re.IGNORECASE)

def _trim_object(s):
    while True:
        new = _TRAILING_JUNK.sub("", s).strip()
        if new == s: break
        s = new
    return _clean(s)


def _isa_lead_title_prefix(title, lead):
    """First-pass IsA: requires title to immediately precede 'is/was'.
    Anchored by construction -- safe regardless of subject-anchoring."""
    if not title: return None
    title_esc = re.escape(title)
    rx = re.compile(
        title_esc + r"\s+(?:is|was|are|were)\s+(?:an?|the)\s+([a-z][a-zA-Z\-]{2,30}(?:\s+[a-z][a-zA-Z\-]{2,30}){0,3})(?=[.;,]|$)",
        re.IGNORECASE,
    )
    m = rx.search(lead)
    if m:
        cat = _clean(m.group(1)).lower()
        cat = _trim_object(cat).lower()
        head_word = cat.split()[0] if cat else ""
        if head_word in _STOP_ISA_NOUNS:
            cat = ""
        if _ok_term(cat, min_len=3, max_len=70):
            return cat, 0.65
    return None


_RX_ISA_FALLBACK = re.compile(
    r"\b(?:is|was|are|were)\s+(?:an?|the)\s+([a-z][a-zA-Z\-]{2,30}(?:\s+[a-z][a-zA-Z\-]{2,30}){0,3})(?=[.;,]|\bthat\b|\bwhich\b|\bwho\b|\bwhere\b|\bin\b|\bof\b|\bon\b|$)"
)


def _isa_lead_fallback_in_sentence(sentence):
    """Fallback IsA on a single (subject-anchored) sentence."""
    m = _RX_ISA_FALLBACK.search(sentence)
    if m:
        cat = _clean(m.group(1)).lower()
        cat = _trim_object(cat).lower()
        head_word = cat.split()[0] if cat else ""
        if head_word in _STOP_ISA_NOUNS:
            cat = ""
        if _ok_term(cat, min_len=3, max_len=60):
            return cat, 0.5
    return None


def extract_lead_patterns(title, text, source_url):
    """Lead-paragraph typed bonds with subject-anchoring (iter-4).

    Two-stage: (a) the title-prefix IsA regex runs against the joined lead
    text -- it's safe by construction since it requires the title to
    immediately precede 'is/was'. (b) All other L3 patterns and the IsA
    fallback run only on sentences within the lead that contain the article
    subject (title or alias). This filters out compound-sentence false
    positives like "His mother was born in Virginia." in the Lincoln lead.
    """
    bonds = []
    if not text or not title: return bonds
    paragraphs = _first_n_paragraphs(text, 3)
    if not paragraphs: return bonds
    lead_text = " ".join(paragraphs)

    aliases = _build_subject_aliases(title)
    sentences = _split_sentences(lead_text)

    # Title-prefix IsA: anchored by construction, run on full lead text.
    isa = _isa_lead_title_prefix(title, lead_text)
    if isa:
        cat, w = isa
        bonds.append({
            "subject": title, "relation": "IsA", "object": cat,
            "weight": w, "confidence": "Inferred",
            "layer": "L3-isa", "source_url": source_url,
        })

    seen_relobj = set()
    # Track whether the title-prefix IsA already fired so the fallback doesn't
    # double-emit for the same article.
    isa_fired = isa is not None

    for sentence in sentences:
        if not _sentence_anchors_subject(sentence, aliases):
            continue

        # Fallback IsA in this sentence (only if title-prefix didn't fire).
        if not isa_fired:
            fb = _isa_lead_fallback_in_sentence(sentence)
            if fb:
                cat, w = fb
                key = ("IsA", cat.lower())
                if key not in seen_relobj:
                    seen_relobj.add(key)
                    bonds.append({
                        "subject": title, "relation": "IsA", "object": cat,
                        "weight": w, "confidence": "Inferred",
                        "layer": "L3-isa", "source_url": source_url,
                    })
                    isa_fired = True

        for entry in _L3_PATTERNS:
            rx, relation, weight, tag, group_idx = entry
            for m in rx.finditer(sentence):
                try:
                    obj = m.group(group_idx)
                except IndexError:
                    continue
                obj = _trim_object(_clean(obj))
                if not _ok_term(obj, min_len=3, max_len=80): continue
                if obj.lower() in _STOP_OBJECTS: continue
                if obj.lower() == title.lower(): continue
                if relation == "IsA":
                    head_word = obj.lower().split()[0] if obj else ""
                    if head_word in _STOP_ISA_NOUNS: continue
                key = (relation, obj.lower())
                if key in seen_relobj: continue
                seen_relobj.add(key)
                bonds.append({
                    "subject": title, "relation": relation, "object": obj,
                    "weight": weight, "confidence": "Inferred",
                    "layer": tag, "source_url": source_url,
                })
    return bonds


# ============================================================
# L4 -- Body proper-noun co-mention walker (RelatedTo)  *** iter-3 ***
# ============================================================
# Capture proper-noun phrases of 1-4 capitalised words. Hyphens and apostrophes
# allowed inside a word (e.g. "Pierre-Joseph Proudhon", "O'Brien"). Multi-word
# captures are kept intact.
_L4_CAP_RX = re.compile(
    r"\b((?:[A-Z][a-z]+(?:[\-'][A-Z]?[a-z]+)?)"  # head word
    r"(?:\s+(?:[A-Z][a-z]+(?:[\-'][A-Z]?[a-z]+)?|of|the|de|van|von|den|du|la)){0,3}"  # 0-3 connectors
    r")\b"
)

def _strip_tail_sections(text):
    """Remove trailing See also / References / External links / Bibliography
    blocks before scanning body. Anchored on the EARLIEST tail marker."""
    earliest = None
    for marker in ("\nSee also\n", "\nReferences\n", "\nExternal links\n",
                   "\nBibliography\n", "\nFurther reading\n", "\nNotes\n",
                   "\nFootnotes\n", "\nCitations\n"):
        idx = text.find(marker)
        if idx > 0 and (earliest is None or idx < earliest):
            earliest = idx
    return text[:earliest] if earliest is not None else text

def _normalize_l4_phrase(phrase):
    """Strip leading determiners, trailing connectors, and trailing
    apostrophe-s possessive."""
    words = phrase.split()
    while words and words[0].lower() in ("the", "a", "an"):
        words = words[1:]
    while words and words[-1].lower() in ("of", "the", "de", "van", "von",
                                          "den", "du", "la", "and", "or"):
        words = words[:-1]
    if words and (words[-1].endswith("'s") or words[-1].endswith("’s")):
        last = words[-1].rstrip("’'s")
        if last:
            words[-1] = last
        else:
            words = words[:-1]
    return " ".join(words), words


def _title_word_prefixes(title, prefix_len=5):
    """Set of lowercase N-char prefixes for words in the title >= prefix_len."""
    out = set()
    for w in re.findall(r"[A-Za-z]+", title):
        if len(w) >= prefix_len:
            out.add(w.lower()[:prefix_len])
    return out


def extract_body_comentions(title, text, source_url, max_emit=80):
    """L4: scan body for capitalised noun phrases mentioned >=2 times,
    emit RelatedTo bonds. Cap at top max_emit by frequency."""
    bonds = []
    if not text or not title: return bonds
    body = _strip_tail_sections(text)
    if not body: return bonds

    title_lower = title.lower()
    title_words = {w.lower() for w in re.findall(r"[A-Za-z]+", title)}
    title_prefixes = _title_word_prefixes(title, prefix_len=5)

    counts = Counter()
    for m in _L4_CAP_RX.finditer(body):
        raw = _clean(m.group(1))
        if not raw: continue
        phrase, words = _normalize_l4_phrase(raw)
        if not phrase or not words: continue

        if len(words) == 1:
            wl = phrase.lower()
            if wl in _L4_SINGLE_WORD_STOP: continue
            if wl in _L4_GENERIC_ADJECTIVES: continue
            if wl in _STOP_OBJECTS: continue
            if len(phrase) < 3: continue
            if wl in title_words: continue
            if len(wl) >= 5 and wl[:5] in title_prefixes:
                continue
        if phrase.lower() == title_lower: continue
        if re.match(r"^\d+$", phrase): continue
        if not _ok_term(phrase, min_len=3, max_len=80): continue
        counts[phrase] += 1

    candidates = [(p, c) for p, c in counts.items() if c >= 2]
    candidates.sort(key=lambda x: (-x[1], x[0]))
    candidates = candidates[:max_emit]

    for phrase, freq in candidates:
        if freq >= 5:
            weight = 0.55
        elif freq >= 3:
            weight = 0.5
        else:
            weight = 0.4
        bonds.append({
            "subject": title, "relation": "RelatedTo", "object": phrase,
            "weight": weight, "confidence": "Unverified",
            "layer": "L4", "source_url": source_url,
            "frequency": freq,
        })
    return bonds


# ============================================================
# L5 -- Numerical / typed attribute extractor  *** iter-3, anchored iter-4 ***
# ============================================================
_RX_BODY_FOUNDED_IN = re.compile(
    r"\b(?:founded|established|created|built|formed|incorporated)\s+in\s+(\d{3,4}(?:\s*BC)?)\b"
)
_RX_POPULATION = re.compile(
    r"\b(?:population|populace)\s+of\s+(?:over\s+|about\s+|approximately\s+|around\s+)?"
    r"([\d,]{2,15}(?:\.\d+)?\s*(?:million|thousand|billion)?)\b",
    re.IGNORECASE,
)
_RX_AREA_KM2 = re.compile(
    r"\b(?:area|covers|spans)\s+(?:of\s+)?(?:about\s+|approximately\s+|around\s+)?"
    r"([\d,]{1,12}(?:\.\d+)?\s*(?:square\s+kilometres?|square\s+kilometers?|km2|km²|sq\s+mi))\b",
    re.IGNORECASE,
)
_RX_ALTITUDE = re.compile(
    r"\b(?:altitude|elevation|height)\s+of\s+(?:about\s+|approximately\s+)?"
    r"([\d,]{1,10}(?:\.\d+)?\s*(?:metres?|meters?|m|feet|ft))\b",
    re.IGNORECASE,
)
_RX_LANG_OFFICIAL = re.compile(
    r"\bofficial\s+language(?:s)?\s+(?:is|are|of\s+\w+\s+(?:is|are))\s+"
    r"([A-Z][a-zA-Z\-]{2,30}(?:(?:,\s+|\s+and\s+)[A-Z][a-zA-Z\-]{2,30}){0,3})\b"
)
_RX_CAPITAL = re.compile(
    r"\bcapital(?:\s+city)?\s+(?:is|of\s+\w+\s+is)\s+"
    r"([A-Z][a-zA-Z\-]{2,30}(?:\s+[A-Z][a-zA-Z\-]{2,30}){0,2})\b"
)
_RX_BORN_DATE = re.compile(
    r"\b(?:born|b\.)\s+(?:on\s+)?"
    r"((?:January|February|March|April|May|June|July|August|September|October|November|December)"
    r"\s+\d{1,2},?\s+\d{3,4})"
)
_RX_DIED_DATE = re.compile(
    r"\b(?:died|d\.)\s+(?:on\s+)?"
    r"((?:January|February|March|April|May|June|July|August|September|October|November|December)"
    r"\s+\d{1,2},?\s+\d{3,4})"
)

_L5_PATTERNS = [
    (_RX_BODY_FOUNDED_IN, "FoundedIn", 0.6, "L5-foundyear"),
    (_RX_POPULATION, "HasPopulation", 0.55, "L5-pop"),
    (_RX_AREA_KM2, "HasArea", 0.55, "L5-area"),
    (_RX_ALTITUDE, "HasAltitude", 0.55, "L5-alt"),
    (_RX_LANG_OFFICIAL, "HasOfficialLanguage", 0.6, "L5-lang"),
    (_RX_CAPITAL, "HasCapital", 0.6, "L5-cap"),
    (_RX_BORN_DATE, "BornOn", 0.6, "L5-borndt"),
    (_RX_DIED_DATE, "DiedOn", 0.6, "L5-dieddt"),
]


def extract_numerical_attrs(title, text, source_url):
    """L5 typed-attribute bonds with subject-anchoring (iter-4).

    Each pattern is now run only on sentences that contain the article
    subject (title or alias). This fixes the Phase A bug where biography
    articles emitted BornOn/DiedOn dates from third-party sentences."""
    bonds = []
    if not text or not title: return bonds
    body = _strip_tail_sections(text)
    if not body: return bonds

    aliases = _build_subject_aliases(title)
    sentences = _split_sentences(body)

    seen = set()
    for sentence in sentences:
        if not _sentence_anchors_subject(sentence, aliases):
            continue
        for rx, relation, weight, tag in _L5_PATTERNS:
            for m in rx.finditer(sentence):
                obj = _trim_object(_clean(m.group(1)))
                if not obj: continue
                if not _ok_term(obj, min_len=2, max_len=80): continue
                if obj.lower() == title.lower(): continue
                key = (relation, obj.lower())
                if key in seen: continue
                seen.add(key)
                bonds.append({
                    "subject": title, "relation": relation, "object": obj,
                    "weight": weight, "confidence": "Inferred",
                    "layer": tag, "source_url": source_url,
                })
    return bonds


# ============================================================
# Top-level extractor: runs all layers, dedups
# ============================================================
def extract_bonds_from_wikipedia_article(article, l4_max_emit=80):
    title = (article.get("title") or "").strip()
    text = article.get("text") or ""
    source_url = article.get("url") or ""
    if not title or not text: return []
    bonds = []
    bonds.extend(extract_see_also(title, text, source_url))
    bonds.extend(extract_section_headings(title, text, source_url))
    bonds.extend(extract_category_tail(title, text, source_url))
    bonds.extend(extract_lead_patterns(title, text, source_url))
    bonds.extend(extract_body_comentions(title, text, source_url,
                                         max_emit=l4_max_emit))
    bonds.extend(extract_numerical_attrs(title, text, source_url))
    seen = set()
    out = []
    for b in bonds:
        key = (b["subject"].lower(), b["relation"], b["object"].lower())
        if key in seen: continue
        seen.add(key)
        out.append(b)
    return out


if __name__ == "__main__":
    import json
    sample = {
        "id": "test",
        "url": "https://en.wikipedia.org/wiki/Test",
        "title": "Albedo",
        "text": (
            "Albedo is the fraction of sunlight that is reflected by a body. "
            "It is measured in dimensionless units. The term albedo was "
            "introduced by Johann Heinrich Lambert in his 1760 work Photometria. "
            "Lambert worked on Photometria for years; Photometria was published "
            "in Augsburg in 1760. Lambert's contributions to optics include "
            "diffuse reflectance, which Lambert formulated rigorously.\n\n"
            "Surface albedo is defined as the ratio of radiosity to irradiance. "
            "Surface albedo of Earth varies. Earth has an average albedo of 0.3.\n\n"
            "Terrestrial albedo\n\n"
            "Any albedo in visible light falls within a range of about 0.9 for snow.\n\n"
            "See also\n\n Cool roof\n Daisyworld\n Emissivity\n Global dimming\n\n"
            "References\n\nExternal links\n\nClimatology\nRadiometry\nMeteorological quantities\n"
        ),
    }
    bonds = extract_bonds_from_wikipedia_article(sample)
    print(json.dumps(bonds, indent=2))
    print(f"\nTOTAL BONDS: {len(bonds)}")
