#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""Fix math/ and language/ - REAL OpenStax API URLs that actually work."""
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

def dl(url, fp, timeout=300):
    try:
        r = S.get(url, stream=True, timeout=(15,timeout), allow_redirects=True)
        r.raise_for_status()
        os.makedirs(os.path.dirname(fp), exist_ok=True)
        with open(fp, 'wb') as f:
            tot = 0
            for c in r.iter_content(65536):
                if c: f.write(c); tot += len(c)
                if tot % (5*1024*1024) < 65536:
                    print(f"\r    {tot/(1024*1024):.0f}MB...", end="", flush=True)
        print()
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

# REAL URLs from OpenStax API (high_resolution_pdf_url field)
MATH = [
    ("Calculus Volume 1", "https://assets.openstax.org/oscms-prodcms/media/documents/Calculus_Volume_1_-_WEB_l4sAIKd.pdf"),
    ("Calculus Volume 2", "https://assets.openstax.org/oscms-prodcms/media/documents/Calculus_Volume_2_-_WEB.pdf"),
    ("Calculus Volume 3", "https://assets.openstax.org/oscms-prodcms/media/documents/Calculus_Volume_3_-_WEB.pdf"),
    ("College Algebra 2e", "https://assets.openstax.org/oscms-prodcms/media/documents/College-Algebra-2e-WEB.pdf"),
    ("Prealgebra 2e", "https://assets.openstax.org/oscms-prodcms/media/documents/Prealgebra2e-WEB_0qbw93r.pdf"),
    ("Precalculus 2e", "https://assets.openstax.org/oscms-prodcms/media/documents/Precalculus_2e-WEB.pdf"),
    ("Introductory Statistics", "https://assets.openstax.org/oscms-prodcms/media/documents/IntroductoryStatistics-OP_i6tAI7e.pdf"),
    ("Statistics", "https://assets.openstax.org/oscms-prodcms/media/documents/Statistics-WEB.pdf"),
    ("Contemporary Mathematics", "https://assets.openstax.org/oscms-prodcms/media/documents/ContemporaryMathematics-WEB.pdf"),
    ("College Physics 2e", "https://assets.openstax.org/oscms-prodcms/media/documents/College_Physics_2e-WEB_7Zesafu.pdf"),
    ("University Physics Vol 1", "https://assets.openstax.org/oscms-prodcms/media/documents/University_Physics_Volume_1_-_WEB.pdf"),
    ("University Physics Vol 2", "https://assets.openstax.org/oscms-prodcms/media/documents/University_Physics_Volume_2_-_WEB.pdf"),
    ("University Physics Vol 3", "https://assets.openstax.org/oscms-prodcms/media/documents/UniversityPhysicsVolume3-WEB.pdf"),
    ("Algebra and Trigonometry 2e", "https://assets.openstax.org/oscms-prodcms/media/documents/Algebra-and-Trigonometry-2e-WEB.pdf"),
    ("Elementary Algebra 2e", "https://assets.openstax.org/oscms-prodcms/media/documents/ElementaryAlgebra2e-WEB_3zxfu3Z.pdf"),
    ("Intermediate Algebra 2e", "https://assets.openstax.org/oscms-prodcms/media/documents/IntermediateAlgebra2e-WEB_RlpFLLx.pdf"),
    # Alt math
    ("Think Stats 2e", "https://greenteapress.com/thinkstats2/thinkstats2.pdf"),
    ("Think Bayes 2e", "https://allendowney.github.io/ThinkBayes2/ThinkBayes2.pdf"),
]

LANG = [
    ("US History", "https://assets.openstax.org/oscms-prodcms/media/documents/US_History_-_WEB.pdf"),
    ("American Government 3e", "https://assets.openstax.org/oscms-prodcms/media/documents/AmericanGovernment3e-WEB.pdf"),
    ("Introduction to Sociology 3e", "https://assets.openstax.org/oscms-prodcms/media/documents/IntroductiontoSociology3e-WEB_9QTqRGQ.pdf"),
    ("Psychology 2e", "https://assets.openstax.org/oscms-prodcms/media/documents/Psychology2e_WEB.pdf"),
    ("Introduction to Philosophy", "https://assets.openstax.org/oscms-prodcms/media/documents/Introduction_to_Philosophy-WEB_cszrKYp.pdf"),
    ("Writing Guide with Handbook", "https://assets.openstax.org/oscms-prodcms/media/documents/WritingGuide-WEB.pdf"),
    ("College Success", "https://assets.openstax.org/oscms-prodcms/media/documents/College_Success_-_WEB_zQGCJTr.pdf"),
    ("Business Ethics", "https://assets.openstax.org/oscms-prodcms/media/documents/BusinessEthics-OP.pdf"),
    ("Introduction to Business", "https://assets.openstax.org/oscms-prodcms/media/documents/IntroductionToBusiness-OP_8D04gAa.pdf"),
    ("Entrepreneurship", "https://assets.openstax.org/oscms-prodcms/media/documents/Entrepreneurship-WEB_qNSArxT.pdf"),
    ("Principles of Economics 3e", "https://assets.openstax.org/oscms-prodcms/media/documents/Economics3e-WEB_AObkTqf.pdf"),
    ("Principles of Management", "https://assets.openstax.org/oscms-prodcms/media/documents/PrinciplesofManagement-OP_mGBMvoU.pdf"),
    ("Organizational Behavior", "https://assets.openstax.org/oscms-prodcms/media/documents/OrganizationalBehavior-OP_TtwWIeQ.pdf"),
    ("Introduction to Political Science", "https://assets.openstax.org/oscms-prodcms/media/documents/Introduction_to_Political_Science_-_WEB.pdf"),
    ("Business Law I Essentials", "https://assets.openstax.org/oscms-prodcms/media/documents/Business_Law_I_Essentials_-_WEB.pdf"),
    ("World History Vol 1", "https://assets.openstax.org/oscms-prodcms/media/documents/World_History_Volume_1-WEB.pdf"),
    ("World History Vol 2", "https://assets.openstax.org/oscms-prodcms/media/documents/World_History_Volume_2-WEB_LdwoslB.pdf"),
    # Alt language
    ("forall x - Intro to Formal Logic", "https://forallx.openlogicproject.org/forallxyyc.pdf"),
    ("Open Logic Text Complete", "https://builds.openlogicproject.org/open-logic-complete.pdf"),
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
                conn.execute(
                    "INSERT OR IGNORE INTO downloads "
                    "(title,author,source,subject,category,url,"
                    "filepath,file_size,download_date,status) "
                    "VALUES (?,?,?,?,?,?,?,?,?,?)",
                    (title,"OpenStax","openstax",category,
                     category,url,str(fp),sz,
                     datetime.now().isoformat(),"downloaded"))
                conn.commit()
            except: pass
        else:
            print(f"  FAIL: {err}"); fail += 1
        time.sleep(1.5)
    return ok, fail

print("="*50)
print("  MATH (real OpenStax URLs from API)")
print("="*50)
m_ok, m_fail = run_list(MATH, "math")

print("\n" + "="*50)
print("  LANGUAGE (real OpenStax URLs from API)")
print("="*50)
l_ok, l_fail = run_list(LANG, "language")

print(f"\nDone: math={m_ok}ok/{m_fail}fail, lang={l_ok}ok/{l_fail}fail")
for cat in ["math","language"]:
    pdfs = list((DOWNLOAD_DIR/cat).glob("*.pdf"))
    print(f"  {cat}/: {len(pdfs)} PDFs")
conn.close()
