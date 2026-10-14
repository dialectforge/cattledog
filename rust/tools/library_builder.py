#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
LibraryBuilder v2.0 - Free & Legal PDF Downloader for Training Lu AI
=====================================================================
Downloads free, legal PDFs from open educational sources to build
an AI training library via Ollama fact extraction.

Sources:
  - OpenStax (free textbooks)
  - arXiv (landmark AI/CS papers)
  - Project Gutenberg (classic literature)
  - GoalKicker (Notes for Professionals books)
  - Free Programming Books (Beej's, Think Python, Eloquent JS, etc.)
  - Free CS/AI/ML Textbooks (SICP, OSTEP, Deep Learning, etc.)
  - Free Math Books (Book of Proof, Calculus Made Easy, etc.)

Usage:
    python library_builder.py --fetch-all
    python library_builder.py --source openstax
    python library_builder.py --source goalkicker
    python library_builder.py --source programming
    python library_builder.py --subject "ai"
    python library_builder.py --search "transformer architecture"
    python library_builder.py --list
    python library_builder.py --status
    python library_builder.py --fix-failed
    python library_builder.py --download-pending
"""

import argparse
import hashlib
import json
import os
import re
import sqlite3
import sys
import time
import traceback
import urllib.parse
from datetime import datetime
from pathlib import Path

# ============================================================
# FIX #1: Unicode encoding on Windows cp1252
# ============================================================
try:
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    sys.stderr.reconfigure(encoding='utf-8', errors='replace')
except Exception:
    pass  # Python < 3.7 or non-standard stdout

# Third-party imports
try:
    import requests
    from bs4 import BeautifulSoup
except ImportError:
    print("ERROR: Missing dependencies. Run this command first:")
    print('  pip install requests beautifulsoup4 --break-system-packages')
    sys.exit(1)

# ============================================================
# CONFIGURATION
# ============================================================

DOWNLOAD_DIR = Path(r"<USER_HOME>\OneDrive\Desktop\Training_Materials")
DB_PATH = Path(r"<AI_FACTORY_ROOT>\tools\library.db")

CATEGORIES = [
    "math", "language", "programming", "cs_fundamentals",
    "ai_ml", "operating_systems", "hardware", "papers",
    "systems",
]

# Polite delay between requests (seconds)
RATE_LIMIT_DELAY = 1.5

HEADERS = {
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
                  "(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 "
                  "LibraryBuilder/2.0 (Educational AI Training)",
    "Accept": "application/pdf,application/octet-stream,*/*",
    "Accept-Language": "en-US,en;q=0.9",
}

# Shared requests session for connection pooling and redirect control
SESSION = None

def get_session():
    """Get or create a shared requests session."""
    global SESSION
    if SESSION is None:
        SESSION = requests.Session()
        SESSION.headers.update(HEADERS)
        SESSION.max_redirects = 10  # Prevent redirect loops (fixes MIT OCW issue)
    return SESSION

# ============================================================
# DATABASE
# ============================================================

def init_db():
    """Create the SQLite tracking database if it doesn't exist."""
    DB_PATH.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(DB_PATH))
    conn.execute("""
        CREATE TABLE IF NOT EXISTS downloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            author TEXT DEFAULT '',
            source TEXT NOT NULL,
            subject TEXT NOT NULL,
            category TEXT NOT NULL,
            url TEXT NOT NULL UNIQUE,
            filepath TEXT DEFAULT '',
            file_size INTEGER DEFAULT 0,
            download_date TEXT DEFAULT '',
            status TEXT DEFAULT 'pending',
            checksum TEXT DEFAULT '',
            error_msg TEXT DEFAULT '',
            retry_count INTEGER DEFAULT 0
        )
    """)
    # Add columns if upgrading from v1
    try:
        conn.execute("ALTER TABLE downloads ADD COLUMN error_msg TEXT DEFAULT ''")
    except sqlite3.OperationalError:
        pass
    try:
        conn.execute("ALTER TABLE downloads ADD COLUMN retry_count INTEGER DEFAULT 0")
    except sqlite3.OperationalError:
        pass
    conn.commit()
    return conn

def record_exists(conn, url):
    """Check if a URL has already been recorded."""
    row = conn.execute("SELECT status FROM downloads WHERE url = ?", (url,)).fetchone()
    return row

def add_record(conn, title, author, source, subject, category, url):
    """Add a new pending download record. Returns True if new."""
    existing = record_exists(conn, url)
    if existing:
        return False
    conn.execute(
        "INSERT INTO downloads (title, author, source, subject, category, url, status) "
        "VALUES (?,?,?,?,?,?,?)",
        (title, author, source, subject, category, url, "pending")
    )
    conn.commit()
    return True

def mark_downloaded(conn, url, filepath, file_size, checksum=""):
    """Mark a record as successfully downloaded."""
    conn.execute(
        "UPDATE downloads SET status='downloaded', filepath=?, file_size=?, "
        "download_date=?, checksum=? WHERE url=?",
        (str(filepath), file_size, datetime.now().isoformat(), checksum, url)
    )
    conn.commit()

def mark_failed(conn, url, error_msg=""):
    """Mark a record as failed with optional error message."""
    conn.execute(
        "UPDATE downloads SET status='failed', error_msg=?, "
        "retry_count=retry_count+1 WHERE url=?",
        (str(error_msg)[:500], url)
    )
    conn.commit()

def get_pending(conn, source=None, subject=None):
    """Get all pending downloads, optionally filtered."""
    query = "SELECT url, title, category, source, subject FROM downloads WHERE status='pending'"
    params = []
    if source:
        query += " AND source = ?"
        params.append(source)
    if subject:
        query += " AND subject LIKE ?"
        params.append(f"%{subject}%")
    return conn.execute(query, params).fetchall()

def get_status_summary(conn):
    """Get counts by status."""
    rows = conn.execute(
        "SELECT status, COUNT(*) FROM downloads GROUP BY status"
    ).fetchall()
    return dict(rows)

