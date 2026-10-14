#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
PDF Knowledge Extraction Tool for Lu's Training Curriculum
Extracts atomic facts from PDFs using Ollama and converts to Lu's JSON format.
"""

import os
import sys
import json
import argparse
import time
import hashlib
import urllib.request
import urllib.error
from datetime import datetime
from pathlib import Path
from collections import defaultdict
import re

# Try to import pdfplumber, fall back to PyPDF2
try:
    import pdfplumber
    PDF_LIBRARY = "pdfplumber"
except ImportError:
    try:
        import PyPDF2
        PDF_LIBRARY = "PyPDF2"
    except ImportError:
        print("ERROR: Neither pdfplumber nor PyPDF2 found. Install one:")
        print("  pip install pdfplumber  # Recommended")
        print("  pip install PyPDF2      # Fallback")
        sys.exit(1)

# Ollama configuration (same as other curriculum scripts)
OLLAMA_URL = "http://localhost:11434/api/generate"
DEFAULT_MODEL = "llama3.1:8b"
REQUEST_TIMEOUT = 300

SYSTEM_PROMPT = """You are a knowledge extraction engine. You output ONLY valid JSON.
No markdown, no explanation, no code blocks, no preamble.
Output exactly this structure:
{"facts": ["fact 1", "fact 2", ...]}
Each fact must be a single, specific, standalone statement.
No vague generalities. No filler like 'this is important because'.
Just clear, concrete, atomic knowledge."""

# PDF categorization by tier
PDF_TIERS = {
    1: {
        "name": "ai_ml",
        "label": "AI & Machine Learning (PDFs)",
        "keywords": [
            "neural", "deep_learning", "transformer", "attention", "gpt", "bert", "llm",
            "language_model", "reinforcement_learning", "diffusion", "gan", "vae", "lora",
            "qlora", "fine_tun", "training", "machine_learning", "nlp", "computer_vision",
            "multimodal", "prompt", "embedding", "vector", "llama", "mistral", "mixtral",
            "bloom", "qwen", "segment_anything", "toolformer", "rag", "retrieval",
            "optimization", "dpo", "ppo", "resnet", "batch_normal", "layer_normal",
            "word2vec", "convnet", "pruning", "image_recognition", "image_synthesis",
            "classification", "data_science", "deep_residual", "vit", "deepcfl",
            "semi-supervised", "think_stats"
        ]
    },
    2: {
        "name": "cs_programming",
        "label": "CS & Programming (PDFs)",
        "keywords": [
            "algorithm", "data_structure", "programming", "python", "java", "javascript",
            "c_programming", "computer_science", "information_systems", "sicp", "assembly",
            "code", "golang", "rust", "cpp"
        ]
    },
    3: {
        "name": "systems_tools",
        "label": "Systems & Tools (PDFs)",
        "keywords": [
            "bash", "linux", "powershell", "git", "sql", "mongodb", "postgresql", "mysql",
            "latex", "matlab", "r_notes", "shell", "embedded", "operating_system", "raft",
            "consensus", "docker"
        ]
    },
    4: {
        "name": "networking_security_hardware",
        "label": "Networking, Security & Hardware (PDFs)",
        "keywords": [
            "cryptograph", "security", "quantum", "hardware",
            "can_bus", "iso_c", "circuit", "tcp", "http", "dns",
            "cloud_infrastructure", "web_and_http", "wsl_and_vm"
        ]
    },
    5: {
        "name": "math_physics_stats",
        "label": "Math, Physics & Statistics (PDFs)",
        "keywords": [
            "algebra", "calculus", "statistics", "physics", "math", "geometry",
            "trigonometry", "number_theory", "discrete", "linear_algebra", "proof",
            "probability", "stochastic"
        ]
    },
    6: {
        "name": "general_education",
        "label": "General Education (PDFs)",
        "keywords": [
            "business", "ethics", "philosophy", "psychology", "sociology", "government",
            "entrepreneurship", "management", "writing", "college", "logic", "history"
        ]
    },
    7: {
        "name": "uncategorized",
        "label": "Uncategorized (PDFs)",
        "keywords": []
    }
}

# Map tiers to layer numbers (avoid collision with layers 0-19)
TIER_TO_LAYER = {1: 20, 2: 21, 3: 22, 4: 23, 5: 24, 6: 25, 7: 26}


def auto_detect_input_dir():
    """Auto-detect the Training_Materials directory."""
    desktop = os.path.join(os.path.expanduser("~"), "OneDrive", "Desktop")
    if not os.path.exists(desktop):
        desktop = os.path.join(os.path.expanduser("~"), "Desktop")
    return os.path.join(desktop, "Training_Materials")


def get_pdf_files(input_dir):
    """Scan directory recursively for PDF files."""
    if not os.path.exists(input_dir):
        print(f"WARNING: Input directory not found: {input_dir}")
        return []

    pdf_files = []
    for root, dirs, files in os.walk(input_dir):
        for file in sorted(files):
            if file.lower().endswith('.pdf'):
                pdf_files.append(os.path.join(root, file))
    pdf_files.sort()
    return pdf_files


# Map subfolder names to tiers (the user's folders are already organized)
SUBFOLDER_TIER_MAP = {
    "ai_ml": 1,
    "papers": 1,        # research papers are mostly AI/ML
    "cs_fundamentals": 2,
    "programming": 2,
    "language": 2,       # programming language books
    "systems": 3,
    "operating_systems": 3,
    "procedures": 3,     # tool/procedure guides
    "hardware": 4,
    "math": 5,
    "general_education": 6,
}


def categorize_pdf(filepath):
    """Categorize PDF by subfolder name first, then filename keywords."""
    # Check if it's in a subfolder we recognize
    parts = filepath.replace("\\", "/").split("/")
    for part in parts:
        part_lower = part.lower()
        if part_lower in SUBFOLDER_TIER_MAP:
            return SUBFOLDER_TIER_MAP[part_lower]

    # Fall back to filename keyword matching
    filename_lower = os.path.basename(filepath).lower()
    for tier in range(1, 7):  # Check tiers 1-6
        for keyword in PDF_TIERS[tier]["keywords"]:
            if keyword.lower() in filename_lower:
                return tier

    return 7  # Uncategorized


def extract_pdf_text_pdfplumber(pdf_path, max_pages=500):
    """Extract text from PDF using pdfplumber."""
    try:
        with pdfplumber.open(pdf_path) as pdf:
            if len(pdf.pages) > max_pages:
                print(f"  Skipping: Too many pages ({len(pdf.pages)} > {max_pages})")
                return None

            text = ""
            for page_num, page in enumerate(pdf.pages):
                try:
                    page_text = page.extract_text() or ""
                    text += page_text + "\n"
                except Exception as e:
                    print(f"  Warning: Failed to extract page {page_num + 1}: {e}")

            return text if text.strip() else None
    except Exception as e:
        print(f"  ERROR: Failed to read PDF: {e}")
        return None


def extract_pdf_text_pypdf2(pdf_path, max_pages=500):
    """Extract text from PDF using PyPDF2."""
    try:
        with open(pdf_path, 'rb') as f:
            reader = PyPDF2.PdfReader(f)
            if len(reader.pages) > max_pages:
                print(f"  Skipping: Too many pages ({len(reader.pages)} > {max_pages})")
                return None

            text = ""
            for page_num, page in enumerate(reader.pages):
                try:
                    page_text = page.extract_text() or ""
                    text += page_text + "\n"
                except Exception as e:
                    print(f"  Warning: Failed to extract page {page_num + 1}: {e}")

            return text if text.strip() else None
    except Exception as e:
        print(f"  ERROR: Failed to read PDF: {e}")
        return None


def extract_pdf_text(pdf_path, max_pages=500):
    """Extract text from PDF using available library."""
    if PDF_LIBRARY == "pdfplumber":
        return extract_pdf_text_pdfplumber(pdf_path, max_pages)
    else:
        return extract_pdf_text_pypdf2(pdf_path, max_pages)


def get_first_page_text(pdf_path):
    """Get first page text for duplicate detection."""
    try:
        if PDF_LIBRARY == "pdfplumber":
            with pdfplumber.open(pdf_path) as pdf:
                if pdf.pages:
                    return (pdf.pages[0].extract_text() or "")[:500]
        else:
            with open(pdf_path, 'rb') as f:
                reader = PyPDF2.PdfReader(f)
                if reader.pages:
                    return (reader.pages[0].extract_text() or "")[:500]
    except:
        pass
    return ""


def detect_duplicates(pdf_files):
    """Detect duplicate PDFs by file size and first-page text."""
    seen = {}
    duplicates = []

    for pdf_path in pdf_files:
        try:
            file_size = os.path.getsize(pdf_path)
            first_page = get_first_page_text(pdf_path)
            key = (file_size, hashlib.md5(first_page.encode()).hexdigest())

            if key in seen:
                duplicates.append((pdf_path, seen[key]))
            else:
                seen[key] = pdf_path
        except Exception as e:
            print(f"  Warning: Couldn't check duplicates for {pdf_path}: {e}")

    return duplicates


def clean_text(text):
    """Clean extracted text: remove headers, footers, page numbers, etc."""
    # Remove page numbers (common patterns)
    text = re.sub(r'^\s*-?\s*\d+\s*-?\s*$', '', text, flags=re.MULTILINE)
    text = re.sub(r'^\s*Page\s+\d+\s*$', '', text, flags=re.MULTILINE)

    # Remove common headers/footers
    text = re.sub(r'^\s*TABLE OF CONTENTS\s*$', '', text, flags=re.MULTILINE | re.IGNORECASE)
    text = re.sub(r'^\s*REFERENCES\s*$', '', text, flags=re.MULTILINE | re.IGNORECASE)
    text = re.sub(r'^\s*BIBLIOGRAPHY\s*$', '', text, flags=re.MULTILINE | re.IGNORECASE)
    text = re.sub(r'^\s*APPENDIX.*?$', '', text, flags=re.MULTILINE | re.IGNORECASE)
    text = re.sub(r'^\s*INDEX\s*$', '', text, flags=re.MULTILINE | re.IGNORECASE)

    # Remove figure/table captions
    text = re.sub(r'(?:Figure|Fig\.|Table|Tab\.)\s+\d+[.:]\s*[^\n]*', '', text, flags=re.IGNORECASE)

    # Remove excessive whitespace
    text = re.sub(r'\n\s*\n\s*\n+', '\n\n', text)
    text = re.sub(r' +', ' ', text)

    return text.strip()


def chunk_text(text, chunk_size=3000):
    """Chunk text into manageable pieces."""
    chunks = []
    current_chunk = ""

    for paragraph in text.split('\n\n'):
        if len(current_chunk) + len(paragraph) > chunk_size:
            if current_chunk.strip():
                chunks.append(current_chunk.strip())
            current_chunk = paragraph
        else:
            current_chunk += '\n\n' + paragraph if current_chunk else paragraph

    if current_chunk.strip():
        chunks.append(current_chunk.strip())

    return chunks


def call_ollama(text_chunk, model=DEFAULT_MODEL):
    """Call Ollama API to extract facts from text chunk."""
    extraction_prompt = f"""Extract the most important atomic facts from the following text.
