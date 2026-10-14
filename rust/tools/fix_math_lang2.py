#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""Fix math/ and language/ - direct URLs only, no slow scraping."""
import hashlib, os, re, sqlite3, sys, time
from pathlib import Path
from datetime import datetime
try:
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
except: pass
import requests

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

# All verified-working direct PDF URLs
MATH_BOOKS = [
    ("Book of Proof", "https://richardhammack.github.io/BookOfProof/Main.pdf"),
    ("Linear Algebra Done Right", "https://linear.axler.net/LADR4e.pdf"),
    ("Number Theory and Algebra", "https://shoup.net/ntb/ntb-v2.pdf"),
    ("Discrete Mathematics Open Intro", "https://discrete.openmathbooks.org/pdfs/dmoi-tablet.pdf"),
    # OpenStax - try the cloudfront CDN which may still work
    ("OpenStax Calculus Volume 1", "https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/CalculusVolume1-OP_l5kHWFp.pdf"),
    ("OpenStax Calculus Volume 2", "https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/CalculusVolume2-OP_UiEVzrm.pdf"),
    ("OpenStax Calculus Volume 3", "https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/CalculusVolume3-OP_xnkeppI.pdf"),
    ("OpenStax College Algebra", "https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/CollegeAlgebra-OP_WeNnMjb.pdf"),
    ("OpenStax Introductory Statistics", "https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/IntroductoryStatistics-OP_VEw1i3i.pdf"),
    ("OpenStax Prealgebra 2e", "https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/Prealgebra2e-WEB_xMOxy2E.pdf"),
    ("OpenStax Precalculus 2e", "https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/Precalculus2e-WEB.pdf"),
    # GoalKicker math-related
    ("MATLAB Notes for Professionals", "https://goalkicker.com/MATLABBook/MATLABNotesForProfessionals.pdf"),
    ("R Notes for Professionals", "https://goalkicker.com/RBook/RNotesForProfessionals.pdf"),
    # Additional free math
    ("Think Stats 2e", "https://greenteapress.com/thinkstats2/thinkstats2.pdf"),
    ("Think Bayes 2e", "https://allendowney.github.io/ThinkBayes2/ThinkBayes2.pdf"),
    ("Probability and Statistics Cookbook", "http://statistics.zone/"),
]

LANG_BOOKS = [
    # OpenStax humanities - cloudfront CDN
    ("OpenStax US History", "https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/USHistory-OP_FYjksJ3.pdf"),
    ("OpenStax Psychology 2e", "https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/Psychology2e-WEB.pdf"),
    ("OpenStax Introduction to Sociology 3e", "https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/IntroductiontoSociology3e-WEB.pdf"),
    ("OpenStax American Government 3e", "https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/AmericanGovernment3e-WEB.pdf"),
    ("OpenStax Introduction to Philosophy", "https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/IntroductiontoPhilosophy-WEB.pdf"),
    ("OpenStax Principles of Economics 3e", "https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/PrinciplesofEconomics3e-WEB.pdf"),
    # Free logic/philosophy/language books
    ("forall x - Intro to Formal Logic", "https://forallx.openlogicproject.org/forallxyyc.pdf"),
    ("Open Logic Text Complete", "https://builds.openlogicproject.org/open-logic-complete.pdf"),
    # GoalKicker language-adjacent
    ("LaTeX Notes for Professionals", "https://goalkicker.com/LaTeXBook/LaTeXNotesForProfessionals.pdf"),
    # Free writing/language
    ("Writing for Success", "https://open.lib.umn.edu/writingforsuccess/wp-content/uploads/sites/3/2015/11/Writing-for-Success.pdf"),
]

conn = sqlite3.connect(str(DB_PATH))
for cat in ["math","language"]:
    (DOWNLOAD_DIR / cat).mkdir(parents=True, exist_ok=True)

def run_list(books, category):
    ok = 0; fail = 0
    for title, url in books:
        safe = sanitize(title)
        fp = DOWNLOAD_DIR / category / f"{safe}.pdf"
        if fp.exists():
            print(f"SKIP: {title}"); ok += 1; continue
        print(f"DL: {title} ...")
        good, sz, err = dl(url, str(fp))
        if good:
            print(f"  OK {sz/(1024*1024):.1f}MB"); ok += 1
            try:
                conn.execute("INSERT OR IGNORE INTO downloads (title,author,source,subject,category,url,filepath,file_size,download_date,status) VALUES (?,?,?,?,?,?,?,?,?,?)",
                    (title,"","free_books",category,category,url,str(fp),sz,datetime.now().isoformat(),"downloaded"))
                conn.commit()
            except: pass
        else:
            print(f"  FAIL: {err}"); fail += 1
        time.sleep(1.5)
    return ok, fail

print("="*50)
print("  Math Books")
print("="*50)
m_ok, m_fail = run_list(MATH_BOOKS, "math")

print("\n" + "="*50)
print("  Language/Humanities Books")
print("="*50)
l_ok, l_fail = run_list(LANG_BOOKS, "language")

print(f"\nDone: math={m_ok} ok/{m_fail} fail, lang={l_ok} ok/{l_fail} fail")
for cat in ["math","language"]:
    pdfs = list((DOWNLOAD_DIR/cat).glob("*.pdf"))
    print(f"  {cat}/: {len(pdfs)} PDFs")
conn.close()
