#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Conversational Foundations Training Pipeline for Lu Brain v2.

Trains Lu on the *structure* of natural conversation using curated
HuggingFace dialogue datasets.  Each dataset is annotated with
conversational metadata so Lu's associative memory can form bonds
between dialogue patterns:

  - Turn-taking (greeting → response → follow-up → closing)
  - Dialogue acts (question, inform, directive, commissive)
  - Emotion / empathy cues
  - Topic management (introduce, transition, stay-on-topic)
  - Multi-turn coherence (turn numbers, speaker roles)
  - Persona consistency
  - Social norms and prosocial behaviour

Datasets (in priority order):
  1. dailydialog        — 13K dialogues with emotion + dialogue-act labels
  2. empathetic         — 25K empathy-grounded conversations
  3. persona-chat       — 18K persona-grounded multi-turn dialogues
  4. blended-skill-talk — 5K conversations blending knowledge+empathy+persona
  5. prosocial-dialog   — 58K safety-aware dialogues with social norms
  6. multiwoz           — 10K task-oriented dialogues (booking, info, etc.)
  7. oasst1            — 84K human assistant dialogue messages (English)
  8. hh-rlhf           — 170K helpful/harmless preference dialogues
  9. ultrachat         — 207K multi-turn synthetic conversations
 10. wizard-of-wiki    — 22K knowledge-grounded dialogues

Usage:
    python train_conversations.py                           # all datasets
    python train_conversations.py --dataset dailydialog     # single dataset
    python train_conversations.py --dry-run                 # preview, no MCP
    python train_conversations.py --limit 500               # cap per dataset
    python train_conversations.py --delay 0.1               # slower pace
    python train_conversations.py --verbose                 # see every doc

Options:
    --dataset NAME     One of the dataset names above
    --dry-run          Print formatted docs without sending to Lu
    --limit N          Stop after N docs per dataset (0 = unlimited)
    --delay F          Seconds between MCP calls (default: 0.05)
    --save-every N     Save brain every N docs (default: 200)
    --min-len N        Minimum text length in chars (default: 80)
    --max-len N        Maximum text length in chars (default: 4000)
    --brain PATH       Override brain file path
    --verbose          Print each doc and MCP response

Dependencies:
    pip install datasets --break-system-packages

MCP binary: <AI_FACTORY_ROOT>\\v2\\target\\release\\lu_mcp_v2.exe
Brain file: <AI_FACTORY_ROOT>\\v2\\brain\\lu_brain.lbr
Log file:   v2/tools/conversation_training_log.csv
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

