#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""Final language/ fix - smaller books first, shorter timeout."""
import os, re, sqlite3, sys, time
from pathlib import Path
from datetime import datetime
try:
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
except: pass
import requests

DD = Path(r"<USER_HOME>\OneDrive\Desktop\Training_Materials\language")
DB = Path(r"<AI_FACTORY_ROOT>\tools\library.db")
S = requests.Session()
S.headers.update({"User-Agent": "Mozilla/5.0 LibraryBuilder/2.0",
                   "Accept": "application/pdf,*/*"})
DD.mkdir(parents=True, exist_ok=True)

def san(n):
    n = re.sub(r'[<>:"/\\|?*]', '_', n)
    return re.sub(r'\s+', '_', n).strip('._')[:200]

def dl(url, fp):
    try:
        r = S.get(url, stream=True, timeout=(10,120),
                  allow_redirects=True)
        r.raise_for_status()
        os.makedirs(os.path.dirname(fp), exist_ok=True)
        with open(fp,'wb') as f:
            for c in r.iter_content(65536):
                if c: f.write(c)
        with open(fp,'rb') as f:
            if f.read(5)!=b'%PDF-':
                os.remove(fp); return False,"Not PDF"
        sz=os.path.getsize(fp)
        if sz<1000: os.remove(fp); return False,"Small"
        return True,""
    except Exception as e:
        if os.path.exists(fp):
            try: os.remove(fp)
            except: pass
        return False,str(e)[:150]

# Sorted smallest to largest - get quick wins first
BOOKS = [
    ("Business Law I Essentials", "https://assets.openstax.org/oscms-prodcms/media/documents/Business_Law_I_Essentials_-_WEB.pdf"),
    ("Business Ethics", "https://assets.openstax.org/oscms-prodcms/media/documents/BusinessEthics-OP.pdf"),
    ("College Success", "https://assets.openstax.org/oscms-prodcms/media/documents/College_Success_-_WEB_zQGCJTr.pdf"),
    ("Entrepreneurship", "https://assets.openstax.org/oscms-prodcms/media/documents/Entrepreneurship-WEB_qNSArxT.pdf"),
    ("Writing Guide with Handbook", "https://assets.openstax.org/oscms-prodcms/media/documents/WritingGuide-WEB.pdf"),
    ("Introduction to Philosophy", "https://assets.openstax.org/oscms-prodcms/media/documents/Introduction_to_Philosophy-WEB_cszrKYp.pdf"),
    ("Psychology 2e", "https://assets.openstax.org/oscms-prodcms/media/documents/Psychology2e_WEB.pdf"),
    ("Principles of Management", "https://assets.openstax.org/oscms-prodcms/media/documents/PrinciplesofManagement-OP_mGBMvoU.pdf"),
    ("Organizational Behavior", "https://assets.openstax.org/oscms-prodcms/media/documents/OrganizationalBehavior-OP_TtwWIeQ.pdf"),
    ("Introduction to Business", "https://assets.openstax.org/oscms-prodcms/media/documents/IntroductionToBusiness-OP_8D04gAa.pdf"),
]

conn = sqlite3.connect(str(DB))
ok=0; fail=0
for title, url in BOOKS:
    fp = DD / f"{san(title)}.pdf"
    if fp.exists():
        print(f"SKIP: {title}"); ok+=1; continue
    print(f"DL: {title}...", flush=True)
    good, err = dl(url, str(fp))
    if good:
        sz=os.path.getsize(str(fp))
        print(f"  OK {sz/(1024*1024):.1f}MB")
        ok+=1
        try:
            conn.execute(
                "INSERT OR IGNORE INTO downloads "
                "(title,author,source,subject,category,url,"
                "filepath,file_size,download_date,status) "
                "VALUES (?,?,?,?,?,?,?,?,?,?)",
                (title,"OpenStax","openstax","language",
                 "language",url,str(fp),sz,
                 datetime.now().isoformat(),"downloaded"))
            conn.commit()
        except: pass
    else:
        print(f"  FAIL: {err}"); fail+=1
    time.sleep(1)

print(f"\n{ok} ok, {fail} fail")
pdfs=list(DD.glob("*.pdf"))
print(f"language/ now has {len(pdfs)} PDFs")
conn.close()
