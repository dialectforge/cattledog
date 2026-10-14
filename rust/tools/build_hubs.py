# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

r"""
Lu Ai - Topic Hub Builder

Creates hierarchical "hub" nodes that serve as conceptual anchors
in the knowledge graph. Hub nodes solve the broad-query problem:
when a user asks "history of computers", keyword matching alone
can't find ENIAC/vacuum tube/Grace Hopper facts because those
atoms don't contain "history" or "computer" as keywords.

Hub nodes act as a table of contents -- a single node like
"Computer history spans from 1940s vacuum tubes to modern smartphones"
gets bonded to all the specific facts. Spreading activation then
flows from the hub to the details.

Two modes:
  1. RETROACTIVE: Analyze existing brain atoms, cluster them by topic,
     create hub nodes for each cluster, bond them.
  2. PIPELINE: Called from train_pdf.py after each chapter to create
     a summary hub for that chapter's facts.

Usage:
    python build_hubs.py                    # auto-discover topics and build hubs
    python build_hubs.py --topics "computer history,networking,AI"  # specific topics
    python build_hubs.py --dry-run          # show what would be created
"""

import sys
import os
import json
import time
import urllib.request
import urllib.error
import argparse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from train import MCPBridge, DEFAULT_MODEL

OLLAMA_URL = "http://localhost:11434/api/generate"
HUB_MODEL = "mistral:7b"


def ask_ollama(prompt, system, model=None):
    """Call Ollama and return the response text."""
    if model is None:
        model = HUB_MODEL
    payload = {
        "model": model,
        "prompt": prompt,
        "system": system,
        "stream": False,
        "format": "json",
        "options": {"temperature": 0.3, "top_p": 0.9}
    }
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        OLLAMA_URL, data=data,
        headers={"Content-Type": "application/json"}
    )
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            result = json.loads(resp.read().decode("utf-8"))
            return result.get("response", "")
    except Exception as e:
        print(f"  [ollama] Error: {e}")
        return None


def recall_topic(bridge, topic, top_k=30):
    """Use brain_recall to find atoms related to a topic."""
    resp = bridge._call("tools/call", {
        "name": "brain_recall",
        "arguments": {"query": topic, "depth": 3, "top_k": top_k}
    })
    if resp and "result" in resp:
        text = bridge._extract_text(resp["result"])
        if text:
            try:
                data = json.loads(text)
                return data.get("activated_nodes", [])
            except:
                pass
    return []


def learn_node(bridge, content):
    """Learn a single node and return its seed hex."""
    resp = bridge._call("tools/call", {
        "name": "brain_learn",
        "arguments": {"content": content}
    })
    if resp and "result" in resp:
        text = bridge._extract_text(resp["result"])
        if text:
            try:
                data = json.loads(text)
                return data.get("seed", None)
            except:
                # might be raw seed string
                return text.strip().strip('"')
    return None


def associate(bridge, seed_a, seed_b, description, weight=2.0):
    """Create a bond between two nodes."""
    resp = bridge._call("tools/call", {
        "name": "brain_associate",
        "arguments": {
            "seed_a": seed_a,
            "seed_b": seed_b,
            "description": description,
            "weight": weight
        }
    })
    return resp is not None and "result" in (resp or {})


# --- Hub Generation ---

SYNTHESIZE_SYSTEM = """You are a knowledge organizer. Given a list of facts about a topic,
create a SINGLE hub summary sentence that captures the essence of the entire topic.
Output ONLY valid JSON with this structure:
{"hub": "One comprehensive sentence summarizing the topic (20-40 words)",
 "subtopics": ["3-5 word subtopic phrase", "another subtopic phrase"]}

RULES:
- The hub sentence must mention the main topic by name
- The hub must be a standalone sentence that gives an overview
- Subtopics are the 3-5 major themes within this topic
- Each subtopic is a short phrase (3-5 words), NOT a full sentence"""


