#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""Download systems books to systems/ folder."""
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
S.headers.update({"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) LibraryBuilder/2.0"})

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
                os.remove(fp); return False, 0, "Not PDF"
        sz = os.path.getsize(fp)
        if sz < 1000: os.remove(fp); return False, 0, "Too small"
        return True, sz, ""
    except Exception as e:
        if os.path.exists(fp):
            try: os.remove(fp)
            except: pass
        return False, 0, str(e)[:200]

# GoalKicker uses books.goalkicker.com (not goalkicker.com)
# OSTEP is a direct PDF, Linux Command Line from linuxcommand.org
BOOKS = [
    ("Linux Notes for Professionals", "https://books.goalkicker.com/LinuxBook/LinuxBook.pdf"),
    ("Docker Notes for Professionals", "https://books.goalkicker.com/DockerBook/DockerBook.pdf"),
    ("Operating Systems Three Easy Pieces", "https://pages.cs.wisc.edu/~remzi/OSTEP/ostep-latest.pdf"),
    ("The Linux Command Line", "https://linuxcommand.org/tlcl.php"),
    ("Bash Notes for Professionals", "https://books.goalkicker.com/BashBook/BashBook.pdf"),
    ("Git Notes for Professionals", "https://books.goalkicker.com/GitBook/GitBook.pdf"),
    ("PowerShell Notes for Professionals", "https://books.goalkicker.com/PowerShellBook/PowerShellBook.pdf"),
]

(DOWNLOAD_DIR / "systems").mkdir(parents=True, exist_ok=True)
conn = sqlite3.connect(str(DB_PATH))
ok_ct = 0; fail_ct = 0

for title, url in BOOKS:
    safe = sanitize(title)
    fp = DOWNLOAD_DIR / "systems" / f"{safe}.pdf"
    if fp.exists():
        print(f"SKIP: {title}"); ok_ct += 1; continue
    print(f"DL: {title} ...")
    good, sz, err = dl(url, str(fp))
    if good:
        print(f"  OK {sz/(1024*1024):.1f}MB")
        try:
            conn.execute("INSERT OR IGNORE INTO downloads (title,author,source,subject,category,url,filepath,file_size,download_date,status) VALUES (?,?,?,?,?,?,?,?,?,?)",
                (title,"","free_books","systems","systems",url,str(fp),sz,datetime.now().isoformat(),"downloaded"))
            conn.commit()
        except: pass
        ok_ct += 1
    else:
        print(f"  FAIL: {err}"); fail_ct += 1
    time.sleep(2)

print(f"\nSystems done: {ok_ct} ok, {fail_ct} failed")
pdfs = list((DOWNLOAD_DIR/"systems").glob("*.pdf"))
print(f"systems/ now has {len(pdfs)} PDFs")
conn.close()
