#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Cat.1 full retrain ingest harness.

Forked from scripts/ingest_curriculum_1pct.py internal notes.
Deltas vs. 1pct:
  - Removes the auto-scale-down to 0.1%; takes FULL slice of each Tier 1 corpus.
  - Adds resumability via checkpoint file at /var/lib/lu/ingest_full_checkpoint.json.
    Checkpoint records {corpus, last_saved_id, bonds_landed, errors_count, ...}
    and is updated only after a successful lu4d_save (so the on-disk brain and
    the checkpoint are always in sync). Resume: stream from the start of each
    corpus and skip-and-discard until last_saved_id is reached.
  - Per-corpus throttle: 30/s Wikipedia, 50/s Visual Genome, 30/s Tier 2.
  - Periodic lu4d_save every ~30 min of in-corpus work (or every 5000 bonds,
    whichever comes first); brain + WALs flushed each save.
  - Three-tier order: Visual Genome FIRST (validates quickly), then full
    Wikipedia, then Tier 2 (no_robots + instruction-dataset). Wikipedia-
    structured / arXiv / StackExchange / Grokopedia are DEFERRED to follow-up.
  - Logs to /var/lib/lu/ingest_full_progress.log with timestamp, bonds_landed,
    corpus, errors_count, ingest_rate.
  - On startup, writes a sentinel /var/lib/lu/ingest_full_DONE on clean exit.

DEVIATIONS FROM PHASE-3 SPEC (documented per C11 demo honesty):
  1. Subprocess.Popen pattern (inherited from 1pct), NOT mkfifo + backgrounded
     server. The 1pct author documented this same deviation: bidirectional
     JSON-RPC over a FIFO needs a response-matching layer that subprocess.Popen
     gives for free. Same long-lived-server outcome; one PID to monitor.
  2. lu4d_configure { ingest_auto_approve_threshold: 0.7 } is a no-op against
     the current binary (md5 07a96d2…f8d, git 1d0d945) — that field is not in
     lu4d_configure's input schema. lu4d_teach_corpus lands bonds directly with
     no pending-queue gating in this binary. The 1pct run successfully landed
     bonds without setting any threshold. Kept the trust hierarchy + hub caps,
     dropped the no-op call.
  3. wikimedia/structured-wikipedia is in the trust table but has no loader in
     the 1pct harness; treating it as deferred along with arXiv/SE/Grokopedia.

Resumability semantics:
  - The checkpoint represents the last KNOWN-PERSISTED position. After a
    successful lu4d_save, the checkpoint is updated to record the last
    processed corpus id at the time of save. On crash, restart of this
    harness re-spawns the MCP server (which loads brain at last-saved state)
    and skips the stream forward to last_saved_id. Up to ~30 min of
    redundant streaming may occur on resume; bonds are not double-counted at
    the brain level because the MCP server reloaded from a clean save point.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime, timezone

# Phase B (2026-05-07): smart extractor v2 (iter-4, subject-anchored).
# Adds six-layer Wikipedia bond extraction. The old
# extract_wikipedia_bonds function is preserved below for reference
# but is no longer the call site -- ingest_wikipedia uses
# extract_bonds_from_wikipedia_article instead. K-gate, trust weights,
# checkpoint logic, hub caps and BatchSink are all unchanged.
import sys as _sys, os as _os
_SCRIPTS_DIR = _os.path.dirname(_os.path.abspath(__file__))
if _SCRIPTS_DIR not in _sys.path:
    _sys.path.insert(0, _SCRIPTS_DIR)
from smart_extractor import extract_bonds_from_wikipedia_article

PROGRESS_LOG = "/var/lib/lu/ingest_full_progress.log"
CHECKPOINT   = "/var/lib/lu/ingest_full_checkpoint.json"
DONE_SENTINEL = "/var/lib/lu/ingest_full_DONE"

RUN_TIMESTAMP = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
RUN_ID_TIER1  = f"cat1-retrain-tier1-{RUN_TIMESTAMP}"
RUN_ID_TIER2  = f"cat1-retrain-tier2-{RUN_TIMESTAMP}"