def get_all_records(conn):
    """Get all records for listing."""
    return conn.execute(
        "SELECT title, source, subject, category, status, filepath "
        "FROM downloads ORDER BY source, category"
    ).fetchall()

# ============================================================
# DOWNLOAD ENGINE
# ============================================================

def sanitize_filename(name):
    """Make a string safe for use as a filename."""
    name = re.sub(r'[<>:"/\\|?*]', '_', name)
    name = re.sub(r'\s+', '_', name)
    name = name.strip('._')
    return name[:200]

def is_valid_pdf(filepath):
    """Check if a file is a valid PDF by reading its header."""
    try:
        with open(filepath, 'rb') as f:
            header = f.read(5)
            return header == b'%PDF-'
    except Exception:
        return False

def download_pdf(url, filepath, timeout=180):
    """
    Download a PDF file with chunked progress.
    Returns (success, file_size, checksum, error_msg).
    """
    session = get_session()
    error_msg = ""
    try:
        resp = session.get(url, stream=True, timeout=(15, timeout), allow_redirects=True)
        resp.raise_for_status()

        # Check content type - allow PDF or octet-stream, reject pure HTML
        ctype = resp.headers.get('Content-Type', '').lower()
        if 'text/html' in ctype and 'pdf' not in url.lower():
            error_msg = f"Got HTML instead of PDF (Content-Type: {ctype})"
            print(f"    WARNING: {error_msg}")
            return False, 0, "", error_msg

        total = int(resp.headers.get('Content-Length', 0))
        downloaded = 0
        hasher = hashlib.md5()

        os.makedirs(os.path.dirname(filepath), exist_ok=True)
        with open(filepath, 'wb') as f:
            for chunk in resp.iter_content(chunk_size=65536):
                if chunk:
                    f.write(chunk)
                    hasher.update(chunk)
                    downloaded += len(chunk)
                    if total > 0:
                        pct = int(downloaded * 100 / total)
                        mb = downloaded / (1024 * 1024)
                        total_mb = total / (1024 * 1024)
                        print(f"\r    Progress: {pct}% ({mb:.1f}/{total_mb:.1f} MB)", end="", flush=True)
                    else:
                        mb = downloaded / (1024 * 1024)
                        print(f"\r    Downloaded: {mb:.1f} MB", end="", flush=True)
        print()  # newline after progress

        # Validate PDF header
        if not is_valid_pdf(filepath):
            # Check if it's actually an HTML error page
            try:
                with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
                    head = f.read(500)
                if '<html' in head.lower() or '<!doctype' in head.lower():
                    error_msg = "Server returned HTML error page instead of PDF"
                else:
                    error_msg = "File does not have valid PDF header"
            except Exception:
                error_msg = "File does not have valid PDF header"
            print(f"    WARNING: {error_msg}, removing")
            os.remove(filepath)
            return False, 0, "", error_msg

        file_size = os.path.getsize(filepath)
        if file_size < 1000:
            error_msg = f"File too small ({file_size} bytes), likely not a real PDF"
            print(f"    WARNING: {error_msg}")
            os.remove(filepath)
            return False, 0, "", error_msg

        return True, file_size, hasher.hexdigest(), ""

    except requests.exceptions.TooManyRedirects:
        error_msg = "Too many redirects (possible redirect loop)"
        print(f"    ERROR: {error_msg}")
    except requests.exceptions.Timeout:
        error_msg = "Download timed out"
        print(f"    ERROR: {error_msg}")
    except requests.exceptions.ConnectionError as e:
        error_msg = f"Connection error: {e}"
        print(f"    ERROR: {error_msg}")
    except requests.exceptions.RequestException as e:
        error_msg = str(e)
        print(f"    ERROR: {error_msg}")
    except Exception as e:
        error_msg = f"Unexpected error: {e}"
        print(f"    ERROR: {error_msg}")

    # Clean up partial download
    if os.path.exists(filepath):
        try:
            os.remove(filepath)
        except Exception:
            pass
    return False, 0, "", error_msg

def process_downloads(conn, source_filter=None, subject_filter=None):
    """Download all pending items."""
    pending = get_pending(conn, source=source_filter, subject=subject_filter)
    if not pending:
        print("No pending downloads found.")
        return

    total = len(pending)
    print(f"\n{'='*60}")
    print(f"  LibraryBuilder -- Downloading {total} file(s)")
    print(f"{'='*60}\n")

    success_count = 0
    fail_count = 0
    skip_count = 0

    for i, (url, title, category, source, subject) in enumerate(pending, 1):
        safe_title = sanitize_filename(title)
        filepath = DOWNLOAD_DIR / category / f"{safe_title}.pdf"

        # Skip if file already exists on disk and is valid
        if filepath.exists() and is_valid_pdf(filepath):
            print(f"[{i}/{total}] SKIP (exists): {title}")
            file_size = os.path.getsize(filepath)
            mark_downloaded(conn, url, str(filepath), file_size)
            skip_count += 1
            continue

        print(f"[{i}/{total}] Downloading: {title}")
        print(f"    Source: {source} | Category: {category}")
        print(f"    URL: {url[:100]}...")

        ok, fsize, checksum, err = download_pdf(url, str(filepath))
        if ok:
            mark_downloaded(conn, url, str(filepath), fsize, checksum)
            print(f"    OK -- {fsize / (1024*1024):.1f} MB saved\n")
            success_count += 1
        else:
            mark_failed(conn, url, err)
            print(f"    FAILED\n")
            fail_count += 1

        # Rate limiting
        time.sleep(RATE_LIMIT_DELAY)

    print(f"\n{'='*60}")
    print(f"  Done! {success_count} downloaded, {skip_count} skipped, {fail_count} failed")
    print(f"{'='*60}\n")


# ============================================================
# SOURCE: OPENSTAX
# ============================================================

