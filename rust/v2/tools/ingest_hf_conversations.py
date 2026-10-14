#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
HuggingFace Conversations Ingestion Pipeline for Lu Brain v2.

Streams and ingests HuggingFace datasets into Lu's brain via the MCP server
(lu_teach tool over stdio JSON-RPC).

Datasets (in priority order):
  1. dailydialog   — li2017dailydialog/daily_dialog   ~11 MB,  13K conversations
  2. eli5          — eli5                             ~113 MB, 272K Q&A pairs
  3. textbooks     — open-phi/textbooks               ~135 MB, 1.8K full textbooks
  4. tiny-textbooks — nampdn-ai/tiny-textbooks        ~1 GB,   420K synthetic docs
  5. ultrachat     — HuggingFaceH4/ultrachat_200k     ~3 GB,   207K multi-turn chats

Usage:
    python ingest_hf_conversations.py                   # ingest all datasets
    python ingest_hf_conversations.py --dataset eli5    # single dataset
    python ingest_hf_conversations.py --dry-run         # preview without MCP calls
    python ingest_hf_conversations.py --limit 500       # stop after 500 docs
    python ingest_hf_conversations.py --verbose

Options:
    --dataset NAME     One of: dailydialog, eli5, textbooks, tiny-textbooks, ultrachat,
                       science-facts, sciq, structured-wikipedia, openbookqa, scienceqa,
                       code-alpaca, python-code, code-feedback, no-robots, evol-code,
                       gsm8k, arc-challenge, commonsense-qa, triviaqa, hotpotqa, mmlu,
                       piqa, social-iqa, boolq, cosmos-qa, hellaswag, winogrande, wikitext, squad,
                       apps, natural-questions,
                       oasst1, prosocial-dialog, persona-chat, empathetic-dialogues,
                       dolly-15k, stackexchange, evol-codealpaca, goodwiki, ag-news
    --dry-run          Print docs without ingesting
    --limit N          Stop after N docs per dataset (0 = unlimited)
    --save-every N     Save brain every N docs (default: 200)
    --min-len N        Minimum text length in chars (default: 100)
    --max-len N        Maximum text length in chars (default: 4000)
    --rate-limit F     Seconds between MCP calls (default: 0.05)
    --brain PATH       Override brain file path
    --verbose          Print each doc and MCP response

Dependencies:
    pip install datasets