# Per-corpus pacing caps (calls/sec). Effective rate is min(cap, capability_rate).
THROTTLE_WIKIPEDIA = 30.0
THROTTLE_VG        = 50.0
THROTTLE_TIER2     = 30.0

# Periodic-save policy: whichever fires first.
SAVE_INTERVAL_SEC  = 30 * 60
SAVE_INTERVAL_BONDS = 5000


class Throttle:
    def __init__(self, rate_hz):
        self.min_interval = 1.0 / rate_hz
        self.last = 0.0
    def wait(self):
        now = time.monotonic()
        sleep_for = self.last + self.min_interval - now
        if sleep_for > 0:
            time.sleep(sleep_for)
        self.last = time.monotonic()


# Port BatchIngest -- see lu4d_teach_corpus_batch in src/mcp.rs.
# Cap is enforced server-side at TEACH_CORPUS_BATCH_MAX_ENTRIES=1000.
# 100 is 10x headroom; gives ~1.5kB-3kB JSON-RPC payloads per batch.
BATCH_SIZE = 100


class BatchSink:
    """Buffer-and-submit wrapper around lu4d_teach_corpus_batch.

    Maintains a per-process seen_triples set for Python-side dedup --
    skips submission entirely when (subject, relation, object) was
    already submitted in this process. The set does NOT persist
    across daemon restarts; checkpoint resume + lu_4_mcp's per-bond
    dedup (existing-edge reinforcement) catch any duplicates that
    re-submit on resume.

    Anti-bypass invariant: each entry in `add()` is submitted via the
    SAME lu4d_teach_corpus_batch tool; that tool routes per-entry
    through tool_teach_corpus, which goes through brain.teach_corpus
    (T.3 gating + TrainingCorpus stamping). The batch is purely an
    IPC speedup -- no per-bond logic is bypassed.
    """

    def __init__(self, client, throttle, batch_size=BATCH_SIZE):
        self.client = client
        self.throttle = throttle
        self.batch_size = batch_size
        self.buffer = []
        self.seen = set()
        # Cumulative counters --- mirror response structuredContent.
        self.bonds_landed = 0       # new bonds from response
        self.bonds_existing = 0     # reinforced existing bonds
        self.bonds_gated = 0        # T.3 PendingLog (below threshold)
        self.bonds_errors = 0       # per-entry errors + whole-batch failures
        self.bonds_skipped_dedup = 0  # Python-side dedup hits

    def add(self, entry):
        """Buffer one entry. Returns False if Python-side dedup skipped
        it OR the entry was malformed; True if it was queued."""
        s = entry.get("subject"); r = entry.get("relation"); o = entry.get("object")
        if not s or not r or not o:
            self.bonds_errors += 1
            return False
        key = (s, r, o)
        if key in self.seen:
            self.bonds_skipped_dedup += 1
            return False
        self.seen.add(key)
        self.buffer.append(entry)
        if len(self.buffer) >= self.batch_size:
            self.flush()
        return True

    def flush(self):
        """Submit the current buffer in one IPC call. No-op if empty."""
        if not self.buffer:
            return
        n = len(self.buffer)
        self.throttle.wait()
        try:
            resp = self.client.call("lu4d_teach_corpus_batch", {"entries": self.buffer})
            if call_ok(resp):
                sc = ((resp.get("result") or {}).get("structuredContent") or {})
                self.bonds_landed   += int(sc.get("bonds_landed", 0))
                self.bonds_existing += int(sc.get("bonds_deduped", 0))
                self.bonds_gated    += int(sc.get("bonds_gated", 0))
                errs = sc.get("errors") or []
                self.bonds_errors   += len(errs)
            else:
                # Whole-batch failure -- count every queued entry.
                self.bonds_errors += n
        except Exception:
            self.bonds_errors += n
        finally:
            self.buffer = []


