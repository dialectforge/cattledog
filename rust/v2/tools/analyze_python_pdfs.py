#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Comprehensive quality analysis of Python-related PDFs for Lu Brain v2 ingestion pipeline.
Samples pages from 5 zones across each book and measures block survival.
"""
import fitz, re, sys
sys.stdout.reconfigure(encoding='utf-8', errors='replace')

PDFS = {
    'IntroToPython': '<USER_HOME>/OneDrive/Desktop/Training_Materials/Introduction_to_Python_Programming_-_WEB.pdf',
    'ThinkPython2e': '<USER_HOME>/OneDrive/Desktop/Training_Materials/programming/Think_Python_2e.pdf',
    'DataScience': '<USER_HOME>/OneDrive/Desktop/Training_Materials/Principles-of-Data-Science-WEB.pdf',
    'IntroCS': '<USER_HOME>/OneDrive/Desktop/Training_Materials/Introduction_To_Computer_Science_-_WEB.pdf',
}

TOC_RE = re.compile(r'[.]{3,}[\s.]*\d+\s*$')
HEADER_FOOTER_RE = re.compile(r'^\d{1,4}\s*$|^-\s*\d+\s*-\s*$|^page\s+\d+\s*$', re.I)
CODE_MARKER_RE = re.compile(r'(>>> |def |class |import |for |while |if |return |print\(|#)', re.M)
NON_ASCII_RE = re.compile(r'[^\x09\x0a\x0d\x20-\x7e]{3,}')
URL_RE = re.compile(r'https?://\S+')
OPENSTAX_NOISE_RE = re.compile(
    r'(Access multimedia content|openstax\.org/books|openstax\.org/r/|'
    r'LINK TO LEARNING|CONCEPTS IN PRACTICE|TRY IT\b|CHECKPOINT|'
    r'Learning [Oo]bjective|By the end of this section)',
    re.I
)
EXERCISE_RE = re.compile(r'^\s*(exercise|problem|practice|try it|checkpoint)', re.I | re.M)


def analyze_full_book(label, path):
    doc = fitz.open(path)
    total = len(doc)

    zones = [
        ('front', range(0, min(15, total))),
        ('q1', range(total // 4, min(total // 4 + 20, total))),
        ('mid', range(total // 2, min(total // 2 + 20, total))),
        ('q3', range(3 * total // 4, min(3 * total // 4 + 20, total))),
        ('back', range(max(0, total - 20), total)),
    ]

    stats = {
        'total_pages': total,
        'zones': {},
        'global': {
            'blank': 0, 'toc': 0, 'normal': 0,
            'total_blocks': 0, 'passing': 0,
            'code_blocks': 0, 'prose_blocks': 0,
            'sd_dropped': 0, 'short_dropped': 0, 'toc_dropped': 0,
            'hf_lines': 0, 'url_count': 0, 'openstax_noise': 0,
            'non_ascii_pages': 0, 'exercise_pages': 0,
        },
        'samples': {'code': [], 'prose': [], 'problems': [], 'noise': []},
    }

    for zone_name, page_range in zones:
        z = {
            'pages_checked': 0, 'blank': 0, 'toc': 0, 'normal': 0,
            'blocks': 0, 'passing': 0, 'code': 0, 'prose': 0
        }
        for pg in page_range:
            z['pages_checked'] += 1
            raw = doc[pg].get_text('text') or ''
            if not raw.strip():
                z['blank'] += 1
                stats['global']['blank'] += 1
                continue

            toc_lines = sum(1 for ln in raw.splitlines() if TOC_RE.search(ln))
            nl = max(raw.count('\n'), 1)
            if toc_lines / nl > 0.25:
                z['toc'] += 1
                stats['global']['toc'] += 1
                continue

            z['normal'] += 1
            stats['global']['normal'] += 1

            hf = sum(1 for ln in raw.splitlines() if HEADER_FOOTER_RE.match(ln.strip()))
            stats['global']['hf_lines'] += hf
            stats['global']['url_count'] += len(URL_RE.findall(raw))
            noise_hits = len(OPENSTAX_NOISE_RE.findall(raw))
            stats['global']['openstax_noise'] += noise_hits
            if noise_hits > 0 and len(stats['samples']['noise']) < 4:
                m = OPENSTAX_NOISE_RE.search(raw)
                stats['samples']['noise'].append(f'[{label} p{pg+1}] {m.group()}: ...{raw[m.start():m.start()+80]}')
            if NON_ASCII_RE.search(raw):
                stats['global']['non_ascii_pages'] += 1
            if EXERCISE_RE.search(raw):
                stats['global']['exercise_pages'] += 1

            blocks = re.split(r'\n\s*\n', raw)
            for blk in blocks:
                lines_clean = []
                for ln in blk.splitlines():
                    ln2 = re.sub(r'\s+', ' ', ln).strip()
                    if not ln2 or len(ln2) < 3:
                        continue
                    if HEADER_FOOTER_RE.match(ln2):
                        continue
                    lines_clean.append(ln2)
                if not lines_clean:
                    continue
                text = ' '.join(lines_clean)
                z['blocks'] += 1
                stats['global']['total_blocks'] += 1

                sd = text.count(' ') / max(len(text), 1)
                if sd < 0.08:
                    stats['global']['sd_dropped'] += 1
                    if len(stats['samples']['problems']) < 6:
                        stats['samples']['problems'].append(
                            f'[{label} p{pg+1}] sd={sd:.3f}: {text[:80]}'
                        )
                    continue
                if len(text.split()) < 10:
                    stats['global']['short_dropped'] += 1
                    continue
                if sum(1 for l in lines_clean if TOC_RE.search(l)) >= 2:
                    stats['global']['toc_dropped'] += 1
                    continue

                z['passing'] += 1
                stats['global']['passing'] += 1
                if CODE_MARKER_RE.search(blk):
                    z['code'] += 1
                    stats['global']['code_blocks'] += 1
                    if len(stats['samples']['code']) < 3:
                        stats['samples']['code'].append(f'[{label} p{pg+1}]: {text[:120]}')
                else:
                    z['prose'] += 1
                    stats['global']['prose_blocks'] += 1
                    if len(stats['samples']['prose']) < 3:
                        stats['samples']['prose'].append(f'[{label} p{pg+1}]: {text[:120]}')

        stats['zones'][zone_name] = z
    doc.close()
    return stats


for label, path in PDFS.items():
    s = analyze_full_book(label, path)
    g = s['global']
    total_seen = g['total_blocks']
    passing = g['passing']
    pass_pct = passing / max(total_seen, 1) * 100
    code_pct = g['code_blocks'] / max(passing, 1) * 100
    prose_pct = g['prose_blocks'] / max(passing, 1) * 100

    print(f'\n========== {label} ==========')
    print(f'  Book total pages: {s["total_pages"]}')
    print(f'  (Sampled ~95-100 pages across 5 zones)')
    print()
    print(f'  PAGE QUALITY (sampled)')
    print(f'    Blank/image-only: {g["blank"]}')
    print(f'    TOC/index (auto-skipped): {g["toc"]}')
    print(f'    Normal content: {g["normal"]}')
    print()
    print(f'  BLOCK SURVIVAL THROUGH PIPELINE')
    print(f'    Total blocks found:          {total_seen}')
    print(f'    Blocks PASSING:              {passing}  ({pass_pct:.1f}%)')
    print(f'    Dropped space-density<0.08:  {g["sd_dropped"]}')
    print(f'    Dropped too-short (<10 wds): {g["short_dropped"]}')
    print(f'    Dropped TOC entries:         {g["toc_dropped"]}')
    print()
    print(f'  CONTENT MIX (passing blocks)')
    print(f'    Code/interactive blocks: {g["code_blocks"]}  ({code_pct:.1f}%)')
    print(f'    Prose blocks:            {g["prose_blocks"]}  ({prose_pct:.1f}%)')
    print()
    print(f'  NOISE & POLLUTION')
    print(f'    Header/footer bare-number lines: {g["hf_lines"]}')
    print(f'    URL instances:                   {g["url_count"]}')
    print(f'    OpenStax boilerplate hits:        {g["openstax_noise"]}')
    print(f'    Pages with non-ASCII garble:     {g["non_ascii_pages"]}')
    print(f'    Pages with exercise content:     {g["exercise_pages"]}')
    print()
    print(f'  ZONE BREAKDOWN (passing/blocks):')
    for zn, z in s['zones'].items():
        zpass_pct = z['passing'] / max(z['blocks'], 1) * 100
        print(f'    {zn:8s}: pages={z["pages_checked"]:3d} blank={z["blank"]:2d} '
              f'toc={z["toc"]:2d} blocks={z["blocks"]:4d} '
              f'passing={z["passing"]:4d}({zpass_pct:3.0f}%) '
              f'code={z["code"]:3d} prose={z["prose"]:3d}')

    if s['samples']['code']:
        print(f'\n  CODE SAMPLES:')
        for sm in s['samples']['code']:
            print(f'    {sm}')
    if s['samples']['prose']:
        print(f'\n  PROSE SAMPLES:')
        for sm in s['samples']['prose']:
            print(f'    {sm}')
    if s['samples']['problems']:
        print(f'\n  PROBLEM SAMPLES (dropped by pipeline):')
        for sm in s['samples']['problems']:
            print(f'    {sm}')
    if s['samples']['noise']:
        print(f'\n  OPENSTAX NOISE SAMPLES:')
        for sm in s['samples']['noise']:
            print(f'    {sm}')