# ── Windows fixes ──────────────────────────────────────────────────────────────
# Force UTF-8 output so emoji/unicode in conversations don't crash the script.
if sys.stdout.encoding and sys.stdout.encoding.lower() not in ("utf-8", "utf8"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
if sys.stderr.encoding and sys.stderr.encoding.lower() not in ("utf-8", "utf8"):
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace")

# Python 3.14 WMI hang workaround — platform.machine() can deadlock.
# Must run before any pandas/datasets import.
try:
    import platform as _platform_mod
    _platform_mod._wmi = None
    del _platform_mod
except Exception:
    pass

# IPv4 preference — HuggingFace DNS returns IPv6 which may hang on some networks.
# Sort IPv4 addresses first so connections succeed immediately.
try:
    import socket as _sock_patch
    _orig_getaddrinfo = _sock_patch.getaddrinfo
    _AF_INET = _sock_patch.AF_INET

    def _getaddrinfo_ipv4_first(host, port, family=0, type=0, proto=0, flags=0):
        results = _orig_getaddrinfo(host, port, family=family, type=type,
                                    proto=proto, flags=flags)
        return sorted(results, key=lambda r: 0 if r[0] == _AF_INET else 1)

    _sock_patch.getaddrinfo = _getaddrinfo_ipv4_first
    del _sock_patch
except Exception:
    pass

# Kill any stale lu_mcp_v2.exe to avoid brain-file lock conflicts.
try:
    subprocess.run(
        ["taskkill", "/F", "/IM", "lu_mcp_v2.exe"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        timeout=10,
    )
    print("[guard] Killed existing lu_mcp_v2.exe process(es).")
except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
    pass  # no existing process — that's fine

# ── Config ─────────────────────────────────────────────────────────────────────

MCP_EXE    = r"<AI_FACTORY_ROOT>\v2\target\release\lu_mcp_v2.exe"
BRAIN_PATH = r"<AI_FACTORY_ROOT>\v2\brain\lu_brain.lbr"

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
LOG_CSV    = os.path.join(SCRIPT_DIR, "conversation_training_log.csv")

DEFAULT_MIN_LEN    = 80     # skip very short texts
DEFAULT_MAX_LEN    = 4000   # chunk texts longer than this
DEFAULT_SAVE_EVERY = 200    # save brain every N docs
DEFAULT_DELAY      = 0.05   # seconds between MCP calls (~20/sec)

# ── Dialogue-act labels (DailyDialog uses integer codes) ───────────────────────
# These labels tell Lu what "kind" of conversational move each utterance is.
DIALOGUE_ACTS = {
    0: "no-act",       # not labeled
    1: "inform",       # stating information
    2: "question",     # asking something
    3: "directive",    # requesting/commanding action
    4: "commissive",   # promising/offering
}

# DailyDialog emotion labels
EMOTIONS = {
    0: "neutral",
    1: "anger",
    2: "disgust",
    3: "fear",
    4: "happiness",
    5: "sadness",
    6: "surprise",
}

# ── Dataset registry ───────────────────────────────────────────────────────────
# Each entry describes one HuggingFace dataset:
#   hf_path      — the HuggingFace dataset identifier
#   split        — which split to use (usually "train")
#   streaming    — True to stream instead of downloading all at once
#   description  — human-readable summary
#   size_hint    — approximate download size
#   max_examples — optional cap on examples from this dataset (0 = unlimited)

DATASETS = {
    "dailydialog": {
        "hf_path": "OpenRL/daily_dialog",
        "split": "train",
        "streaming": True,
        "description": "DailyDialog — 13K conversations with dialogue-act + emotion labels",
        "size_hint": "~11 MB",
    },
    "empathetic": {
        "hf_path": "facebook/empathetic_dialogues",
        "split": "train",
        "streaming": True,
        "description": "EmpatheticDialogues — 25K emotion-grounded conversations",
        "size_hint": "~30 MB",
    },
    "persona-chat": {
        "hf_path": "AlekseyKorshuk/persona-chat",
        "split": "train",
        "streaming": True,
        "description": "Persona-Chat — 18K persona-grounded multi-turn dialogues",
        "size_hint": "~10 MB",
    },
    "blended-skill-talk": {
        "hf_path": "facebook/blended_skill_talk",
        "split": "train",
        "streaming": True,
        "description": "Blended Skill Talk — 5K convos blending knowledge + empathy + persona",
        "size_hint": "~40 MB",
    },
    "prosocial-dialog": {
        "hf_path": "allenai/prosocial-dialog",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "ProSocial Dialog — 58K safety-aware dialogues with social norms",
        "size_hint": "~80 MB",
    },
    "multiwoz": {
        "hf_path": "pfb30/multi_woz_v22",
        "split": "train",
        "streaming": True,
        "max_examples": 10000,
        "description": "MultiWOZ 2.2 — 10K task-oriented dialogues (hotels, restaurants, etc.)",
        "size_hint": "~50 MB",
    },
    "oasst1": {
        "hf_path": "OpenAssistant/oasst1",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "OpenAssistant OASST1 — 84K human assistant dialogue messages (English)",
        "size_hint": "~40 MB",
    },
    "hh-rlhf": {
        "hf_path": "Anthropic/hh-rlhf",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "Anthropic HH-RLHF — 170K helpful/harmless preference dialogues",
        "size_hint": "~150 MB",
    },
    "ultrachat": {
        "hf_path": "HuggingFaceH4/ultrachat_200k",
        "split": "train_sft",
        "streaming": True,
        "max_examples": 50000,
        "description": "UltraChat 200K — 207K multi-turn synthetic conversations",
        "size_hint": "~3 GB",
    },
    "wizard-of-wiki": {
        "hf_path": "wiki_dialog",
        "hf_config": "en",
        "split": "train",
        "streaming": True,
        "max_examples": 50000,
        "description": "WikiDialog EN — 11M knowledge-grounded Q&A dialogue turns",
        "size_hint": "~7 GB",
    },
}

# The order datasets are trained in — small/high-quality first.
DATASET_ORDER = [
    "dailydialog",          # best: has dialogue-act + emotion labels
    "empathetic",           # teaches emotional awareness
    "persona-chat",         # teaches persona consistency
    "blended-skill-talk",   # blends multiple conversation skills
    "prosocial-dialog",     # teaches social norms
    "multiwoz",             # task-oriented dialogue structure
    "oasst1",               # human-written assistant dialogue
    "hh-rlhf",              # helpful/harmless conversation patterns
    "ultrachat",            # large-scale multi-turn
    "wizard-of-wiki",       # knowledge-grounded dialogue
]


# ══════════════════════════════════════════════════════════════════════════════
#  EXTRACTORS — one function per dataset
#
#  Each extractor takes (example_dict, index) and returns (doc_id, text).
#  The text is annotated with conversational metadata tags so Lu can form
#  associations between dialogue patterns, emotions, and turn structures.
#
#  Tags used:
#    [CONV]              — marks this as a conversation (vs. factual text)
#    [TURN N]            — turn number for multi-turn coherence
#    [ACT: inform]       — dialogue act label
#    [EMO: happiness]    — emotion label
#    [TOPIC: ...]        — topic/context marker
#    [PERSONA: ...]      — speaker personality traits
#    [NORM: ...]         — social norm / rule of thumb
#    [TASK: ...]         — task-oriented dialogue domain
#    Speaker A / B       — alternating speakers
#    Human / Assistant   — for assistant-style dialogues
# ══════════════════════════════════════════════════════════════════════════════


def extract_dailydialog(example: dict, idx: int) -> tuple:
    """
    DailyDialog: rich conversational data with TWO label tracks:
      - dialog:  list of utterance strings
      - act:     list of ints (1=inform, 2=question, 3=directive, 4=commissive)
      - emotion: list of ints (0=neutral .. 6=surprise)

    We annotate each turn with its act and emotion so Lu learns
    what KIND of conversational move each utterance represents.
    """
    dialog   = example.get("dialog") or example.get("dialogue") or []
    acts     = example.get("act") or []
    emotions = example.get("emotion") or []

    if not dialog:
        return str(idx), ""

    lines = ["[CONV]"]
    for i, utterance in enumerate(dialog):
        speaker = "A" if i % 2 == 0 else "B"
        utt = str(utterance).strip()
        if not utt:
            continue

        # Build metadata tags for this turn
        tags = []
        tags.append(f"[TURN {i+1}]")
        if i < len(acts) and acts[i] in DIALOGUE_ACTS:
            tags.append(f"[ACT: {DIALOGUE_ACTS[acts[i]]}]")
        if i < len(emotions) and emotions[i] in EMOTIONS:
            emo = EMOTIONS[emotions[i]]
            if emo != "neutral":  # only tag non-neutral emotions
                tags.append(f"[EMO: {emo}]")

        tag_str = " ".join(tags)
        lines.append(f"{tag_str} {speaker}: {utt}")

    return f"dailydialog_{idx}", "\n".join(lines)


def extract_empathetic(example: dict, idx: int) -> tuple:
    """
    EmpatheticDialogues: conversations grounded in a specific emotion.
    Fields:
      - context:          the emotion-triggering situation description
      - prompt:           the speaker's opening line
      - utterance:        the listener's empathetic response
      - selfeval:         self-evaluation of the response
      - tags:             emotion label (e.g. "grateful", "anxious")
      - conv_id:          conversation ID
      - utterance_idx:    turn index within the conversation

    Each row is ONE turn, not a full conversation.  We format it as
    a mini dialogue snippet with the emotion context.
    """
    conv_id  = example.get("conv_id") or str(idx)
    context  = (example.get("context") or "").strip()
    prompt   = (example.get("prompt") or "").strip()
    utterance = (example.get("utterance") or "").strip()
    emotion  = (example.get("tags") or "").strip()
    turn_idx = example.get("utterance_idx", 0)

    lines = ["[CONV]"]
    if emotion:
        lines.append(f"[EMO: {emotion}]")
    if context:
        lines.append(f"[TOPIC: {context}]")
    if prompt:
        lines.append(f"[TURN {turn_idx}] A: {prompt}")
    if utterance:
        lines.append(f"[TURN {turn_idx + 1}] B: {utterance}")

    return f"empathetic_{conv_id}_{turn_idx}", "\n".join(lines)


def extract_persona_chat(example: dict, idx: int) -> tuple:
    """
    Persona-Chat: conversations where each speaker has a defined persona.
    Fields:
      - personality:  list of persona trait sentences
      - utterances:   list of {history: [...], candidates: [...]}

    We include the persona traits so Lu learns that speakers have
    consistent personalities that influence their responses.
    """
    personality = example.get("personality") or []
    utterances  = example.get("utterances") or []

    lines = ["[CONV]"]

    # Add persona traits as context
    if personality:
        persona_str = " | ".join(str(p).strip() for p in personality if p)
        lines.append(f"[PERSONA: {persona_str}]")

    # Use the last utterance block (has the fullest history)
    if utterances:
        last_utt   = utterances[-1]
        history    = last_utt.get("history") or []
        candidates = last_utt.get("candidates") or []

        for i, turn in enumerate(history):
            speaker = "A" if i % 2 == 0 else "B"
            lines.append(f"[TURN {i+1}] {speaker}: {str(turn).strip()}")

        # Last candidate is the gold-standard response
        if candidates:
            turn_num = len(history) + 1
            lines.append(f"[TURN {turn_num}] B: {str(candidates[-1]).strip()}")

    return f"personachat_{idx}", "\n".join(lines)


def extract_blended_skill_talk(example: dict, idx: int) -> tuple:
    """
    Blended Skill Talk: conversations that blend three skills:
      - Knowledge (Wizard of Wikipedia)
      - Empathy (EmpatheticDialogues)
      - Personality (Persona-Chat)

    Fields:
      - personas:           list of persona sentences for each speaker
      - additional_context: topic sentence from Wizard of Wikipedia
      - previous_utterance: list of prior turns
      - free_messages:      list of unconstrained responses
      - guided_messages:    list of guided (skill-blended) responses
    """
    personas = example.get("personas") or []
    context  = (example.get("additional_context") or "").strip()
    prev     = example.get("previous_utterance") or []
    free_msg = example.get("free_messages") or []
    guided   = example.get("guided_messages") or []

    lines = ["[CONV]"]

    if personas:
        for i, p_list in enumerate(personas):
            if isinstance(p_list, list):
                p_str = " | ".join(str(s).strip() for s in p_list if s)
            else:
                p_str = str(p_list).strip()
            if p_str:
                speaker = "A" if i == 0 else "B"
                lines.append(f"[PERSONA {speaker}: {p_str}]")

    if context:
        lines.append(f"[TOPIC: {context}]")

    # Previous utterances are the conversation history
    turn_num = 1
    for utt in prev:
        speaker = "A" if turn_num % 2 == 1 else "B"
        lines.append(f"[TURN {turn_num}] {speaker}: {str(utt).strip()}")
        turn_num += 1

    # Prefer guided messages (skill-blended) over free messages
    responses = guided if guided else free_msg
    for msg in responses:
        speaker = "A" if turn_num % 2 == 1 else "B"
        lines.append(f"[TURN {turn_num}] {speaker}: {str(msg).strip()}")
        turn_num += 1

    return f"bst_{idx}", "\n".join(lines)


def extract_prosocial_dialog(example: dict, idx: int) -> tuple:
    """
    ProSocial Dialog: safety-aware dialogue with social norm annotations.
    Fields:
      - context:      list of prior conversation turns
      - response:     the prosocial (safe, appropriate) response
      - rots:         "rules of thumb" — social norms that apply
      - safety_label: how safe/unsafe the context is
    """
    context  = example.get("context") or []
    response = (example.get("response") or "").strip()
    rots     = example.get("rots") or []
    safety   = (example.get("safety_label") or "").strip()

    lines = ["[CONV]"]

    # Social norms provide the "why" behind appropriate responses
    if rots:
        norms = "; ".join(str(r).strip() for r in rots[:3] if r)
        if norms:
            lines.append(f"[NORM: {norms}]")

    # Conversation turns
    for i, turn in enumerate(context):
        speaker = "A" if i % 2 == 0 else "B"
        t = str(turn).strip()
        if t:
            lines.append(f"[TURN {i+1}] {speaker}: {t}")

    # The prosocial response
    if response:
        turn_num = len(context) + 1
        lines.append(f"[TURN {turn_num}] B: {response}")

    return f"prosocial_{idx}", "\n".join(lines)


def extract_multiwoz(example: dict, idx: int) -> tuple:
    """
    MultiWOZ 2.2: task-oriented dialogue (booking hotels, restaurants, etc.).
    Fields:
      - dialogue_id:  unique conversation ID
      - services:     list of domains involved (hotel, restaurant, taxi, etc.)
      - turns:        dict with 'speaker' list and 'utterance' list
    """
    dlg_id   = example.get("dialogue_id") or str(idx)
    services = example.get("services") or []
    turns    = example.get("turns") or {}

    speakers   = turns.get("speaker") or []
    utterances = turns.get("utterance") or []

    lines = ["[CONV]"]

    # Mark the task domains so Lu learns task-oriented dialogue structure
    if services:
        domain_str = ", ".join(str(s).strip() for s in services if s)
        lines.append(f"[TASK: {domain_str}]")

    for i, (spk, utt) in enumerate(zip(speakers, utterances)):
        spk_label = str(spk).strip()
        utt_text  = str(utt).strip()
        if not utt_text:
            continue

        # MultiWOZ uses 0/1 or USER/SYSTEM for speakers
        if spk_label in ("0", "USER"):
            speaker = "User"
        else:
            speaker = "System"

        lines.append(f"[TURN {i+1}] {speaker}: {utt_text}")

    return f"multiwoz_{dlg_id}", "\n".join(lines)


def extract_oasst1(example: dict, idx: int) -> tuple:
    """
    OpenAssistant OASST1: human-written assistant dialogue messages.
    Each row is a SINGLE message in a conversation tree.
    Fields:
      - message_id:  unique message ID
      - parent_id:   ID of the message this replies to (None for roots)
      - text:        the message content
      - role:        "prompter" or "assistant"
      - lang:        language code (we filter to English only)
    """
    msg_id = (example.get("message_id") or str(idx))[:20]
    role   = (example.get("role") or "prompter").strip()
    text   = (example.get("text") or "").strip()
    lang   = (example.get("lang") or "en").strip()

    # Skip non-English messages
    if lang != "en" or not text:
        return f"oasst_{msg_id}", ""

    # Tag the role so Lu learns Human↔Assistant turn patterns
    lines = ["[CONV]"]
    if role == "prompter":
        lines.append(f"[ACT: question] Human: {text}")
    else:
        lines.append(f"[ACT: inform] Assistant: {text}")

    return f"oasst_{msg_id}", "\n".join(lines)


def extract_hh_rlhf(example: dict, idx: int) -> tuple:
    """
    Anthropic HH-RLHF: helpful/harmless preference dialogues.
    Each row has 'chosen' (preferred) and 'rejected' dialogue strings.
    Format: "\\n\\nHuman: ...\\n\\nAssistant: ..."
    We use only 'chosen' — the better conversation.
    """
    chosen = (example.get("chosen") or "").strip()
    if not chosen:
        return f"hhrlhf_{idx}", ""

    lines = ["[CONV]"]

    # Parse the Human/Assistant turn structure
    # Split on the double-newline + role pattern
    parts = re.split(r'\n\n(Human|Assistant):\s*', chosen)

    turn_num = 0
    i = 0
    # parts[0] is usually empty (text before first \n\nHuman:)
    # Then alternating: role, content, role, content, ...
    if parts and not parts[0].strip():
        i = 1  # skip empty first element

    while i < len(parts) - 1:
        role = parts[i].strip()
        content = parts[i + 1].strip() if i + 1 < len(parts) else ""
        if role in ("Human", "Assistant") and content:
            turn_num += 1
            lines.append(f"[TURN {turn_num}] {role}: {content}")
        i += 2

    return f"hhrlhf_{idx}", "\n".join(lines)


def extract_ultrachat(example: dict, idx: int) -> tuple:
    """
    UltraChat 200K: large-scale multi-turn conversations.
    Fields:
      - prompt_id:  unique conversation ID
      - messages:   list of {role: "user"/"assistant", content: "..."}
    """
    prompt_id = example.get("prompt_id") or str(idx)
    messages  = example.get("messages") or []

    lines = ["[CONV]"]
    for i, msg in enumerate(messages):
        role    = str(msg.get("role", "")).strip()
        content = str(msg.get("content", "")).strip()
        if not content:
            continue

        # Map role names to consistent labels
        if role == "user":
            speaker = "Human"
        elif role == "assistant":
            speaker = "Assistant"
        else:
            speaker = role.capitalize()

        lines.append(f"[TURN {i+1}] {speaker}: {content}")

    return f"ultrachat_{prompt_id}", "\n".join(lines)


def extract_wizard_of_wiki(example: dict, idx: int) -> tuple:
    """
    WikiDialog EN: knowledge-grounded Q&A dialogue generated from Wikipedia.
    Fields:
      - title:        Wikipedia article title (the topic)
      - utterances:   list of alternating user/agent turns
    """
    title      = (example.get("title") or "").strip()
    utterances = example.get("utterances") or []

    if not utterances:
        return f"wikidlg_{idx}", ""

    lines = ["[CONV]"]
    if title:
        lines.append(f"[TOPIC: {title}]")

    for i, utt in enumerate(utterances):
        speaker = "A" if i % 2 == 0 else "B"
        text = str(utt).strip()
        if text:
            lines.append(f"[TURN {i+1}] {speaker}: {text}")

    doc_id = title.replace(" ", "_")[:40] if title else str(idx)
    return f"wikidlg_{doc_id}_{idx}", "\n".join(lines)


# Map dataset names to their extractor functions
EXTRACTORS = {
    "dailydialog":        extract_dailydialog,
    "empathetic":         extract_empathetic,
    "persona-chat":       extract_persona_chat,
    "blended-skill-talk": extract_blended_skill_talk,
    "prosocial-dialog":   extract_prosocial_dialog,
    "multiwoz":           extract_multiwoz,
    "oasst1":             extract_oasst1,
    "hh-rlhf":            extract_hh_rlhf,
    "ultrachat":          extract_ultrachat,
    "wizard-of-wiki":     extract_wizard_of_wiki,
}


# ══════════════════════════════════════════════════════════════════════════════
#  CSV LOG — tracks which docs have been ingested for resume capability
# ══════════════════════════════════════════════════════════════════════════════

LOG_FIELDNAMES = ["dataset", "doc_id", "ingested_at", "ku_count"]


def load_ingested_ids(log_path: str) -> set:
    """Load already-ingested doc IDs from the CSV log.
    Used for --resume: if a doc_id is in here, we skip it."""
    if not os.path.isfile(log_path):
        return set()
    ingested = set()
    try:
        with open(log_path, "r", encoding="utf-8", newline="") as f:
            reader = csv.DictReader(f)
            for row in reader:
                if row.get("doc_id"):
                    ingested.add(row["doc_id"])
    except Exception as e:
        print(f"  WARNING: Could not read log file {log_path}: {e}")
    return ingested


def append_log(log_path: str, dataset: str, doc_id: str, ku_count: int):
    """Write one row to the CSV log after successfully ingesting a doc."""
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

def chunk_text(text: str, max_len: int) -> list:
    """Split text exceeding max_len at sentence boundaries.
    Returns a list of chunks, each <= max_len characters."""
    if len(text) <= max_len:
        return [text]

    # Try to split at sentence endings first
    sentences = re.split(r'(?<=[.!?])\s+', text)
    chunks = []
    current = ""
    for sent in sentences:
        if len(current) + len(sent) + 1 <= max_len:
            current = (current + " " + sent).strip() if current else sent
        else:
            if current:
                chunks.append(current)
            # If a single sentence is too long, hard-split it
            if len(sent) > max_len:
                for i in range(0, len(sent), max_len):
                    chunks.append(sent[i:i + max_len])
                current = ""
            else:
                current = sent
    if current:
        chunks.append(current)
    return chunks


def sanitize_text(text: str) -> str:
    """Remove null bytes and control chars that crash Windows pipes."""
    text = text.replace("\x00", "")
    text = re.sub(r"[\x01-\x08\x0b\x0c\x0e-\x1f\x7f]", "", text)
    return text.strip()


# ══════════════════════════════════════════════════════════════════════════════
#  MCP CLIENT — talks to lu_mcp_v2.exe over stdin/stdout JSON-RPC
#
#  This is the same proven McpClient from ingest_hf_conversations.py.
#  It launches the MCP server as a subprocess and sends JSON-RPC messages.
# ══════════════════════════════════════════════════════════════════════════════

class McpClient:
    """Thin JSON-RPC 2.0 client for lu_mcp_v2.exe over stdio."""

    def __init__(self, exe_path: str, brain_path: str):
        # Launch the MCP server process
        self.proc = subprocess.Popen(
            [exe_path, brain_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            bufsize=0,
        )
        self._q = queue.Queue()      # incoming JSON responses
        self._req_id = 0             # auto-incrementing request ID
        self._lock = threading.Lock()

        # Background threads to read stdout/stderr without blocking
        self._reader = threading.Thread(target=self._read_loop, daemon=True)
        self._reader.start()
        self._err_reader = threading.Thread(target=self._err_loop, daemon=True)
        self._err_reader.start()

        # Give the process a moment to start up
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
        """Background thread: read JSON lines from MCP server stdout."""
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
        """Background thread: capture last 20 stderr lines for diagnostics."""
        self._stderr_lines = []
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
        """Send a JSON-RPC request and wait for the response."""
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

        # Wait for matching response (by request ID)
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                resp = self._q.get(timeout=min(5.0, deadline - time.time()))
                if resp.get("id") == req_id:
                    return resp
                else:
                    self._q.put(resp)  # not ours, put it back
            except queue.Empty:
                pass
        return {"error": {"message": "timeout"}}

    def notify(self, method: str, params: dict):
        """Send a JSON-RPC notification (no response expected)."""
        msg = {"jsonrpc": "2.0", "method": method, "params": params}
        data = json.dumps(msg, ensure_ascii=False) + "\n"
        try:
            self.proc.stdin.write(data.encode("utf-8"))
            self.proc.stdin.flush()
        except Exception:
            pass

    def tool_call(self, name: str, arguments: dict, timeout: float = 30.0) -> dict:
        """Call a specific MCP tool (like lu_teach or lu_save)."""
        return self.send("tools/call", {"name": name, "arguments": arguments}, timeout=timeout)

    def close(self):
        """Shut down the MCP server cleanly."""
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.wait(timeout=10)
        except Exception:
            self.proc.kill()


# ── MCP helper functions ──────────────────────────────────────────────────────

def extract_result_text(resp: dict) -> str:
    """Pull the human-readable text out of an MCP JSON-RPC response."""
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


def init_mcp(client: McpClient):
    """Initialize the MCP session and wait for brain to load."""
    # Step 1: Send the MCP initialize handshake
    resp = client.send("initialize", {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "train-conversations", "version": "1.0"},
    })
    if resp.get("error"):
        print(f"  WARNING: initialize error: {resp['error']}")
    else:
        print("  Session initialized OK")

    client.notify("notifications/initialized", {})
    time.sleep(0.3)

    # Step 2: Wait for brain to finish loading.
    # The Rust MCP loads the brain file synchronously. During this time it
    # does NOT read stdin, so sending requests would fill the pipe buffer
    # and deadlock. We just sleep and check the process is still alive.
    brain_load_wait = 120  # seconds — large brains can take 60-90s
    poll_interval = 5  # print every 5s so external monitors don't timeout
    print(f"  Waiting {brain_load_wait}s for brain to load...", flush=True)
    for elapsed in range(0, brain_load_wait, poll_interval):
        if client.proc.poll() is not None:
            raise RuntimeError(
                f"MCP server exited during brain load (code {client.proc.returncode})"
            )
        print(f"  ... {elapsed}s elapsed", flush=True)
        time.sleep(poll_interval)
    print(f"  Brain load wait complete. Configuring...", flush=True)

    # Step 3: Set autonomous mode so lu_teach doesn't ask for confirmation
    resp = client.tool_call("lu_configure", {"autonomy_tier": "Autonomous"}, timeout=60.0)
    result = extract_result_text(resp)
    print(f"  Configure: {result}", flush=True)
    if "loading" in result.lower():
        print(f"  WARNING: Brain may still be loading. Proceeding anyway.", flush=True)


def teach(client: McpClient, text: str, verbose: bool = False) -> int:
    """Send a single text chunk to lu_teach.  Returns KU count learned."""
    text = sanitize_text(text)
    if not text:
        return 0

    resp = client.tool_call("lu_teach", {
        "ku_type":    "Raw",
        "content":    text,
        "confidence": "Terminal",
    }, timeout=30.0)

    result_text = extract_result_text(resp)
    if verbose:
        print(f"      -> {result_text}")

    # Parse the "Learned N" count from the response
    if result_text and "[error]" not in result_text:
        m = re.search(r'Learned\s+(\d+)', result_text or "")
        return int(m.group(1)) if m else 1
    return 0


def save_brain(client: McpClient) -> str:
    """Tell Lu to persist its brain to disk. Can take a while for large brains."""
    resp = client.tool_call("lu_save", {}, timeout=600.0)
    return extract_result_text(resp) or "saved"


# ── Dataset streaming ──────────────────────────────────────────────────────────

_HF_LOAD_TIMEOUT = 120  # seconds; HF Hub can be slow for anonymous users


def _load_dataset_timed(hf_path: str, load_kwargs: dict, timeout: int = _HF_LOAD_TIMEOUT):
    """Call load_dataset() on a background thread with a timeout.
    This prevents the script from hanging forever if HF Hub is down."""
    from datasets import load_dataset

    result = [None]
    error  = [None]

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
            "(HF Hub may be rate-limiting)"
        )
    if error[0] is not None:
        raise error[0]
    return result[0]


def stream_dataset(name: str, cfg: dict):
    """Yield (idx, example) from a HuggingFace dataset using streaming.
    Handles the import-timeout issue from Python 3.14 WMI bug."""

    # Import datasets library with a timeout safety net
    _ld_result = [None]
    _ld_error  = [None]

    def _do_import():
        try:
            from datasets import load_dataset as _fn
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

    hf_path   = cfg["hf_path"]
    split     = cfg["split"]
    streaming = cfg.get("streaming", True)

    print(f"  Loading {hf_path} (split={split}, streaming={streaming})...", flush=True)

    load_kwargs = dict(split=split, streaming=streaming)

    # Some datasets need a config name
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


# ── Per-dataset ingestion loop ────────────────────────────────────────────────

def ingest_dataset(name: str, client, ingested_ids: set, args) -> dict:
    """Stream and ingest a single conversational dataset.
    Returns a stats dict with counts of what happened."""
    cfg       = DATASETS[name]
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

    # Figure out the effective limit for this dataset
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

        # Extract and format the conversation with metadata tags
        try:
            doc_id, text = extractor(example, idx)
        except Exception as e:
            stats["errors"] += 1
            if args.verbose:
                print(f"  WARNING: extraction error at idx {idx}: {e}")
            continue

        # Resume: skip docs we already ingested in a previous run
        if doc_id in ingested_ids:
            stats["skipped_resume"] += 1
            continue

        text = text.strip()
        if len(text) < args.min_len:
            stats["skipped_short"] += 1
            continue

        # Split long conversations into chunks
        chunks = chunk_text(text, args.max_len)

        if args.dry_run:
            # Just print a preview, don't send to Lu
            print(f"  [{name}:{idx}] {text[:120]}{'...' if len(text) > 120 else ''}")
            stats["ingested"] += 1
            append_log(LOG_CSV, name, doc_id, 0)
        else:
            # Check MCP server is still alive
            if client.proc.poll() is not None:
                print(f"  ERROR: MCP server died (exit {client.proc.returncode}). "
                      f"Stopping dataset.", flush=True)
                print(f"  MCP stderr tail:\n{client.get_stderr_tail()}", flush=True)
                break

            ku_total_for_doc = 0
            mcp_died = False
            for chunk in chunks:
                if len(chunk) < args.min_len:
                    continue

                # Send this chunk to Lu via MCP
                ku = teach(client, chunk, verbose=args.verbose)

                # Check if MCP died mid-teach
                if client.proc.poll() is not None:
                    print(f"  ERROR: MCP server died mid-teach "
                          f"(exit {client.proc.returncode}).", flush=True)
                    print(f"  MCP stderr tail:\n{client.get_stderr_tail()}",
                          flush=True)
                    mcp_died = True
                    break

                ku_total_for_doc += ku
                if ku == 0:
                    stats["errors"] += 1
                stats["ku_total"] += ku
                time.sleep(args.delay)

            if mcp_died:
                break

            stats["ingested"] += 1
            append_log(LOG_CSV, name, doc_id, ku_total_for_doc)

            # Periodic save — protects against crashes losing progress
            if stats["ingested"] - last_save_at >= args.save_every:
                save_result = save_brain(client)
                last_save_at = stats["ingested"]
                print(f"  [SAVE at {stats['ingested']}] {save_result}",
                      flush=True)
                if "[error]" in save_result and client.proc.poll() is not None:
                    print(f"  ERROR: MCP died during save.", flush=True)
                    break

        # Progress report every 10 seconds
        now = time.time()
        if now - last_stat_at >= 10.0:
            elapsed = now - start_time
            rate = stats["ingested"] / elapsed if elapsed > 0 else 0
            ku_rate = stats["ku_total"] / elapsed if elapsed > 0 else 0
            print(
                f"  [{name}] seen={stats['seen']}  ingested={stats['ingested']}  "
                f"skipped(resume={stats['skipped_resume']}, "
                f"short={stats['skipped_short']})  "
                f"KUs={stats['ku_total']}  "
                f"rate={rate:.1f} doc/s  ku_rate={ku_rate:.1f} ku/s",
                flush=True,
            )
            last_stat_at = now

        # Check if we've hit the limit for this dataset
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
        description="Train Lu on conversational foundations using HuggingFace datasets.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument(
        "--dataset", choices=list(DATASETS.keys()),
        help="Ingest only this dataset (default: all in priority order)",
    )
    p.add_argument("--brain", default=BRAIN_PATH,
                   help="Path to the .lbr brain file")
    p.add_argument("--dry-run", action="store_true",
                   help="Print formatted docs without sending to Lu")
    p.add_argument("--limit", type=int, default=0, metavar="N",
                   help="Stop after N docs per dataset (0 = unlimited)")
    p.add_argument("--delay", type=float, default=DEFAULT_DELAY, metavar="F",
                   help=f"Seconds between MCP calls (default: {DEFAULT_DELAY})")
    p.add_argument("--save-every", type=int, default=DEFAULT_SAVE_EVERY, metavar="N",
                   help=f"Save brain every N docs (default: {DEFAULT_SAVE_EVERY})")
    p.add_argument("--min-len", type=int, default=DEFAULT_MIN_LEN, metavar="N",
                   help=f"Minimum text length (default: {DEFAULT_MIN_LEN})")
    p.add_argument("--max-len", type=int, default=DEFAULT_MAX_LEN, metavar="N",
                   help=f"Maximum text length before chunking (default: {DEFAULT_MAX_LEN})")
    p.add_argument("--verbose", action="store_true",
                   help="Print each doc and MCP response")
    return p.parse_args()


