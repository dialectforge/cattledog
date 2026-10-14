#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""Download arXiv papers to papers/ folder."""
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
S.headers.update({"User-Agent": "LibraryBuilder/2.0 (Educational)"})

def sanitize(n):
    n = re.sub(r'[<>:"/\\|?*]', '_', n)
    return re.sub(r'\s+', '_', n).strip('._')[:200]

def dl(url, fp, timeout=120):
    try:
        r = S.get(url, stream=True, timeout=(15,timeout), allow_redirects=True)
        r.raise_for_status()
        os.makedirs(os.path.dirname(fp), exist_ok=True)
        h = hashlib.md5()
        with open(fp, 'wb') as f:
            for c in r.iter_content(65536):
                if c: f.write(c); h.update(c)
        with open(fp,'rb') as f:
            if f.read(5) != b'%PDF-':
                os.remove(fp); return False, 0, f"Not PDF"
        sz = os.path.getsize(fp)
        if sz < 1000: os.remove(fp); return False, 0, "Too small"
        return True, sz, ""
    except Exception as e:
        if os.path.exists(fp):
            try: os.remove(fp)
            except: pass
        return False, 0, str(e)[:200]

PAPERS = [
    ("2303.08774", "GPT-4 Technical Report"),
    ("2312.11805", "Gemini - A Family of Highly Capable Multimodal Models"),
    ("2401.02954", "Mixtral of Experts"),
    ("2310.06825", "Mistral 7B"),
    ("2305.14314", "QLoRA Efficient Finetuning of Quantized LLMs"),
    ("2106.09685", "LoRA Low-Rank Adaptation of Large Language Models"),
    ("2307.03172", "Flash Attention 2"),
    ("2205.14135", "Flash Attention"),
    ("2203.02155", "Chinchilla - Training Compute-Optimal LLMs"),
    ("2204.02311", "PaLM Scaling Language Modeling with Pathways"),
    ("2304.03442", "Segment Anything"),
    ("2302.05442", "Toolformer LMs Teaching Themselves to Use Tools"),
    ("2305.10601", "Tree of Thoughts Deliberate Problem Solving with LLMs"),
    ("2210.03629", "ReAct Reasoning and Acting in LLMs"),
    ("2201.11903", "Chain-of-Thought Prompting Elicits Reasoning in LLMs"),
    ("2005.11401", "Retrieval-Augmented Generation RAG"),
    ("2308.12950", "Code Llama Open Foundation Models for Code"),
    ("2309.16609", "Qwen Technical Report"),
    ("2402.17764", "The Era of 1-bit LLMs"),
    ("2211.05100", "BLOOM 176B Parameter Open-Access Multilingual LM"),
]

(DOWNLOAD_DIR / "papers").mkdir(parents=True, exist_ok=True)
conn = sqlite3.connect(str(DB_PATH))
ok_ct = 0; fail_ct = 0

for arxiv_id, title in PAPERS:
    safe = sanitize(title)
    fp = DOWNLOAD_DIR / "papers" / f"{safe}.pdf"
    if fp.exists():
        print(f"SKIP: {title}"); ok_ct += 1; continue
    # Check if exists in ai_ml/
    ai = DOWNLOAD_DIR / "ai_ml" / f"{safe}.pdf"
    if ai.exists():
        import shutil; shutil.copy2(str(ai), str(fp))
        print(f"COPIED: {title}"); ok_ct += 1; continue
    url = f"https://arxiv.org/pdf/{arxiv_id}"
    print(f"DL: {title} ...")
    good, sz, err = dl(url, str(fp))
    if good:
        print(f"  OK {sz/(1024*1024):.1f}MB")
        try:
            conn.execute("INSERT OR IGNORE INTO downloads (title,author,source,subject,category,url,filepath,file_size,download_date,status) VALUES (?,?,?,?,?,?,?,?,?,?)",
                (title,"","arxiv",title.lower(),"papers",url,str(fp),sz,datetime.now().isoformat(),"downloaded"))
            conn.commit()
        except: pass
        ok_ct += 1
    else:
        print(f"  FAIL: {err}"); fail_ct += 1
    time.sleep(3)

print(f"\nPapers done: {ok_ct} ok, {fail_ct} failed")
pdfs = list((DOWNLOAD_DIR/"papers").glob("*.pdf"))
print(f"papers/ now has {len(pdfs)} PDFs")
conn.close()