OPENSTAX_BOOKS = [
    ("algebra-and-trigonometry", "Algebra and Trigonometry", "math"),
    ("college-algebra", "College Algebra", "math"),
    ("prealgebra-2e", "Prealgebra 2e", "math"),
    ("elementary-algebra-2e", "Elementary Algebra 2e", "math"),
    ("intermediate-algebra-2e", "Intermediate Algebra 2e", "math"),
    ("precalculus-2e", "Precalculus 2e", "math"),
    ("calculus-volume-1", "Calculus Volume 1", "math"),
    ("calculus-volume-2", "Calculus Volume 2", "math"),
    ("calculus-volume-3", "Calculus Volume 3", "math"),
    ("introductory-statistics", "Introductory Statistics", "math"),
    ("introductory-business-statistics", "Introductory Business Statistics", "math"),
    ("statistics", "Statistics", "math"),
    ("college-physics-2e", "College Physics 2e", "math"),
    ("university-physics-volume-1", "University Physics Volume 1", "math"),
    ("university-physics-volume-2", "University Physics Volume 2", "math"),
    ("university-physics-volume-3", "University Physics Volume 3", "math"),
    ("chemistry-2e", "Chemistry 2e", "math"),
    ("chemistry-atoms-first-2e", "Chemistry Atoms First 2e", "math"),
    ("biology-2e", "Biology 2e", "math"),
    ("concepts-biology", "Concepts of Biology", "math"),
    ("microbiology", "Microbiology", "math"),
    ("anatomy-and-physiology-2e", "Anatomy and Physiology 2e", "math"),
    ("astronomy-2e", "Astronomy 2e", "math"),
    ("principles-economics-3e", "Principles of Economics 3e", "language"),
    ("principles-macroeconomics-3e", "Principles of Macroeconomics 3e", "language"),
    ("principles-microeconomics-3e", "Principles of Microeconomics 3e", "language"),
    ("us-history", "U.S. History", "language"),
    ("american-government-3e", "American Government 3e", "language"),
    ("introduction-sociology-3e", "Introduction to Sociology 3e", "language"),
    ("psychology-2e", "Psychology 2e", "language"),
    ("introduction-philosophy", "Introduction to Philosophy", "language"),
    ("introduction-intellectual-property", "Introduction to Intellectual Property", "language"),
    ("introduction-political-science", "Introduction to Political Science", "language"),
    ("writing-guide", "Writing Guide with Handbook", "language"),
    ("contemporary-mathematics", "Contemporary Mathematics", "math"),
    ("college-success", "College Success", "language"),
    ("college-success-concise", "College Success Concise", "language"),
    ("business-ethics", "Business Ethics", "language"),
    ("introduction-business", "Introduction to Business", "language"),
    ("organizational-behavior", "Organizational Behavior", "language"),
    ("principles-management", "Principles of Management", "language"),
    ("entrepreneurship", "Entrepreneurship", "language"),
    ("business-law-i-essentials", "Business Law I Essentials", "language"),
    ("principles-accounting-volume-1-financial-accounting", "Principles of Accounting Vol 1", "math"),
    ("principles-accounting-volume-2-managerial-accounting", "Principles of Accounting Vol 2", "math"),
]

def fetch_openstax(conn):
    """Register all known OpenStax textbooks for download using real API PDF URLs."""
    print("\n[OpenStax] Discovering free textbooks...")
    count = 0
    session = get_session()

    # Use the OpenStax API with high_resolution_pdf_url field (the REAL download link)
    try:
        resp = session.get(
            "https://openstax.org/apps/cms/api/v2/pages/",
            params={
                "type": "books.Book",
                "fields": "title,book_state,high_resolution_pdf_url",
                "limit": 200
            },
            timeout=30
        )
        if resp.status_code == 200:
            data = resp.json()
            items = data.get("items", [])
            print(f"    Found {len(items)} books via API")
            for item in items:
                title = item.get("title", "")
                if not title:
                    continue
                # Use the real PDF URL from the API (not a guessed slug URL)
                pdf_url = item.get("high_resolution_pdf_url")
                if not pdf_url:
                    continue
                # Skip non-English books (Polish, Spanish, etc.)
                state = item.get("book_state", "")
                if any(lang in title.lower() for lang in ["física", "cálculo", "química",
                       "introducción", "estadística", "precálculo", "fizyka",
                       "psychologia", "makroekonomia", "mikroekonomia",
                       "marketing-podstawy", "zywienie"]):
                    continue
                category = categorize_openstax(title)
                if add_record(conn, title, "OpenStax", "openstax", title.lower(), category, pdf_url):
                    count += 1
    except Exception as e:
        print(f"    API call failed ({e}), using built-in book list only")

    # Always also register the known list as reliable fallback (with corrected URLs)
    for slug, title, category in OPENSTAX_BOOKS:
        # These use the old slug-based URLs; the API above should catch most books
        # Only add if not already registered via API
        pdf_url = f"https://assets.openstax.org/oscms-prodcms/media/documents/{slug}.pdf"
        if add_record(conn, title, "OpenStax", "openstax", title.lower(), category, pdf_url):
            count += 1

    print(f"    Registered {count} new books for download")
    return count

def categorize_openstax(title):
    """Guess a category from an OpenStax book title."""
    t = title.lower()
    math_kw = ["algebra", "calculus", "statistic", "math", "physics", "chemistry",
               "biology", "anatomy", "astronomy", "accounting", "precalculus",
               "trigonometry", "microbiology"]
    lang_kw = ["history", "government", "sociology", "psychology", "philosophy",
               "writing", "success", "ethics", "business", "management", "law",
               "political", "entrepreneurship", "economics"]
    for kw in math_kw:
        if kw in t:
            return "math"
    for kw in lang_kw:
        if kw in t:
            return "language"
    if "programming" in t or "computer" in t:
        return "programming"
    return "language"


# ============================================================
# SOURCE: ARXIV
# ============================================================

