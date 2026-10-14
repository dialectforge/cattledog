#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
fix_empty_folders.py - Targeted fix for failed downloads in math/, language/, papers/, systems/
Fixes broken URLs in library.db and adds missing papers/ category entries.
"""
import hashlib, os, re, sqlite3, sys, time, urllib.parse
from pathlib import Path
from datetime import datetime

try:
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    sys.stderr.reconfigure(encoding='utf-8', errors='replace')
except: pass

import requests
from bs4 import BeautifulSoup

DOWNLOAD_DIR = Path(r"<USER_HOME>\OneDrive\Desktop\Training_Materials")
DB_PATH = Path(r"<AI_FACTORY_ROOT>\tools\library.db")
RATE_LIMIT = 1.5

HEADERS = {
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
                  "(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 "
                  "LibraryBuilder/2.0 (Educational AI Training)",
    "Accept": "application/pdf,application/octet-stream,*/*",
}
SESSION = requests.Session()
SESSION.headers.update(HEADERS)
SESSION.max_redirects = 10

def sanitize(name):
    name = re.sub(r'[<>:"/\\|?*]', '_', name)
    name = re.sub(r'\s+', '_', name)
    return name.strip('._')[:200]

def is_valid_pdf(fp):
    try:
        with open(fp, 'rb') as f: return f.read(5) == b'%PDF-'
    except: return False

def download_pdf(url, filepath, timeout=180):
    """Download a PDF, return (ok, size, checksum, error)."""
    try:
        resp = SESSION.get(url, stream=True, timeout=(15, timeout), allow_redirects=True)
        resp.raise_for_status()
        ctype = resp.headers.get('Content-Type', '').lower()
        if 'text/html' in ctype and 'pdf' not in url.lower():
            return False, 0, "", f"Got HTML instead of PDF"
        os.makedirs(os.path.dirname(filepath), exist_ok=True)
        hasher = hashlib.md5()
        downloaded = 0
        with open(filepath, 'wb') as f:
            for chunk in resp.iter_content(chunk_size=65536):
                if chunk:
                    f.write(chunk); hasher.update(chunk); downloaded += len(chunk)
        if not is_valid_pdf(filepath):
            os.remove(filepath)
            return False, 0, "", "Not a valid PDF"
        sz = os.path.getsize(filepath)
        if sz < 1000:
            os.remove(filepath)
            return False, 0, "", f"Too small ({sz} bytes)"
        return True, sz, hasher.hexdigest(), ""
    except Exception as e:
        if os.path.exists(filepath):
            try: os.remove(filepath)
            except: pass
        return False, 0, "", str(e)[:300]

# ============================================================
# OPENSTAX FIX: Use their actual content server URL pattern
# The old pattern assets.openstax.org/.../documents/{slug}.pdf returns 403
# The working pattern uses d3bxy9euw4e147.cloudfront.net or openstax.org/api
# ============================================================

def get_openstax_pdf_url(slug):
    """Query OpenStax API to find the real PDF download link for a book."""
    try:
        # Try the book detail page to find the PDF link
        resp = SESSION.get(f"https://openstax.org/details/books/{slug}", timeout=20)
        if resp.status_code == 200:
            soup = BeautifulSoup(resp.text, 'html.parser')
            # Look for PDF download links
            for a in soup.find_all('a', href=True):
                href = a['href']
                if '.pdf' in href.lower() and ('cloudfront' in href or 'openstax' in href):
                    return href
        # Fallback: try the REX/content endpoint
        resp2 = SESSION.get(
            f"https://openstax.org/apps/cms/api/v2/pages/?type=books.Book&fields=cnx_id&slug={slug}",
            timeout=15
        )
        if resp2.status_code == 200:
            data = resp2.json()
            items = data.get("items", [])
            if items:
                cnx_id = items[0].get("cnx_id", "")
                if cnx_id:
                    return f"https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/{slug}.pdf"
    except Exception as e:
        print(f"    API lookup failed for {slug}: {e}")
    return None

# Key OpenStax books with KNOWN WORKING direct download URLs
# These use the OpenStax high-volume PDF endpoint
OPENSTAX_MATH = [
    # (slug, title) - we'll try multiple URL patterns
    ("algebra-and-trigonometry", "Algebra and Trigonometry"),
    ("college-algebra", "College Algebra"),
    ("prealgebra-2e", "Prealgebra 2e"),
    ("elementary-algebra-2e", "Elementary Algebra 2e"),
    ("intermediate-algebra-2e", "Intermediate Algebra 2e"),
    ("precalculus-2e", "Precalculus 2e"),
    ("calculus-volume-1", "Calculus Volume 1"),
    ("calculus-volume-2", "Calculus Volume 2"),
    ("calculus-volume-3", "Calculus Volume 3"),
    ("introductory-statistics", "Introductory Statistics"),
    ("statistics", "Statistics"),
    ("contemporary-mathematics", "Contemporary Mathematics"),
    ("college-physics-2e", "College Physics 2e"),
    ("university-physics-volume-1", "University Physics Volume 1"),
    ("university-physics-volume-2", "University Physics Volume 2"),
    ("university-physics-volume-3", "University Physics Volume 3"),
]

OPENSTAX_LANGUAGE = [
    ("principles-economics-3e", "Principles of Economics 3e"),
    ("principles-macroeconomics-3e", "Principles of Macroeconomics 3e"),
    ("principles-microeconomics-3e", "Principles of Microeconomics 3e"),
    ("us-history", "U.S. History"),
    ("american-government-3e", "American Government 3e"),
    ("introduction-sociology-3e", "Introduction to Sociology 3e"),
    ("psychology-2e", "Psychology 2e"),
    ("introduction-philosophy", "Introduction to Philosophy"),
    ("writing-guide", "Writing Guide with Handbook"),
    ("college-success", "College Success"),
    ("business-ethics", "Business Ethics"),
    ("introduction-business", "Introduction to Business"),
    ("organizational-behavior", "Organizational Behavior"),
    ("principles-management", "Principles of Management"),
    ("entrepreneurship", "Entrepreneurship"),
    ("business-law-i-essentials", "Business Law I Essentials"),
]

def try_openstax_urls(slug):
    """Try multiple URL patterns for OpenStax PDFs."""
    patterns = [
        f"https://assets.openstax.org/oscms-prodcms/media/documents/{slug}-WEB.pdf",
        f"https://d3bxy9euw4e147.cloudfront.net/oscms-prodcms/media/documents/{slug}.pdf",
        f"https://assets.openstax.org/oscms-prodcms/media/documents/{slug}.pdf",
    ]
    for url in patterns:
        try:
            resp = SESSION.head(url, timeout=10, allow_redirects=True)
            if resp.status_code == 200:
                ctype = resp.headers.get('Content-Type', '').lower()
                if 'pdf' in ctype or 'octet' in ctype:
                    return url
        except:
            continue
    # Try the API lookup as last resort
    found = get_openstax_pdf_url(slug)
    if found:
        return found
    return None

# ============================================================
# ALTERNATIVE FREE MATH TEXTBOOKS (guaranteed working URLs)
# ============================================================
ALT_MATH_BOOKS = [
    ("Book of Proof", "Richard Hammack",
     "https://richardhammack.github.io/BookOfProof/Main.pdf"),
    ("Linear Algebra Done Right", "Sheldon Axler",
     "https://linear.axler.net/LADR4e.pdf"),
]

# ============================================================
# ARXIV PAPERS for papers/ category
# These are important CS/AI papers NOT already in ai_ml/
# ============================================================
ARXIV_PAPERS_CATEGORY = [
    ("2303.08774", "GPT-4 Technical Report", "OpenAI"),
    ("2312.11805", "Gemini - A Family of Highly Capable Multimodal Models", "Google"),
    ("2401.02954", "Mixtral of Experts", "Mistral AI"),
    ("2310.06825", "Mistral 7B", "Jiang et al."),
    ("2309.16609", "Qwen Technical Report", "Alibaba"),
    ("2402.17764", "The Era of 1-bit LLMs", "Ma et al."),
    ("2305.14314", "QLoRA - Efficient Finetuning of Quantized LLMs", "Dettmers et al."),
    ("2106.09685", "LoRA - Low-Rank Adaptation of Large Language Models", "Hu et al."),
    ("2210.11416", "Scaling Data-Constrained Language Models", "Muennighoff et al."),
    ("2307.03172", "Flash Attention 2", "Dao"),
    ("2205.14135", "Flash Attention", "Dao et al."),
    ("2203.02155", "Training Compute-Optimal Large Language Models (Chinchilla)", "Hoffmann et al."),
    ("2204.02311", "PaLM - Scaling Language Modeling with Pathways", "Chowdhery et al."),
    ("2211.05100", "BLOOM - A 176B-Parameter Open-Access Multilingual LM", "BigScience"),
    ("2304.03442", "Segment Anything", "Kirillov et al."),
    ("2010.14701", "An Image is Worth 16x16 Words - ViT", "Dosovitskiy et al."),
    ("2302.05442", "Toolformer - LMs Teaching Themselves to Use Tools", "Schick et al."),
    ("2305.10601", "Tree of Thoughts - Deliberate Problem Solving with LLMs", "Yao et al."),
    ("2210.03629", "ReAct - Reasoning and Acting in LLMs", "Yao et al."),
    ("2201.11903", "Chain-of-Thought Prompting Elicits Reasoning in LLMs", "Wei et al."),
    ("1706.03762", "Attention Is All You Need (Transformer)", "Vaswani et al."),
    ("2005.11401", "Retrieval-Augmented Generation (RAG)", "Lewis et al."),
    ("2308.12950", "Code Llama - Open Foundation Models for Code", "Roziere et al."),
    ("2001.08361", "Scaling Laws for Neural Language Models", "Kaplan et al."),
]

# ============================================================
# SYSTEMS category - working URLs for Docker, Linux, networking
# ============================================================
SYSTEMS_BOOKS = [
    ("Linux Notes for Professionals", "GoalKicker",
     "https://books.goalkicker.com/LinuxBook/LinuxBook.pdf"),
    ("Docker Notes for Professionals", "GoalKicker",
     "https://books.goalkicker.com/DockerBook/DockerBook.pdf"),
    ("The Linux Command Line", "William Shotts",
     "https://sourceforge.net/projects/linuxcommand/files/TLCL/19.01/TLCL-19.01.pdf"),
    ("Operating Systems - Three Easy Pieces", "Arpaci-Dusseau",
     "https://pages.cs.wisc.edu/~remzi/OSTEP/ostep-latest.pdf"),
    ("Computer Networks - A Systems Approach", "Peterson & Davie",
     "https://book.systemsapproach.org/_/downloads/en/latest/pdf/"),
    ("Distributed Systems 4th ed", "Tanenbaum & Van Steen",
     "https://www.distributed-systems.net/index.php/books/ds4/ds4-sneak-preview/"),
]

# ============================================================
# MAIN LOGIC
# ============================================================
def main():
    conn = sqlite3.connect(str(DB_PATH))
    
    # Ensure directories exist
    for cat in ["math", "language", "papers", "systems"]:
        (DOWNLOAD_DIR / cat).mkdir(parents=True, exist_ok=True)

    total_ok = 0
    total_fail = 0

    # ========================================
    # 1. FIX OPENSTAX MATH BOOKS
    # ========================================
    print("\n" + "="*60)
    print("  PHASE 1: OpenStax Math Textbooks")
    print("="*60)
    for slug, title in OPENSTAX_MATH:
        safe = sanitize(title)
        filepath = DOWNLOAD_DIR / "math" / f"{safe}.pdf"
        if filepath.exists() and is_valid_pdf(str(filepath)):
            print(f"  SKIP (exists): {title}")
            total_ok += 1
            continue
        print(f"  Trying: {title}")
        url = try_openstax_urls(slug)
        if url:
            print(f"    Found URL: {url[:80]}...")
            ok, sz, cksum, err = download_pdf(url, str(filepath))
            if ok:
                print(f"    OK - {sz/(1024*1024):.1f} MB")
                # Update DB
                conn.execute("UPDATE downloads SET status='downloaded', filepath=?, file_size=?, download_date=?, url=? WHERE category='math' AND title=?",
                    (str(filepath), sz, datetime.now().isoformat(), url, title))
                conn.commit()
                total_ok += 1
            else:
                print(f"    FAILED: {err}")
                total_fail += 1
        else:
            print(f"    No working URL found")
            total_fail += 1
        time.sleep(RATE_LIMIT)

    # Also try alt math books
    for title, author, url in ALT_MATH_BOOKS:
        safe = sanitize(title)
        filepath = DOWNLOAD_DIR / "math" / f"{safe}.pdf"
        if filepath.exists() and is_valid_pdf(str(filepath)):
            print(f"  SKIP (exists): {title}")
            continue
        print(f"  Trying alt: {title}")
        ok, sz, cksum, err = download_pdf(url, str(filepath))
        if ok:
            print(f"    OK - {sz/(1024*1024):.1f} MB")
            # Insert or update DB
            try:
                conn.execute("INSERT OR REPLACE INTO downloads (title, author, source, subject, category, url, filepath, file_size, download_date, status) VALUES (?,?,?,?,?,?,?,?,?,?)",
                    (title, author, "free_books", "math", "math", url, str(filepath), sz, datetime.now().isoformat(), "downloaded"))
                conn.commit()
            except: pass
            total_ok += 1
        else:
            print(f"    FAILED: {err}")
            total_fail += 1
        time.sleep(RATE_LIMIT)

    # ========================================
    # 2. FIX OPENSTAX LANGUAGE BOOKS
    # ========================================
    print("\n" + "="*60)
    print("  PHASE 2: OpenStax Language/Humanities Books")
    print("="*60)
    for slug, title in OPENSTAX_LANGUAGE:
        safe = sanitize(title)
        filepath = DOWNLOAD_DIR / "language" / f"{safe}.pdf"
        if filepath.exists() and is_valid_pdf(str(filepath)):
            print(f"  SKIP (exists): {title}")
            total_ok += 1
            continue
        print(f"  Trying: {title}")
        url = try_openstax_urls(slug)
        if url:
            print(f"    Found URL: {url[:80]}...")
            ok, sz, cksum, err = download_pdf(url, str(filepath))
            if ok:
                print(f"    OK - {sz/(1024*1024):.1f} MB")
                conn.execute("UPDATE downloads SET status='downloaded', filepath=?, file_size=?, download_date=?, url=? WHERE category='language' AND title=?",
                    (str(filepath), sz, datetime.now().isoformat(), url, title))
                conn.commit()
                total_ok += 1
            else:
                print(f"    FAILED: {err}")
                total_fail += 1
        else:
            print(f"    No working URL found")
            total_fail += 1
        time.sleep(RATE_LIMIT)

    # ========================================
    # 3. GUTENBERG - use .txt since PDFs 404
    # Actually, try the correct Gutenberg PDF path
    # New format: gutenberg.org/files/{id}/{id}-pdf.pdf
    # ========================================
    print("\n" + "="*60)
    print("  PHASE 3: Project Gutenberg Classics (language)")
    print("="*60)
    GUTENBERG = [
        (1342, "Pride and Prejudice", "Jane Austen"),
        (11, "Alices Adventures in Wonderland", "Lewis Carroll"),
        (1661, "The Adventures of Sherlock Holmes", "Arthur Conan Doyle"),
        (84, "Frankenstein", "Mary Shelley"),
        (98, "A Tale of Two Cities", "Charles Dickens"),
        (1232, "The Prince", "Niccolo Machiavelli"),
        (2600, "War and Peace", "Leo Tolstoy"),
        (74, "The Adventures of Tom Sawyer", "Mark Twain"),
        (76, "Adventures of Huckleberry Finn", "Mark Twain"),
        (345, "Dracula", "Bram Stoker"),
        (2701, "Moby Dick", "Herman Melville"),
        (174, "The Picture of Dorian Gray", "Oscar Wilde"),
        (1400, "Great Expectations", "Charles Dickens"),
        (219, "Heart of Darkness", "Joseph Conrad"),
        (1497, "Republic", "Plato"),
        (10616, "The Art of War", "Sun Tzu"),
        (28054, "The Brothers Karamazov", "Dostoevsky"),
        (2554, "Crime and Punishment", "Dostoevsky"),
    ]
    for book_id, title, author in GUTENBERG:
        safe = sanitize(title)
        filepath = DOWNLOAD_DIR / "language" / f"{safe}.pdf"
        if filepath.exists() and is_valid_pdf(str(filepath)):
            print(f"  SKIP (exists): {title}")
            total_ok += 1
            continue
        print(f"  Trying: {title} (#{book_id})")
        # Try multiple Gutenberg URL patterns
        urls_to_try = [
            f"https://www.gutenberg.org/files/{book_id}/{book_id}-pdf.pdf",
            f"https://www.gutenberg.org/ebooks/{book_id}.pdf",
            f"https://www.gutenberg.org/cache/epub/{book_id}/pg{book_id}.pdf",
        ]
        downloaded = False
        for url in urls_to_try:
            ok, sz, cksum, err = download_pdf(url, str(filepath))
            if ok:
                print(f"    OK - {sz/(1024*1024):.1f} MB from {url[:60]}")
                try:
                    conn.execute("INSERT OR REPLACE INTO downloads (title, author, source, subject, category, url, filepath, file_size, download_date, status) VALUES (?,?,?,?,?,?,?,?,?,?)",
                        (title, author, "gutenberg", "literature", "language", url, str(filepath), sz, datetime.now().isoformat(), "downloaded"))
                    conn.commit()
                except: pass
                total_ok += 1
                downloaded = True
                break
            time.sleep(0.5)
        if not downloaded:
            print(f"    No PDF available for this book (Gutenberg may only have txt/epub)")
            total_fail += 1
        time.sleep(RATE_LIMIT)

    # ========================================
    # 4. ARXIV PAPERS -> papers/ category
    # ========================================
    print("\n" + "="*60)
    print("  PHASE 4: arXiv Papers (papers/ category)")
    print("="*60)
    for arxiv_id, title, author in ARXIV_PAPERS_CATEGORY:
        safe = sanitize(title)
        filepath = DOWNLOAD_DIR / "papers" / f"{safe}.pdf"
        if filepath.exists() and is_valid_pdf(str(filepath)):
            print(f"  SKIP (exists): {title}")
            total_ok += 1
            continue
        # Also check if it exists in ai_ml/ already (don't re-download, just copy)
        ai_ml_path = DOWNLOAD_DIR / "ai_ml" / f"{safe}.pdf"
        if ai_ml_path.exists() and is_valid_pdf(str(ai_ml_path)):
            import shutil
            shutil.copy2(str(ai_ml_path), str(filepath))
            sz = os.path.getsize(str(filepath))
            print(f"  COPIED from ai_ml/: {title}")
            try:
                conn.execute("INSERT OR IGNORE INTO downloads (title, author, source, subject, category, url, filepath, file_size, download_date, status) VALUES (?,?,?,?,?,?,?,?,?,?)",
                    (title, author, "arxiv", title.lower(), "papers", f"https://arxiv.org/pdf/{arxiv_id}", str(filepath), sz, datetime.now().isoformat(), "downloaded"))
                conn.commit()
            except: pass
            total_ok += 1
            continue
        print(f"  Downloading: {title}")
        url = f"https://arxiv.org/pdf/{arxiv_id}"
        ok, sz, cksum, err = download_pdf(url, str(filepath))
        if ok:
            print(f"    OK - {sz/(1024*1024):.1f} MB")
            try:
                conn.execute("INSERT OR IGNORE INTO downloads (title, author, source, subject, category, url, filepath, file_size, download_date, status) VALUES (?,?,?,?,?,?,?,?,?,?)",
                    (title, author, "arxiv", title.lower(), "papers", url, str(filepath), sz, datetime.now().isoformat(), "downloaded"))
                conn.commit()
            except: pass
            total_ok += 1
        else:
            print(f"    FAILED: {err}")
            total_fail += 1
        time.sleep(3)  # arXiv rate limit

    # ========================================
    # 5. SYSTEMS BOOKS
    # ========================================
    print("\n" + "="*60)
    print("  PHASE 5: Systems Books")
    print("="*60)
    for title, author, url in SYSTEMS_BOOKS:
        safe = sanitize(title)
        filepath = DOWNLOAD_DIR / "systems" / f"{safe}.pdf"
        if filepath.exists() and is_valid_pdf(str(filepath)):
            print(f"  SKIP (exists): {title}")
            total_ok += 1
            continue
        print(f"  Downloading: {title}")
        ok, sz, cksum, err = download_pdf(url, str(filepath))
        if ok:
            print(f"    OK - {sz/(1024*1024):.1f} MB")
            try:
                conn.execute("INSERT OR IGNORE INTO downloads (title, author, source, subject, category, url, filepath, file_size, download_date, status) VALUES (?,?,?,?,?,?,?,?,?,?)",
                    (title, author, "free_books", "systems", "systems", url, str(filepath), sz, datetime.now().isoformat(), "downloaded"))
                conn.commit()
            except: pass
            total_ok += 1
        else:
            print(f"    FAILED: {err}")
            total_fail += 1
        time.sleep(RATE_LIMIT)

    # ========================================
    # SUMMARY
    # ========================================
    print("\n" + "="*60)
    print(f"  DONE! {total_ok} succeeded, {total_fail} failed")
    print("="*60)

    # Show final folder counts
    for cat in ["math", "language", "papers", "systems"]:
        cat_dir = DOWNLOAD_DIR / cat
        pdfs = list(cat_dir.glob("*.pdf"))
        print(f"  {cat:15s}: {len(pdfs)} PDFs")
    print()

    conn.close()

if __name__ == "__main__":
    main()