# ── Main ───────────────────────────────────────────────────────────────────────

def main():
    args = parse_args()

    # Which datasets to train on
    targets = [args.dataset] if args.dataset else DATASET_ORDER

    print("=" * 70)
    print("  LU BRAIN v2 — CONVERSATIONAL FOUNDATIONS TRAINING")
    print("=" * 70)
    print(f"  Datasets:   {', '.join(targets)}")
    print(f"  Brain:      {args.brain}")
    print(f"  Log CSV:    {LOG_CSV}")
    print(f"  Min len:    {args.min_len} chars")
    print(f"  Max len:    {args.max_len} chars")
    print(f"  Delay:      {args.delay}s between calls")
    print(f"  Save every: {args.save_every} docs")
    print(f"  Dry run:    {args.dry_run}")
    if args.limit:
        print(f"  Limit:      {args.limit} docs/dataset")
    print()

    # ── Step 1: Load resume state ─────────────────────────────────────────────
    print("[1/4] Loading ingestion log (resume state)...")
    ingested_ids = load_ingested_ids(LOG_CSV)
    print(f"  Already ingested: {len(ingested_ids)} docs")

    # ── Step 2: Start MCP server ──────────────────────────────────────────────
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

    # ── Step 3: Ingest datasets ───────────────────────────────────────────────
    print(f"\n[3/4] Training on {len(targets)} conversational dataset(s)...")
    grand_total = {"seen": 0, "ingested": 0, "ku_total": 0, "errors": 0}
    pipeline_start = time.time()

    for name in targets:
        ds_stats = ingest_dataset(name, client, ingested_ids, args)
        for k in grand_total:
            grand_total[k] += ds_stats.get(k, 0)

    # ── Step 4: Final save + summary ──────────────────────────────────────────
    if not args.dry_run and client:
        print(f"\n[4/4] Final save...")
        save_result = save_brain(client)
        print(f"  {save_result}")

        resp = client.tool_call("lu_status", {}, timeout=30.0)
        print(f"  Status: {extract_result_text(resp)}")

        client.close()
    else:
        print(f"\n[4/4] No save needed (dry run).")

    # ── Summary ───────────────────────────────────────────────────────────────
    elapsed = time.time() - pipeline_start
    print(f"\n{'=' * 70}")
    print(f"  CONVERSATIONAL TRAINING COMPLETE")
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
    print(f"  Training log:       {LOG_CSV}")
    print(f"{'=' * 70}")


if __name__ == "__main__":
    main()