def generate_hub(topic, atoms):
    """Use Ollama to synthesize a hub node from a set of related atoms."""
    # Build context from the top atoms
    atom_texts = []
    for atom in atoms[:20]:  # max 20 to stay within context
        label = atom.get("label", atom.get("content", ""))
        if label and len(label) > 10:
            atom_texts.append(f"- {label}")

    if len(atom_texts) < 3:
        print(f"  [hub] Too few atoms ({len(atom_texts)}) for topic '{topic}', skipping")
        return None

    prompt = f"""Topic: {topic}

Known facts:
{chr(10).join(atom_texts)}

Create a hub summary and identify subtopics."""

    response = ask_ollama(prompt, SYNTHESIZE_SYSTEM)
    if not response:
        return None

    try:
        data = json.loads(response)
        return data
    except:
        print(f"  [hub] Failed to parse Ollama response for '{topic}'")
        return None


# --- Topic Discovery ---

DISCOVER_SYSTEM = """You are analyzing a list of knowledge atoms from an AI's brain.
Identify the MAJOR TOPICS that these atoms cover.
Output ONLY valid JSON:
{"topics": ["topic name", "another topic", ...]}

RULES:
- Each topic should be 2-4 words (e.g., "computer history", "network security", "data types")
- Only include topics with 5+ related atoms
- Aim for 10-30 distinct topics
- Be specific: "computer history" not just "computers"
- Focus on textbook-level categories"""


def discover_topics(bridge):
    """Sample the brain and discover major topic clusters."""
    print("[hub] Discovering topics from brain contents...")

    # Sample broad queries to get a diverse cross-section
    sample_queries = [
        "technology", "computers", "software", "hardware",
        "internet", "networking", "security", "data",
        "programming", "operating systems", "applications",
        "history", "communication", "business", "education",
        "artificial intelligence", "mobile devices", "cloud computing",
        "databases", "web development", "ethics", "privacy",
    ]

    all_atoms = []
    seen_seeds = set()

    for query in sample_queries:
        atoms = recall_topic(bridge, query, top_k=15)
        for atom in atoms:
            seed = atom.get("seed_hex", "")
            if seed not in seen_seeds:
                seen_seeds.add(seed)
                all_atoms.append(atom)

    print(f"  [hub] Sampled {len(all_atoms)} unique atoms from {len(sample_queries)} queries")

    # Feed a sample to Ollama for topic discovery
    sample_labels = [a.get("label", "")[:80] for a in all_atoms[:150]]
    prompt = f"""Here are {len(sample_labels)} knowledge atoms from an AI brain:

{chr(10).join(f'- {l}' for l in sample_labels if len(l) > 10)}

What major topics do these atoms cover?"""

    response = ask_ollama(prompt, DISCOVER_SYSTEM)
    if not response:
        print("  [hub] Topic discovery failed")
        return []

    try:
        data = json.loads(response)
        topics = data.get("topics", [])
        print(f"  [hub] Discovered {len(topics)} topics: {', '.join(topics[:10])}...")
        return topics
    except:
        print("  [hub] Failed to parse topic discovery response")
        return []


# --- Main ---