MCP binary: <AI_FACTORY_ROOT>\\v2\\target\\release\\lu_mcp_v2.exe
Brain file: <AI_FACTORY_ROOT>\\v2\\brain\\lu_brain.lbr
Log file:   v2/tools/hf_conversations_log.csv
"""

import argparse
import csv
import io
import json
import os
import queue
import re
import subprocess
import sys
import threading
import time
from datetime import datetime, timezone

# Force UTF-8 output on Windows
if sys.stdout.encoding and sys.stdout.encoding.lower() not in ("utf-8", "utf8"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
if sys.stderr.encoding and sys.stderr.encoding.lower() not in ("utf-8", "utf8"):
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace")

# ── WMI hang workaround ────────────────────────────────────────────────────────
# Python 3.14 platform.machine() calls _wmi.exec_query("SELECT Architecture FROM
# Win32_Processor") which can hang indefinitely when the WMI service is slow.
# pandas.compat._constants imports platform.machine() during its module body,
# so this deadlock prevents 'import pandas' from ever completing.
# Setting platform._wmi = None forces the fast fallback (PROCESSOR_ARCHITECTURE
# env var) and must be done before any pandas import.
try:
    import platform as _platform_mod
    _platform_mod._wmi = None  # disable WMI queries in platform module
    del _platform_mod
except Exception:
    pass

# ── IPv4 preference patch ──────────────────────────────────────────────────────
# DNS for huggingface.co returns IPv6-only results on this machine, but the local
# network has no IPv6 route. socket.create_connection() tries each address in
# order; all IPv6 attempts hang until timeout before IPv4 is tried. Sorting IPv4
# addresses first makes every HF Hub request connect immediately without changing
# fallback behaviour (IPv6 is still tried if IPv4 fails).
try:
    import socket as _sock_patch
    _orig_getaddrinfo = _sock_patch.getaddrinfo
    _AF_INET = _sock_patch.AF_INET  # capture value; _sock_patch will be deleted

    def _getaddrinfo_ipv4_first(host, port, family=0, type=0, proto=0, flags=0):
        results = _orig_getaddrinfo(host, port, family=family, type=type,
                                    proto=proto, flags=flags)
        return sorted(results, key=lambda r: 0 if r[0] == _AF_INET else 1)

    _sock_patch.getaddrinfo = _getaddrinfo_ipv4_first
    del _sock_patch
except Exception:
    pass

# ── Process Guard: kill stale lu_mcp_v2.exe to prevent file-lock conflicts ───
try:
    subprocess.run(
        ["taskkill", "/F", "/IM", "lu_mcp_v2.exe"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        timeout=10,
    )
    print("[guard] Killed existing lu_mcp_v2.exe process(es).")
except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
    pass  # no existing process – nothing to kill

# ── Config ─────────────────────────────────────────────────────────────────────

MCP_EXE   = r"<AI_FACTORY_ROOT>\v2\target\release\lu_mcp_v2.exe"
BRAIN_PATH = r"<AI_FACTORY_ROOT>\v2\brain\lu_brain.lbr"

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
LOG_CSV    = os.path.join(SCRIPT_DIR, "hf_conversations_log.csv")

DEFAULT_MIN_LEN    = 100
DEFAULT_MAX_LEN    = 4000
DEFAULT_SAVE_EVERY = 200
DEFAULT_RATE_LIMIT = 0.05   # seconds between MCP calls (~20/sec max)

# ── Dataset registry ───────────────────────────────────────────────────────────

DATASETS = {
    "dailydialog": {
        "hf_path": "li2017dailydialog/daily_dialog",
        "split": "train",
        "streaming": True,
        "description": "DailyDialog — 13K human conversations with emotion labels",
        "size_hint": "~11 MB",
    },
    "eli5": {
        "hf_path": "eli5",
        "split": "train_eli5",
        "streaming": True,
        "description": "ELI5 — 272K Explain-Like-I'm-5 Q&A pairs",
        "size_hint": "~113 MB",
    },
    "textbooks": {
        "hf_path": "open-phi/textbooks",
        "split": "train",
        "streaming": True,
        "description": "open-phi/textbooks — 1,800 full textbooks",
        "size_hint": "~135 MB",
    },
    "tiny-textbooks": {
        "hf_path": "nampdn-ai/tiny-textbooks",
        "split": "train",
        "streaming": True,
        "description": "nampdn-ai/tiny-textbooks — 420K synthetic textbook docs",
        "size_hint": "~1 GB",
    },
    "ultrachat": {
        "hf_path": "HuggingFaceH4/ultrachat_200k",
        "split": "train_sft",
        "streaming": True,
        "description": "UltraChat 200K — 207K multi-turn conversations",
        "size_hint": "~3 GB",
    },
    # ── New factual datasets ────────────────────────────────────────────────
    "science-facts": {
        "hf_path": "Royal-lobster/10001-Science-Facts",
        "split": "train",
        "streaming": True,
        "description": "10,001 Science Facts — short factual statements across all sciences",
        "size_hint": "~2 MB",
    },
    "sciq": {
        "hf_path": "allenai/sciq",
        "split": "train",
        "streaming": True,
        "description": "SciQ — 13.7K science exam Q&A with supporting paragraphs",
        "size_hint": "~5 MB",
    },
    "structured-wikipedia": {
        "hf_path": "wikimedia/structured-wikipedia",
        "hf_config": "20240916.en",
        "split": "train",
        "streaming": True,
        "description": "Structured Wikipedia — 6.5M English articles with sections",
        "size_hint": "~22 GB (streaming)",
    },
    "openbookqa": {
        "hf_path": "allenai/openbookqa",
        "hf_config": "main",
        "split": "train",
        "streaming": True,
        "description": "OpenBookQA — 5.9K elementary science facts + Q&A",
        "size_hint": "~2 MB",
    },
    "scienceqa": {
        "hf_path": "lmms-lab/ScienceQA",
        "hf_config": "ScienceQA-FULL",
        "split": "test",
        "streaming": True,
        "description": "ScienceQA — 21K multimodal science Q&A with explanations",
        "size_hint": "~700 MB",
    },
    # ── Programming & CS datasets ─────────────────────────────────────────────
    "code-alpaca": {
        "hf_path": "sahil2801/CodeAlpaca-20k",
        "split": "train",
        "streaming": True,
        "description": "CodeAlpaca 20K — code instruction/response pairs across languages",
        "size_hint": "~10 MB",
    },
    "python-code": {
        "hf_path": "iamtarun/python_code_instructions_18k_alpaca",
        "split": "train",
        "streaming": True,
        "description": "Python Code Instructions 18K — Python-specific instructions with explanations",
        "size_hint": "~10 MB",
    },
    "code-feedback": {
        "hf_path": "m-a-p/Code-Feedback",
        "split": "train",
        "streaming": True,
        "description": "Code-Feedback — multi-turn code Q&A with detailed feedback",
        "size_hint": "~500 MB",
    },
    "no-robots": {
        "hf_path": "HuggingFaceH4/no_robots",
        "split": "train",
        "streaming": True,
        "description": "No Robots — 10K high-quality human-written instruction responses",
        "size_hint": "~15 MB",
    },
    "evol-code": {
        "hf_path": "nickrosh/Evol-Instruct-Code-80k-v1",
        "split": "train",
        "streaming": True,
        "description": "Evol-Instruct Code 80K — evolved code instructions with solutions",
        "size_hint": "~100 MB",
    },
    # ── Reasoning / Math ──────────────────────────────────────────────────────
    "gsm8k": {
        "hf_path": "openai/gsm8k",
        "hf_config": "main",
        "split": "train",
        "streaming": True,
        "description": "GSM8K — 8.5K grade school math word problems with step-by-step solutions",
        "size_hint": "~5 MB",
    },
    "arc-challenge": {
        "hf_path": "allenai/ai2_arc",
        "hf_config": "ARC-Challenge",
        "split": "train",
        "streaming": True,
        "description": "ARC-Challenge — 1.1K challenging science exam Q&A requiring reasoning",
        "size_hint": "~1 MB",
    },
    "commonsense-qa": {
        "hf_path": "tau/commonsense_qa",
        "split": "train",
        "streaming": True,
        "description": "CommonsenseQA — 9.7K multiple-choice commonsense reasoning Q&A",
        "size_hint": "~3 MB",
    },
    # ── World Knowledge ───────────────────────────────────────────────────────
    "triviaqa": {
        "hf_path": "trivia_qa",
        "hf_config": "rc.nocontext",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "TriviaQA — 88K trivia Q&A pairs across many domains (50K limit)",
        "size_hint": "~200 MB",
    },
    "hotpotqa": {
        "hf_path": "hotpot_qa",
        "hf_config": "fullwiki",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "HotpotQA FullWiki — 90K multi-hop reasoning Q&A with supporting facts (50K limit)",
        "size_hint": "~350 MB",
    },
    "mmlu": {
        "hf_path": "cais/mmlu",
        "hf_config": "all",
        "split": "test",
        "streaming": True,
        "max_examples": 50000,
        "description": "MMLU — 57 academic subjects, multi-task language understanding (50K limit)",
        "size_hint": "~100 MB",
    },
    # ── Social / Practical ────────────────────────────────────────────────────
    "piqa": {
        "hf_path": "ybisk/piqa",
        "split": "train",
        "streaming": True,
        "description": "PIQA — 16.1K physical-intuition QA: how to achieve everyday physical goals",
        "size_hint": "~5 MB",
    },
    "social-iqa": {
        "hf_path": "allenai/social_i_qa",
        "split": "train",
        "streaming": True,
        "description": "Social IQa — 38K social-commonsense reasoning about people's actions and reactions",
        "size_hint": "~8 MB",
    },
    "boolq": {
        "hf_path": "google/boolq",
        "split": "train",
        "streaming": True,
        "description": "BoolQ — 9.4K yes/no reading-comprehension from Wikipedia passages",
        "size_hint": "~4 MB",
    },
    "hellaswag": {
        "hf_path": "Rowan/hellaswag",
        "split": "train",
        "streaming": True,
        "max_examples": 40000,
        "description": "HellaSwag — 40K commonsense NLI activity-description completions (40K limit)",
        "size_hint": "~90 MB",
    },
    "winogrande": {
        "hf_path": "winogrande",
        "hf_config": "winogrande_xl",
        "split": "train",
        "streaming": True,
        "max_examples": 40000,
        "description": "Winogrande XL — 40K pronoun-resolution commonsense coreference",
        "size_hint": "~40 MB",
    },
    # ── Encyclopedic ──────────────────────────────────────────────────────────
    "wikitext": {
        "hf_path": "wikitext",
        "hf_config": "wikitext-103-v1",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "WikiText-103 — curated Wikipedia article text for language modeling (50K limit)",
        "size_hint": "~500 MB",
    },
    "squad": {
        "hf_path": "rajpurkar/squad",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "SQuAD — 87K Wikipedia reading-comprehension Q&A pairs (50K limit)",
        "size_hint": "~30 MB",
    },
    # ── More CS / Code ────────────────────────────────────────────────────────
    # Note: bigcode/the-stack-smol and nampdn-ai/tiny-codes are gated; codeparrot/apps
    # uses a deprecated loading script.  Using microsoft/orca-math-word-problems-200k
    # (200K grade-school math word problems, public Parquet, fields: question/answer).
    "orca-math": {
        "hf_path": "microsoft/orca-math-word-problems-200k",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "Orca Math — 200K grade-school math word problems with step-by-step solutions (50K limit)",
        "size_hint": "~80 MB",
    },
    # ── Open-domain Q&A ───────────────────────────────────────────────────────
    "natural-questions": {
        "hf_path": "google-research-datasets/nq_open",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "Natural Questions Open — 87K Google-search Q&A from real user queries (50K limit)",
        "size_hint": "~20 MB",
    },
    # ── Conversational / social ───────────────────────────────────────────────
    "oasst1": {
        "hf_path": "OpenAssistant/oasst1",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "OpenAssistant OASST1 — 84K human-written assistant dialogue messages (EN, 50K limit)",
        "size_hint": "~40 MB",
    },
    "prosocial-dialog": {
        "hf_path": "allenai/prosocial-dialog",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "ProSocial Dialog — 120K safety-aware conversations with social-norm rules-of-thumb (50K limit)",
        "size_hint": "~80 MB",
    },
    "persona-chat": {
        "hf_path": "AlekseyKorshuk/persona-chat",
        "split": "train",
        "streaming": True,
        "description": "Persona-Chat — 18K persona-grounded multi-turn human conversations",
        "size_hint": "~10 MB",
    },
    "hh-rlhf": {
        "hf_path": "Anthropic/hh-rlhf",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "Anthropic HH-RLHF — 170K helpful/harmless preference dialogues (50K limit)",
        "size_hint": "~150 MB",
    },
    # ── Practical / how-to ────────────────────────────────────────────────────
    "dolly-15k": {
        "hf_path": "databricks/databricks-dolly-15k",
        "split": "train",
        "streaming": True,
        "description": "Databricks Dolly 15K — 15K human-written diverse instruction-response pairs",
        "size_hint": "~10 MB",
    },
    # ── CS depth / technical Q&A ──────────────────────────────────────────────
    "stackexchange": {
        "hf_path": "sentence-transformers/stackexchange-duplicates",
        "hf_config": "post-post-pair",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "StackExchange Post Pairs — 250K full-body technical posts (SO/ServerFault/Security.SE, 50K limit)",
        "size_hint": "~500 MB",
    },
    "evol-codealpaca": {
        "hf_path": "theblackcat102/evol-codealpaca-v1",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "Evol-CodeAlpaca — 111K WizardCoder-style evolved code+CS instructions (50K limit)",
        "size_hint": "~200 MB",
    },
    # ── Encyclopedic / news ───────────────────────────────────────────────────
    "goodwiki": {
        "hf_path": "euirim/goodwiki",
        "split": "train",
        "streaming": True,
        "description": "GoodWiki — 45K Featured and Good Wikipedia articles (highest editorial quality)",
        "size_hint": "~800 MB",
    },
    "ag-news": {
        "hf_path": "fancyzhx/ag_news",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "AG News — 120K news articles across World/Sports/Business/Sci-Tech (50K limit)",
        "size_hint": "~30 MB",
    },
}

DATASET_ORDER = [
    "eli5", "textbooks", "tiny-textbooks", "ultrachat",
    # Factual datasets (priority for filling knowledge gaps)
    "science-facts", "sciq", "structured-wikipedia", "openbookqa", "scienceqa",
    # Programming & CS datasets
    "code-alpaca", "python-code", "code-feedback", "no-robots", "evol-code",
    # Reasoning / Math
    "gsm8k", "arc-challenge", "commonsense-qa",
    # World knowledge
    "triviaqa", "hotpotqa", "mmlu",
    # Social / Practical
    "boolq", "dolly-15k", "hellaswag", "winogrande",
    # Encyclopedic
    "wikitext", "squad",
    # More CS / Math
    "orca-math",
    # Open-domain Q&A
    "natural-questions",
    # Conversational / social
    "oasst1", "prosocial-dialog", "persona-chat", "hh-rlhf",
    # CS depth / technical
    "stackexchange", "evol-codealpaca",
    # Encyclopedic / news
    "goodwiki", "ag-news",
]


# ── Text extraction ────────────────────────────────────────────────────────────

def extract_dailydialog(example: dict, idx: int) -> tuple[str, str]:
    """
    DailyDialog: each example has a `dialog` list of utterances.
    Returns (doc_id, text).
    """
    dialog = example.get("dialog") or example.get("dialogue") or []
    if not dialog:
        return str(idx), ""

    # Format as alternating speaker turns
    lines = []
    for i, utterance in enumerate(dialog):
        speaker = "A" if i % 2 == 0 else "B"
        utt = str(utterance).strip()
        if utt:
            lines.append(f"{speaker}: {utt}")

    return f"dailydialog_{idx}", "\n".join(lines)


def extract_eli5(example: dict, idx: int) -> tuple[str, str]:
    """
    ELI5: each example has `q_id`, `title` (question), and `answers.text` list.
    Returns (doc_id, text).
    """
    q_id   = example.get("q_id") or str(idx)
    title  = (example.get("title") or "").strip()
    selftext = (example.get("selftext") or "").strip()

    # Get best answer (highest score or first)
    answers_dict = example.get("answers") or {}
    texts  = answers_dict.get("text") or []
    scores = answers_dict.get("score") or []

    best_answer = ""
    if texts:
        if scores and len(scores) == len(texts):
            best_idx = scores.index(max(scores))
        else:
            best_idx = 0
        best_answer = str(texts[best_idx]).strip()

    parts = []
    if title:
        parts.append(f"Q: {title}")
    if selftext and selftext not in ("[deleted]", "[removed]"):
        parts.append(f"Context: {selftext}")
    if best_answer:
        parts.append(f"A: {best_answer}")

    return f"eli5_{q_id}", "\n".join(parts)


def extract_textbooks(example: dict, idx: int) -> tuple[str, str]:
    """
    open-phi/textbooks: typically has a `textbook` or `text` field with full content.
    Returns (doc_id, text).
    """
    text = (
        example.get("textbook")
        or example.get("text")
        or example.get("content")
        or ""
    )
    return f"textbooks_{idx}", str(text).strip()


def extract_tiny_textbooks(example: dict, idx: int) -> tuple[str, str]:
    """
    nampdn-ai/tiny-textbooks: has a `text` field with synthetic textbook content.
    Returns (doc_id, text).
    """
    text = example.get("text") or example.get("content") or ""
    return f"tinytextbooks_{idx}", str(text).strip()


def extract_ultrachat(example: dict, idx: int) -> tuple[str, str]:
    """
    UltraChat 200K: has `prompt_id` and `messages` list of {role, content}.
    Returns (doc_id, text).
    """
    prompt_id = example.get("prompt_id") or str(idx)
    messages  = example.get("messages") or []

    lines = []
    for msg in messages:
        role    = str(msg.get("role", "")).capitalize()
        content = str(msg.get("content", "")).strip()
        if role and content:
            lines.append(f"{role}: {content}")

    return f"ultrachat_{prompt_id}", "\n".join(lines)


def extract_science_facts(example: dict, idx: int) -> tuple[str, str]:
    """
    Royal-lobster/10001-Science-Facts: short factual statements.
    Fields vary — try common column names: fact, text, Fact, content.
    """
    text = (
        example.get("fact")
        or example.get("Fact")
        or example.get("text")
        or example.get("content")
        or example.get("science_fact")
        or ""
    )
    # Some versions have category/topic field — prepend it for context
    topic = example.get("category") or example.get("topic") or example.get("Category") or ""
    if topic and text:
        text = f"{topic}: {text}"
    return f"scifact_{idx}", str(text).strip()


def extract_sciq(example: dict, idx: int) -> tuple[str, str]:
    """
    allenai/sciq: science exam questions with supporting paragraphs.
    Fields: question, correct_answer, support, distractor1, distractor2, distractor3.
    We keep the question, correct answer, and support paragraph (the real factual content).
    """
    question = (example.get("question") or "").strip()
    answer   = (example.get("correct_answer") or "").strip()
    support  = (example.get("support") or "").strip()

    parts = []
    if support:
        parts.append(support)  # Put the factual paragraph first — it's the real content
    if question and answer:
        parts.append(f"Q: {question}")
        parts.append(f"A: {answer}")

    return f"sciq_{idx}", "\n".join(parts)


def extract_structured_wikipedia(example: dict, idx: int) -> tuple[str, str]:
    """
    wikimedia/structured-wikipedia: full Wikipedia articles.
    Fields: name (title), abstract, url, sections (list of {name, text, ...}).
    We take the abstract + first few section texts to stay within size limits.
    """
    title    = (example.get("name") or example.get("title") or "").strip()
    abstract = (example.get("abstract") or "").strip()

    parts = []
    if title:
        parts.append(title)
    if abstract:
        parts.append(abstract)

    # Extract section text (sections is a list of dicts or nested structure)
    sections = example.get("sections") or []
    for sec in sections[:5]:  # limit to first 5 sections to stay reasonable
        if isinstance(sec, dict):
            sec_text = (sec.get("text") or sec.get("content") or "").strip()
            sec_name = (sec.get("name") or sec.get("title") or "").strip()
            if sec_text:
                if sec_name:
                    parts.append(f"{sec_name}: {sec_text}")
                else:
                    parts.append(sec_text)
        elif isinstance(sec, str):
            sec = sec.strip()
            if sec:
                parts.append(sec)

    doc_id = title.replace(" ", "_")[:60] if title else str(idx)
    return f"wiki_{doc_id}_{idx}", "\n\n".join(parts)


def extract_openbookqa(example: dict, idx: int) -> tuple[str, str]:
    """
    allenai/openbookqa: elementary science facts with multiple-choice Q&A.
    Fields: id, question_stem, choices (dict with text/label lists),
            answerKey, fact1 (the core science fact).
    We prioritize fact1 — that's the real factual content Lu needs.
    """
    fact     = (example.get("fact1") or "").strip()
    question = (example.get("question_stem") or "").strip()
    answer_key = (example.get("answerKey") or "").strip()
    q_id     = example.get("id") or str(idx)

    choices_dict = example.get("choices") or {}
    labels = choices_dict.get("label") or []
    texts  = choices_dict.get("text") or []

    correct = ""
    if answer_key and labels and texts and len(labels) == len(texts):
        try:
            ci = labels.index(answer_key)
            correct = texts[ci].strip()
        except ValueError:
            pass

    parts = []
    if fact:
        parts.append(fact)  # The core science fact — most valuable
    if question and correct:
        parts.append(f"Q: {question}")
        parts.append(f"A: {correct}")

    return f"obqa_{q_id}", "\n".join(parts)


def extract_scienceqa(example: dict, idx: int) -> tuple[str, str]:
    """
    lmms-lab/ScienceQA: multimodal science Q&A with explanations.
    Fields: question, choices (list), answer (index), hint, lecture, solution,
            subject, topic, category, grade.
    We skip image-only questions and focus on text + lecture + solution.
    """
    question = (example.get("question") or "").strip()
    choices  = example.get("choices") or []
    answer_idx = example.get("answer")
    hint     = (example.get("hint") or "").strip()
    lecture  = (example.get("lecture") or "").strip()
    solution = (example.get("solution") or "").strip()
    subject  = (example.get("subject") or "").strip()
    topic    = (example.get("topic") or "").strip()

    # Skip image-dependent questions (no text content)
    if not question and not lecture:
        return f"sciqa_{idx}", ""

    parts = []
    if subject and topic:
        parts.append(f"[{subject} — {topic}]")
    if lecture:
        parts.append(lecture)  # The actual educational content — most valuable
    if question:
        parts.append(f"Q: {question}")
    if choices and answer_idx is not None:
        try:
            correct = choices[int(answer_idx)]
            parts.append(f"A: {correct}")
        except (IndexError, ValueError, TypeError):
            pass
    if hint:
        parts.append(f"Hint: {hint}")
    if solution:
        parts.append(f"Explanation: {solution}")

    return f"sciqa_{idx}", "\n".join(parts)


def extract_code_alpaca(example: dict, idx: int) -> tuple[str, str]:
    """
    sahil2801/CodeAlpaca-20k: code instruction/response pairs.
    Fields: instruction, input, output.
    """
    instruction = (example.get("instruction") or "").strip()
    inp = (example.get("input") or "").strip()
    output = (example.get("output") or "").strip()

    parts = []
    if instruction:
        parts.append(f"Task: {instruction}")
    if inp:
        parts.append(f"Input: {inp}")
    if output:
        parts.append(f"Solution:\n{output}")

    return f"codealp_{idx}", "\n".join(parts)


def extract_python_code(example: dict, idx: int) -> tuple[str, str]:
    """
    iamtarun/python_code_instructions_18k_alpaca: Python instructions with code.
    Fields: instruction, input, output, prompt.
    """
    instruction = (example.get("instruction") or "").strip()
    inp = (example.get("input") or "").strip()
    output = (example.get("output") or "").strip()

    parts = []
    if instruction:
        parts.append(f"Task: {instruction}")
    if inp:
        parts.append(f"Input: {inp}")
    if output:
        parts.append(f"Solution:\n{output}")

    return f"pycode_{idx}", "\n".join(parts)


def extract_code_feedback(example: dict, idx: int) -> tuple[str, str]:
    """
    m-a-p/Code-Feedback: multi-turn code Q&A conversations.
    Fields: id, messages (list of {role, content}).
    """
    msg_id = example.get("id") or str(idx)
    messages = example.get("messages") or []

    lines = []
    for msg in messages:
        role = str(msg.get("role", "")).capitalize()
        content = str(msg.get("content", "")).strip()
        if role and content:
            lines.append(f"{role}: {content}")

    return f"codefb_{msg_id}", "\n\n".join(lines)


def extract_no_robots(example: dict, idx: int) -> tuple[str, str]:
    """
    HuggingFaceH4/no_robots: high-quality human-written instruction responses.
    Fields: prompt, prompt_id, messages (list of {role, content}), category.
    """
    prompt_id = example.get("prompt_id") or str(idx)
    category = (example.get("category") or "").strip()
    messages = example.get("messages") or []

    lines = []
    if category:
        lines.append(f"[{category}]")
    for msg in messages:
        role = str(msg.get("role", "")).capitalize()
        content = str(msg.get("content", "")).strip()
        if role and content:
            lines.append(f"{role}: {content}")

    return f"norobots_{prompt_id[:12]}_{idx}", "\n\n".join(lines)


def extract_evol_code(example: dict, idx: int) -> tuple[str, str]:
    """
    nickrosh/Evol-Instruct-Code-80k-v1: evolved code instructions.
    Fields: instruction, output.
    """
    instruction = (example.get("instruction") or "").strip()
    output = (example.get("output") or "").strip()

    parts = []
    if instruction:
        parts.append(f"Task: {instruction}")
    if output:
        parts.append(f"Solution:\n{output}")

    return f"evolcode_{idx}", "\n".join(parts)


def extract_gsm8k(example: dict, idx: int) -> tuple[str, str]:
    """
    openai/gsm8k: grade school math word problems.
    Fields: question, answer (reasoning chain ending with "#### N").
    Keep the full answer — the chain-of-thought reasoning is the valuable content.
    """
    question = (example.get("question") or "").strip()
    answer   = (example.get("answer") or "").strip()

    parts = []
    if question:
        parts.append(f"Problem: {question}")
    if answer:
        parts.append(f"Solution: {answer}")

    return f"gsm8k_{idx}", "\n".join(parts)


def extract_arc_challenge(example: dict, idx: int) -> tuple[str, str]:
    """
    allenai/ai2_arc (ARC-Challenge): challenging science exam questions.
    Fields: id, question, choices (dict: text/label lists), answerKey.
    """
    q_id       = example.get("id") or str(idx)
    question   = (example.get("question") or "").strip()
    choices_dict = example.get("choices") or {}
    labels     = choices_dict.get("label") or []
    texts      = choices_dict.get("text") or []
    answer_key = (example.get("answerKey") or "").strip()

    correct = ""
    if answer_key and labels and texts and len(labels) == len(texts):
        try:
            ci = labels.index(answer_key)
            correct = str(texts[ci]).strip()
        except ValueError:
            pass

    parts = []
    if question:
        parts.append(f"Q: {question}")
    if correct:
        parts.append(f"A: {correct}")

    return f"arc_{q_id}", "\n".join(parts)


def extract_commonsense_qa(example: dict, idx: int) -> tuple[str, str]:
    """
    tau/commonsense_qa: multiple-choice commonsense Q&A.
    Fields: id, question, question_concept, choices (dict: text/label), answerKey.
    """
    q_id       = example.get("id") or str(idx)
    question   = (example.get("question") or "").strip()
    concept    = (example.get("question_concept") or "").strip()
    choices_dict = example.get("choices") or {}
    labels     = choices_dict.get("label") or []
    texts      = choices_dict.get("text") or []
    answer_key = (example.get("answerKey") or "").strip()

    correct = ""
    if answer_key and labels and texts and len(labels) == len(texts):
        try:
            ci = labels.index(answer_key)
            correct = str(texts[ci]).strip()
        except ValueError:
            pass

    parts = []
    if concept:
        parts.append(f"Concept: {concept}")
    if question:
        parts.append(f"Q: {question}")
    if correct:
        parts.append(f"A: {correct}")

    return f"csqa_{q_id}", "\n".join(parts)


def extract_triviaqa(example: dict, idx: int) -> tuple[str, str]:
    """
    trivia_qa (rc.nocontext): open-domain trivia Q&A pairs.
    Fields: question_id, question, answer (dict with value/aliases/normalized_value).
    """
    q_id     = example.get("question_id") or str(idx)
    question = (example.get("question") or "").strip()
    ans_dict = example.get("answer") or {}
    answer   = (ans_dict.get("value") or "").strip()
    if not answer:
        aliases = ans_dict.get("normalized_aliases") or []
        if aliases:
            answer = str(aliases[0]).strip()

    parts = []
    if question:
        parts.append(f"Q: {question}")
    if answer:
        parts.append(f"A: {answer}")

    return f"trivia_{str(q_id)[:20]}_{idx}", "\n".join(parts)


def extract_hotpotqa(example: dict, idx: int) -> tuple[str, str]:
    """
    hotpot_qa (fullwiki): multi-hop Q&A with supporting facts.
    Fields: id, question, answer, supporting_facts (dict: title/sent_id),
            context (dict: title/sentences lists).
    Emit supporting passages + Q + A so Lu learns the reasoning chain.
    """
    q_id     = example.get("id") or str(idx)
    question = (example.get("question") or "").strip()
    answer   = (example.get("answer") or "").strip()

    context  = example.get("context") or {}
    titles   = context.get("title") or []
    sent_lists = context.get("sentences") or []
    sup_facts = example.get("supporting_facts") or {}
    sup_titles = set(sup_facts.get("title") or [])

    ctx_parts = []
    for title, sentences in zip(titles, sent_lists):
        if title in sup_titles:
            text = " ".join(str(s) for s in sentences if s).strip()
            if text:
                ctx_parts.append(f"{title}: {text}")

    parts = ctx_parts[:3]  # at most 3 supporting passages
    if question:
        parts.append(f"Q: {question}")
    if answer:
        parts.append(f"A: {answer}")

    return f"hotpot_{str(q_id)[:20]}_{idx}", "\n".join(parts)


def extract_mmlu(example: dict, idx: int) -> tuple[str, str]:
    """
    cais/mmlu (all): 57-subject academic Q&A.
    Fields: question, choices (list of 4 strings), answer (int 0-3), subject.
    """
    question   = (example.get("question") or "").strip()
    choices    = example.get("choices") or []
    answer_idx = example.get("answer")
    subject    = (example.get("subject") or "").strip()

    correct = ""
    if answer_idx is not None and choices:
        try:
            correct = str(choices[int(answer_idx)]).strip()
        except (IndexError, ValueError, TypeError):
            pass

    parts = []
    if subject:
        parts.append(f"[{subject}]")
    if question:
        parts.append(f"Q: {question}")
    if correct:
        parts.append(f"A: {correct}")

    return f"mmlu_{idx}", "\n".join(parts)


def extract_piqa(example: dict, idx: int) -> tuple[str, str]:
    """
    piqa: physical intuition Q&A — how to achieve a physical goal.
    Fields: goal, sol1, sol2, label (0 or 1).
    """
    goal  = (example.get("goal") or "").strip()
    sol1  = (example.get("sol1") or "").strip()
    sol2  = (example.get("sol2") or "").strip()
    label = example.get("label")

    correct = sol1 if label == 0 else (sol2 if label == 1 else "")

    parts = []
    if goal:
        parts.append(f"Goal: {goal}")
    if correct:
        parts.append(f"Solution: {correct}")

    return f"piqa_{idx}", "\n".join(parts)


def extract_social_iqa(example: dict, idx: int) -> tuple[str, str]:
    """
    social_i_qa: social commonsense reasoning.
    Fields: context, question, answerA, answerB, answerC, label ("1"/"2"/"3").
    """
    context  = (example.get("context") or "").strip()
    question = (example.get("question") or "").strip()
    a_opts   = {
        "1": (example.get("answerA") or "").strip(),
        "2": (example.get("answerB") or "").strip(),
        "3": (example.get("answerC") or "").strip(),
    }
    label   = str(example.get("label") or "").strip()
    correct = a_opts.get(label, "")

    parts = []
    if context:
        parts.append(context)
    if question:
        parts.append(f"Q: {question}")
    if correct:
        parts.append(f"A: {correct}")

    return f"siqa_{idx}", "\n".join(parts)


def extract_hellaswag(example: dict, idx: int) -> tuple[str, str]:
    """
    Rowan/hellaswag: commonsense NLI — pick the correct activity continuation.
    Fields: activity_label, ctx, endings (list of 4 strings), label (str/int index).
    """
    activity = (example.get("activity_label") or "").strip()
    ctx      = (example.get("ctx") or "").strip()
    endings  = example.get("endings") or []
    label    = example.get("label")

    correct = ""
    if label is not None and endings:
        try:
            correct = str(endings[int(label)]).strip()
        except (IndexError, ValueError, TypeError):
            pass

    parts = []
    if activity:
        parts.append(f"Activity: {activity}")
    if ctx:
        parts.append(ctx)
    if correct:
        parts.append(f"Continuation: {correct}")

    return f"hellaswag_{idx}", "\n".join(parts)


def extract_winogrande(example: dict, idx: int) -> tuple[str, str]:
    """
    winogrande (winogrande_xl): pronoun-resolution coreference sentences.
    Fields: sentence (with '_' blank), option1, option2, answer ("1" or "2").
    Fill the blank with the correct option to produce a coherent sentence.
    """
    sentence = (example.get("sentence") or "").strip()
    option1  = (example.get("option1") or "").strip()
    option2  = (example.get("option2") or "").strip()
    answer   = str(example.get("answer") or "").strip()

    correct = option1 if answer == "1" else (option2 if answer == "2" else "")
    if sentence and correct:
        text = sentence.replace("_", correct, 1)
    else:
        text = sentence

    return f"wino_{idx}", text


def extract_wikitext(example: dict, idx: int) -> tuple[str, str]:
    """
    wikitext (wikitext-103-v1): Wikipedia article text for language modeling.
    Fields: text. Skip empty lines and section-header-only lines (= Title =).
    """
    text = (example.get("text") or "").strip()
    # Skip bare section headers like "= History =" or "== Sub-section =="
    if re.match(r'^=+\s.*\s=+$', text):
        return f"wiki103_{idx}", ""
    return f"wiki103_{idx}", text


def extract_squad(example: dict, idx: int) -> tuple[str, str]:
    """
    rajpurkar/squad: Wikipedia reading-comprehension Q&A.
    Fields: id, title, context (passage), question, answers (dict: text/answer_start lists).
    Emit passage + Q + best answer so Lu learns both the fact and the answer span.
    """
    q_id        = example.get("id") or str(idx)
    title       = (example.get("title") or "").strip()
    context     = (example.get("context") or "").strip()
    question    = (example.get("question") or "").strip()
    ans_dict    = example.get("answers") or {}
    ans_texts   = ans_dict.get("text") or []
    best_answer = str(ans_texts[0]).strip() if ans_texts else ""

    parts = []
    if title:
        parts.append(f"[{title}]")
    if context:
        parts.append(context)
    if question and best_answer:
        parts.append(f"Q: {question}")
        parts.append(f"A: {best_answer}")

    return f"squad_{str(q_id)[:20]}_{idx}", "\n".join(parts)


def extract_the_stack_smol(example: dict, idx: int) -> tuple[str, str]:
    """
    bigcode/the-stack-smol: code samples across many programming languages.
    Fields: content, lang (language name).
    Prefix with language tag so Lu learns language-specific patterns.
    NOTE: gated dataset — replaced by orca-math in DATASETS/DATASET_ORDER.
    """
    content = (example.get("content") or "").strip()
    lang    = (example.get("lang") or example.get("language") or "").strip()

    if not content:
        return f"stack_{idx}", ""

    text = f"[{lang}]\n{content}" if lang else content
    return f"stack_{idx}", text


def extract_orca_math(example: dict, idx: int) -> tuple[str, str]:
    """
    microsoft/orca-math-word-problems-200k: 200K grade-school math word problems
    with step-by-step solutions generated by GPT-4.
    Fields: question (str), answer (str with full solution).
    """
    question = (example.get("question") or "").strip()
    answer   = (example.get("answer") or "").strip()

    if not question:
        return f"orcamath_{idx}", ""

    if answer:
        return f"orcamath_{idx}", f"Problem: {question}\nSolution: {answer}"
    return f"orcamath_{idx}", question


def extract_natural_questions(example: dict, idx: int) -> tuple[str, str]:
    """
    google-research-datasets/nq_open: real Google-search Q&A pairs.
    Fields: question (str), answer (list of accepted answer strings).
    """
    question = (example.get("question") or "").strip()
    answers  = example.get("answer") or []
    best_ans = str(answers[0]).strip() if answers else ""

    parts = []
    if question:
        parts.append(f"Q: {question}")
    if best_ans:
        parts.append(f"A: {best_ans}")

    return f"nq_{idx}", "\n".join(parts)


def extract_boolq(example: dict, idx: int) -> tuple[str, str]:
    """
    google/boolq: yes/no reading comprehension from Wikipedia passages.
    Fields: question, passage, answer (bool).
    Emit passage + Q + yes/no answer.
    """
    question = (example.get("question") or "").strip()
    passage  = (example.get("passage") or "").strip()
    answer   = example.get("answer")  # bool True/False

    answer_str = "Yes" if answer is True else ("No" if answer is False else "")

    parts = []
    if passage:
        parts.append(passage)
    if question:
        parts.append(f"Q: {question}")
    if answer_str:
        parts.append(f"A: {answer_str}")

    return f"boolq_{idx}", "\n".join(parts)


def extract_cosmos_qa(example: dict, idx: int) -> tuple[str, str]:
    """
    allenai/cosmos_qa: commonsense reading comprehension on everyday narratives.
    Fields: id, context, question, answer0..answer3, label (0-3).
    """
    q_id    = example.get("id") or str(idx)
    context = (example.get("context") or "").strip()
    question = (example.get("question") or "").strip()
    answers  = [
        (example.get("answer0") or "").strip(),
        (example.get("answer1") or "").strip(),
        (example.get("answer2") or "").strip(),
        (example.get("answer3") or "").strip(),
    ]
    label = example.get("label")

    correct = ""
    if label is not None:
        try:
            correct = answers[int(label)]
        except (IndexError, ValueError, TypeError):
            pass

    parts = []
    if context:
        parts.append(context)
    if question:
        parts.append(f"Q: {question}")
    if correct:
        parts.append(f"A: {correct}")

    return f"cosmos_{str(q_id)[:16]}_{idx}", "\n".join(parts)


def extract_apps(example: dict, idx: int) -> tuple[str, str]:
    """
    codeparrot/apps: competitive-programming problems with Python solutions.
    Fields: problem_id, question, solutions (JSON string of list), difficulty,
            input_output (JSON), starter_code, url, source.
    Emit problem description + first solution.
    """
    problem_id = example.get("problem_id") or str(idx)
    question   = (example.get("question") or "").strip()
    difficulty = (example.get("difficulty") or "").strip()
    solutions_raw = example.get("solutions") or ""

    # solutions is a JSON-encoded list of solution strings
    first_solution = ""
    if solutions_raw:
        try:
            import json as _json
            sols = _json.loads(solutions_raw)
            if sols and isinstance(sols, list):
                first_solution = str(sols[0]).strip()
        except Exception:
            pass

    parts = []
    if difficulty:
        parts.append(f"[Difficulty: {difficulty}]")
    if question:
        parts.append(question)
    if first_solution:
        parts.append(f"Solution:\n{first_solution}")

    return f"apps_{problem_id}_{idx}", "\n".join(parts)


def extract_oasst1(example: dict, idx: int) -> tuple[str, str]:
    """
    OpenAssistant/oasst1: human assistant dialogue tree, one message per row.
    Fields: message_id, parent_id, text, role (prompter/assistant), lang, rank.
    Filter to English and format role prefix so Lu understands turn structure.
    """
    msg_id = (example.get("message_id") or str(idx))[:20]
    role   = (example.get("role") or "prompter").strip()
    text   = (example.get("text") or "").strip()
    lang   = (example.get("lang") or "en").strip()

    # Skip non-English messages
    if lang != "en":
        return f"oasst_{msg_id}", ""

    prefix = "Human" if role == "prompter" else "Assistant"
    return f"oasst_{msg_id}", f"{prefix}: {text}" if text else ""


def extract_prosocial_dialog(example: dict, idx: int) -> tuple[str, str]:
    """
    allenai/prosocial-dialog: safety-aware dialogue with social norms.
    Fields: context (list of prior turns), response (safe reply),
            rots (list of rules-of-thumb), safety_label.
    """
    context  = example.get("context") or []
    response = (example.get("response") or "").strip()
    rots     = example.get("rots") or []

    parts = []
    for i, turn in enumerate(context):
        speaker = "A" if i % 2 == 0 else "B"
        t = str(turn).strip()
        if t:
            parts.append(f"{speaker}: {t}")
    if response:
        parts.append(f"B: {response}")
    if rots:
        norms = "; ".join(str(r) for r in rots[:3] if r)
        if norms:
            parts.append(f"[Social norms: {norms}]")

    return f"prosocial_{idx}", "\n".join(parts)


def extract_persona_chat(example: dict, idx: int) -> tuple[str, str]:
    """
    AlekseyKorshuk/persona-chat: persona-grounded multi-turn conversations.
    Fields: personality (list of persona sentences),
            utterances (list of {history: list, candidates: list}).
    Use the last utterance's full history + ground-truth response (last candidate).
    """
    personality = example.get("personality") or []
    utterances  = example.get("utterances") or []

    parts = []
    if personality:
        persona_str = " | ".join(str(p).strip() for p in personality if p)
        parts.append(f"[Persona: {persona_str}]")

    if utterances:
        last_utt = utterances[-1]
        history    = last_utt.get("history") or []
        candidates = last_utt.get("candidates") or []
        for i, turn in enumerate(history):
            speaker = "A" if i % 2 == 0 else "B"
            parts.append(f"{speaker}: {str(turn).strip()}")
        # Last candidate is the gold-standard response
        if candidates:
            parts.append(f"B: {str(candidates[-1]).strip()}")

    return f"personachat_{idx}", "\n".join(parts)


def extract_hh_rlhf(example: dict, idx: int) -> tuple[str, str]:
    """
    Anthropic/hh-rlhf: helpful/harmless preference dataset.
    Each row has 'chosen' and 'rejected' — full multi-turn dialogue strings.
    We use only 'chosen' (the preferred response), which is formatted as:
      "\n\nHuman: ...\n\nAssistant: ..."
    Strip the role labels and return as a clean conversation.
    """
    chosen = (example.get("chosen") or "").strip()
    if not chosen:
        return f"hhrlhf_{idx}", ""

    # chosen already is a readable conversation — just clean up double newlines
    text = chosen.replace("\n\nHuman:", "\nHuman:").replace("\n\nAssistant:", "\nAssistant:").strip()
    return f"hhrlhf_{idx}", text


def extract_dolly_15k(example: dict, idx: int) -> tuple[str, str]:
    """
    databricks/databricks-dolly-15k: human-written diverse instruction responses.
    Fields: instruction, context, response, category.
    Categories: open_qa, closed_qa, general_qa, classification, brainstorming,
                creative_writing, information_extraction, summarization.
    """
    instruction = (example.get("instruction") or "").strip()
    context     = (example.get("context") or "").strip()
    response    = (example.get("response") or "").strip()
    category    = (example.get("category") or "").strip()

    parts = []
    if category:
        parts.append(f"[{category}]")
    if instruction:
        parts.append(f"Task: {instruction}")
    if context:
        parts.append(f"Context: {context}")
    if response:
        parts.append(f"Response: {response}")

    return f"dolly_{idx}", "\n".join(parts)


def extract_stackexchange(example: dict, idx: int) -> tuple[str, str]:
    """
    sentence-transformers/stackexchange-duplicates (post-post config):
    Pairs of full post bodies from StackExchange (SO, ServerFault, Security.SE, etc.).
    Fields: post1, post2 (full HTML-stripped post bodies).
    Emit both posts as a technical Q&A pair.
    """
    post1 = (example.get("post1") or "").strip()
    post2 = (example.get("post2") or "").strip()

    parts = []
    if post1:
        parts.append(post1)
    if post2:
        parts.append(post2)

    return f"se_{idx}", "\n\n---\n\n".join(parts)


def extract_evol_codealpaca(example: dict, idx: int) -> tuple[str, str]:
    """
    theblackcat102/evol-codealpaca-v1: WizardCoder-style evolved code instructions.
    Fields: instruction, output.
    Covers system design, algorithms, networking, security, and game dev beyond syntax.
    """
    instruction = (example.get("instruction") or "").strip()
    output      = (example.get("output") or "").strip()

    parts = []
    if instruction:
        parts.append(f"Task: {instruction}")
    if output:
        parts.append(f"Solution:\n{output}")

    return f"evolca_{idx}", "\n".join(parts)


def extract_goodwiki(example: dict, idx: int) -> tuple[str, str]:
    """
    euirim/goodwiki: Featured and Good Wikipedia articles (highest editorial quality).
    Fields: title, description, categories (list), markdown (full article).
    The markdown field is the complete article text — strip heading markers.
    """
    title       = (example.get("title") or "").strip()
    description = (example.get("description") or "").strip()
    markdown    = (example.get("markdown") or "").strip()

    # Strip markdown heading syntax (## Title) to plain text headings
    markdown = re.sub(r'^#{1,6}\s+', '', markdown, flags=re.MULTILINE)

    parts = []
    if title:
        parts.append(title)
    if description and description != title:
        parts.append(description)
    if markdown:
        parts.append(markdown)

    doc_id = title.replace(" ", "_")[:50] if title else str(idx)
    return f"goodwiki_{doc_id}_{idx}", "\n\n".join(parts)


def extract_ag_news(example: dict, idx: int) -> tuple[str, str]:
    """
    fancyzhx/ag_news: news article snippets across four topic categories.
    Fields: text (article snippet ~200 words), label (int 0-3).
    Label map: 0=World, 1=Sports, 2=Business, 3=Sci/Tech.
    """
    text  = (example.get("text") or "").strip()
    label = example.get("label")

    label_names = {0: "World", 1: "Sports", 2: "Business", 3: "Sci/Tech"}
    category = label_names.get(label, "") if label is not None else ""

    if category and text:
        full_text = f"[{category}] {text}"
    else:
        full_text = text

    return f"agnews_{idx}", full_text


EXTRACTORS = {
    "dailydialog":           extract_dailydialog,
    "eli5":                  extract_eli5,
    "textbooks":             extract_textbooks,
    "tiny-textbooks":        extract_tiny_textbooks,
    "ultrachat":             extract_ultrachat,
    "science-facts":         extract_science_facts,
    "sciq":                  extract_sciq,
    "structured-wikipedia":  extract_structured_wikipedia,
    "openbookqa":            extract_openbookqa,
    "scienceqa":             extract_scienceqa,
    "code-alpaca":           extract_code_alpaca,
    "python-code":           extract_python_code,
    "code-feedback":         extract_code_feedback,
    "no-robots":             extract_no_robots,
    "evol-code":             extract_evol_code,
    # New datasets
    "gsm8k":                 extract_gsm8k,
    "arc-challenge":         extract_arc_challenge,
    "commonsense-qa":        extract_commonsense_qa,
    "triviaqa":              extract_triviaqa,
    "hotpotqa":              extract_hotpotqa,
    "mmlu":                  extract_mmlu,
    "boolq":                 extract_boolq,
    "piqa":                  extract_piqa,
    "social-iqa":            extract_social_iqa,
    # cosmos-qa / apps use deprecated loading scripts — kept for reference but
    # replaced in DATASETS/DATASET_ORDER with dolly-15k and orca-math respectively.
    "cosmos-qa":             extract_cosmos_qa,
    "apps":                  extract_apps,
    "hellaswag":             extract_hellaswag,
    "winogrande":            extract_winogrande,
    "wikitext":              extract_wikitext,
    "squad":                 extract_squad,
    "orca-math":             extract_orca_math,
    "natural-questions":     extract_natural_questions,
    # Conversational / social
    "oasst1":                extract_oasst1,
    "prosocial-dialog":      extract_prosocial_dialog,
    "persona-chat":          extract_persona_chat,
    "hh-rlhf":               extract_hh_rlhf,
    # Practical / how-to
    "dolly-15k":             extract_dolly_15k,
    # CS depth / technical
    "stackexchange":         extract_stackexchange,
    "evol-codealpaca":       extract_evol_codealpaca,
    # Encyclopedic / news
    "goodwiki":              extract_goodwiki,
    "ag-news":               extract_ag_news,
}


# ── CSV log ────────────────────────────────────────────────────────────────────

LOG_FIELDNAMES = ["dataset", "doc_id", "ingested_at", "ku_count"]


def load_ingested_ids(log_path: str) -> set[str]:
    """Load already-ingested doc IDs from the CSV log for resume capability."""
    if not os.path.isfile(log_path):
        return set()
    ingested = set()
    try:
        with open(log_path, "r", encoding="utf-8", newline="") as f:
            reader = csv.DictReader(f)
            for row in reader:
                ingested.add(row["doc_id"])
    except Exception as e:
        print(f"  WARNING: Could not read log file {log_path}: {e}")
    return ingested


def append_log(log_path: str, dataset: str, doc_id: str, ku_count: int):
    """Append a single ingested record to the CSV log."""
    write_header = not os.path.isfile(log_path)
    with open(log_path, "a", encoding="utf-8", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=LOG_FIELDNAMES)
        if write_header:
            writer.writeheader()
        writer.writerow({
            "dataset":     dataset,
            "doc_id":      doc_id,
            "ingested_at": datetime.now(timezone.utc).isoformat(),
            "ku_count":    ku_count,
        })


# ── Text chunking ──────────────────────────────────────────────────────────────

def chunk_text(text: str, max_len: int) -> list[str]:
    """Split text exceeding max_len at sentence boundaries."""
    if len(text) <= max_len:
        return [text]

    sentences = re.split(r'(?<=[.!?])\s+', text)
    chunks = []
    current = ""
    for sent in sentences:
        if len(current) + len(sent) + 1 <= max_len:
            current = (current + " " + sent).strip() if current else sent
        else:
            if current:
                chunks.append(current)
            # If a single sentence is itself too long, hard-split it
            if len(sent) > max_len:
                for i in range(0, len(sent), max_len):
                    chunks.append(sent[i:i + max_len])
                current = ""
            else:
                current = sent
    if current:
        chunks.append(current)
    return chunks


# ── MCP client ─────────────────────────────────────────────────────────────────

class McpClient:
    """Thin JSON-RPC 2.0 client for lu_mcp_v2.exe over stdio."""

    def __init__(self, exe_path: str, brain_path: str):
        self.proc = subprocess.Popen(
            [exe_path, brain_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            bufsize=0,
        )
        self._q: queue.Queue = queue.Queue()
        self._req_id = 0
        self._lock = threading.Lock()

        self._reader = threading.Thread(target=self._read_loop, daemon=True)
        self._reader.start()
        self._err_reader = threading.Thread(target=self._err_loop, daemon=True)
        self._err_reader.start()

        time.sleep(0.8)
        if self.proc.poll() is not None:
            try:
                stderr_out = self.proc.stderr.read(2000).decode("utf-8", errors="replace")
            except Exception:
                stderr_out = ""
            raise RuntimeError(
                f"lu_mcp_v2.exe exited immediately (code {self.proc.returncode}). "
                f"stderr: {stderr_out}"
            )

    def _read_loop(self):
        try:
            for raw in self.proc.stdout:
                line = raw.decode("utf-8", errors="replace").strip()
                if line:
                    try:
                        self._q.put(json.loads(line))
                    except json.JSONDecodeError:
                        pass
        except Exception:
            pass

    def _err_loop(self):
        """Capture last 20 stderr lines for crash diagnostics."""
        self._stderr_lines: list[str] = []
        try:
            for raw in self.proc.stderr:
                line = raw.decode("utf-8", errors="replace").rstrip()
                self._stderr_lines.append(line)
                if len(self._stderr_lines) > 20:
                    self._stderr_lines.pop(0)
        except Exception:
            pass

    def get_stderr_tail(self) -> str:
        lines = getattr(self, "_stderr_lines", [])
        return "\n".join(lines) if lines else "(no stderr captured)"

    def _next_id(self) -> int:
        with self._lock:
            self._req_id += 1
            return self._req_id

    def send(self, method: str, params: dict, timeout: float = 60.0) -> dict:
        req_id = self._next_id()
        request = {
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params,
        }
        data = json.dumps(request, ensure_ascii=False) + "\n"
        try:
            self.proc.stdin.write(data.encode("utf-8"))
            self.proc.stdin.flush()
        except (OSError, BrokenPipeError) as e:
            return {"error": {"message": str(e)}}

        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                resp = self._q.get(timeout=min(5.0, deadline - time.time()))
                if resp.get("id") == req_id:
                    return resp
                else:
                    self._q.put(resp)
            except queue.Empty:
                pass
        return {"error": {"message": "timeout"}}

    def notify(self, method: str, params: dict):
        msg = {"jsonrpc": "2.0", "method": method, "params": params}
        data = json.dumps(msg, ensure_ascii=False) + "\n"
        try:
            self.proc.stdin.write(data.encode("utf-8"))
            self.proc.stdin.flush()
        except Exception:
            pass

    def tool_call(self, name: str, arguments: dict, timeout: float = 30.0) -> dict:
        return self.send("tools/call", {"name": name, "arguments": arguments}, timeout=timeout)

    def close(self):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.wait(timeout=10)
        except Exception:
            self.proc.kill()


def extract_result_text(resp: dict) -> str:
    if not resp or "result" not in resp:
        err = resp.get("error", {})
        return f"[error] {err.get('message', str(resp))}"
    result = resp["result"]
    if isinstance(result, dict) and "content" in result:
        parts = []
        for item in result["content"]:
            if item.get("type") == "text":
                parts.append(item.get("text", ""))
        return " | ".join(parts) if parts else str(result)
    return str(result)


# ── MCP helpers ────────────────────────────────────────────────────────────────

def init_mcp(client: McpClient):
    resp = client.send("initialize", {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "ingest-hf-conversations", "version": "1.0"},
    })
    if resp.get("error"):
        print(f"  WARNING: initialize error: {resp['error']}")
    else:
        print("  Session initialized OK")

    client.notify("notifications/initialized", {})
    time.sleep(0.3)

    # Wait for brain to finish loading before teaching.
    # The Rust MCP loads the brain synchronously — while loading it does NOT read stdin.
    # Sending MCP requests during this time fills the stdin pipe buffer and blocks Python.
    # Strategy: sleep without writing to stdin, then send lu_configure once after the delay.
    brain_load_wait = 120  # seconds; 3.3M-node brain takes ~60-90s on this machine
    poll_interval = 10
    print(f"  Waiting {brain_load_wait}s for brain to load (no MCP calls during load)...", flush=True)
    for elapsed in range(0, brain_load_wait, poll_interval):
        if client.proc.poll() is not None:
            raise RuntimeError(f"MCP server exited during brain load (code {client.proc.returncode})")
        print(f"  ... {elapsed}s elapsed", flush=True)
        time.sleep(poll_interval)
    print(f"  Brain load wait complete ({brain_load_wait}s). Configuring...", flush=True)

    resp = client.tool_call("lu_configure", {"autonomy_tier": "Autonomous"}, timeout=60.0)
    result = extract_result_text(resp)
    print(f"  Configure: {result}", flush=True)
    if "still loading" in result.lower() or "loading" in result.lower():
        print(f"  WARNING: Brain still loading after {brain_load_wait}s. Proceeding anyway.", flush=True)


def teach(client: McpClient, text: str, verbose: bool = False) -> int:
    """Send a single text chunk to lu_teach. Returns KU count."""
    resp = client.tool_call("lu_teach", {
        "ku_type":    "Raw",
        "content":    text,
        "confidence": "Terminal",
    }, timeout=30.0)

    result_text = extract_result_text(resp)
    if verbose:
        print(f"      -> {result_text}")

    if result_text and "[error]" not in result_text:
        m = re.search(r'Learned\s+(\d+)', result_text or "")
        return int(m.group(1)) if m else 1
    return 0


def save_brain(client: McpClient) -> str:
    resp = client.tool_call("lu_save", {}, timeout=600.0)
    return extract_result_text(resp) or "saved"


# ── Dataset streaming ──────────────────────────────────────────────────────────

_HF_LOAD_TIMEOUT = 120  # seconds; HF Hub can be slow/rate-limited for anonymous users


def _load_dataset_timed(hf_path: str, load_kwargs: dict, timeout: int = _HF_LOAD_TIMEOUT):
    """Call load_dataset() on a daemon thread with a timeout.
    Raises TimeoutError if HF Hub doesn't respond within *timeout* seconds.
    """
    from datasets import load_dataset

    result: list = [None]
    error:  list = [None]

    def _load():
        try:
            result[0] = load_dataset(hf_path, **load_kwargs)
        except Exception as exc:
            error[0] = exc

    t = threading.Thread(target=_load, daemon=True)
    t.start()
    t.join(timeout=timeout)
    if t.is_alive():
        raise TimeoutError(
            f"load_dataset('{hf_path}') timed out after {timeout}s "
            "(HF Hub may be rate-limiting anonymous requests)"
        )
    if error[0] is not None:
        raise error[0]
    return result[0]


def stream_dataset(name: str, cfg: dict):
    """
    Yield (idx, example) tuples from a HuggingFace dataset using streaming.
    Handles both streaming IterableDatasets and regular Datasets.
    """
    # Import datasets library (pandas/WMI hang is pre-empted by the platform._wmi=None
    # workaround applied at module load time).  Keep a daemon-thread wrapper as a
    # safety net in case some other blocking import slips through.
    _ld_result: list = [None]
    _ld_error:  list = [None]

    def _do_import():
        try:
            from datasets import load_dataset as _fn  # noqa
            _ld_result[0] = _fn
        except ImportError as exc:
            _ld_error[0] = exc

    _imp_thread = threading.Thread(target=_do_import, daemon=True)
    _imp_thread.start()
    _imp_thread.join(timeout=60)

    if _imp_thread.is_alive():
        os.write(1, b"  SKIP: 'datasets' import timed out after 60s\n")
        return
    if _ld_error[0] is not None:
        os.write(1, f"  ERROR: 'datasets' not installed: {_ld_error[0]}\n".encode())
        sys.exit(1)

    load_dataset = _ld_result[0]

    hf_path = cfg["hf_path"]
    split   = cfg["split"]
    streaming = cfg.get("streaming", True)

    print(f"  Loading {hf_path} (split={split}, streaming={streaming})...", flush=True)

    # Newer datasets library (3.x) removed trust_remote_code; omit it.
    load_kwargs = dict(split=split, streaming=streaming)

    # Some datasets require a config name (e.g. wikimedia/structured-wikipedia)
    hf_config = cfg.get("hf_config")
    if hf_config:
        load_kwargs["name"] = hf_config

    try:
        ds = _load_dataset_timed(hf_path, load_kwargs)
    except TimeoutError as e:
        print(f"  SKIP: {e}", flush=True)
        return
    except Exception as e:
        print(f"  ERROR: Could not load {hf_path}: {e}", flush=True)
        return

    for idx, example in enumerate(ds):
        yield idx, example


# ── Per-dataset ingestion ──────────────────────────────────────────────────────

def ingest_dataset(
    name: str,
    client: McpClient | None,
    ingested_ids: set[str],
    args,
) -> dict:
    """
    Stream and ingest a single dataset.
    Returns stats dict.
    """
    cfg      = DATASETS[name]
    extractor = EXTRACTORS[name]

    print(f"\n{'─' * 60}", flush=True)
    print(f"  Dataset: {name}", flush=True)
    print(f"  {cfg['description']}  ({cfg['size_hint']})", flush=True)
    print(f"{'─' * 60}", flush=True)

    stats = {
        "seen": 0,
        "skipped_resume": 0,
        "skipped_short": 0,
        "ingested": 0,
        "ku_total": 0,
        "errors": 0,
    }

    # Determine effective per-dataset limit:
    # dataset-level max_examples takes precedence unless --limit is tighter.
    dataset_max = cfg.get("max_examples", 0)
    if args.limit and dataset_max:
        effective_limit = min(args.limit, dataset_max)
    elif args.limit:
        effective_limit = args.limit
    else:
        effective_limit = dataset_max  # 0 means unlimited

    start_time = time.time()
    last_save_at = 0
    last_stat_at = time.time()

    for idx, example in stream_dataset(name, cfg):
        stats["seen"] += 1

        # Extract text from this dataset's format
        try:
            doc_id, text = extractor(example, idx)
        except Exception as e:
            stats["errors"] += 1
            if args.verbose:
                print(f"  WARNING: extraction error at idx {idx}: {e}")
            continue

        # Resume: skip already-ingested docs
        if doc_id in ingested_ids:
            stats["skipped_resume"] += 1
            continue

        text = text.strip()
        if len(text) < args.min_len:
            stats["skipped_short"] += 1
            continue

        # Split long texts into chunks
        chunks = chunk_text(text, args.max_len)

        if args.dry_run:
            print(f"  [{name}:{idx}] {text[:120]}{'...' if len(text) > 120 else ''}")
            stats["ingested"] += 1
            append_log(LOG_CSV, name, doc_id, 0)
        else:
            # Check MCP is still alive before teaching
            if client.proc.poll() is not None:
                print(f"  ERROR: MCP server died (exit {client.proc.returncode}). Stopping dataset.", flush=True)
                print(f"  MCP stderr tail:\n{client.get_stderr_tail()}", flush=True)
                break

            ku_total_for_doc = 0
            mcp_died = False
            for chunk in chunks:
                if len(chunk) < args.min_len:
                    continue
                ku = teach(client, chunk, verbose=args.verbose)
                # If MCP died mid-chunk, stop immediately — don't log this doc
                if client.proc.poll() is not None:
                    print(f"  ERROR: MCP server died mid-teach (exit {client.proc.returncode}). Stopping dataset.", flush=True)
                    print(f"  MCP stderr tail:\n{client.get_stderr_tail()}", flush=True)
                    mcp_died = True
                    break
                ku_total_for_doc += ku
                if ku == 0:
                    stats["errors"] += 1
                stats["ku_total"] += ku
                time.sleep(args.rate_limit)

            if mcp_died:
                break

            stats["ingested"] += 1
            append_log(LOG_CSV, name, doc_id, ku_total_for_doc)

            # Periodic save
            if stats["ingested"] - last_save_at >= args.save_every:
                save_result = save_brain(client)
                last_save_at = stats["ingested"]
                print(f"  [SAVE at {stats['ingested']}] {save_result}", flush=True)
                # If save failed with a pipe error, MCP may have died
                if "[error]" in save_result and client.proc.poll() is not None:
                    print(f"  ERROR: MCP died during save. Stopping dataset.", flush=True)
                    break

        # Progress stats every 10 seconds
        now = time.time()
        if now - last_stat_at >= 10.0:
            elapsed = now - start_time
            rate = stats["ingested"] / elapsed if elapsed > 0 else 0
            ku_rate = stats["ku_total"] / elapsed if elapsed > 0 else 0
            print(
                f"  [{name}] seen={stats['seen']}  ingested={stats['ingested']}  "
                f"skipped(resume={stats['skipped_resume']}, short={stats['skipped_short']})  "
                f"KUs={stats['ku_total']}  "
                f"rate={rate:.1f} doc/s  ku_rate={ku_rate:.1f} ku/s",
                flush=True,
            )
            last_stat_at = now

        if effective_limit and stats["ingested"] >= effective_limit:
            src = "max_examples" if dataset_max and not args.limit else "--limit"
            print(f"  Reached {src} {effective_limit}, stopping {name}.")
            break

    # Final stats for this dataset
    elapsed = time.time() - start_time
    rate = stats["ingested"] / elapsed if elapsed > 0 else 0
    print(f"\n  [{name}] DONE — "
          f"seen={stats['seen']}  ingested={stats['ingested']}  "
          f"errors={stats['errors']}  KUs={stats['ku_total']}  "
          f"time={elapsed:.0f}s  rate={rate:.1f} doc/s", flush=True)

    return stats


# ── Argument parsing ───────────────────────────────────────────────────────────

def parse_args():
    p = argparse.ArgumentParser(
        description="Ingest HuggingFace datasets into Lu Brain v2.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument(
        "--dataset", choices=list(DATASETS.keys()),
        help="Ingest only this dataset (default: all in priority order)",
    )
    p.add_argument("--brain", default=BRAIN_PATH, help="Path to the .lbr brain file")
    p.add_argument("--dry-run", action="store_true",
                   help="Print docs without MCP ingestion")
    p.add_argument("--limit", type=int, default=0, metavar="N",
                   help="Stop after N docs per dataset (0 = unlimited)")
    p.add_argument("--save-every", type=int, default=DEFAULT_SAVE_EVERY, metavar="N",
                   help=f"Save brain every N docs (default: {DEFAULT_SAVE_EVERY})")
    p.add_argument("--min-len", type=int, default=DEFAULT_MIN_LEN, metavar="N",
                   help=f"Minimum text length (default: {DEFAULT_MIN_LEN})")
    p.add_argument("--max-len", type=int, default=DEFAULT_MAX_LEN, metavar="N",
                   help=f"Maximum text length before chunking (default: {DEFAULT_MAX_LEN})")
    p.add_argument("--rate-limit", type=float, default=DEFAULT_RATE_LIMIT, metavar="F",
                   help=f"Seconds between MCP calls (default: {DEFAULT_RATE_LIMIT})")
    p.add_argument("--verbose", action="store_true",
                   help="Print each doc and MCP response")
    return p.parse_args()


# ── Main ───────────────────────────────────────────────────────────────────────

def main():
    args = parse_args()

    targets = [args.dataset] if args.dataset else DATASET_ORDER

    print("=" * 70)
    print("  LU BRAIN v2 — HUGGINGFACE CONVERSATIONS INGESTION")
    print("=" * 70)
    print(f"  Datasets:   {', '.join(targets)}")
    print(f"  Brain:      {args.brain}")
    print(f"  Log CSV:    {LOG_CSV}")
    print(f"  Min len:    {args.min_len} chars")
    print(f"  Max len:    {args.max_len} chars")
    print(f"  Rate limit: {args.rate_limit}s between calls")
    print(f"  Save every: {args.save_every} docs")
    print(f"  Dry run:    {args.dry_run}")
    if args.limit:
        print(f"  Limit:      {args.limit} docs/dataset")
    print()

    # Load resume state from CSV
    print("[1/4] Loading ingestion log (resume state)...")
    ingested_ids = load_ingested_ids(LOG_CSV)
    print(f"  Already ingested: {len(ingested_ids)} docs")

    # Start MCP server
    client = None
    if not args.dry_run:
        if not os.path.isfile(MCP_EXE):
            print(f"ERROR: MCP server not found: {MCP_EXE}")
            sys.exit(1)

        print("\n[2/4] Starting MCP server...")
        try:
            client = McpClient(MCP_EXE, args.brain)
        except RuntimeError as e:
            print(f"  FATAL: {e}")
            sys.exit(1)
        print(f"  MCP server running (PID {client.proc.pid})")
        init_mcp(client)
    else:
        print("[2/4] DRY RUN — MCP server not started")

    # Ingest datasets
    print(f"\n[3/4] Ingesting {len(targets)} dataset(s)...")
    grand_total = {"seen": 0, "ingested": 0, "ku_total": 0, "errors": 0}
    pipeline_start = time.time()

    for name in targets:
        ds_stats = ingest_dataset(name, client, ingested_ids, args)
        for k in grand_total:
            grand_total[k] += ds_stats.get(k, 0)

    # Final save + status
    if not args.dry_run and client:
        print(f"\n[4/4] Final save...")
        save_result = save_brain(client)
        print(f"  {save_result}")

        resp = client.tool_call("lu_status", {}, timeout=30.0)
        print(f"  Status: {extract_result_text(resp)}")

        client.close()

    # Summary
    elapsed = time.time() - pipeline_start
    print(f"\n{'=' * 70}")
    print(f"  INGESTION COMPLETE")
    print(f"{'=' * 70}")
    print(f"  Datasets processed: {len(targets)}")
    print(f"  Docs seen:          {grand_total['seen']}")
    print(f"  Docs ingested:      {grand_total['ingested']}")
    if not args.dry_run:
        print(f"  KUs learned:        {grand_total['ku_total']}")
        print(f"  Errors:             {grand_total['errors']}")
    print(f"  Total time:         {elapsed:.0f}s ({elapsed / 60:.1f}m)")
    if grand_total["ingested"] > 0 and elapsed > 0:
        print(f"  Throughput:         {grand_total['ingested'] / elapsed:.1f} doc/s")
    print(f"{'=' * 70}")


if __name__ == "__main__":
    main()