ARXIV_LANDMARK_PAPERS = [
    ("1706.03762", "Attention Is All You Need", "Vaswani et al.", "ai_ml"),
    ("1810.04805", "BERT - Pre-training of Deep Bidirectional Transformers", "Devlin et al.", "ai_ml"),
    ("2005.14165", "Language Models are Few-Shot Learners (GPT-3)", "Brown et al.", "ai_ml"),
    ("1512.03385", "Deep Residual Learning for Image Recognition (ResNet)", "He et al.", "ai_ml"),
    ("1406.2661", "Generative Adversarial Networks", "Goodfellow et al.", "ai_ml"),
    ("1412.6980", "Adam - A Method for Stochastic Optimization", "Kingma & Ba", "ai_ml"),
    ("1409.1556", "Very Deep Convolutional Networks (VGGNet)", "Simonyan & Zisserman", "ai_ml"),
    ("1409.0473", "Neural Machine Translation by Jointly Learning to Align", "Bahdanau et al.", "ai_ml"),
    ("1301.3781", "Efficient Estimation of Word Representations (Word2Vec)", "Mikolov et al.", "ai_ml"),
    ("1607.06450", "Layer Normalization", "Ba et al.", "ai_ml"),
    ("1502.03167", "Batch Normalization", "Ioffe & Szegedy", "ai_ml"),
    ("1609.02907", "Semi-Supervised Classification with Graph CNNs", "Kipf & Welling", "ai_ml"),
    ("1711.05101", "Neural Discrete Representation Learning (VQ-VAE)", "van den Oord et al.", "ai_ml"),
    ("2010.11929", "An Image is Worth 16x16 Words (ViT)", "Dosovitskiy et al.", "ai_ml"),
    ("2103.14030", "Swin Transformer", "Liu et al.", "ai_ml"),
    ("2203.15556", "Training language models to follow instructions (InstructGPT)", "Ouyang et al.", "ai_ml"),
    ("2302.13971", "LLaMA - Open and Efficient Foundation Language Models", "Touvron et al.", "ai_ml"),
    ("2307.09288", "Llama 2 - Open Foundation and Fine-Tuned Chat Models", "Touvron et al.", "ai_ml"),
    ("1312.6114", "Auto-Encoding Variational Bayes (VAE)", "Kingma & Welling", "ai_ml"),
    ("2006.11239", "Denoising Diffusion Probabilistic Models", "Ho et al.", "ai_ml"),
    ("2112.10752", "High-Resolution Image Synthesis with Latent Diffusion", "Rombach et al.", "ai_ml"),
    ("2001.08361", "Scaling Laws for Neural Language Models", "Kaplan et al.", "ai_ml"),
    ("1706.01427", "Proximal Policy Optimization (PPO)", "Schulman et al.", "ai_ml"),
    ("1509.06461", "Pointer Networks", "Vinyals et al.", "ai_ml"),
    ("1503.02531", "Distilling the Knowledge in a Neural Network", "Hinton et al.", "ai_ml"),
    ("2305.18290", "Direct Preference Optimization (DPO)", "Rafailov et al.", "ai_ml"),
]

ARXIV_SEARCH_TOPICS = [
    ("transformer architecture survey", "papers"),
    ("large language model survey", "papers"),
    ("neural network optimization", "papers"),
    ("deep learning fundamentals", "papers"),
    ("reinforcement learning survey", "papers"),
    ("natural language processing survey", "papers"),
    ("computer vision deep learning", "papers"),
    ("retrieval augmented generation", "papers"),
    ("efficient inference large language models", "papers"),
    ("operating system design", "operating_systems"),
    ("computer architecture survey", "hardware"),
]

def fetch_arxiv(conn):
    """Register all curated arXiv papers + search for additional ones."""
    print("\n[arXiv] Discovering landmark AI/CS papers...")
    count = 0

    # Register curated landmark papers
    for arxiv_id, title, author, category in ARXIV_LANDMARK_PAPERS:
        # arXiv PDF URL format
        pdf_url = f"https://arxiv.org/pdf/{arxiv_id}"
        if add_record(conn, title, author, "arxiv", title.lower(), category, pdf_url):
            count += 1
    print(f"    Registered {count} landmark papers")

    # Search for additional papers by topic
    search_count = 0
    for query, category in ARXIV_SEARCH_TOPICS:
        found = search_arxiv(conn, query, category, max_results=5)
        search_count += found
        time.sleep(3)  # arXiv rate limit: 3 seconds between API calls

    total = count + search_count
    print(f"    Total new papers registered: {total}")
    return total

def search_arxiv(conn, query, category="papers", max_results=10):
    """Search arXiv API and register results."""
    print(f"    Searching arXiv for: {query}")
    encoded = urllib.parse.quote(query)
    api_url = (f"http://export.arxiv.org/api/query?"
               f"search_query=all:{encoded}&start=0&max_results={max_results}"
               f"&sortBy=relevance")
    session = get_session()
    try:
        resp = session.get(api_url, timeout=30)
        resp.raise_for_status()
        soup = BeautifulSoup(resp.text, 'html.parser')
        entries = soup.find_all('entry')
        count = 0
        for entry in entries:
            title_tag = entry.find('title')
            if not title_tag:
                continue
            title = title_tag.text.strip().replace('\n', ' ')
            entry_id = entry.find('id').text.strip()
            # Extract arXiv ID from URL like http://arxiv.org/abs/1234.5678v1
            arxiv_id = entry_id.split('/abs/')[-1] if '/abs/' in entry_id else entry_id.split('/')[-1]
            # Remove version suffix for cleaner URL
            arxiv_id_clean = re.sub(r'v\d+$', '', arxiv_id)
            authors = [a.find('name').text for a in entry.find_all('author')]
            author_str = ", ".join(authors[:3])
            if len(authors) > 3:
                author_str += " et al."
            pdf_url = f"https://arxiv.org/pdf/{arxiv_id_clean}"
            if add_record(conn, title, author_str, "arxiv", query, category, pdf_url):
                count += 1
        print(f"      Found {count} new papers")
        return count
    except Exception as e:
        print(f"      ERROR searching arXiv: {e}")
        return 0


# ============================================================
# SOURCE: PROJECT GUTENBERG
# ============================================================