def build_hubs_for_topics(bridge, topics, dry_run=False):
    """Build hub nodes for a list of topics."""
    total_hubs = 0
    total_bonds = 0

    for i, topic in enumerate(topics):
        print(f"\n[{i+1}/{len(topics)}] Processing topic: {topic}")

        # Step 1: Recall related atoms
        atoms = recall_topic(bridge, topic, top_k=30)
        if len(atoms) < 3:
            print(f"  Skipping -- only {len(atoms)} atoms found")
            continue

        # Filter to atoms with meaningful contribution
        strong_atoms = [a for a in atoms if a.get("contribution", 0) >= 0.02]
        if len(strong_atoms) < 3:
            strong_atoms = atoms[:10]  # fallback to top 10

        print(f"  Found {len(atoms)} atoms ({len(strong_atoms)} strong)")

        # Step 2: Generate hub content via Ollama
        hub_data = generate_hub(topic, strong_atoms)
        if not hub_data:
            print(f"  Skipping -- hub generation failed")
            continue

        hub_text = hub_data.get("hub", "")
        subtopics = hub_data.get("subtopics", [])

        if not hub_text or len(hub_text) < 10:
            print(f"  Skipping -- empty hub text")
            continue

        print(f"  Hub: {hub_text}")
        print(f"  Subtopics: {', '.join(subtopics)}")

        if dry_run:
            print(f"  [DRY RUN] Would create hub + {len(strong_atoms)} bonds")
            total_hubs += 1
            total_bonds += len(strong_atoms)
            continue

        # Step 3: Learn the hub node
        hub_seed = learn_node(bridge, hub_text)
        if not hub_seed:
            print(f"  ERROR: Failed to learn hub node")
            continue

        print(f"  Hub seed: {hub_seed}")
        total_hubs += 1

        # Step 4: Bond hub to all strong atoms (weight 2.0 -- hubs are important)
        bonds_made = 0
        for atom in strong_atoms[:20]:  # cap at 20 bonds per hub
            atom_seed = atom.get("seed_hex", "")
            if atom_seed and atom_seed != hub_seed:
                ok = associate(bridge, hub_seed, atom_seed,
                             f"hub:{topic}", weight=2.0)
                if ok:
                    bonds_made += 1

        total_bonds += bonds_made
        print(f"  Created {bonds_made} bonds")

        # Step 5: Create subtopic hubs too (lighter weight)
        for sub in subtopics[:5]:
            sub_atoms = recall_topic(bridge, sub, top_k=10)
            sub_strong = [a for a in sub_atoms if a.get("contribution", 0) >= 0.03]
            if len(sub_strong) < 2:
                continue

            # Bond subtopic atoms to the main hub
            for atom in sub_strong[:10]:
                atom_seed = atom.get("seed_hex", "")
                if atom_seed and atom_seed != hub_seed:
                    associate(bridge, hub_seed, atom_seed,
                            f"hub:{topic}:{sub}", weight=1.5)
                    total_bonds += 1

        # Save periodically
        if (i + 1) % 5 == 0:
            bridge.save()
            print(f"  [saved] Progress: {total_hubs} hubs, {total_bonds} bonds")

        time.sleep(0.5)  # brief pause between topics

    # Final save
    if not dry_run:
        bridge.save()

    return total_hubs, total_bonds


def main():
    global HUB_MODEL
    parser = argparse.ArgumentParser(description="Build topic hub nodes in Lu's brain")
    parser.add_argument("--topics", type=str, default=None,
                       help="Comma-separated topic list (auto-discovers if omitted)")
    parser.add_argument("--dry-run", action="store_true",
                       help="Show what would be created without making changes")
    parser.add_argument("--model", type=str, default=HUB_MODEL,
                       help=f"Ollama model (default: {HUB_MODEL})")
    args = parser.parse_args()

    HUB_MODEL = args.model

    # Start MCP bridge
    bridge = MCPBridge()
    bridge.start()

    try:
        # Get topics
        if args.topics:
            topics = [t.strip() for t in args.topics.split(",")]
        else:
            topics = discover_topics(bridge)

        if not topics:
            print("[hub] No topics to process.")
            return

        print(f"\n{'='*60}")
        print(f"Building hubs for {len(topics)} topics")
        if args.dry_run:
            print("(DRY RUN -- no changes will be made)")
        print(f"{'='*60}\n")

        hubs, bonds = build_hubs_for_topics(bridge, topics, dry_run=args.dry_run)

        print(f"\n{'='*60}")
        print(f"DONE: {hubs} hub nodes created, {bonds} bonds created")
        stats = bridge._get_stats()
        if stats:
            print(f"Brain now: {stats['active_nodes']} nodes, {stats['active_bonds']} bonds")
        print(f"{'='*60}")

    finally:
        bridge.stop()


if __name__ == "__main__":
    main()