class McpClient:
    def __init__(self, brain_path, keyfile, log_stderr_to=None):
        self.brain_path = brain_path
        self.keyfile = keyfile
        self._stderr = open(log_stderr_to, "ab") if log_stderr_to else subprocess.DEVNULL
        self.proc = subprocess.Popen(
            ["/usr/local/bin/lu_4_mcp", "--mcp", "--require-encrypted",
             "--keyfile", keyfile, "--brain", brain_path,
             "--brain-format-out", "v3enc"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self._stderr,
            bufsize=1, text=True,
        )
        self._next_id = 1
        self._init()
    def _send(self, obj):
        self.proc.stdin.write(json.dumps(obj) + "\n")
        self.proc.stdin.flush()
    def _recv(self):
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError("MCP server closed stdout unexpectedly")
        return json.loads(line)
    def _init(self):
        self._send({"jsonrpc":"2.0","id":self._next_id,"method":"initialize",
                    "params":{"protocolVersion":"2024-11-05","capabilities":{},
                             "clientInfo":{"name":"ingest-full","version":"0.1"}}})
        self._next_id += 1
        self._recv()
        self._send({"jsonrpc":"2.0","method":"notifications/initialized"})
    def call(self, name, args):
        rid = self._next_id
        self._next_id += 1
        self._send({"jsonrpc":"2.0","id":rid,"method":"tools/call",
                    "params":{"name":name,"arguments":args}})
        return self._recv()
    def close(self):
        try:
            if self.proc.stdin and not self.proc.stdin.closed:
                self.proc.stdin.close()
            self.proc.wait(timeout=600)
        except Exception:
            self.proc.kill()
        finally:
            if hasattr(self._stderr, "close"):
                try: self._stderr.close()
                except: pass


def log_progress(line):
    ts = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    try:
        with open(PROGRESS_LOG, "a") as f:
            f.write(f"{ts} {line}\n"); f.flush()
    except Exception:
        pass
    print(f"[{ts}] {line}", flush=True)


def call_ok(resp):
    return isinstance(resp, dict) and "error" not in resp


# ---- checkpoint ----

def load_checkpoint():
    if not os.path.exists(CHECKPOINT):
        return {}
    try:
        with open(CHECKPOINT) as f:
            return json.load(f)
    except Exception as e:
        log_progress(f"WARNING: checkpoint load failed: {e}; starting fresh")
        return {}

def save_checkpoint(cp):
    cp["updated_at"] = datetime.now(timezone.utc).isoformat()
    tmp = CHECKPOINT + ".tmp"
    with open(tmp, "w") as f:
        json.dump(cp, f, indent=2)
    os.replace(tmp, CHECKPOINT)


# ---- bond extraction (same as 1pct) ----

IS_A_PATTERN = re.compile(
    r"\b(?:is|was|are|were)\s+(?:an?|the)\s+([a-z][\w\s\-]{2,60}?)\s+(?:that|which|who|where|in|of|on|located|known|used|with|by|for|from|to|and|\.|,|;)",
    re.IGNORECASE,
)

def first_sentence(text):
    if not text: return ""
    m = re.match(r"([^.!?\n]{20,400}[.!?])", text.strip())
    return m.group(1).strip() if m else text.strip()[:200]

def extract_wikipedia_bonds(title, text, source_url):
    bonds = []
    if not text or not title: return bonds
    lead = first_sentence(text)
    if not lead: return bonds
    bonds.append((title, "RelatedTo", lead[:200], 0.5, "Inferred"))
    m = IS_A_PATTERN.search(lead)
    if m:
        cat = m.group(1).strip().rstrip(",.;:").lower()
        if 2 <= len(cat) <= 60:
            bonds.append((title, "IsA", cat, 0.6, "Inferred"))
    return bonds

def extract_response_bond(prompt, response):
    if not response: return None
    s = first_sentence(response)
    if not s or len(s) < 10: return None
    subj = (prompt or "prompt").strip()[:80]
    if not subj: subj = "prompt"
    return (subj, "RelatedTo", s[:200], 0.4, "Inferred")


# ---- ingest funcs ----

def _maybe_save(client, last_save_t, bonds_since_save, cp, corpus, last_id):
    """Periodic save. Returns (last_save_t, bonds_since_save) updated."""
    now = time.monotonic()
    if (now - last_save_t) >= SAVE_INTERVAL_SEC or bonds_since_save >= SAVE_INTERVAL_BONDS:
        log_progress(f"{corpus}: lu4d_save (interval) at last_id={last_id}")
        try:
            r = client.call("lu4d_save", {})
            ok = call_ok(r)
        except Exception as e:
            ok = False
            log_progress(f"{corpus}: lu4d_save FAILED: {e}")
        if ok:
            cp[corpus] = cp.get(corpus, {})
            cp[corpus]["last_saved_id"] = last_id
            save_checkpoint(cp)
            log_progress(f"{corpus}: checkpoint updated last_saved_id={last_id}")
        return time.monotonic(), 0
    return last_save_t, bonds_since_save


def ingest_visual_genome(client, cp, smoke_limit=None):
    import importlib.util, pathlib
    loader_path = pathlib.Path(__file__).resolve().parent / "visual_genome_loader.py"
    spec = importlib.util.spec_from_file_location("vg_loader", loader_path)
    vg = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(vg)

    corpus_id = "visual_genome"
    target_total = smoke_limit if smoke_limit else 2_316_104  # canonical VG v1.4
    log_progress(f"visual_genome: starting; target={target_total} smoke={bool(smoke_limit)}")

    throttle = Throttle(THROTTLE_VG)
    sink = BatchSink(client, throttle)
    sink.bonds_landed   = cp.get(corpus_id, {}).get("bonds_landed", 0)
    sink.bonds_errors   = cp.get(corpus_id, {}).get("errors_count", 0)
    sink.bonds_skipped_dedup = cp.get(corpus_id, {}).get("bonds_skipped_dedup", 0)
    last_saved_id = cp.get(corpus_id, {}).get("last_saved_id", -1)
    images_seen = set()
    skipped_reason = None
    last_save_t = time.monotonic()
    last_landed = sink.bonds_landed
    pos = 0
    try:
        for s_name, raw_pred, o_name, image_id in vg.stream_triplets(max_triplets=target_total):
            if pos <= last_saved_id:
                pos += 1
                continue
            images_seen.add(image_id)
            relation = vg.map_predicate(raw_pred)
            url = f"vg-image-{image_id}"
            try:
                sink.add({
                    "corpus": corpus_id, "source_url": url, "run_id": RUN_ID_TIER1,
                    "subject": s_name, "relation": relation, "object": o_name,
                    "weight": 0.5, "confidence": "Unverified",
                })
            except Exception:
                sink.bonds_errors += 1
            pos += 1
            if pos % 500 == 0:
                # Flush whatever is buffered so the progress log
                # reflects bonds actually landed up to now.
                sink.flush()
                bonds_since_save = sink.bonds_landed - last_landed
                log_progress(f"visual_genome: pos={pos}/{target_total} landed={sink.bonds_landed} "
                             f"errors={sink.bonds_errors} images={len(images_seen)} "
                             f"deduped={sink.bonds_skipped_dedup} gated={sink.bonds_gated}")
                cp[corpus_id] = {"bonds_landed":sink.bonds_landed,"errors_count":sink.bonds_errors,
                                 "bonds_skipped_dedup":sink.bonds_skipped_dedup,
                                 "last_saved_id":last_saved_id}
                save_checkpoint(cp)
                new_t, new_b = _maybe_save(client, last_save_t, bonds_since_save, cp, corpus_id, pos-1)
                if new_b == 0 and new_t != last_save_t:
                    last_saved_id = pos - 1
                    last_landed = sink.bonds_landed
                last_save_t = new_t
        # Flush any tail entries.
        sink.flush()
        bonds_landed = sink.bonds_landed
        bonds_errors = sink.bonds_errors
    except FileNotFoundError as e:
        skipped_reason = f"cache missing: {e}"
        log_progress(f"visual_genome: SKIPPED — {skipped_reason}")
    except Exception as e:
        skipped_reason = f"VG stream failed: {type(e).__name__}: {str(e)[:200]}"
        log_progress(f"visual_genome: SKIPPED — {skipped_reason}")

    # final save for corpus
    try:
        r = client.call("lu4d_save", {})
        if call_ok(r):
            cp[corpus_id] = {"bonds_landed":bonds_landed,"errors_count":bonds_errors,
                             "last_saved_id":pos-1, "completed": True,
                             "skipped_reason": skipped_reason}
            save_checkpoint(cp)
    except Exception:
        pass
    log_progress(f"visual_genome: DONE landed={bonds_landed} errors={bonds_errors} "
                 f"deduped={sink.bonds_skipped_dedup} gated={sink.bonds_gated} "
                 f"images={len(images_seen)} skip={skipped_reason}")
    return {"bonds_landed":bonds_landed,"bonds_errors":bonds_errors,
            "bonds_skipped_dedup":sink.bonds_skipped_dedup,
            "bonds_gated":sink.bonds_gated,
            "images_processed":len(images_seen),"skipped_reason":skipped_reason}


def ingest_wikipedia(client, cp, smoke_limit=None):
    from datasets import load_dataset
    corpus_id = "wikimedia/wikipedia"
    target_total = smoke_limit if smoke_limit else 6_700_000
    log_progress(f"wikipedia: starting load_dataset(streaming) target={target_total} smoke={bool(smoke_limit)}")
    ds = load_dataset("wikimedia/wikipedia", "20231101.en", split="train", streaming=True)

    throttle = Throttle(THROTTLE_WIKIPEDIA)
    sink = BatchSink(client, throttle)
    sink.bonds_landed   = cp.get(corpus_id, {}).get("bonds_landed", 0)
    sink.bonds_errors   = cp.get(corpus_id, {}).get("errors_count", 0)
    sink.bonds_skipped_dedup = cp.get(corpus_id, {}).get("bonds_skipped_dedup", 0)
    last_saved_id = cp.get(corpus_id, {}).get("last_saved_id", -1)
    last_save_t = time.monotonic()
    last_landed = sink.bonds_landed
    articles = 0
    start_t = time.monotonic()
    for ex in ds:
        if articles >= target_total: break
        if articles <= last_saved_id:
            articles += 1
            continue
        title = ex.get("title",""); text = ex.get("text",""); url = ex.get("url","")
        # Phase B: smart extractor v2 (iter-4). Returns list of dicts shaped
        # for sink.add; extra fields like "layer"/"frequency" are ignored
        # by sink.add. K-gate (T.3) inside lu4d_teach_corpus_batch is the
        # quality filter.
        bonds = extract_bonds_from_wikipedia_article(ex)
        for b in bonds:
            try:
                sink.add({
                    "corpus": corpus_id,
                    "source_url": b.get("source_url") or url,
                    "run_id": RUN_ID_TIER1,
                    "subject": b["subject"],
                    "relation": b["relation"],
                    "object": b["object"],
                    "weight": b["weight"],
                    "confidence": b["confidence"],
                })
            except Exception:
                sink.bonds_errors += 1
        articles += 1
        if articles % 1000 == 0:
            sink.flush()
            bonds_since_save = sink.bonds_landed - last_landed
            elapsed = time.monotonic() - start_t
            rate = articles / elapsed if elapsed > 0 else 0
            eta = (target_total - articles) / rate if rate > 0 else float("inf")
            log_progress(f"wikipedia: articles={articles}/{target_total} "
                         f"bonds_landed={sink.bonds_landed} errors={sink.bonds_errors} "
                         f"deduped={sink.bonds_skipped_dedup} gated={sink.bonds_gated} "
                         f"rate={rate:.1f}art/s eta_remaining_s={eta:.0f}")
            cp[corpus_id] = {"bonds_landed":sink.bonds_landed,"errors_count":sink.bonds_errors,
                             "bonds_skipped_dedup":sink.bonds_skipped_dedup,
                             "last_saved_id":last_saved_id}
            save_checkpoint(cp)
            new_t, new_b = _maybe_save(client, last_save_t, bonds_since_save, cp, corpus_id, articles-1)
            if new_b == 0 and new_t != last_save_t:
                last_saved_id = articles - 1
                last_landed = sink.bonds_landed
            last_save_t = new_t
    sink.flush()
    bonds_landed = sink.bonds_landed
    bonds_errors = sink.bonds_errors
    # final save
    try:
        r = client.call("lu4d_save", {})
        if call_ok(r):
            cp[corpus_id] = {"bonds_landed":bonds_landed,"errors_count":bonds_errors,
                             "last_saved_id":articles-1, "completed": True}
            save_checkpoint(cp)
    except Exception:
        pass
    log_progress(f"wikipedia: DONE articles={articles} landed={bonds_landed} errors={bonds_errors}")
    return {"articles_processed":articles,"bonds_landed":bonds_landed,"bonds_errors":bonds_errors}


def _ingest_tier2(client, cp, name, dataset_repo, dataset_split,
                  prompt_field, response_field, total_estimate, smoke_limit=None):
    from datasets import load_dataset
    corpus_id = dataset_repo
    target_total = smoke_limit if smoke_limit else total_estimate
    log_progress(f"{name}: starting load_dataset(streaming) target={target_total} smoke={bool(smoke_limit)}")

    throttle = Throttle(THROTTLE_TIER2)
    sink = BatchSink(client, throttle)
    sink.bonds_landed   = cp.get(corpus_id, {}).get("bonds_landed", 0)
    sink.bonds_errors   = cp.get(corpus_id, {}).get("errors_count", 0)
    sink.bonds_skipped_dedup = cp.get(corpus_id, {}).get("bonds_skipped_dedup", 0)
    statements_landed = cp.get(corpus_id, {}).get("statements_landed", 0)
    last_saved_id = cp.get(corpus_id, {}).get("last_saved_id", -1)
    last_save_t = time.monotonic(); last_landed = sink.bonds_landed
    examples = 0
    skipped_reason = None
    try:
        ds = load_dataset(dataset_repo, split=dataset_split, streaming=True)
        for ex in ds:
            if examples >= target_total: break
            if examples <= last_saved_id:
                examples += 1; continue
            # Fetch prompt + response — heuristics handle no_robots vs instruction-dataset
            if prompt_field == "messages":
                msgs = ex.get("messages", []) or []
                prompt, response = "", ""
                for m in msgs:
                    if m.get("role") == "user" and not prompt: prompt = m.get("content","")
                    elif m.get("role") == "assistant" and not response: response = m.get("content","")
            else:
                prompt = ex.get(prompt_field,"") or ex.get("instruction","")
                response = ex.get(response_field,"") or ex.get("response","") or ex.get("output","")
            if prompt:
                throttle.wait()
                try:
                    r = client.call("lu4d_user_statement",
                        {"text": prompt[:1000], "source_label": corpus_id})
                    if call_ok(r): statements_landed += 1
                except Exception:
                    pass
            bond = extract_response_bond(prompt, response)
            if bond:
                s,rel,o,w,c = bond
                try:
                    sink.add({
                        "corpus":corpus_id, "run_id":RUN_ID_TIER2,
                        "subject":s, "relation":rel, "object":o,
                        "weight":w, "confidence":c,
                    })
                except Exception:
                    sink.bonds_errors += 1
            examples += 1
            if examples % 200 == 0:
                sink.flush()
                bonds_since_save = sink.bonds_landed - last_landed
                log_progress(f"{name}: examples={examples}/{target_total} "
                             f"statements={statements_landed} bonds_landed={sink.bonds_landed} "
                             f"errors={sink.bonds_errors} deduped={sink.bonds_skipped_dedup} "
                             f"gated={sink.bonds_gated}")
                cp[corpus_id] = {"bonds_landed":sink.bonds_landed,"errors_count":sink.bonds_errors,
                                 "bonds_skipped_dedup":sink.bonds_skipped_dedup,
                                 "statements_landed":statements_landed,
                                 "last_saved_id":last_saved_id}
                save_checkpoint(cp)
                new_t, new_b = _maybe_save(client, last_save_t, bonds_since_save, cp, corpus_id, examples-1)
                if new_b == 0 and new_t != last_save_t:
                    last_saved_id = examples - 1
                    last_landed = sink.bonds_landed
                last_save_t = new_t
        sink.flush()
        bonds_landed = sink.bonds_landed
        bonds_errors = sink.bonds_errors
    except Exception as e:
        skipped_reason = f"{name} load failed: {type(e).__name__}: {str(e)[:200]}"
        log_progress(f"{name}: SKIPPED — {skipped_reason}")
    try:
        r = client.call("lu4d_save", {})
        if call_ok(r):
            cp[corpus_id] = {"bonds_landed":bonds_landed,"errors_count":bonds_errors,
                             "statements_landed":statements_landed,
                             "last_saved_id":examples-1, "completed": True,
                             "skipped_reason": skipped_reason}
            save_checkpoint(cp)
    except Exception:
        pass
    log_progress(f"{name}: DONE examples={examples} statements={statements_landed} "
                 f"landed={bonds_landed} errors={bonds_errors} skip={skipped_reason}")
    return {"examples_processed":examples,"statements_landed":statements_landed,
            "bonds_landed":bonds_landed,"bonds_errors":bonds_errors,
            "skipped_reason":skipped_reason}

def ingest_no_robots(client, cp, smoke_limit=None):
    return _ingest_tier2(client, cp, "no_robots", "HuggingFaceH4/no_robots",
                          "train", "messages", "messages", 10000, smoke_limit)

def ingest_instruction_dataset(client, cp, smoke_limit=None):
    return _ingest_tier2(client, cp, "instruction-dataset",
                          "HuggingFaceH4/instruction-dataset", "test",
                          "prompt", "completion", 330, smoke_limit)


# ---- main ----

def main():
    global PROGRESS_LOG, CHECKPOINT, DONE_SENTINEL
    parser = argparse.ArgumentParser()
    parser.add_argument("--brain", default="/var/lib/lu/brain-retrain-2026-05-04.lbr")
    parser.add_argument("--keyfile", default="/var/lib/lu/master.key")
    parser.add_argument("--mcp-stderr", default="/var/lib/lu/ingest_full_mcp.err")
    parser.add_argument("--smoke", type=int, default=None,
                        help="Smoke mode: cap each corpus to this many examples")
    parser.add_argument("--tier", action="append", type=int, choices=[1,2], default=None,
                        help="Restrict to listed tier(s); default both")
    parser.add_argument("--summary-out", default="/var/lib/lu/ingest_full_summary.json")
    parser.add_argument("--pipe", default=None,
                        help="Accepted-and-ignored for spec compatibility (subprocess.Popen used)")
    parser.add_argument("--checkpoint", default=CHECKPOINT,
                        help="Override default checkpoint path")
    parser.add_argument("--progress-log", default=PROGRESS_LOG,
                        help="Override default progress-log path")
    args = parser.parse_args()

    # Smoke-vs-production checkpoint isolation (Cat.1-retrain post-mortem):
    # When --smoke is set, automatically route checkpoint and progress-log
    # to *_smoke siblings so smoke verification cannot pollute the production
    # state. The 2026-05-04 retrain shipped with smoke + full sharing the
    # default checkpoint, leaving stale {last_saved_id: 49, completed: true}
    # markers from Phase 2 visible to Phase 3.
    smoke_redirect_msg = None
    if args.smoke is not None:
        def _smoke_path(p):
            base, dot, ext = p.rpartition(".")
            return f"{base}_smoke.{ext}" if dot else f"{p}_smoke"
        orig_cp = args.checkpoint
        orig_pl = args.progress_log
        orig_su = args.summary_out
        orig_ds = DONE_SENTINEL
        args.checkpoint = _smoke_path(args.checkpoint)
        args.progress_log = _smoke_path(args.progress_log)
        args.summary_out = _smoke_path(args.summary_out)
        DONE_SENTINEL = _smoke_path(DONE_SENTINEL)
        smoke_redirect_msg = (
            f"smoke mode (--smoke={args.smoke}): rerouted "
            f"checkpoint {orig_cp} -> {args.checkpoint}, "
            f"progress {orig_pl} -> {args.progress_log}, "
            f"summary {orig_su} -> {args.summary_out}, "
            f"sentinel {orig_ds} -> {DONE_SENTINEL}"
        )

    PROGRESS_LOG = args.progress_log
    CHECKPOINT = args.checkpoint

    tiers = set(args.tier or [1, 2])
    log_progress(f"=== INGEST FULL START brain={args.brain} smoke={args.smoke} tiers={sorted(tiers)} ===")
    if smoke_redirect_msg:
        log_progress(smoke_redirect_msg)

    cp = load_checkpoint()
    log_progress(f"checkpoint: {json.dumps({k:v for k,v in cp.items() if k!='updated_at'})[:600]}")

    summary = {"started_at": datetime.now(timezone.utc).isoformat(),
               "brain": args.brain, "smoke": args.smoke,
               "run_timestamp": RUN_TIMESTAMP, "tiers": sorted(tiers)}

    client = McpClient(args.brain, args.keyfile, log_stderr_to=args.mcp_stderr)
    try:
        # Status before
        resp = client.call("lu4d_status", {})
        sc = resp.get("result", {}).get("structuredContent", {}) if call_ok(resp) else {}
        summary["bond_count_before"] = sc.get("bond_count", -1)
        summary["node_count_before"] = sc.get("node_count", -1)
        log_progress(f"status_before: bonds={summary['bond_count_before']} nodes={summary['node_count_before']}")

        # Cat.1-retrain perf tweaks internal notes
        # Hub-cap perf mitigation for Wikipedia-scale ingest:
        # default 5000 caps cause O(degree) eviction lookups on hub nodes
        # (year, person, place, ...) once at cap. Raised to 50000 just for
        # the retrain run; re-tighten via runtime config post-retrain.
        # NOTE: lu4d_configure does not accept hub_* fields in this binary;
        # the dedicated tool is lu4d_hub_set (one key/value per call).
        for k, v in (("max_in_degree", 50000), ("max_out_degree", 50000)):
            r = client.call("lu4d_hub_set", {"key": k, "value": v})
            log_progress(f"startup tweak: lu4d_hub_set {k}={v} ok={call_ok(r)}")
        # Pause T.6 auto-tick for the retrain duration so curiosity research
        # does not compete with ingest cycles for the MCP server.
        # Resume manually post-retrain with lu4d_auto_tick_resume.
        r = client.call("lu4d_auto_tick_pause", {})
        log_progress(f"startup tweak: lu4d_auto_tick_pause ok={call_ok(r)}")


        if 1 in tiers:
            summary["visual_genome"] = ingest_visual_genome(client, cp, args.smoke)
            summary["wikipedia"] = ingest_wikipedia(client, cp, args.smoke)
        if 2 in tiers:
            summary["no_robots"] = ingest_no_robots(client, cp, args.smoke)
            summary["instruction_dataset"] = ingest_instruction_dataset(client, cp, args.smoke)

        # final save
        log_progress("final lu4d_save")
        save_resp = client.call("lu4d_save", {})
        summary["final_save"] = save_resp.get("result", save_resp)

        resp = client.call("lu4d_status", {})
        sc = resp.get("result", {}).get("structuredContent", {}) if call_ok(resp) else {}
        summary["bond_count_after"] = sc.get("bond_count", -1)
        summary["node_count_after"] = sc.get("node_count", -1)
        log_progress(f"status_after: bonds={summary['bond_count_after']} nodes={summary['node_count_after']}")

        # success sentinel for the validator task
        with open(DONE_SENTINEL, "w") as f:
            json.dump({"completed_at": datetime.now(timezone.utc).isoformat(),
                       "bond_count_after": summary["bond_count_after"]}, f)
        log_progress(f"=== DONE — sentinel written to {DONE_SENTINEL} ===")
    finally:
        summary["ended_at"] = datetime.now(timezone.utc).isoformat()
        try:
            with open(args.summary_out, "w") as f:
                json.dump(summary, f, indent=2, default=str)
        except Exception as e:
            log_progress(f"summary write failed: {e}")
        client.close()

if __name__ == "__main__":
    main()
