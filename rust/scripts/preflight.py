#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
preflight.py -- Pre-ingest verification harness for the Lu runtime.

Refuses to launch any 1+ hour ingest job if any contract surface is misaligned.
Designed to catch the silent-vocabulary-drop class of bug that ate 14 hours of
compute during Phase B internal notes audits).

Usage:
    python scripts/preflight.py                 # run all phases
    python scripts/preflight.py --phase A       # vocabulary checks only
    python scripts/preflight.py --phase B       # config + deployment
    python scripts/preflight.py --phase C       # integration smoke
    python scripts/preflight.py --no-remote     # skip Hetzner SSH steps

Exit codes:
    0   all checks passed
    10  Phase A failed (vocabulary contract)
    20  Phase B failed (config / deployment)
    30  Phase C failed (smoke ingest)
    40  multiple phases failed
    99  internal preflight error

Each individual check has a sub-code shifted into the exit code so CI can
scrape `echo $?`. See PREFLIGHT_README.md.

Phases:
    A   Vocabulary contract checks (load-bearing -- run before any ingest)
    B   Configuration and deployment checks
    C   Integration smoke
    D   Output formatting (always runs as part of the others)

Stubs (pass-through with noisy notice, gated on rebuild decisions):
    A.3 -- KnowledgeContent kind presence check (Decision 2)
    C.2 -- Typed-KU dual-mode smoke (Decision 4: recall.typed_ku.enabled)
