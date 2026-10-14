#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""Fix math/ and language/ folders - OpenStax API + alternative sources."""
import hashlib, os, re, sqlite3, sys, time, json
from pathlib import Path
from datetime import datetime
try:
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
except: pass
import requests
from bs4 import BeautifulSoup

DOWNLOAD_DIR = Path(r"<USER_HOME>\OneDrive\Desktop\Training_Materials")
DB_PATH = Path(r"<AI_FACTORY_ROOT>\tools\library.db")
S = requests.Session()
S.headers.update({
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) LibraryBuilder/2.0",
    "Accept": "application/pdf,*/*",
})

def sanitize(n):
    n = re.sub(r'[<>:"/\\|?*]', '_', n)
    return re.sub(r'\s+', '_', n).strip('._')[:200]

def dl(url, fp, timeout=180):
    try:
        r = S.get(url, stream=True, timeout=(15,timeout), allow_redirects=True)
        r.raise_for_status()
        ct = r.headers.get('Content-Type','').lower()
        if 'text/html' in ct and '.pdf' not in url.lower():
            return False, 0, "Got HTML not PDF"
        os.makedirs(os.path.dirname(fp), exist_ok=True)
        with open(fp, 'wb') as f:
            for c in r.iter_content(65536):
                if c: f.write(c)
        with open(fp,'rb') as f:
            if f.read(5) != b'%PDF-':
                os.remove(fp); return False, 0, "Not valid PDF"
        sz = os.path.getsize(fp)
        if sz < 1000: os.remove(fp); return False, 0, "Too small"
        return True, sz, ""
    except Exception as e:
        if os.path.exists(fp):
            try: os.remove(fp)
            except: pass
        return False, 0, str(e)[:200]

def find_openstax_pdf(slug):
    """Try to find real PDF URL from OpenStax book page."""
    try:
        # Method 1: Check the details page for PDF download link
        r = S.get(f"https://openstax.org/details/books/{slug}",
                  timeout=20, allow_redirects=True)
        if r.status_code == 200:
            # Look for PDF links in page source
            text = r.text
            # OpenStax embeds PDF URLs in JSON data or direct links
            patterns = [
                r'https://[^"\']+\.pdf',
            ]
            for pat in patterns:
                matches = re.findall(pat, text)
                for m in matches:
                    if slug in m.lower() or 'openstax' in m.lower():
                        # Verify it's accessible
                        try:
                            hr = S.head(m, timeout=10, allow_redirects=True)
                            if hr.status_code == 200:
                                return m
                        except: continue
    except Exception as e:
        print(f"    API error: {e}")
    # Method 2: Try alternate CDN patterns
    alts = [
        f"https://assets.openstax.org/oscms-prodcms/media/documents/{slug}-WEB.pdf",
        f"https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/{slug}.pdf",
    ]
    for u in alts:
        try:
            hr = S.head(u, timeout=10, allow_redirects=True)
            if hr.status_code == 200: return u
        except: continue
    return None

MATH_SLUGS = [
    ("algebra-and-trigonometry", "Algebra and Trigonometry"),
    ("college-algebra", "College Algebra"),
    ("prealgebra-2e", "Prealgebra 2e"),
    ("precalculus-2e", "Precalculus 2e"),
    ("calculus-volume-1", "Calculus Volume 1"),
    ("calculus-volume-2", "Calculus Volume 2"),
    ("calculus-volume-3", "Calculus Volume 3"),
    ("introductory-statistics", "Introductory Statistics"),
    ("contemporary-mathematics", "Contemporary Mathematics"),
    ("college-physics-2e", "College Physics 2e"),
]

LANG_SLUGS = [
    ("us-history", "U.S. History"),
    ("american-government-3e", "American Government 3e"),
    ("introduction-sociology-3e", "Introduction to Sociology 3e"),
    ("psychology-2e", "Psychology 2e"),
    ("introduction-philosophy", "Introduction to Philosophy"),
    ("writing-guide", "Writing Guide with Handbook"),
    ("college-success", "College Success"),
    ("business-ethics", "Business Ethics"),
    ("introduction-business", "Introduction to Business"),
    ("entrepreneurship", "Entrepreneurship"),
]

# Alternative guaranteed-working math/language books
ALT_MATH = [
    ("Book of Proof", "https://richardhammack.github.io/BookOfProof/Main.pdf"),
    ("Linear Algebra Done Right", "https://linear.axler.net/LADR4e.pdf"),
    ("Calculus Made Easy", "https://calculusmadeeasy.org/calculusmadeeasy.pdf"),
    ("A Computational Introduction to Number Theory and Algebra",
     "https://shoup.net/ntb/ntb-v2.pdf"),
    ("Discrete Mathematics An Open Introduction",
     "https://discrete.openmathbooks.org/pdfs/dmoi-tablet.pdf"),
]

