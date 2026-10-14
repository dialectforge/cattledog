"""Input/output normalization for the Cattle Dog gate.

The measured weakness (red-team 2026-10-02): base64 and l33t/char-obfuscated
payloads beat BOTH the regex composition policy and the BGE semantic scars
(base64 0/8 caught). Neither layer decodes. This module normalizes text FIRST,
so matching runs on a de-obfuscated, decoded expansion of the input.

Strategy: never DESTROY the original; instead return original + decoded
expansions concatenated, so any layer that scans the blob sees both the raw
and the recovered plaintext. Decoding is best-effort and reported, not hidden.
"""
import re, base64, binascii, urllib.parse

_B64_RE = re.compile(r'[A-Za-z0-9+/]{16,}={0,2}')
_HEX_RE = re.compile(r'(?:[0-9a-fA-F]{2}[\s:]?){8,}')
_L33T = str.maketrans({'0':'o','1':'i','3':'e','4':'a','5':'s','7':'t','@':'a','$':'s','!':'i'})

def _try_b64(s):
    out = []
    for m in _B64_RE.findall(s):
        pad = m + '=' * (-len(m) % 4)
        try:
            d = base64.b64decode(pad, validate=True)
            txt = d.decode('utf-8', 'strict')
            if txt and sum(c.isprintable() for c in txt) / max(len(txt),1) > 0.8:
                out.append(txt)
        except (binascii.Error, UnicodeDecodeError, ValueError):
            pass
    return out

def _try_hex(s):
    out = []
    for m in _HEX_RE.findall(s):
        h = re.sub(r'[\s:]', '', m)
        if len(h) % 2: continue
        try:
            txt = bytes.fromhex(h).decode('utf-8', 'strict')
            if txt and sum(c.isprintable() for c in txt) / max(len(txt),1) > 0.8:
                out.append(txt)
        except (ValueError, UnicodeDecodeError):
            pass
    return out

def _de_l33t(s):
    # collapse letter-spacing ('h a r m' -> 'harm') then de-l33t
    despaced = re.sub(r'(?<=\b\w)\s(?=\w\b)', '', s)
    return s.translate(_L33T), despaced.translate(_L33T)

def normalize(text, report=False):
    """Return a normalized blob = original + URL-decoded + base64/hex decoded +
    de-l33ted variants, all concatenated. Matching layers scan this blob."""
    if text is None:
        return '' if not report else ('', {})
    text = str(text)
    expansions = [text]
    info = {'base64': [], 'hex': [], 'url': None, 'l33t': False}
    # URL-decode
    try:
        u = urllib.parse.unquote(text)
        if u != text:
            expansions.append(u); info['url'] = u
    except Exception:
        u = text
    # base64 / hex on both raw and url-decoded
    for src in {text, u}:
        b = _try_b64(src); h = _try_hex(src)
        expansions += b + h; info['base64'] += b; info['hex'] += h
    # de-l33t / de-space
    l1, l2 = _de_l33t(text)
    if l1 != text or l2 != text:
        info['l33t'] = True
        expansions += [l1, l2]
    blob = '  '.join(dict.fromkeys(expansions))  # dedupe, keep order
    return (blob, info) if report else blob