GUTENBERG_BOOKS = [
    (1342, "Pride and Prejudice", "Jane Austen"),
    (11, "Alices Adventures in Wonderland", "Lewis Carroll"),
    (1661, "The Adventures of Sherlock Holmes", "Arthur Conan Doyle"),
    (84, "Frankenstein", "Mary Shelley"),
    (98, "A Tale of Two Cities", "Charles Dickens"),
    (1232, "The Prince", "Niccolo Machiavelli"),
    (2600, "War and Peace", "Leo Tolstoy"),
    (74, "The Adventures of Tom Sawyer", "Mark Twain"),
    (76, "Adventures of Huckleberry Finn", "Mark Twain"),
    (1952, "The Yellow Wallpaper", "Charlotte Perkins Gilman"),
    (5200, "Metamorphosis", "Franz Kafka"),
    (345, "Dracula", "Bram Stoker"),
    (1080, "A Modest Proposal", "Jonathan Swift"),
    (2701, "Moby Dick", "Herman Melville"),
    (174, "The Picture of Dorian Gray", "Oscar Wilde"),
    (43, "The Strange Case of Dr Jekyll and Mr Hyde", "Robert Louis Stevenson"),
    (16328, "Beowulf", "Anonymous"),
    (4300, "Ulysses", "James Joyce"),
    (1400, "Great Expectations", "Charles Dickens"),
    (219, "Heart of Darkness", "Joseph Conrad"),
    (5740, "Tractatus Logico-Philosophicus", "Ludwig Wittgenstein"),
    (4280, "The Critique of Pure Reason", "Immanuel Kant"),
    (10616, "The Art of War", "Sun Tzu"),
    (3207, "Leviathan", "Thomas Hobbes"),
    (1497, "Republic", "Plato"),
    (8438, "The Communist Manifesto", "Karl Marx"),
    (1228, "On Liberty", "John Stuart Mill"),
    (3600, "Essays of Michel de Montaigne", "Michel de Montaigne"),
    (28054, "The Brothers Karamazov", "Fyodor Dostoevsky"),
    (2554, "Crime and Punishment", "Fyodor Dostoevsky"),
    (7370, "The Elements", "Euclid"),
    (33283, "Calculus Made Easy", "Silvanus Thompson"),
    (29526, "An Introduction to Mathematics", "Alfred North Whitehead"),
]

def fetch_gutenberg(conn):
    """Register Project Gutenberg books for download."""
    print("\n[Project Gutenberg] Registering classic literature...")
    count = 0
    for book_id, title, author in GUTENBERG_BOOKS:
        # Gutenberg's cache epub PDF is the most reliable format
        pdf_url = f"https://www.gutenberg.org/cache/epub/{book_id}/pg{book_id}.pdf"
        if add_record(conn, title, author, "gutenberg", "literature", "language", pdf_url):
            count += 1
    print(f"    Registered {count} classic texts")
    return count


# ============================================================
# SOURCE: MIT OCW (Disabled - redirect loop issues)
# ============================================================

def fetch_mit_ocw(conn):
    """MIT OCW has aggressive redirect loops. Skipping for now."""
    print("\n[MIT OCW] SKIPPED - redirect loop issues with their CDN.")
    print("    Use --source programming or --source cs_books for free CS textbooks instead.")
    return 0


# ============================================================
# SOURCE: GOALKICKER (Notes for Professionals)
# ============================================================

# Direct links to GoalKicker PDFs - these are compiled from StackOverflow docs
# All freely available at https://goalkicker.com/
GOALKICKER_BOOKS = [
    ("AndroidBook", "Android Notes for Professionals", "programming"),
    ("AlgorithmsBook", "Algorithms Notes for Professionals", "cs_fundamentals"),
    ("AngularJSBook", "AngularJS Notes for Professionals", "programming"),
    ("Angular2Book", "Angular 2+ Notes for Professionals", "programming"),
    ("BashBook", "Bash Notes for Professionals", "programming"),
    ("CBook", "C Notes for Professionals", "programming"),
    ("CppBook", "C++ Notes for Professionals", "programming"),
    ("CSharpBook", "C-Sharp Notes for Professionals", "programming"),
    ("CSS3Book", "CSS Notes for Professionals", "programming"),
    ("DotNETFrameworkBook", "DotNET Framework Notes for Professionals", "programming"),
    ("DockerBook", "Docker Notes for Professionals", "systems"),
    ("EntityFrameworkBook", "Entity Framework Notes for Professionals", "programming"),
    ("ExcelVBABook", "Excel VBA Notes for Professionals", "programming"),
    ("GitBook", "Git Notes for Professionals", "programming"),
    ("GoBook", "Go Notes for Professionals", "programming"),
    ("HaskellBook", "Haskell Notes for Professionals", "programming"),
    ("HTML5Book", "HTML5 Notes for Professionals", "programming"),
    ("HTML5CanvasBook", "HTML5 Canvas Notes for Professionals", "programming"),
    ("iOSBook", "iOS Notes for Professionals", "programming"),
    ("JavaBook", "Java Notes for Professionals", "programming"),
    ("JavaScriptBook", "JavaScript Notes for Professionals", "programming"),
    ("jQueryBook", "jQuery Notes for Professionals", "programming"),
    ("KnockoutJSBook", "KnockoutJS Notes for Professionals", "programming"),
    ("KotlinBook", "Kotlin Notes for Professionals", "programming"),
    ("LaTeXBook", "LaTeX Notes for Professionals", "programming"),
    ("LinuxBook", "Linux Notes for Professionals", "systems"),
    ("MATLABBook", "MATLAB Notes for Professionals", "programming"),
    ("MongoDBBook", "MongoDB Notes for Professionals", "programming"),
    ("MySQLBook", "MySQL Notes for Professionals", "programming"),
    ("NodeJSBook", "NodeJS Notes for Professionals", "programming"),
    ("ObjectiveCBook", "Objective-C Notes for Professionals", "programming"),
    ("OracleDatabaseBook", "Oracle Database Notes for Professionals", "programming"),
    ("PerlBook", "Perl Notes for Professionals", "programming"),
    ("PHPBook", "PHP Notes for Professionals", "programming"),
    ("PostgreSQLBook", "PostgreSQL Notes for Professionals", "programming"),
    ("PowerShellBook", "PowerShell Notes for Professionals", "programming"),
    ("PythonBook", "Python Notes for Professionals", "programming"),
    ("Python3Book", "Python 3 Notes for Professionals", "programming"),
    ("RBook", "R Notes for Professionals", "programming"),
    ("ReactJSBook", "ReactJS Notes for Professionals", "programming"),
    ("ReactNativeBook", "React Native Notes for Professionals", "programming"),
    ("RubyBook", "Ruby Notes for Professionals", "programming"),
    ("RubyOnRailsBook", "Ruby on Rails Notes for Professionals", "programming"),
    ("RustBook", "Rust Notes for Professionals", "programming"),
    ("SpringFrameworkBook", "Spring Framework Notes for Professionals", "programming"),
    ("SQLBook", "SQL Notes for Professionals", "programming"),
    ("SwiftBook", "Swift Notes for Professionals", "programming"),
    ("TypeScriptBook", "TypeScript Notes for Professionals", "programming"),
    ("VBABook", "VBA Notes for Professionals", "programming"),
    ("VisualBasicDotNETBook", "Visual Basic .NET Notes for Professionals", "programming"),
    ("WordPressBook", "WordPress Notes for Professionals", "programming"),
    ("XamarinFormsBook", "Xamarin.Forms Notes for Professionals", "programming"),
]