ALT_LANG = [
    ("Introduction to Logic - Smith", "https://builds.openlogicproject.org/courses/phil379/phil379-screen.pdf"),
    ("forall x - An Introduction to Formal Logic", "https://forallx.openlogicproject.org/forallxyyc.pdf"),
    ("Introduction to Linguistics", "https://essentialsoflinguistics.pressbooks.com/open/download?type=pdf"),
]

conn = sqlite3.connect(str(DB_PATH))
for cat in ["math","language"]:
    (DOWNLOAD_DIR / cat).mkdir(parents=True, exist_ok=True)
ok_ct = 0; fail_ct = 0

# Phase 1: Try OpenStax math
print("="*50)
print("  Phase 1: OpenStax Math")
print("="*50)
for slug, title in MATH_SLUGS:
    safe = sanitize(title)
    fp = DOWNLOAD_DIR / "math" / f"{safe}.pdf"
    if fp.exists():
        print(f"SKIP: {title}"); ok_ct += 1; continue
    print(f"Finding URL: {title} ...")
    url = find_openstax_pdf(slug)
    if url:
        print(f"  Trying: {url[:70]}...")
        good, sz, err = dl(url, str(fp))
        if good:
            print(f"  OK {sz/(1024*1024):.1f}MB"); ok_ct += 1
            try: conn.execute("UPDATE downloads SET status='downloaded',filepath=?,file_size=?,download_date=?,url=? WHERE category='math' AND url LIKE ?",
                (str(fp),sz,datetime.now().isoformat(),url,f"%{slug}%")); conn.commit()
            except: pass
        else:
            print(f"  FAIL: {err}"); fail_ct += 1
    else:
        print(f"  No working URL found"); fail_ct += 1
    time.sleep(2)

# Phase 2: Alt math books
print("\n  Alt math books...")
for title, url in ALT_MATH:
    safe = sanitize(title)
    fp = DOWNLOAD_DIR / "math" / f"{safe}.pdf"
    if fp.exists():
        print(f"SKIP: {title}"); ok_ct += 1; continue
    print(f"DL: {title} ...")
    good, sz, err = dl(url, str(fp))
    if good:
        print(f"  OK {sz/(1024*1024):.1f}MB"); ok_ct += 1
        try: conn.execute("INSERT OR IGNORE INTO downloads (title,author,source,subject,category,url,filepath,file_size,download_date,status) VALUES (?,?,?,?,?,?,?,?,?,?)",
            (title,"","free_books","math","math",url,str(fp),sz,datetime.now().isoformat(),"downloaded")); conn.commit()
        except: pass
    else:
        print(f"  FAIL: {err}"); fail_ct += 1
    time.sleep(2)

# Phase 3: OpenStax language
print("\n" + "="*50)
print("  Phase 3: OpenStax Language/Humanities")
print("="*50)
for slug, title in LANG_SLUGS:
    safe = sanitize(title)
    fp = DOWNLOAD_DIR / "language" / f"{safe}.pdf"
    if fp.exists():
        print(f"SKIP: {title}"); ok_ct += 1; continue
    print(f"Finding URL: {title} ...")
    url = find_openstax_pdf(slug)
    if url:
        print(f"  Trying: {url[:70]}...")
        good, sz, err = dl(url, str(fp))
        if good:
            print(f"  OK {sz/(1024*1024):.1f}MB"); ok_ct += 1
        else:
            print(f"  FAIL: {err}"); fail_ct += 1
    else:
        print(f"  No working URL found"); fail_ct += 1
    time.sleep(2)

# Phase 4: Alt language books
print("\n  Alt language books...")
for title, url in ALT_LANG:
    safe = sanitize(title)
    fp = DOWNLOAD_DIR / "language" / f"{safe}.pdf"
    if fp.exists():
        print(f"SKIP: {title}"); ok_ct += 1; continue
    print(f"DL: {title} ...")
    good, sz, err = dl(url, str(fp))
    if good:
        print(f"  OK {sz/(1024*1024):.1f}MB"); ok_ct += 1
        try: conn.execute("INSERT OR IGNORE INTO downloads (title,author,source,subject,category,url,filepath,file_size,download_date,status) VALUES (?,?,?,?,?,?,?,?,?,?)",
            (title,"","free_books","language","language",url,str(fp),sz,datetime.now().isoformat(),"downloaded")); conn.commit()
        except: pass
    else:
        print(f"  FAIL: {err}"); fail_ct += 1
    time.sleep(2)

print(f"\nDone: {ok_ct} ok, {fail_ct} failed")
for cat in ["math","language"]:
    pdfs = list((DOWNLOAD_DIR/cat).glob("*.pdf"))
    print(f"  {cat}/: {len(pdfs)} PDFs")
conn.close()