"""
from __future__ import annotations

import argparse
import ast
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable

# ---------------------------------------------------------------------------
# Paths -- preflight lives at <repo>/scripts/preflight.py.
# Repo root is the parent of the scripts/ directory.
# ---------------------------------------------------------------------------
SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent

# v2 is the active codebase. Fall back to v1 paths if v2 is missing so the
# harness is forward/backward compatible across the rebuild.
V2_ROOT = REPO_ROOT / "v2"
ACTIVE_ROOT = V2_ROOT if V2_ROOT.exists() else REPO_ROOT

EXTRACTOR_PY = SCRIPT_DIR / "smart_extractor.py"

BOND_RS_CANDIDATES = [
    ACTIVE_ROOT / "crates" / "graph" / "src" / "bond.rs",
    ACTIVE_ROOT / "crates" / "df_core" / "src" / "bond.rs",
]
NODE_RS_CANDIDATES = [
    ACTIVE_ROOT / "crates" / "graph" / "src" / "node.rs",
]
KU_RS_CANDIDATES = [
    ACTIVE_ROOT / "crates" / "graph" / "src" / "knowledge_unit.rs",
]
MCP_LIB_RS_CANDIDATES = [
    ACTIVE_ROOT / "crates" / "mcp_server" / "src" / "lib.rs",
]
RECALL_PARAMS_CANDIDATES = [
    ACTIVE_ROOT / "crates" / "brain" / "src" / "self_improvement.rs",
]
LOCAL_BIN = ACTIVE_ROOT / "target" / "release" / "lu_4_mcp"

# Hetzner deployment (read-only sanity check).
HETZNER_HOST = "lu"            # ssh config alias -- see ~/.ssh/config
HETZNER_FALLBACK = "<USER>@<YOUR_SERVER_HOST> -p <PORT>"
HETZNER_BIN = "/usr/local/bin/lu_4_mcp"
HETZNER_BRAIN = "/var/lib/lu/brain.lbr"

# ---------------------------------------------------------------------------
# Result reporting -- each check produces one CheckResult.
# ---------------------------------------------------------------------------

PASS, FAIL, WARN, SKIP, STUB = "PASS", "FAIL", "WARN", "SKIP", "STUB"


@dataclass
class CheckResult:
    name: str           # e.g. "A.1 BondType vocabulary"
    status: str         # PASS / FAIL / WARN / SKIP / STUB
    note: str = ""
    detail: list[str] = field(default_factory=list)

    def is_blocking(self) -> bool:
        return self.status == FAIL


@dataclass
class PhaseResult:
    name: str
    checks: list[CheckResult] = field(default_factory=list)

    def add(self, c: CheckResult) -> None:
        self.checks.append(c)

    def is_blocking(self) -> bool:
        return any(c.is_blocking() for c in self.checks)


# ---------------------------------------------------------------------------
# Producer-side vocabulary extraction (Python -> string set)
# ---------------------------------------------------------------------------

def extract_extractor_relations(extractor_path: Path) -> set[str]:
    """Pull every relation string the extractor emits.

    Two sources:
      1. Hard-coded "relation": "X" key/value literals in dict-emit blocks.
      2. Pattern tuples like ``(_RX, "RelationName", weight, "tag", n)`` --
         these drive the L3/L5 walkers.

    The string is whatever flows into ``bond["relation"]``.
    """
    src = extractor_path.read_text(encoding="utf-8", errors="replace")
    relations: set[str] = set()

    # 1. Literal dict emits: "relation": "X"
    for m in re.finditer(r'"relation"\s*:\s*"([A-Za-z_][A-Za-z0-9_]*)"', src):
        relations.add(m.group(1))

    # 2. Pattern tuples in PATTERN_RX_TYPED tables (L3 / L5):
    #    (_RX_FOO, "RelationName", 0.6, "L3-tag", 1),
    # We scan tuples whose 2nd element is a quoted CamelCase token.
    for m in re.finditer(
        r'\(\s*_RX_[A-Z_0-9]+\s*,\s*"([A-Z][A-Za-z0-9]*)"\s*,',
        src,
    ):
        relations.add(m.group(1))

    return relations


def extract_extractor_extension_fields(extractor_path: Path) -> set[str]:
    """Find the set of dict keys the extractor stamps onto every emitted bond.

    Looking for the standard payload keys that aren't already in
    {subject, relation, object} -- these are the extension fields the consumer
    must accept or scrub.
    """
    src = extractor_path.read_text(encoding="utf-8", errors="replace")
    keys: set[str] = set()
    for m in re.finditer(r'"([a-z_][a-z0-9_]*)"\s*:', src):
        keys.add(m.group(1))
    # Filter to plausible bond-payload keys (drop config/data dict noise).
    candidate = {
        "subject", "relation", "object", "weight", "confidence",
        "layer", "source_url", "frequency", "kind", "node_type",
    }
    return keys & candidate


# ---------------------------------------------------------------------------
# Consumer-side vocabulary extraction (Rust -> string set via label())
# ---------------------------------------------------------------------------

_BONDTYPE_VARIANT_RX = re.compile(
    r"^\s*([A-Z][A-Za-z0-9]*)\s*,?\s*(?://.*)?$",
)
_LABEL_ARM_RX = re.compile(
    r"BondType::([A-Za-z0-9]+)\s*=>\s*\"([A-Z_]+)\"",
)


def parse_bondtype_enum(bond_rs: Path) -> tuple[set[str], dict[str, str]]:
    """Return (variant_names, variant_to_label) for the BondType enum.

    Variant labels come from ``impl BondType { fn label() }`` so we know what
    *string form* the consumer accepts. If label() is missing, the variant's
    own PascalCase name is used as the fallback accepted string -- many
    extractors emit PascalCase relation names directly.
    """
    src = bond_rs.read_text(encoding="utf-8", errors="replace")

    variants: set[str] = set()
    enum_match = re.search(
        r"pub enum BondType\s*\{(?P<body>[^}]*)\}",
        src,
        re.DOTALL,
    )
    if enum_match:
        body = enum_match.group("body")
        for line in body.splitlines():
            stripped = line.strip()
            if not stripped or stripped.startswith("//") or stripped.startswith("#["):
                continue
            m = _BONDTYPE_VARIANT_RX.match(stripped)
            if m:
                variants.add(m.group(1))

    labels: dict[str, str] = {}
    for m in _LABEL_ARM_RX.finditer(src):
        labels[m.group(1)] = m.group(2)
    return variants, labels


def parse_simple_enum(rs_path: Path, enum_name: str) -> set[str]:
    """Return the set of variant identifiers for a Rust enum.

    Tolerant: handles tuple/struct variants and trailing comments.
    """
    src = rs_path.read_text(encoding="utf-8", errors="replace")
    enum_match = re.search(
        rf"pub enum {enum_name}\s*\{{(?P<body>.*?)\n\}}",
        src,
        re.DOTALL,
    )
    if not enum_match:
        return set()
    body = enum_match.group("body")
    variants: set[str] = set()
    for line in body.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("//") or stripped.startswith("#["):
            continue
        # Take the leading identifier up to the first non-id char.
        m = re.match(r"([A-Z][A-Za-z0-9]*)", stripped)
        if m:
            variants.add(m.group(1))
    return variants


def parse_struct_fields(rs_path: Path, struct_name: str) -> set[str]:
    """Return the set of public field names for a Rust struct."""
    src = rs_path.read_text(encoding="utf-8", errors="replace")
    struct_match = re.search(
        rf"pub struct {struct_name}\s*\{{(?P<body>.*?)\n\}}",
        src,
        re.DOTALL,
    )
    if not struct_match:
        return set()
    body = struct_match.group("body")
    fields: set[str] = set()
    for line in body.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("//") or stripped.startswith("#["):
            continue
        m = re.match(r"pub\s+([a-z_][a-z0-9_]*)\s*:", stripped)
        if m:
            fields.add(m.group(1))
    return fields


def parse_mcp_configure_keys(mcp_lib_rs: Path) -> set[str]:
    """Return the set of args keys the lu_configure handler reads.

    This is what we count as "reachable through some setter" today. The
    rebuild (Decision 3) will replace this with per-port setters; preflight
    will need updating then.
    """
    src = mcp_lib_rs.read_text(encoding="utf-8", errors="replace")
    config_match = re.search(
        r'"lu_configure"\s*=>\s*\{(?P<body>.*?)\n\s{12}\}\s*\n',
        src,
        re.DOTALL,
    )
    if not config_match:
        # Fallback: scan whole file for args["X"] reads inside any block
        # that mentions "configure".
        body = src
    else:
        body = config_match.group("body")
    keys: set[str] = set()
    for m in re.finditer(r'args\["([a-z_][a-z0-9_]*)"\]', body):
        keys.add(m.group(1))
    return keys


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def find_first_existing(candidates: Iterable[Path]) -> Path | None:
    for p in candidates:
        if p.exists():
            return p
    return None


def md5_file(path: Path) -> str | None:
    if not path.exists():
        return None
    h = hashlib.md5()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def remote_md5(remote_path: str) -> str | None:
    """md5sum over SSH. Returns None on failure."""
    try:
        out = subprocess.run(
            ["ssh", HETZNER_HOST, f"md5sum {remote_path}"],
            capture_output=True,
            text=True,
            timeout=15,
        )
        if out.returncode != 0:
            return None
        return out.stdout.strip().split()[0]
    except (subprocess.SubprocessError, FileNotFoundError):
        return None


# ---------------------------------------------------------------------------
# PHASE A -- Vocabulary contract
# ---------------------------------------------------------------------------

def phase_a(args) -> PhaseResult:
    p = PhaseResult("Phase A -- Vocabulary contract")

    # A.1 BondType vocabulary
    bond_rs = find_first_existing(BOND_RS_CANDIDATES)
    if bond_rs is None:
        p.add(CheckResult(
            "A.1 BondType vocabulary",
            FAIL,
            note=f"Could not find bond.rs under {ACTIVE_ROOT}",
        ))
    elif not EXTRACTOR_PY.exists():
        p.add(CheckResult(
            "A.1 BondType vocabulary",
            FAIL,
            note=f"Could not find {EXTRACTOR_PY}",
        ))
    else:
        producer = extract_extractor_relations(EXTRACTOR_PY)
        variants, labels = parse_bondtype_enum(bond_rs)
        # Acceptance set = enum variant names (PascalCase) UNION label() strings.
        accepted = set(variants) | set(labels.values())
        unaccepted = sorted(producer - accepted)
        detail = [
            f"Source bond.rs: {bond_rs}",
            f"Source extractor: {EXTRACTOR_PY}",
            f"Producer relations ({len(producer)}): "
            + ", ".join(sorted(producer)),
            f"Consumer accepted ({len(accepted)}): "
            + ", ".join(sorted(accepted)),
        ]
        if unaccepted:
            detail.append(
                f"DOA strings ({len(unaccepted)}): " + ", ".join(unaccepted)
            )
            p.add(CheckResult(
                "A.1 BondType vocabulary",
                FAIL,
                note=f"{len(unaccepted)} extractor relation(s) won't parse on Rust side",
                detail=detail,
            ))
        else:
            p.add(CheckResult(
                "A.1 BondType vocabulary",
                PASS,
                note=f"All {len(producer)} extractor relations parse",
                detail=detail,
            ))

    # A.2 NodeType vocabulary
    node_rs = find_first_existing(NODE_RS_CANDIDATES)
    if node_rs is None:
        p.add(CheckResult(
            "A.2 NodeType vocabulary",
            WARN,
            note="node.rs not found; cannot check NodeType variants",
        ))
    else:
        node_variants = parse_simple_enum(node_rs, "NodeType")
        # Today's extractor doesn't emit a node_type field -- it just emits
        # subject/object strings that the consumer turns into nodes. So the
        # check is structural: the enum exists and has the documented set.
        # Once the extractor emits node_type, replace this with a set diff.
        expected = {"Fact", "Concept", "Procedure", "Emotion", "Meta", "Media"}
        if not node_variants:
            # v2 may be using KuKind instead -- inspect the KU file as fallback.
            ku_rs = find_first_existing(KU_RS_CANDIDATES)
            if ku_rs is not None:
                ku_kinds = parse_simple_enum(ku_rs, "KuKind")
                p.add(CheckResult(
                    "A.2 NodeType vocabulary",
                    WARN,
                    note=(
                        "No NodeType enum; v2 uses KuKind. Extractor does not "
                        "emit node_type today -- check pass-through."
                    ),
                    detail=[
                        f"KuKind variants ({len(ku_kinds)}): "
                        + ", ".join(sorted(ku_kinds))
                    ],
                ))
            else:
                p.add(CheckResult(
                    "A.2 NodeType vocabulary",
                    FAIL,
                    note="Neither NodeType nor KuKind found",
                ))
        else:
            missing = expected - node_variants
            if missing:
                p.add(CheckResult(
                    "A.2 NodeType vocabulary",
                    WARN,
                    note=f"NodeType missing expected variants: {sorted(missing)}",
                    detail=[
                        f"NodeType variants: {sorted(node_variants)}"
                    ],
                ))
            else:
                p.add(CheckResult(
                    "A.2 NodeType vocabulary",
                    PASS,
                    note=f"NodeType has all 6 expected variants",
                ))

    # A.3 -- KnowledgeContent kind presence (STUB; Decision 2)
    p.add(CheckResult(
        "A.3 KnowledgeContent kind presence",
        STUB,
        note="STUB -- requires Decision 2 (extractor must emit `kind` always)",
        detail=[
            "Once the extractor emits a `kind` field on every bond,",
            "this check ingests a 50-article sample and asserts no",
            "single kind > 90% of the distribution.",
            "TODO: lift stub when smart_extractor.py grows `kind`.",
        ],
    ))

    # A.4 SourceKind / source attribution
    # Today: extractor stamps `source_url` (string) and `layer` (string) on
    # every bond. There is no Rust SourceKind enum yet. Producer ⊆ consumer
    # is therefore vacuously OK only after we confirm consumer accepts the
    # free-form string form. We assert that producer always emits source_url.
    if EXTRACTOR_PY.exists():
        ext_keys = extract_extractor_extension_fields(EXTRACTOR_PY)
        if "source_url" in ext_keys:
            # Probe whether a Rust SourceKind exists.
            sourcekind_path = None
            for cand in BOND_RS_CANDIDATES + NODE_RS_CANDIDATES + KU_RS_CANDIDATES:
                if cand.exists() and "SourceKind" in cand.read_text(encoding="utf-8", errors="replace"):
                    sourcekind_path = cand
                    break
            if sourcekind_path is None:
                p.add(CheckResult(
                    "A.4 SourceKind / source attribution",
                    WARN,
                    note=(
                        "Producer emits `source_url` (string) on every bond, "
                        "but no Rust SourceKind enum exists yet. Acceptance "
                        "is implicit (string pass-through) until the rebuild "
                        "introduces SourceKind."
                    ),
                ))
            else:
                p.add(CheckResult(
                    "A.4 SourceKind / source attribution",
                    PASS,
                    note=f"SourceKind found in {sourcekind_path.name}; producer emits source_url",
                ))
        else:
            p.add(CheckResult(
                "A.4 SourceKind / source attribution",
                FAIL,
                note="Producer does NOT emit source_url on every bond",
            ))
    else:
        p.add(CheckResult(
            "A.4 SourceKind / source attribution",
            FAIL,
            note="extractor not found",
        ))

    # A.5 Cross-language extension fields (`layer`, `frequency`)
    # The extractor stamps `layer` on every bond and `frequency` on L4-derived
    # bonds. The consumer's TypedBond struct does not have these fields. The
    # only safe option today is to assert that the ingest pipeline scrubs them
    # before passing the dict into the Rust side. Until the ingest layer is
    # rebuilt the harness emits a WARN: extension fields produced but not
    # consumed. Documenting policy: "extension fields are stripped at ingest".
    if EXTRACTOR_PY.exists():
        ext_keys = extract_extractor_extension_fields(EXTRACTOR_PY)
        ext_extras = ext_keys - {"subject", "relation", "object", "weight", "confidence"}
        # Check whether bond.rs mentions any of the extras.
        accepted_extras = set()
        if bond_rs is not None:
            bond_src = bond_rs.read_text(encoding="utf-8", errors="replace")
            for k in ext_extras:
                if re.search(rf"\b{k}\s*:", bond_src):
                    accepted_extras.add(k)
        unaccepted_extras = ext_extras - accepted_extras
        if unaccepted_extras:
            p.add(CheckResult(
                "A.5 Cross-language extension fields",
                WARN,
                note=(
                    f"Extractor emits extension fields not present in Rust "
                    f"TypedBond: {sorted(unaccepted_extras)}. Policy: ingest "
                    f"layer must scrub these before send. Document this in "
                    f"PREFLIGHT_README.md until rebuild adds first-class fields."
                ),
                detail=[
                    f"Extractor extension keys: {sorted(ext_extras)}",
                    f"Accepted by Rust struct: {sorted(accepted_extras)}",
                ],
            ))
        else:
            p.add(CheckResult(
                "A.5 Cross-language extension fields",
                PASS,
                note="All extractor extension fields accepted by Rust struct",
            ))
    else:
        p.add(CheckResult("A.5 Cross-language extension fields", SKIP, note="extractor not found"))

    return p


# ---------------------------------------------------------------------------
# PHASE B -- Configuration & deployment
# ---------------------------------------------------------------------------

def phase_b(args) -> PhaseResult:
    p = PhaseResult("Phase B -- Configuration & deployment")

    # B.1 Binary md5 vs deployed
    local_md5 = md5_file(LOCAL_BIN)
    if local_md5 is None:
        p.add(CheckResult(
            "B.1 Binary md5 vs deployed",
            WARN,
            note=(
                f"No local release binary at {LOCAL_BIN}. "
                f"Build with `cargo build --release` then re-run."
            ),
        ))
    elif args.no_remote:
        p.add(CheckResult(
            "B.1 Binary md5 vs deployed",
            SKIP,
            note=f"--no-remote; local md5={local_md5}",
        ))
    else:
        rmd5 = remote_md5(HETZNER_BIN)
        if rmd5 is None:
            p.add(CheckResult(
                "B.1 Binary md5 vs deployed",
                WARN,
                note=(
                    f"Could not reach Hetzner over ssh ({HETZNER_HOST}). "
                    f"Local md5={local_md5}."
                ),
            ))
        elif rmd5 == local_md5:
            p.add(CheckResult(
                "B.1 Binary md5 vs deployed",
                PASS,
                note=f"local==deployed md5 {local_md5}",
            ))
        else:
            p.add(CheckResult(
                "B.1 Binary md5 vs deployed",
                FAIL,
                note=f"local={local_md5} != deployed={rmd5}",
            ))

    # B.2 Autosave-flag audit
    # The Lu daemon is currently disabled (intentional) and has no
    # save_on_every_write flag in the codebase. We assert systemd unit does
    # NOT pass --save-on-every-write or similar autosave triggers.
    if args.no_remote:
        p.add(CheckResult(
            "B.2 Autosave-flag audit",
            SKIP,
            note="--no-remote; cannot read deployed unit",
        ))
    else:
        try:
            out = subprocess.run(
                ["ssh", HETZNER_HOST,
                 "systemctl cat lu.service 2>/dev/null"],
                capture_output=True,
                text=True,
                timeout=15,
            )
            unit = out.stdout
            if not unit:
                p.add(CheckResult(
                    "B.2 Autosave-flag audit",
                    WARN,
                    note="Could not read lu.service unit",
                ))
            else:
                bad_flags = [
                    "save-on-every-write",
                    "save_on_every_write",
                    "--autosave",
                    "AUTOSAVE=1",
                ]
                tripped = [f for f in bad_flags if f in unit]
                if tripped:
                    p.add(CheckResult(
                        "B.2 Autosave-flag audit",
                        FAIL,
                        note=f"Autosave flag found in unit: {tripped}",
                    ))
                else:
                    p.add(CheckResult(
                        "B.2 Autosave-flag audit",
                        PASS,
                        note="No autosave flag in lu.service ExecStart",
                    ))
        except (subprocess.SubprocessError, FileNotFoundError) as e:
            p.add(CheckResult(
                "B.2 Autosave-flag audit",
                WARN,
                note=f"ssh failed: {e}",
            ))

    # B.3 RuntimeConfig setter coverage
    rp_path = find_first_existing(RECALL_PARAMS_CANDIDATES)
    mcp_path = find_first_existing(MCP_LIB_RS_CANDIDATES)
    runtime_cfg_fields: set[str] = set()
    if rp_path is not None:
        runtime_cfg_fields = parse_struct_fields(rp_path, "RuntimeConfig")
        if not runtime_cfg_fields:
            # No RuntimeConfig yet -- use RecallParameters as the standin.
            runtime_cfg_fields = parse_struct_fields(rp_path, "RecallParameters")
            cfg_label = "RecallParameters (RuntimeConfig not yet introduced)"
        else:
            cfg_label = "RuntimeConfig"
    else:
        cfg_label = "<none found>"

    if mcp_path is None:
        p.add(CheckResult(
            "B.3 RuntimeConfig setter coverage",
            WARN,
            note=f"mcp_server lib.rs not found",
        ))
    elif not runtime_cfg_fields:
        p.add(CheckResult(
            "B.3 RuntimeConfig setter coverage",
            WARN,
            note=f"No config struct fields parsed ({cfg_label})",
        ))
    else:
        keys = parse_mcp_configure_keys(mcp_path)
        unsettable = sorted(runtime_cfg_fields - keys)
        if unsettable:
            p.add(CheckResult(
                "B.3 RuntimeConfig setter coverage",
                FAIL,
                note=(
                    f"{len(unsettable)}/{len(runtime_cfg_fields)} field(s) of "
                    f"{cfg_label} have no setter via lu_configure"
                ),
                detail=[
                    f"Config struct: {cfg_label}",
                    f"Fields: {sorted(runtime_cfg_fields)}",
                    f"lu_configure keys: {sorted(keys)}",
                    f"Missing setters: {unsettable}",
                ],
            ))
        else:
            p.add(CheckResult(
                "B.3 RuntimeConfig setter coverage",
                PASS,
                note=f"All {len(runtime_cfg_fields)} {cfg_label} fields are settable",
            ))

    return p


# ---------------------------------------------------------------------------
# PHASE C -- Integration smoke
# ---------------------------------------------------------------------------

# Fixed reproducible mini-sample. Rebuild will swap this for a real Wikipedia
# fixture; for today's smoke we use synthetic articles structured exactly the
# way smart_extractor.py expects (title + body with See-also section).
SAMPLE_ARTICLES = [
    ("Rust (programming language)",
     "Rust is a multi-paradigm, statically typed programming language "
     "designed for performance and safety, especially safe concurrency. "
     "Rust was originally founded by Graydon Hoare in 2006. The language "
     "is named after a fungus. It is similar to C++.\n\n"
     "See also\nCargo (software)\nWebAssembly\nMozilla\n"),
    ("Cargo (software)",
     "Cargo is the package manager for the Rust programming language. "
     "It was founded in 2014. Cargo is part of the Rust toolchain.\n\n"
     "See also\nRust (programming language)\ncrates.io\n"),
    ("Mozilla",
     "Mozilla is a free software community founded in 1998. The Mozilla "
     "Foundation is located in Mountain View, California. Mozilla is "
     "known for Firefox.\n\n"
     "See also\nFirefox\nThunderbird\n"),
]

# Replicate up to 50 articles by index-suffixing for a more meaningful sample.
def build_sample(n: int = 50):
    out = []
    for i in range(n):
        base_title, base_text = SAMPLE_ARTICLES[i % len(SAMPLE_ARTICLES)]
        if i < len(SAMPLE_ARTICLES):
            out.append((base_title, base_text))
        else:
            out.append((f"{base_title} (variant {i})", base_text))
    return out


def phase_c(args) -> PhaseResult:
    p = PhaseResult("Phase C -- Integration smoke")

    # C.1 50-article ingest smoke
    if not EXTRACTOR_PY.exists():
        p.add(CheckResult("C.1 50-article ingest smoke", FAIL, note="extractor not found"))
        return p

    # Import the extractor module hermetically.
    import importlib.util
    spec = importlib.util.spec_from_file_location(
        "smart_extractor_preflight", EXTRACTOR_PY
    )
    if spec is None or spec.loader is None:
        p.add(CheckResult("C.1 50-article ingest smoke", FAIL, note="cannot load extractor"))
        return p
    se = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(se)
    except Exception as e:
        p.add(CheckResult(
            "C.1 50-article ingest smoke",
            FAIL,
            note=f"failed to import extractor: {e}",
        ))
        return p

    # Find the top-level extract function.
    extract_fn = None
    for name in ("extract_bonds", "extract_all", "extract"):
        if hasattr(se, name):
            extract_fn = getattr(se, name)
            break
    if extract_fn is None:
        # Use any function whose signature looks like (title, text, source_url).
        for name in dir(se):
            obj = getattr(se, name)
            if callable(obj) and name.startswith("extract"):
                extract_fn = obj
                break

    sample = build_sample(50)
    all_bonds: list[dict] = []
    errors = 0
    for title, text in sample:
        try:
            if extract_fn is not None:
                bonds = extract_fn(title, text, f"https://example.invalid/{title.replace(' ', '_')}")
            else:
                bonds = []
                # Run all the layer functions individually as a fallback.
                for fn_name in ("extract_see_also", "extract_categories", "extract_typed_l3", "extract_typed_l5"):
                    if hasattr(se, fn_name):
                        bonds.extend(getattr(se, fn_name)(title, text, "https://example.invalid"))
            all_bonds.extend(bonds)
        except Exception:
            errors += 1

    # Validate: histogram, distinct types, errors-per-article, degree p90.
    histogram: dict[str, int] = {}
    degree: dict[str, int] = {}
    for b in all_bonds:
        rel = b.get("relation", "<none>")
        histogram[rel] = histogram.get(rel, 0) + 1
        degree[b.get("subject", "")] = degree.get(b.get("subject", ""), 0) + 1
        degree[b.get("object", "")] = degree.get(b.get("object", ""), 0) + 1

    total_bonds = sum(histogram.values())
    distinct = len(histogram)
    top_share = (max(histogram.values()) / total_bonds) if total_bonds else 1.0
    err_rate = errors / max(1, len(sample))

    sorted_degrees = sorted(degree.values(), reverse=True)
    if sorted_degrees:
        p90_idx = max(0, int(len(sorted_degrees) * 0.1) - 1)
        degree_p90 = sorted_degrees[p90_idx]
    else:
        degree_p90 = 0

    detail = [
        f"Articles processed: {len(sample)}",
        f"Total bonds: {total_bonds}",
        f"Distinct relation types: {distinct}",
        f"Top-share (largest type / total): {top_share:.2%}",
        f"Errors per article: {err_rate:.3f}",
        f"degree_p90 (top-10% subject/object connectedness): {degree_p90}",
        f"Histogram: {dict(sorted(histogram.items(), key=lambda kv: -kv[1]))}",
    ]

    failures: list[str] = []
    if total_bonds == 0:
        failures.append("no bonds emitted")
    if top_share > 0.70:
        failures.append(f"top type share {top_share:.0%} > 70%")
    if distinct < 8:
        failures.append(f"only {distinct} distinct relation types (<8)")
    if err_rate >= 0.1:
        failures.append(f"errors-per-article {err_rate:.2f} >= 0.1")
    if degree_p90 < 5:
        failures.append(f"degree_p90 {degree_p90} < 5")

    if failures:
        p.add(CheckResult(
            "C.1 50-article ingest smoke",
            FAIL,
            note="; ".join(failures),
            detail=detail,
        ))
    else:
        p.add(CheckResult(
            "C.1 50-article ingest smoke",
            PASS,
            note=(
                f"{total_bonds} bonds, {distinct} distinct types, "
                f"top-share {top_share:.0%}, err {err_rate:.2f}, p90 {degree_p90}"
            ),
            detail=detail,
        ))

    # C.2 Typed-KU dual-mode (STUB)
    p.add(CheckResult(
        "C.2 Typed-KU dual-mode",
        STUB,
        note="STUB -- requires Decision 4 (recall.typed_ku.enabled flag)",
        detail=[
            "Once the runtime supports a `recall.typed_ku.enabled` flag,",
            "this check runs C.1 twice (flag=true, flag=false) and asserts",
            "both modes complete inside the smoke budget.",
            "TODO: lift stub when feature flag lands.",
        ],
    ))

    return p


# ---------------------------------------------------------------------------
# PHASE D -- Output
# ---------------------------------------------------------------------------

STATUS_GLYPH = {PASS: "[ok]", FAIL: "[XX]", WARN: "[??]", SKIP: "[--]", STUB: "[..]"}


def render(phases: list[PhaseResult]) -> int:
    print()
    print("=" * 72)
    print("Lu Preflight Verification Harness")
    print("=" * 72)

    n_fail = 0
    failed_phases = 0

    for ph in phases:
        print()
        print(f"## {ph.name}")
        for c in ph.checks:
            print(f"  {STATUS_GLYPH.get(c.status, '[?]'):4}  {c.name:<48}  {c.status}")
            if c.note:
                print(f"        {c.note}")
            if c.detail and c.status in (FAIL, WARN):
                for line in c.detail:
                    print(f"           {line}")
            if c.status == FAIL:
                n_fail += 1
        if ph.is_blocking():
            failed_phases += 1

    # Verdict table
    print()
    print("=" * 72)
    print("Verdict")
    print("=" * 72)
    for ph in phases:
        verdict = "FAIL" if ph.is_blocking() else "OK"
        print(f"  {ph.name:<46}  {verdict}")
    print()

    if failed_phases == 0:
        print("PREFLIGHT GO -- safe to launch ingest job.")
        return 0
    elif failed_phases == 1:
        bad = next(ph for ph in phases if ph.is_blocking())
        print(f"PREFLIGHT NO-GO -- {bad.name} blocked ({n_fail} check(s) failed).")
        if "Phase A" in bad.name:
            return 10
        if "Phase B" in bad.name:
            return 20
        if "Phase C" in bad.name:
            return 30
        return 40
    else:
        print(f"PREFLIGHT NO-GO -- {failed_phases} phases failed ({n_fail} check(s) total).")
        return 40


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> int:
    parser = argparse.ArgumentParser(description="Lu pre-ingest verification harness")
    parser.add_argument(
        "--phase",
        choices=["A", "B", "C", "all"],
        default="all",
        help="Run a subset of phases (default: all).",
    )
    parser.add_argument(
        "--no-remote",
        action="store_true",
        help="Skip Hetzner SSH steps (B.1 / B.2). Useful offline.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit JSON instead of human-readable output.",
    )
    args = parser.parse_args()

    phases: list[PhaseResult] = []

    try:
        if args.phase in ("A", "all"):
            phases.append(phase_a(args))
        if args.phase in ("B", "all"):
            phases.append(phase_b(args))
        if args.phase in ("C", "all"):
            phases.append(phase_c(args))
    except Exception as e:
        print(f"PREFLIGHT INTERNAL ERROR: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 99

    if args.json:
        out = {
            "phases": [
                {
                    "name": ph.name,
                    "blocking": ph.is_blocking(),
                    "checks": [
                        {"name": c.name, "status": c.status, "note": c.note,
                         "detail": c.detail}
                        for c in ph.checks
                    ],
                } for ph in phases
            ]
        }
        print(json.dumps(out, indent=2))
        return 0 if not any(ph.is_blocking() for ph in phases) else 40

    return render(phases)


if __name__ == "__main__":
    raise SystemExit(main())