def fetch_goalkicker(conn):
    """Register GoalKicker 'Notes for Professionals' books."""
    print("\n[GoalKicker] Registering Notes for Professionals books...")
    count = 0

    for book_id, title, category in GOALKICKER_BOOKS:
        # Correct URL pattern: {BookId}/{TitleNoSpaces}NotesForProfessionals.pdf
        # e.g. LinuxBook/LinuxNotesForProfessionals.pdf
        notes_name = book_id.replace("Book", "NotesForProfessionals")
        pdf_url = f"https://goalkicker.com/{book_id}/{notes_name}.pdf"
        if add_record(conn, title, "GoalKicker", "goalkicker", title.lower(), category, pdf_url):
            count += 1

    # Also try to scrape the main page for any we missed
    try:
        session = get_session()
        resp = session.get("https://goalkicker.com/", timeout=20)
        if resp.status_code == 200:
            soup = BeautifulSoup(resp.text, 'html.parser')
            links = soup.find_all('a', href=True)
            for link in links:
                href = link.get('href', '')
                if href.endswith('Book/') or href.endswith('Book'):
                    book_slug = href.strip('/')
                    if book_slug:
                        text = link.get_text(strip=True)
                        if text:
                            title = f"{text} Notes for Professionals"
                        else:
                            title = f"{book_slug} Notes for Professionals"
                        pdf_url = f"https://goalkicker.com/{book_slug}/{book_slug}.pdf"
                        if add_record(conn, title, "GoalKicker", "goalkicker",
                                      title.lower(), "programming", pdf_url):
                            count += 1
    except Exception as e:
        print(f"    Could not scrape GoalKicker main page: {e}")

    print(f"    Registered {count} new GoalKicker books")
    return count


# ============================================================
# SOURCE: FREE PROGRAMMING BOOKS
# ============================================================

FREE_PROGRAMMING_BOOKS = [
    # Python
    ("Think Python 2e", "Allen B. Downey",
     "https://greenteapress.com/thinkpython2/thinkpython2.pdf",
     "python", "programming"),
    ("Automate the Boring Stuff with Python", "Al Sweigart",
     "https://automatetheboringstuff.com/2e/Automate_the_Boring_Stuff_with_Python.pdf",
     "python", "programming"),

    # JavaScript
    ("Eloquent JavaScript 4th Edition", "Marijn Haverbeke",
     "https://eloquentjavascript.net/Eloquent_JavaScript.pdf",
     "javascript", "programming"),

    # C / C++
    ("Beejs Guide to C Programming", "Brian Hall",
     "https://beej.us/guide/bgc/pdf/bgc_usl_c_1.pdf",
     "c programming", "programming"),
    ("Beejs Guide to Network Programming", "Brian Hall",
     "https://beej.us/guide/bgnet/pdf/bgnet_usl_c_1.pdf",
     "network programming", "programming"),

    # Java
    ("Think Java 2e", "Allen B. Downey & Chris Mayfield",
     "https://greenteapress.com/thinkjava7/thinkjava2.pdf",
     "java", "programming"),
    ("Introduction to Programming Using Java 9th ed", "David J. Eck",
     "https://math.hws.edu/eck/cs124/downloads/javanotes9-linked.pdf",
     "java", "programming"),

    # Go
    ("An Introduction to Programming in Go", "Caleb Doxsey",
     "https://www.golang-book.com/public/pdf/gobook.pdf",
     "go programming", "programming"),

    # Assembly
    ("Programming from the Ground Up", "Jonathan Bartlett",
     "https://savannah.nongnu.org/download/pgubook/ProgrammingGroundUp-1-0-booksize.pdf",
     "assembly", "programming"),
    ("PC Assembly Language", "Paul A. Carter",
     "https://pacman128.github.io/static/pcasm-book.pdf",
     "assembly", "programming"),
]

def fetch_programming_books(conn):
    """Register free programming language books."""
    print("\n[Programming Books] Registering free programming books...")
    count = 0

    for title, author, url, subject, category in FREE_PROGRAMMING_BOOKS:
        if add_record(conn, title, author, "free_books", subject, category, url):
            count += 1

    print(f"    Registered {count} new programming books")
    return count


# ============================================================
# SOURCE: FREE CS / AI / ML TEXTBOOKS
# ============================================================