Each fact should be a single, concrete, standalone statement that teaches something specific.
Skip: table of contents, references, bibliographies, figure captions, page headers/footers,
exercise questions, and filler text.
Focus on: definitions, key concepts, how things work, important relationships, and specific technical details.

TEXT:
{text_chunk}

Generate up to 30 facts from this text passage."""

    payload = {
        "model": model,
        "prompt": extraction_prompt,
        "system": SYSTEM_PROMPT,
        "stream": False
    }

    try:
        req = urllib.request.Request(
            OLLAMA_URL,
            data=json.dumps(payload).encode('utf-8'),
            headers={"Content-Type": "application/json"}
        )

        with urllib.request.urlopen(req, timeout=REQUEST_TIMEOUT) as response:
            result = json.loads(response.read().decode('utf-8'))

            if 'response' not in result:
                return []

            # Parse the response as JSON
            response_text = result['response'].strip()

            # Handle markdown code blocks
            if response_text.startswith('```json'):
                response_text = response_text[7:]
            if response_text.startswith('```'):
                response_text = response_text[3:]
            if response_text.endswith('```'):
                response_text = response_text[:-3]

            response_text = response_text.strip()

            parsed = json.loads(response_text)
            facts = parsed.get('facts', [])

            # Validate facts
            return [f.strip() for f in facts if isinstance(f, str) and len(f.strip()) > 5]

    except urllib.error.URLError as e:
        print(f"  ERROR: Ollama connection failed: {e}")
        print(f"  Make sure Ollama is running: ollama serve")
        return []
    except json.JSONDecodeError as e:
        print(f"  WARNING: Failed to parse Ollama response: {e}")
        return []
    except Exception as e:
        print(f"  ERROR: Ollama call failed: {e}")
        return []


def deduplicate_facts(facts):
    """Deduplicate facts within a PDF."""
    seen = set()
    unique = []
    for fact in facts:
        fact_lower = fact.lower().strip()
        if fact_lower not in seen and len(fact_lower) > 0:
            seen.add(fact_lower)
            unique.append(fact)
    return unique


def process_pdf(pdf_path, model=DEFAULT_MODEL, skip_large_mb=300, max_pages=500):
    """Process a single PDF and extract facts."""
    pdf_name = os.path.basename(pdf_path)

    # Check file size
    file_size_mb = os.path.getsize(pdf_path) / (1024 * 1024)
    if file_size_mb > skip_large_mb:
        print(f"  Skipping: File too large ({file_size_mb:.1f} MB > {skip_large_mb} MB)")
        return None

    print(f"  Processing: {pdf_name} ({file_size_mb:.1f} MB)")

    # Extract text
    text = extract_pdf_text(pdf_path, max_pages)
    if not text:
        return None

    # Clean text
    text = clean_text(text)

    # Chunk text
    chunks = chunk_text(text, chunk_size=3000)
    print(f"    Chunks: {len(chunks)}")

    # Extract facts from each chunk
    all_facts = []
    for chunk_idx, chunk in enumerate(chunks):
        if not chunk.strip():
            continue

        facts = call_ollama(chunk, model)
        all_facts.extend(facts)
        print(f"      Chunk {chunk_idx + 1}/{len(chunks)}: {len(facts)} facts")

        # Small delay to avoid overwhelming Ollama
        time.sleep(0.1)

    # Deduplicate
    unique_facts = deduplicate_facts(all_facts)
    print(f"    Total facts: {len(all_facts)} -> {len(unique_facts)} (deduplicated)")

    return {
        "filename": pdf_name,
        "facts": unique_facts,
        "page_count": len(text.split('\n')) // 30  # Rough estimate
    }


def load_existing_output(output_path):
    """Load existing curriculum_pdf_data.json if it exists."""
    if os.path.exists(output_path):
        try:
            with open(output_path, 'r') as f:
                return json.load(f)
        except Exception as e:
            print(f"WARNING: Could not load existing output: {e}")
    return None


def save_output(data, output_path):
    """Save output to curriculum_pdf_data.json."""
    try:
        with open(output_path, 'w') as f:
            json.dump(data, f, indent=2)
        print(f"  Saved: {output_path}")
    except Exception as e:
        print(f"ERROR: Failed to save output: {e}")


def build_curriculum_json(results_by_tier, model):
    """Build the curriculum JSON in the same format as other scripts."""
    total_facts = sum(len(r["facts"]) for results in results_by_tier.values() for r in results)

    domains = []
    for tier in sorted(results_by_tier.keys()):
        if not results_by_tier[tier]:
            continue

        tier_info = PDF_TIERS[tier]
        topics = []
        tier_facts = 0

        for result in results_by_tier[tier]:
            facts_with_associations = [
                {
                    "content": fact,
                    "associations": list(set(
                        [w.lower() for w in fact.split() if len(w) > 3][:5]
                    ))
                }
                for fact in result["facts"]
            ]

            topic = {
                "name": result["filename"].replace('.pdf', '').lower().replace(' ', '_'),
                "source_file": result["filename"],
                "facts": facts_with_associations,
                "count": len(result["facts"])
            }
            topics.append(topic)
            tier_facts += len(result["facts"])

        domain = {
            "name": tier_info["name"],
            "label": tier_info["label"],
            "layer": TIER_TO_LAYER[tier],
            "tier": tier,
            "topics": topics,
            "total_facts": tier_facts
        }
        domains.append(domain)

    curriculum = {
        "version": "2.0",
        "generated_at": datetime.now().isoformat(),
        "model": model,
        "total_facts": total_facts,
        "source": "pdf_extraction",
        "domain_count": len(domains),
        "domains": domains
    }

    return curriculum


def cmd_scan(args):
    """Scan directory and show PDFs by tier."""
    input_dir = args.input_dir or auto_detect_input_dir()
    pdf_files = get_pdf_files(input_dir)

    if not pdf_files:
        print(f"No PDFs found in {input_dir}")
        return

    print(f"\nFound {len(pdf_files)} PDFs in {input_dir}\n")

    by_tier = defaultdict(list)
    for pdf_path in pdf_files:
        tier = categorize_pdf(os.path.basename(pdf_path))
        by_tier[tier].append(pdf_path)

    for tier in sorted(by_tier.keys()):
        tier_info = PDF_TIERS[tier]
        print(f"TIER {tier}: {tier_info['label']} ({len(by_tier[tier])} PDFs)")
        for pdf_path in by_tier[tier]:
            size_mb = os.path.getsize(pdf_path) / (1024 * 1024)
            print(f"  - {os.path.basename(pdf_path)} ({size_mb:.1f} MB)")
        print()


def cmd_generate(args):
    """Extract facts from PDFs."""
    input_dir = args.input_dir or auto_detect_input_dir()
    pdf_files = get_pdf_files(input_dir)

    if not pdf_files:
        print(f"No PDFs found in {input_dir}")
        return

    print(f"\nProcessing {len(pdf_files)} PDFs with model: {args.model}")

    # Detect duplicates
    duplicates = detect_duplicates(pdf_files)
    if duplicates:
        print(f"\nFound {len(duplicates)} duplicate PDFs:")
        for dup, original in duplicates:
            print(f"  - {os.path.basename(dup)} (duplicate of {os.path.basename(original)})")

        # Remove duplicates from processing list
        dup_paths = {dup for dup, _ in duplicates}
        pdf_files = [p for p in pdf_files if p not in dup_paths]
        print(f"\nProcessing {len(pdf_files)} unique PDFs\n")

    # Load existing output if resuming
    output_path = os.path.join(input_dir, "curriculum_pdf_data.json")
    processed_files = set()

    if args.resume:
        existing = load_existing_output(output_path)
        if existing:
            for domain in existing.get('domains', []):
                for topic in domain.get('topics', []):
                    processed_files.add(topic['source_file'])
            print(f"Resuming: {len(processed_files)} PDFs already processed\n")

    # Filter PDFs if tier specified
    if args.tier:
        pdf_files = [p for p in pdf_files if categorize_pdf(os.path.basename(p)) == args.tier]
        print(f"Filtering to tier {args.tier}: {len(pdf_files)} PDFs\n")

    # Process PDFs
    results_by_tier = defaultdict(list)
    start_time = time.time()
    processed_count = 0

    for pdf_path in pdf_files:
        pdf_name = os.path.basename(pdf_path)

        if args.resume and pdf_name in processed_files:
            print(f"Skipping (already processed): {pdf_name}")
            continue

        tier = categorize_pdf(pdf_name)
        result = process_pdf(pdf_path, args.model, args.skip_large, args.max_pages)

        if result:
            results_by_tier[tier].append(result)
            processed_count += 1

        print()

    # Build and save curriculum
    if results_by_tier:
        curriculum = build_curriculum_json(results_by_tier, args.model)
        save_output(curriculum, output_path)

        # Print summary
        elapsed = time.time() - start_time
        print("\n" + "="*60)
        print(f"SUMMARY")
        print("="*60)
        print(f"Total PDFs processed: {processed_count}")
        print(f"Total facts extracted: {curriculum['total_facts']}")
        print(f"Time elapsed: {elapsed:.1f} seconds ({elapsed/60:.1f} minutes)")
        if processed_count > 0:
            print(f"Average: {curriculum['total_facts'] / processed_count:.0f} facts/PDF")
        print()
        for tier in sorted(results_by_tier.keys()):
            tier_info = PDF_TIERS[tier]
            tier_facts = sum(len(r["facts"]) for r in results_by_tier[tier])
            print(f"  Tier {tier}: {tier_facts} facts ({len(results_by_tier[tier])} PDFs)")
        print()
    else:
        print("No PDFs processed.")


def cmd_file(args):
    """Process a single PDF file."""
    if not os.path.exists(args.file):
        print(f"ERROR: File not found: {args.file}")
        return

    pdf_name = os.path.basename(args.file)
    tier = categorize_pdf(pdf_name)

    print(f"\nProcessing: {pdf_name}")
    print(f"Tier: {tier} ({PDF_TIERS[tier]['label']})\n")

    result = process_pdf(args.file, args.model, args.skip_large, args.max_pages)

    if result:
        results_by_tier = {tier: [result]}
        curriculum = build_curriculum_json(results_by_tier, args.model)

        output_dir = os.path.dirname(args.file)
        output_path = os.path.join(output_dir, "curriculum_single_pdf.json")
        save_output(curriculum, output_path)

        print(f"\nExtracted {result['count']} facts")
    else:
        print("Failed to process PDF")


def main():
    parser = argparse.ArgumentParser(
        description="Extract knowledge from PDFs and convert to Lu training format"
    )

    subparsers = parser.add_subparsers(dest='command', help='Command to execute')

    # Scan command
    scan_parser = subparsers.add_parser('scan', help='Scan directory and show PDFs by tier')
    scan_parser.add_argument('--input-dir', help='PDF directory')

    # Generate command
    gen_parser = subparsers.add_parser('generate', help='Extract facts from all PDFs')
    gen_parser.add_argument('--input-dir', help='PDF directory')
    gen_parser.add_argument('--model', default=DEFAULT_MODEL, help='Ollama model')
    gen_parser.add_argument('--tier', type=int, help='Only process tier N (1-7)')
    gen_parser.add_argument('--resume', action='store_true', help='Skip already processed PDFs')
    gen_parser.add_argument('--max-pages', type=int, default=500, help='Max pages per PDF')
    gen_parser.add_argument('--skip-large', type=int, default=300, help='Skip PDFs larger than N MB')

    # File command
    file_parser = subparsers.add_parser('file', help='Process a single PDF')
    file_parser.add_argument('file', help='Path to PDF file')
    file_parser.add_argument('--model', default=DEFAULT_MODEL, help='Ollama model')
    file_parser.add_argument('--max-pages', type=int, default=500, help='Max pages per PDF')
    file_parser.add_argument('--skip-large', type=int, default=300, help='Skip PDFs larger than N MB')

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        return

    print(f"Using PDF library: {PDF_LIBRARY}")

    if args.command == 'scan':
        cmd_scan(args)
    elif args.command == 'generate':
        cmd_generate(args)
    elif args.command == 'file':
        cmd_file(args)


if __name__ == '__main__':
    main()