FREE_CS_AI_BOOKS = [
    # General CS
    ("Structure and Interpretation of Computer Programs (SICP)", "Abelson & Sussman",
     "https://web.mit.edu/6.001/6.037/sicp.pdf",
     "computer science", "cs_fundamentals"),
    ("Algorithms by Jeff Erickson", "Jeff Erickson",
     "https://jeffe.cs.illinois.edu/teaching/algorithms/book/Algorithms-JeffE.pdf",
     "algorithms", "cs_fundamentals"),

    # Operating Systems
    ("Operating Systems - Three Easy Pieces", "Remzi & Andrea Arpaci-Dusseau",
     "https://pages.cs.wisc.edu/~remzi/OSTEP/ostep-latest.pdf",
     "operating systems", "operating_systems"),
    ("The Linux Command Line", "William Shotts",
     "https://sourceforge.net/projects/linuxcommand/files/TLCL/19.01/TLCL-19.01.pdf/download",
     "linux", "systems"),

    # AI / ML
    ("Reinforcement Learning - An Introduction 2nd ed", "Sutton & Barto",
     "http://incompleteideas.net/book/RLbook2020.pdf",
     "reinforcement learning", "ai_ml"),
    ("Mathematics for Machine Learning", "Deisenroth, Faisal & Ong",
     "https://mml-book.github.io/book/mml-book.pdf",
     "machine learning math", "ai_ml"),
    ("Neural Networks and Deep Learning", "Michael Nielsen",
     "http://static.latexstudio.net/article/2018/0912/neuralnetworksanddeeplearning.pdf",
     "deep learning", "ai_ml"),
    ("Speech and Language Processing 3rd ed draft", "Jurafsky & Martin",
     "https://web.stanford.edu/~jurafsky/slp3/ed3bookfeb3_2024.pdf",
     "nlp", "ai_ml"),
    ("Probabilistic Machine Learning - An Introduction", "Kevin Murphy",
     "https://probml.github.io/pml-book/book1.html",
     "machine learning", "ai_ml"),

    # Math
    ("Book of Proof", "Richard Hammack",
     "https://www.people.vcu.edu/~rhammack/BookOfProof/Main.pdf",
     "discrete math", "math"),
    ("Calculus Made Easy", "Silvanus P. Thompson",
     "https://www.gutenberg.org/cache/epub/33283/pg33283.pdf",
     "calculus", "math"),
]

def fetch_cs_ai_books(conn):
    """Register free CS, AI/ML, and math textbooks."""
    print("\n[CS/AI/ML Books] Registering free textbooks...")
    count = 0

    for title, author, url, subject, category in FREE_CS_AI_BOOKS:
        if add_record(conn, title, author, "free_books", subject, category, url):
            count += 1

    print(f"    Registered {count} new CS/AI/ML books")
    return count


# ============================================================
# SOURCE: OPEN TEXTBOOK LIBRARY (limited - skip deep scraping)
# ============================================================

def fetch_open_textbook_library(conn):
    """Open Textbook Library - simplified approach."""
    print("\n[Open Textbook Library] Skipping deep scrape (unreliable PDF links).")
    print("    Use GoalKicker and free book sources instead for better results.")
    return 0


# ============================================================
# SEARCH FUNCTION (cross-source)
# ============================================================

def search_and_register(conn, query):
    """Search for a specific topic across arXiv and register results."""
    print(f"\n[Search] Looking for: '{query}'")
    count = 0

    q = query.lower()
    if any(kw in q for kw in ["os", "kernel", "operating"]):
        cat = "operating_systems"
    elif any(kw in q for kw in ["hardware", "circuit", "digital", "embedded", "architecture"]):
        cat = "hardware"
    elif any(kw in q for kw in ["algorithm", "data structure", "networking", "graph"]):
        cat = "cs_fundamentals"
    elif any(kw in q for kw in ["python", "rust", "javascript", "programming", "code", "go ", "java"]):
        cat = "programming"
    elif any(kw in q for kw in ["grammar", "writing", "linguistic", "literature"]):
        cat = "language"
    elif any(kw in q for kw in ["math", "calculus", "algebra", "statistics", "linear"]):
        cat = "math"
    else:
        cat = "ai_ml"

    count += search_arxiv(conn, query, cat, max_results=15)
    return count


# ============================================================
# CLI COMMANDS
# ============================================================

def cmd_fetch_all(conn):
    """Discover and register items from ALL sources, then download."""
    print("\n" + "=" * 60)
    print("  LibraryBuilder v2.0 -- Full Fetch Mode")
    print("  Discovering free educational resources...")
    print("=" * 60)

    total_new = 0
    total_new += fetch_openstax(conn)
    total_new += fetch_arxiv(conn)
    total_new += fetch_gutenberg(conn)
    total_new += fetch_goalkicker(conn)
    total_new += fetch_programming_books(conn)
    total_new += fetch_cs_ai_books(conn)

    print(f"\n  Total new items registered: {total_new}")

    # Now download everything pending
    process_downloads(conn)

def cmd_fetch_source(conn, source):
    """Fetch from a specific source."""
    source = source.lower().strip()
    fetchers = {
        "openstax": fetch_openstax,
        "arxiv": fetch_arxiv,
        "gutenberg": fetch_gutenberg,
        "goalkicker": fetch_goalkicker,
        "programming": fetch_programming_books,
        "cs_books": fetch_cs_ai_books,
        "mit_ocw": fetch_mit_ocw,
        "mit": fetch_mit_ocw,
        "otl": fetch_open_textbook_library,
    }
    if source not in fetchers:
        print(f"ERROR: Unknown source '{source}'")
        print(f"Available sources: {', '.join(sorted(fetchers.keys()))}")
        return
    fetchers[source](conn)
    # Map source name to DB source field
    source_db_map = {
        "openstax": "openstax",
        "arxiv": "arxiv",
        "gutenberg": "gutenberg",
        "goalkicker": "goalkicker",
        "programming": "free_books",
        "cs_books": "free_books",
        "mit_ocw": "mit_ocw",
        "mit": "mit_ocw",
        "otl": "open_textbook_library",
    }
    process_downloads(conn, source_filter=source_db_map.get(source, source))

def cmd_fetch_subject(conn, subject):
    """Fetch by subject, discover + download matching items."""
    print(f"\nFetching resources for subject: {subject}")
    fetch_openstax(conn)
    fetch_arxiv(conn)
    fetch_gutenberg(conn)
    fetch_goalkicker(conn)
    fetch_programming_books(conn)
    fetch_cs_ai_books(conn)
    process_downloads(conn, subject_filter=subject)

def cmd_search(conn, query):
    """Search for specific topics and download matches."""
    search_and_register(conn, query)
    process_downloads(conn)

def cmd_list(conn):
    """List all tracked items."""
    records = get_all_records(conn)
    if not records:
        print("No items tracked yet. Run --fetch-all to discover resources.")
        return

    print(f"\n{'='*80}")
    print(f"  LibraryBuilder -- Library Contents ({len(records)} items)")
    print(f"{'='*80}\n")

    current_source = None
    for title, source, subject, category, status, filepath in records:
        if source != current_source:
            current_source = source
            print(f"\n  --- {source.upper()} ---")
        if status == "downloaded":
            icon = "[OK]"
        elif status == "failed":
            icon = "[XX]"
        else:
            icon = "[..]"
        print(f"  {icon} [{category}] {title[:60]}")

def cmd_status(conn):
    """Show download status summary."""
    summary = get_status_summary(conn)
    total = sum(summary.values())

    print(f"\n{'='*50}")
    print(f"  LibraryBuilder v2.0 -- Status Report")
    print(f"{'='*50}")
    print(f"  Total tracked:  {total}")
    print(f"  Downloaded:     {summary.get('downloaded', 0)}")
    print(f"  Pending:        {summary.get('pending', 0)}")
    print(f"  Failed:         {summary.get('failed', 0)}")
    print(f"{'='*50}")

    # Show disk usage
    total_size = 0
    for cat in CATEGORIES:
        cat_dir = DOWNLOAD_DIR / cat
        if cat_dir.exists():
            pdf_files = list(cat_dir.glob("*.pdf"))
            cat_size = sum(f.stat().st_size for f in pdf_files if f.is_file())
            count = len(pdf_files)
            if count > 0:
                print(f"  {cat:20s}: {count:3d} files, {cat_size/(1024*1024):.1f} MB")
            total_size += cat_size
    print(f"  {'TOTAL':20s}: {total_size/(1024*1024):.1f} MB")
    print(f"{'='*50}\n")

def cmd_fix_failed(conn):
    """Reset all failed downloads to pending and retry."""
    cursor = conn.execute("SELECT COUNT(*) FROM downloads WHERE status='failed'")
    fail_count = cursor.fetchone()[0]
    if fail_count == 0:
        print("No failed downloads to retry.")
        return
    print(f"Resetting {fail_count} failed downloads to pending...")
    conn.execute("UPDATE downloads SET status='pending' WHERE status='failed'")
    conn.commit()
    process_downloads(conn)

def cmd_reset_all(conn):
    """Reset entire database - start fresh."""
    conn.execute("DELETE FROM downloads")
    conn.commit()
    print("Database cleared. Run --fetch-all to rediscover all sources.")


# ============================================================
# MAIN
# ============================================================

def main():
    parser = argparse.ArgumentParser(
        description="LibraryBuilder v2.0 -- Free & Legal PDF Downloader for Training Lu AI",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python library_builder.py --fetch-all                 Download everything from all sources
  python library_builder.py --source openstax           Download from OpenStax only
  python library_builder.py --source arxiv              Download arXiv papers only
  python library_builder.py --source gutenberg          Download Project Gutenberg classics
  python library_builder.py --source goalkicker         Download GoalKicker books (50+ languages)
  python library_builder.py --source programming        Download free programming books
  python library_builder.py --source cs_books           Download free CS/AI/ML textbooks
  python library_builder.py --subject "ai"              Download AI-related items
  python library_builder.py --search "transformer"      Search & download specific topic
  python library_builder.py --list                      Show all tracked items
  python library_builder.py --status                    Show download progress
  python library_builder.py --fix-failed                Retry failed downloads
  python library_builder.py --download-pending          Download pending (no discovery)
  python library_builder.py --reset                     Clear database and start over

Sources: openstax, arxiv, gutenberg, goalkicker, programming, cs_books
Download folder: <USER_HOME>\\OneDrive\\Desktop\\Training_Materials\\
        """
    )

    parser.add_argument("--fetch-all", action="store_true",
                        help="Discover and download from ALL sources")
    parser.add_argument("--source", type=str,
                        help="Download from a specific source")
    parser.add_argument("--subject", type=str,
                        help="Download by subject keyword")
    parser.add_argument("--search", type=str,
                        help="Search for a topic and download matches")
    parser.add_argument("--list", action="store_true",
                        help="List all tracked items")
    parser.add_argument("--status", action="store_true",
                        help="Show download status summary")
    parser.add_argument("--fix-failed", action="store_true",
                        help="Retry all failed downloads")
    parser.add_argument("--retry", action="store_true",
                        help="Alias for --fix-failed")
    parser.add_argument("--download-pending", action="store_true",
                        help="Download all pending items (no discovery)")
    parser.add_argument("--reset", action="store_true",
                        help="Clear database and start fresh")

    args = parser.parse_args()

    # If no arguments, show help
    if len(sys.argv) == 1:
        parser.print_help()
        return

    # Ensure download directories exist
    for cat in CATEGORIES:
        (DOWNLOAD_DIR / cat).mkdir(parents=True, exist_ok=True)

    # Initialize database
    conn = init_db()

    try:
        if args.fetch_all:
            cmd_fetch_all(conn)
        elif args.source:
            cmd_fetch_source(conn, args.source)
        elif args.subject:
            cmd_fetch_subject(conn, args.subject)
        elif args.search:
            cmd_search(conn, args.search)
        elif args.list:
            cmd_list(conn)
        elif args.status:
            cmd_status(conn)
        elif args.fix_failed or args.retry:
            cmd_fix_failed(conn)
        elif args.download_pending:
            process_downloads(conn)
        elif args.reset:
            cmd_reset_all(conn)
        else:
            parser.print_help()
    finally:
        conn.close()

if __name__ == "__main__":
    main()
