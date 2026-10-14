#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Lu AI — Systems, OS & Media Curriculum v2

Generates knowledge about file types, operating systems, command line tools,
and media formats (image/video/audio) using Ollama.
Produces curriculum_systems_data.json for batch-loading into Lu's brain.

Target: ~4,000 atomic facts across 6 knowledge domains.

Usage:
    python train_curriculum_systems.py --generate
    python train_curriculum_systems.py --generate --domain filetypes
    python train_curriculum_systems.py --dry-run
    python train_curriculum_systems.py --generate --resume

Created by John Dean Martin (Lu AI Project)
"""

import json
import os
import sys
import argparse
import time
import re
import urllib.request
import urllib.error
from typing import Optional

# --- Config -----------------------------------------------------------------

OLLAMA_URL = "http://localhost:11434/api/generate"
DEFAULT_MODEL = "llama3.1:8b"
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
OUTPUT_FILE = os.path.join(SCRIPT_DIR, "curriculum_systems_data.json")

MAX_RETRIES = 3
RETRY_DELAY = 5  # seconds
REQUEST_TIMEOUT = 300  # 5 minutes per Ollama call


# --- Ollama Interface -------------------------------------------------------

SYSTEM_PROMPT = """You are a knowledge extraction engine. You output ONLY valid JSON.
No markdown, no explanation, no code blocks, no preamble.
Output exactly this structure:
{"facts": ["fact 1", "fact 2", ...]}
Each fact must be a single, specific, standalone statement.
No vague generalities. No filler like 'this is important because'.
Just clear, concrete, atomic knowledge."""


def ask_ollama(prompt: str, model: str = DEFAULT_MODEL) -> Optional[str]:
    """Call Ollama API and return the response text."""
    payload = {
        "model": model,
        "prompt": prompt,
        "system": SYSTEM_PROMPT,
        "stream": False,
        "format": "json",
        "options": {
            "temperature": 0.5,
            "top_p": 0.9,
            "num_predict": 4096,
        }
    }
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        OLLAMA_URL, data=data,
        headers={"Content-Type": "application/json"}
    )
    try:
        with urllib.request.urlopen(req, timeout=REQUEST_TIMEOUT) as resp:
            result = json.loads(resp.read().decode("utf-8"))
            return result.get("response", "")
    except urllib.error.URLError as e:
        print(f"  [ollama] Connection error: {e}")
        return None
    except Exception as e:
        print(f"  [ollama] Unexpected error: {e}")
        return None


def parse_facts(text: str) -> list[str]:
    """Parse fact list from Ollama JSON response. Handles messy output."""
    if not text:
        return []

    text = text.strip()
    # Strip markdown fences
    if "```" in text:
        text = re.sub(r'```(?:json)?\n?', '', text).strip()

    # Remove trailing commas before ] or }
    text = re.sub(r',\s*([}\]])', r'\1', text)

    # Try direct parse
    try:
        data = json.loads(text)
        facts = data.get("facts", [])
        if isinstance(facts, list):
            return [f.strip() for f in facts if isinstance(f, str) and f.strip()]
    except json.JSONDecodeError:
        pass

    # Try extracting JSON object
    match = re.search(r'\{[\s\S]*\}', text)
    if match:
        try:
            data = json.loads(match.group())
            facts = data.get("facts", [])
            if isinstance(facts, list):
                return [f.strip() for f in facts if isinstance(f, str) and f.strip()]
        except json.JSONDecodeError:
            pass

    # Last resort: try to find a JSON array
    match = re.search(r'\[[\s\S]*\]', text)
    if match:
        try:
            arr = json.loads(match.group())
            if isinstance(arr, list):
                return [f.strip() for f in arr if isinstance(f, str) and f.strip()]
        except json.JSONDecodeError:
            pass

    print(f"  [parse] Could not parse response: {text[:200]}...")
    return []


def generate_facts_for_topic(topic_name: str, prompt: str, model: str,
                              target_count: int = 30) -> list[str]:
    """Generate facts for a single topic with retries."""
    all_facts = []

    for attempt in range(MAX_RETRIES):
        remaining = target_count - len(all_facts)
        if remaining <= 0:
            break

        full_prompt = (
            f"{prompt}\n\n"
            f"Generate exactly {remaining} facts. "
            f"Each fact must be a single atomic statement. "
            f"Be specific and concrete — no vague generalities."
        )

        response = ask_ollama(full_prompt, model)
        if response is None:
            print(f"  [retry] Attempt {attempt + 1}/{MAX_RETRIES} failed for '{topic_name}'")
            if attempt < MAX_RETRIES - 1:
                time.sleep(RETRY_DELAY)
            continue

        facts = parse_facts(response)
        if facts:
            all_facts.extend(facts)
            if len(all_facts) >= target_count:
                break
        else:
            print(f"  [retry] No facts parsed on attempt {attempt + 1} for '{topic_name}'")
            if attempt < MAX_RETRIES - 1:
                time.sleep(RETRY_DELAY)

    # Deduplicate while preserving order
    seen = set()
    unique = []
    for f in all_facts:
        normalized = f.lower().strip().rstrip('.')
        if normalized not in seen:
            seen.add(normalized)
            unique.append(f)

    return unique[:target_count]


# --- Curriculum Definition --------------------------------------------------
#
# Each domain is a dict with:
#   - name: domain identifier (used for --domain flag)
#   - label: human-readable name
#   - layer: training order (lower = train first)
#   - topics: list of {name, prompt, count, associations}
#
# The prompt tells Ollama exactly what kind of facts to generate.
# associations are default tags that get attached to every fact in that topic.

CURRICULUM = [
    # ════════════════════════════════════════════════════════════════════════
    # LAYER 14: FILE TYPES & FORMATS
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "filetypes",
        "label": "File Types & Formats",
        "layer": 14,
        "topics": [
            {
                "name": "text_formats",
                "prompt": "Generate atomic facts about text file formats. Cover: TXT (plain text, no formatting), CSV (comma-separated values, data in rows/columns, delimiter issues), TSV (tab-separated), JSON (JavaScript Object Notation, key-value pairs, nested structure, used in APIs), XML (eXtensible Markup Language, tags, attributes, hierarchical), YAML (human-readable, indentation-based, key-value), TOML (Tom's Obvious Minimal Language, config files, tables), INI (initialization files, sections, key=value), LOG (log files, timestamps, levels: DEBUG/INFO/WARN/ERROR). For each format: what it is, structure, use cases, when to use which format, pros and cons.",
                "count": 70,
                "associations": ["text format", "file format", "data", "serialization"]
            },
            {
                "name": "document_formats",
                "prompt": "Generate atomic facts about document file formats. Cover: PDF (Portable Document Format, vector graphics, text, layout preservation, internally uses PostScript), DOCX (Office Open XML, zipped container, relationships.xml, document.xml, can embed media), ODT (OpenDocument Text, zipped, used by LibreOffice), RTF (Rich Text Format, control codes for formatting), EPUB (Electronic Publication, zipped XML, used for ebooks), LaTeX (typesetting language, text-based, used for technical docs). For each: internal structure, capabilities, limitations, when to use, software that opens it.",
                "count": 65,
                "associations": ["document format", "file format", "office", "formatting"]
            },
            {
                "name": "archive_compression",
                "prompt": "Generate atomic facts about archive and compression formats. Cover: ZIP (zipped archive, deflate compression, multiple files, supported everywhere), TAR (tape archive, no compression, groups files, used on Unix/Linux), GZ (Gzip, single-file compression, usually paired with tar), BZ2 (Bzip2, block sorting, higher compression than gzip), XZ (LZMA compression, best compression ratio), 7Z (7-Zip, proprietary, excellent compression), RAR (WinRAR, proprietary, good compression). For each: compression algorithm, compression ratio typical, use cases, platform support, how to decompress.",
                "count": 65,
                "associations": ["compression", "archive", "file format", "zip", "tar"]
            },
            {
                "name": "executable_binary",
                "prompt": "Generate atomic facts about executable and binary file formats. Cover: EXE (Windows executable, PE format, contains code and data), DLL (Dynamic Link Library, Windows, shared code, loaded at runtime), SO (Shared Object, Linux, ELF format, .so.1.2.3 versioning), ELF (Executable and Linkable Format, Unix/Linux standard, sections .text .data .bss, segments), Mach-O (Mach Object, macOS/iOS, fat binaries, code signing), WASM (WebAssembly, web browsers, bytecode, sandboxed). For each: internal structure, architecture-specific variants, how OS loads/executes, security considerations.",
                "count": 70,
                "associations": ["executable", "binary", "file format", "ELF", "PE"]
            },
            {
                "name": "image_formats_raster",
                "prompt": "Generate atomic facts about raster image file formats. Cover: PNG (lossless, transparency alpha channel, deflate compression, good for icons/web, 8/16-bit per channel), JPG/JPEG (lossy, DCT compression, quality setting 0-100, good for photos, no transparency), GIF (indexed color max 256 colors, lossless, animation support, transparency), BMP (bitmap, uncompressed, simple, large file sizes, used in legacy), TIFF (tagged, lossless or lossy, multi-page support, used in professional). For each format: compression type, color depth, transparency support, use cases, quality vs file size tradeoff.",
                "count": 70,
                "associations": ["image format", "raster", "compression", "file format"]
            },
            {
                "name": "image_formats_advanced",
                "prompt": "Generate atomic facts about advanced image file formats. Cover: WebP (developed by Google, lossy and lossless, smaller files than JPEG/PNG, growing browser support), SVG (Scalable Vector Graphics, text-based XML, scales without quality loss, good for logos), ICO (icon files, small, multiple resolutions in one file), RAW (raw sensor data, unprocessed, used in photography, format varies by camera), HEIF (High Efficiency Image Format, modern, smaller than JPEG, iOS default), AVIF (AV1 Image Format, next-generation, best compression, limited support). For each: how encoding works, advantages, limitations, browser support, use cases.",
                "count": 70,
                "associations": ["image format", "file format", "WebP", "SVG", "RAW"]
            },
            {
                "name": "audio_formats",
                "prompt": "Generate atomic facts about audio file formats. Cover: MP3 (MPEG-1 Layer 3, lossy, psychoacoustic model, bitrates 128-320 kbps, nearly universal support), WAV (uncompressed or ADPCM, linear PCM, large files, professional audio), FLAC (Free Lossless Audio Codec, lossless, good compression, supports metadata), AAC (Advanced Audio Codec, lossy, better quality than MP3 at same bitrate, used in iTunes/Apple), OGG (Vorbis codec, lossy, open source, free alternative to MP3), OPUS (modern codec, lossy, excellent quality at low bitrate, designed for streaming), M4A (MPEG-4 Audio, AAC codec in MP4 container). For each: bitrate, sample rate, channels, use cases, codec vs container distinction.",
                "count": 70,
                "associations": ["audio format", "file format", "codec", "MP3", "FLAC"]
            },
            {
                "name": "audio_codecs",
                "prompt": "Generate atomic facts about audio codecs and their characteristics. Cover: H.264 (video codec, not audio), H.265/HEVC (video codec), AAC (Advanced Audio Codec, better efficiency than MP3, stereo/surround), MP3 (MPEG-1 Layer 3, lossy, frequency-domain compression), Opus (modern, designed for internet, can adapt bitrate, excellent quality 64kbps stereo), FLAC (lossless, CRC check, uses LPC prediction), Vorbis (lossy, open source, used in OGG). For each audio codec: how compression works, quality levels, bitrates, use cases, licensing.",
                "count": 55,
                "associations": ["audio codec", "compression", "encoding", "file format"]
            },
            {
                "name": "video_formats",
                "prompt": "Generate atomic facts about video file formats and containers. Cover: MP4 (MPEG-4 container, most compatible, H.264 or H.265 video, AAC audio, widely supported), MKV (Matroska, open source, supports any codec, chapter support, high-quality but less compatible), AVI (Audio Video Interleave, older, less efficient, still used on Windows), MOV (QuickTime, Apple standard, similar to MP4, proprietary), WebM (Google, VP8/VP9 video, Vorbis/Opus audio, for web), WMV (Windows Media Video, proprietary Microsoft), FLV (Flash Video, older web format, mostly obsolete). For each: what codecs it can hold, compatibility, streaming capability, use cases.",
                "count": 70,
                "associations": ["video format", "container", "file format", "MP4", "MKV"]
            },
            {
                "name": "video_codecs",
                "prompt": "Generate atomic facts about video encoding codecs. Cover: H.264/AVC (MPEG-4 Part 10, widely supported, industry standard, I/P/B frames, bitrate ~1-8 Mbps for 1080p), H.265/HEVC (newer, 50% better compression than H.264, requires licensing, 4K capable), AV1 (newest, best compression, open source, slow to encode, limited hardware support), VP8 (VP8, older Google codec, used in WebM), VP9 (VP9, modern, used in WebM and YouTube, good compression). For each: compression ratio, encoding speed, hardware support, bitrate requirements, quality/file size tradeoff.",
                "count": 65,
                "associations": ["video codec", "encoding", "compression", "H.264", "H.265"]
            },
            {
                "name": "database_file_formats",
                "prompt": "Generate atomic facts about database and structured data file formats. Cover: SQLite (single file database, embedded in apps, .db/.sqlite extension, no server needed, ACID transactions, internal B-tree structure), MDB (Microsoft Access, proprietary, Jet engine, .mdb/.accdb, GUI tool), CSV (comma-separated values, flat, human-readable, no schema enforcement), Parquet (columnar storage, Apache format, highly compressed, fast analytical queries), Arrow (in-memory columnar format, zero-copy data sharing, used in data pipelines). For each: use cases, advantages, query capabilities, compression, when to choose which.",
                "count": 60,
                "associations": ["database", "file format", "structured data", "storage"]
            },
            {
                "name": "config_and_data_exchange",
                "prompt": "Generate atomic facts about configuration and data exchange file formats. Cover: JSON (key-value pairs, nested objects/arrays, no comments in standard, used in APIs), XML (tags, attributes, hierarchical, self-describing, verbose), YAML (human-readable, indentation, flow and block styles, no native comments issue), TOML (tables, simple, readable, good for app config), Protocol Buffers (binary, compact, schema-based, Google format), MessagePack (compact binary, schema-less), CSV (tabular data, parsing ambiguity with delimiters). For each: structure, parsing, schema validation, use cases, readability vs compactness.",
                "count": 65,
                "associations": ["config", "data exchange", "serialization", "format"]
            },
            {
                "name": "font_formats",
                "prompt": "Generate atomic facts about font file formats. Cover: TTF (TrueType Font, outline curves using quadratic Bezier, hinting for screen display, most widely supported), OTF (OpenType Font, developed by Adobe/Microsoft, cubic Bezier curves, more advanced features), WOFF (Web Open Font Format, TTF/OTF compressed for web, includes metadata, born from web usage), WOFF2 (improved compression, smaller than WOFF), EOT (Embedded OpenType, Microsoft, for IE embedding). For each: internal structure, outline math, color fonts, variable fonts, licensing/subsetting, use cases.",
                "count": 60,
                "associations": ["font", "file format", "typography", "web"]
            },
            {
                "name": "3d_cad_formats",
                "prompt": "Generate atomic facts about 3D and CAD file formats. Cover: OBJ (Wavefront, simple text format, vertices/normals/textures, no color/material embedded in format), STL (Stereolithography, triangular mesh, ASCII or binary, no color/texture, used in 3D printing), FBX (Autodesk, binary, complex, supports rigging/animation, widely used in game engines), GLTF (glTF, text-based JSON or binary GLB, designed for web/games, includes materials/textures, modern standard), STEP (Standard for Exchange of Product model Data, CAD interchange, parametric), IGES (Initial Graphics Exchange Specification, older CAD standard). For each: mesh vs parametric, supported features, size, use cases.",
                "count": 60,
                "associations": ["3D", "CAD", "model", "file format", "mesh"]
            },
            {
                "name": "disk_image_formats",
                "prompt": "Generate atomic facts about disk image file formats. Cover: ISO (International Organization for Standardization, sector-by-sector copy of optical media, bootable, read-only), IMG (generic disk image, can be raw sectors or sparse), DMG (Apple Disk Image, macOS, compression/encryption support, mounting directly), VHD/VHDX (Virtual Hard Disk, Microsoft, for Hyper-V, dynamic or fixed size), VMDK (Virtual Machine Disk, VMware format, growable or preallocated), QCOW2 (Quick Copy-on-Write, KVM/QEMU, snapshots, sparse allocation). For each: contents, mounting capabilities, virtualization use, compression, snapshots.",
                "count": 60,
                "associations": ["disk image", "ISO", "virtual machine", "file format"]
            },
            {
                "name": "markup_stylesheet_formats",
                "prompt": "Generate atomic facts about markup and stylesheet file formats. Cover: HTML (HyperText Markup Language, structure of web pages, tags/attributes, semantic elements), CSS (Cascading Style Sheets, styling rules, selectors, cascade/inheritance, responsive design), Markdown (lightweight markup, easy to read raw text, converts to HTML, used in documentation), reStructuredText (rst, Python docs standard, more features than Markdown), AsciiDoc (similar to rst, used in O'Reilly), LaTeX (typesetting language, mathematical notation, used in academic). For each: purpose, syntax, rendering, use cases, conversion tools.",
                "count": 60,
                "associations": ["markup", "stylesheet", "HTML", "CSS", "file format"]
            },
            {
                "name": "certificate_key_formats",
                "prompt": "Generate atomic facts about certificate and key file formats. Cover: PEM (Privacy Enhanced Mail, base64-encoded, human-readable, used for certs/keys/CSRs, -----BEGIN/END blocks), DER (Distinguished Encoding Rules, binary, compact, not human-readable, used for certs/keys), PKCS#12/PFX (package certificate and private key together, binary, password-protected), CRT/CER (certificate files, can be PEM or DER encoded), CSR (Certificate Signing Request, sent to CA, public key + identifying info), KEY (private key file, PEM or DER format, must be protected). For each: structure, encoding, X.509 standard, certificate chains, use cases.",
                "count": 60,
                "associations": ["certificate", "key", "cryptography", "PEM", "X.509"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 15: WINDOWS COMMAND LINE
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "windows_cli",
        "label": "Windows Command Line",
        "layer": 15,
        "topics": [
            {
                "name": "cmd_basics",
                "prompt": "Generate atomic facts about basic Windows CMD commands. Cover: DIR (list directory, /A for attributes, /S for subdirectories), CD (change directory, /D to change drive), CLS (clear screen), ECHO (print text, can redirect to file), TYPE (display file contents), COPY (copy files, /Y to overwrite), MOVE (move/rename files), DEL (delete files, /S recursive), MKDIR (create directory), RMDIR (remove empty directory), REN (rename file), PATH (show/set executable path). For each command: basic syntax, common flags, examples, when to use.",
                "count": 60,
                "associations": ["CMD", "Windows", "command line", "shell"]
            },
            {
                "name": "cmd_file_operations",
                "prompt": "Generate atomic facts about advanced Windows CMD file operations. Cover: XCOPY (copy with subdirs, /E copy empty dirs, /H copy hidden/system, /Y suppress prompt), ROBOCOPY (robust copy, multithreaded, /MIR mirror, /R retry count), FC (file compare, finds differences between files), COMP (compare bytes of files), ATTRIB (show/change file attributes, +H hide, +R readonly, +S system, +A archive), TREE (display directory structure graphically), FIND (search text in files), FINDSTR (find using regular expressions), WHERE (find file in PATH). For each: what it does, common options, output format, use cases.",
                "count": 60,
                "associations": ["CMD", "file operations", "Windows", "command line"]
            },
            {
                "name": "cmd_system_commands",
                "prompt": "Generate atomic facts about Windows CMD system commands. Cover: TASKLIST (show running processes, /V verbose, /M show DLLs), TASKKILL (terminate process, /F force, /PID process id, /IM image name), SYSTEMINFO (hardware info, OS version, memory, network adapters), HOSTNAME (show computer name), IPCONFIG (show network config, /all detailed, /release DHCP, /renew DHCP), PING (test reachability, -n count, -w timeout), TRACERT (trace route to destination), NETSTAT (network statistics, -an show all ports), NSLOOKUP (DNS lookup, query DNS servers). For each: output format, common options, what information it reveals.",
                "count": 65,
                "associations": ["CMD", "system commands", "networking", "Windows"]
            },
            {
                "name": "cmd_environment",
                "prompt": "Generate atomic facts about Windows CMD environment and variables. Cover: SET (display/set environment variables, command line, /A arithmetic, /P prompt for input), SETX (permanently set variable, HKLM for system or default HKCU for user), PATH variable (list of directories searched for executables, separated by semicolons), environment variables: %USERPROFILE% (user home), %APPDATA% (roaming app data), %TEMP% (temp folder), %SYSTEMROOT% (Windows install path, usually C:\\\\Windows), %PROCESSOR_ARCHITECTURE% (x86 or x64), %COMPUTERNAME% (PC name). For each: what it contains, how to set permanently, how to use in batch files.",
                "count": 60,
                "associations": ["CMD", "environment", "variables", "Windows"]
            },
            {
                "name": "cmd_scripting",
                "prompt": "Generate atomic facts about Windows batch file scripting. Cover: Batch files (.bat or .cmd extension, commands executed line by line), Variables (%VAR% syntax, %* for all arguments, %1-%9 for numbered args), IF statements (IF EXIST file, IF ERRORLEVEL, IF '%VAR%'=='value', IF DEFINED), FOR loops (FOR %I IN (list) DO command, FOR /R recursive, FOR /F parse lines), GOTO (jump to label, :LABEL), CALL (call another batch file or function), ERRORLEVEL (exit code, set by programs and commands, 0=success), Pipes (| for command output), Redirection (> output to file, >> append, < input from file, 2> error output). For each: syntax, examples, variable scoping issues.",
                "count": 65,
                "associations": ["batch", "scripting", "CMD", "Windows"]
            },
            {
                "name": "powershell_basics",
                "prompt": "Generate atomic facts about basic PowerShell commands. Cover: Get-ChildItem (list dir/files, -Recurse for subdirs, -Filter pattern), Set-Location (change directory, alias cd), Write-Output (print text, alias echo), Get-Content (read file, alias cat, -Head first N lines, -Tail last N lines), Copy-Item (copy file/folder, -Recurse), Move-Item (move file/folder), Remove-Item (delete, -Recurse), New-Item (create file/folder, -ItemType), Get-Process (list running processes), Stop-Process (kill process, -Name or -Id), Get-Service (list services). For each command: PowerShell name, aliases, common parameters, examples.",
                "count": 65,
                "associations": ["PowerShell", "command line", "Windows"]
            },
            {
                "name": "powershell_advanced",
                "prompt": "Generate atomic facts about advanced PowerShell commands and pipeline. Cover: Get-Process (list processes with details), Stop-Process (terminate process, -Force), Get-Service (service status, -DisplayName pattern), Get-EventLog (Windows event log, -LogName Application/System/Security, -Newest N), Invoke-WebRequest (HTTP requests, -Uri, -Method GET/POST, -Body for data, -Headers custom headers), Select-Object (select properties, -Property name, -First N), Where-Object (filter objects, condition block), ForEach-Object (iterate objects, transform), Pipeline (| chains output of one command as input to next). For each: syntax, common use patterns, piping examples.",
                "count": 65,
                "associations": ["PowerShell", "advanced", "pipeline", "Windows"]
            },
            {
                "name": "powershell_scripting",
                "prompt": "Generate atomic facts about PowerShell scripting language. Cover: Variables ($VAR = value, $_ automatic), Arrays (@(1,2,3), $arr[0] indexing), Hashtables (@{key='value', another='data'}), Functions (function name { params code }), Conditional (if/elseif/else, switch statement), Loops (foreach, while, do-while, for), Try/Catch (exception handling, $_.Exception), Modules (Import-Module, PSM1 files), Execution policy (Set-ExecutionPolicy, Restricted/RemoteSigned/Unrestricted), Profiles ($PROFILE auto-load on startup). For each: syntax, examples, scope rules, best practices.",
                "count": 65,
                "associations": ["PowerShell", "scripting", "programming", "Windows"]
            },
            {
                "name": "windows_registry",
                "prompt": "Generate atomic facts about Windows Registry. Cover: REGEDIT (GUI registry editor), REG command (command line, REG ADD, REG DELETE, REG QUERY, REG EXPORT, REG IMPORT), Registry structure (HKLM Hardware/Software/System, HKCU user settings, HKCR file associations), Registry keys (folders containing values, HKLM path like HKEY_LOCAL_MACHINE\\\\Software\\\\Microsoft), Value types (REG_SZ string, REG_DWORD 32-bit number, REG_QWORD 64-bit, REG_BINARY binary data, REG_MULTI_SZ list), Registry paths in scripts (regedit /s file.reg import, reg import/export). For each: what it stores, how to edit, registry hives, backing up registry.",
                "count": 60,
                "associations": ["registry", "Windows", "system configuration"]
            },
            {
                "name": "windows_system_tools",
                "prompt": "Generate atomic facts about Windows system management tools. Cover: SFC /SCANNOW (System File Checker, verifies system file integrity, /OFFLINE to scan offline), CHKDSK (Check Disk, checks drive for errors, /F fix errors, /R recover bad sectors), DISM (Deployment Image Servicing Management, repair Windows image, /Online current system, /Cleanup-Image, /RestoreHealth), MSCONFIG (System Configuration, startup programs tab, services tab, diagnostic/selective startup), Services.msc (Services console, enable/disable/start/stop services, startup type automatic/manual/disabled), Devmgmt.msc (Device Manager, show hardware, drivers, update drivers), Diskmgmt.msc (Disk Management, volumes, partitions, format), Eventvwr (Event Viewer, Application/System/Security logs). For each: what it does, common tasks, when to use.",
                "count": 65,
                "associations": ["system tools", "Windows", "administration"]
            },
            {
                "name": "windows_networking_commands",
                "prompt": "Generate atomic facts about Windows networking commands. Cover: NETSH (network shell, netsh interface, netsh firewall rules, port forwarding, tunnel), IPCONFIG /ALL (detailed network info, DNS, DHCP server, MAC address), ARP -A (Address Resolution Protocol table, IP to MAC mapping), ROUTE PRINT (routing table, display routes), NBTSTAT (NetBIOS statistics, -n local computer name table, -c cached remote names), NET USE (map network drive, NET USE Z: \\\\\\\\server\\\\share, authentication), NET SHARE (share local folder, NET SHARE sharename=path /GRANT user,FULL), NET USER (user accounts, password, local users), NETSTAT -AN (network statistics, show listening ports and established connections). For each: output interpretation, common use cases, troubleshooting scenarios.",
                "count": 65,
                "associations": ["networking", "Windows", "command line"]
            },
            {
                "name": "windows_package_managers",
                "prompt": "Generate atomic facts about Windows package managers and tools. Cover: WINGET (Windows Package Manager, winget install package, winget search, winget upgrade, app repository), Chocolatey (third-party package manager, choco install package, elevated rights needed), Scoop (lightweight package manager, for developers, scoop install), WSL (Windows Subsystem for Linux, wsl --install, Linux distros, WSL 2 with Hyper-V), WMIC (Windows Management Instrumentation Command-line, deprecated in recent Windows, query system info). For each: installation, basic commands, use cases, differences.",
                "count": 55,
                "associations": ["package manager", "Windows", "installation", "tools"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 16: LINUX & BASH
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "linux_bash",
        "label": "Linux & Bash",
        "layer": 16,
        "topics": [
            {
                "name": "linux_filesystem_hierarchy",
                "prompt": "Generate atomic facts about Linux filesystem hierarchy. Cover: / (root directory, top of tree), /bin (essential binaries: ls, cp, mv), /sbin (system binaries: reboot, fsck, for root), /usr (user programs/libraries, /usr/bin, /usr/lib), /usr/local (locally installed software), /etc (configuration files, /etc/passwd, /etc/hosts, /etc/sudoers), /var (variable data: logs /var/log, mail /var/mail, spool /var/spool), /tmp (temporary files, world-writable, often cleared on reboot), /home (user home directories), /opt (optional software), /proc (virtual filesystem, process info, kernel info, /proc/cpuinfo, /proc/meminfo), /sys (kernel and device information), /dev (device files, /dev/sda disk, /dev/null, /dev/zero), /mnt (mount points, /mnt/usb), /media (removable media). For each: what it contains, who can write, typical contents.",
                "count": 70,
                "associations": ["filesystem", "directory structure", "Linux", "/"]
            },
            {
                "name": "bash_basics",
                "prompt": "Generate atomic facts about basic Bash commands. Cover: LS (list files, -l detailed, -a all including hidden, -R recursive, -t by time), CD (change directory, .. parent, ~ home, - previous), PWD (print working directory), CAT (concatenate/display file), ECHO (print text, -n no newline, -e escape sequences), CP (copy, -r recursive, -f force, -v verbose), MV (move/rename, -f force), RM (remove, -r recursive, -f force, -i interactive), MKDIR (make directory, -p parents), RMDIR (remove empty directory), TOUCH (create empty file, update timestamp), HEAD (first N lines, -n count), TAIL (last N lines, -n count, -f follow), LESS/MORE (page through file). For each: syntax, common flags, examples.",
                "count": 70,
                "associations": ["Bash", "command line", "Linux", "shell"]
            },
            {
                "name": "file_permissions",
                "prompt": "Generate atomic facts about Linux file permissions. Cover: CHMOD (change mode, u/g/o/a for user/group/other/all, +/-/= for add/remove/set, rwx for read/write/execute, octal notation 755=rwxr-xr-x, 644=rw-r--r--, 777=rwxrwxrwx), CHOWN (change owner, chown user:group file, -R recursive), CHGRP (change group), Permission bits (r=4 read, w=2 write, x=1 execute, sum to get octal), File vs directory permissions (x on dir=enter/list, x on file=execute), Umask (default permissions, 0022 means 755 for dirs/644 for files), Special permissions (4000 SUID, 2000 SGID, 1000 sticky bit), Symbolic links (ln -s, lrwxrwxrwx), ACLs (extended permissions). For each: how to use, examples, security implications.",
                "count": 70,
                "associations": ["permissions", "chmod", "ownership", "Linux", "security"]
            },
            {
                "name": "text_processing",
                "prompt": "Generate atomic facts about Linux text processing commands. Cover: GREP (search text, -i case-insensitive, -v invert/exclude, -c count lines, -n show line numbers, regex support), SED (stream editor, sed 's/old/new/g' substitute, delete lines with d, print ranges with p), AWK (text processing language, awk '{print $1}' field splitting, -F field separator, variables NR/NF), SORT (sort lines, -n numeric, -r reverse, -k field, -u unique), UNIQ (remove duplicate lines, -c count, -d only duplicates), CUT (cut columns, -d delimiter, -f field numbers), TR (translate/transform characters, tr 'a-z' 'A-Z' uppercase), PASTE (merge lines from files), DIFF (compare files), COMM (common lines between sorted files), TEE (read from stdin, output to stdout and file). For each: syntax, regex patterns, piping examples.",
                "count": 75,
                "associations": ["text processing", "grep", "sed", "awk", "Linux"]
            },
            {
                "name": "process_management",
                "prompt": "Generate atomic facts about Linux process management commands. Cover: PS (show processes, ps aux shows all with details, ps -ef another format, ps -C name by command), TOP (interactive process monitor, CPU/memory usage, P sort by CPU, M sort by memory), HTOP (improved top, colorful, tree view), KILL (send signal to process, kill -9 SIGKILL, kill -15 SIGTERM graceful), KILLALL (kill by name), NICE/RENICE (process priority, lower nice=higher priority, -20 to 19 range), BG (send job to background), FG (bring job to foreground), JOBS (list background jobs), NOHUP (continue running after logout), DISOWN (detach from terminal), SCREEN/TMUX (terminal multiplexer, sessions/windows). For each: syntax, when to use, signal types, job control.",
                "count": 70,
                "associations": ["process", "management", "Linux", "kill", "CPU"]
            },
            {
                "name": "package_management",
                "prompt": "Generate atomic facts about Linux package managers. Cover: APT/APT-GET (Debian/Ubuntu, apt update, apt install package, apt upgrade, apt remove, apt search), YUM/DNF (RedHat/CentOS/Fedora, yum install, dnf is newer), PACMAN (Arch Linux, pacman -S install, -R remove, -Q query, -U upgrade), SNAP (universal packages, snap install, snap update, containerized), FLATPAK (universal packages, flatpak install, sandboxed), DPKG (Debian package manager, dpkg -i install .deb file, -l list, -r remove), RPM (RedHat package manager, rpm -i install, -U upgrade, -q query, -e erase). Dependencies, repositories, system vs user packages. For each: how to install/remove, update system, search packages.",
                "count": 65,
                "associations": ["package manager", "apt", "yum", "Linux", "installation"]
            },
            {
                "name": "bash_scripting",
                "prompt": "Generate atomic facts about Bash shell scripting. Cover: Variables ($VAR, $1-$9 positional args, $@ all args, $# arg count, $0 script name), Quoting (single quotes literal, double quotes variable expansion, backticks/$(command substitution), $((arithmetic))), Conditionals (if/elif/else, test conditions -f file exists -d dir exists, [ ] test syntax), Case statement (case $var in pattern) esac), For loops (for item in list do done, for (( )) C-style), While/until loops (while condition, until condition), Functions (function name() { body; }), Exit codes ($? status, return from function), Special variables ($?, $!, $$, $-, $*, $@), Positional parameters and shift. For each: syntax, examples, common patterns.",
                "count": 75,
                "associations": ["Bash", "scripting", "shell", "programming", "Linux"]
            },
            {
                "name": "shell_features",
                "prompt": "Generate atomic facts about Bash shell features and I/O redirection. Cover: Pipes (| chain commands), Output redirection (> write to file, >> append, 2> stderr to file, &> both stdout and stderr, 2>&1 redirect stderr to stdout), Input redirection (< read from file), Here documents (<<EOF multi-line input EOF), Command substitution ($() or backticks, capture output), Subshells ((command) run in subshell), Process substitution (<(command) or >(command)), Job control (&, bg, fg, jobs), Globbing (*, ?, [abc], [a-z]), Brace expansion ({a,b,c}, {1..10}). For each: syntax, use cases, combining operations.",
                "count": 70,
                "associations": ["shell", "I/O", "redirection", "pipe", "Bash"]
            },
            {
                "name": "disk_and_storage",
                "prompt": "Generate atomic facts about Linux disk and storage commands. Cover: DF (disk free, show mounted filesystems, usage percentage, df -h human-readable), DU (disk usage, shows directory size, du -h, du -s summary, du -x exclude other filesystems), MOUNT (mount filesystem, mount /dev/sda1 /mnt, mount -t type), UMOUNT (unmount, umount /mnt), FDISK (partition table editor, fdisk -l list, fdisk device edit), LSBLK (list block devices in tree), BLKID (block device attributes, UUID, TYPE), MKFS (make filesystem, mkfs.ext4 /dev/sdX), FSTAB (/etc/fstab auto-mount at boot, format: device mount-point type options dump pass), LVM (Logical Volume Manager, logical volumes, snapshots, flexible resizing), RAID (redundant arrays). For each: typical usage, understanding output, data safety.",
                "count": 70,
                "associations": ["disk", "storage", "filesystem", "mount", "Linux"]
            },
            {
                "name": "networking_commands",
                "prompt": "Generate atomic facts about Linux networking commands. Cover: IP (modern replacement for ifconfig, ip addr show addresses, ip route show routes, ip link show interfaces), IFCONFIG (old-style, show interface config, inet IP address, HWaddr MAC), SS (socket statistics, ss -an show all listening ports and connections, replaces netstat), NETSTAT (network statistics, netstat -an, netstat -tulpn), CURL (HTTP requests, curl -X GET URL, -d data, -H headers, -L follow redirects), WGET (download files, wget URL, -O output file, -c continue partial), NC/NETCAT (network cat, read/write network connections, port scanning), DIG (DNS query, dig example.com A, dig @nameserver), HOST (DNS lookup), PING (test reachability, ping -c count, ping -W timeout), TRACEROUTE (show path to destination), IPTABLES/NFTABLES (firewall rules, stateful filtering). For each: common tasks, options, understanding output.",
                "count": 75,
                "associations": ["networking", "IP", "DNS", "Linux", "command line"]
            },
            {
                "name": "system_administration",
                "prompt": "Generate atomic facts about Linux system administration commands. Cover: SYSTEMCTL (manage systemd services, systemctl start/stop/restart service, systemctl enable auto-start, systemctl status), JOURNALCTL (systemd journal logs, journalctl -u service logs for service, -n last N lines, -f follow), CRONTAB (scheduled tasks, crontab -e edit, crontab -l list, minute hour day month weekday), AT (one-time scheduled task, at HH:MM date, atq queue, atrm remove), USERADD (create user, useradd -m create home dir), USERMOD (modify user, usermod -aG group user add to group), PASSWD (change password), GROUPS (show group membership), SUDO/VISUDO (run as root, /etc/sudoers file, NOPASSWD), /etc/passwd (/etc/shadow for password hashes, format user:x:uid:gid:info:home:shell), Runlevels/targets (single-user, multi-user, graphical). For each: common tasks, security considerations, file formats.",
                "count": 75,
                "associations": ["administration", "systemd", "user", "Linux", "root"]
            },
            {
                "name": "environment_and_shell_config",
                "prompt": "Generate atomic facts about Linux environment and shell configuration. Cover: .bashrc (loaded for interactive non-login shells, functions/aliases), .bash_profile (loaded for login shells, environment variables, sourcing .bashrc), .profile (POSIX shell config, login shell), PATH (colon-separated directories for executables, export PATH=$PATH:/new/path), EXPORT (make variable available to child processes, export VAR=value), ALIAS (create command shortcut, alias ll='ls -l', unalias to remove), SOURCE/DOT (load script into current shell, source .bashrc), Login vs non-login shell (interactive login runs .profile/.bash_profile, non-login runs .bashrc), Interactive vs non-interactive (interactive has prompts/colors, non-interactive runs scripts), Environment variables (PATH, HOME, USER, SHELL, TERM, LANG, EDITOR). For each: when it loads, what to put where, persistence.",
                "count": 70,
                "associations": ["shell config", "environment", "bashrc", "Linux"]
            },
            {
                "name": "compression_and_archiving",
                "prompt": "Generate atomic facts about Linux compression and archiving commands. Cover: TAR (tape archive, tar -cf archive.tar files create, -xf extract, -tf list, -z gzip compression, -j bzip2, -J xz, -C directory), GZIP/GUNZIP (compress .gz, gzip file compresses to file.gz, gunzip file.gz), BZIP2 (compression, typically slower but better ratio than gzip), XZ (xz compression, best compression), ZIP (zip archive.zip files create, unzip archive.zip), ZCAT (read gzipped file without decompressing, zcat file.gz), ZGREP (grep gzipped files). Tar common patterns: tar -czf archive.tar.gz dir (create), tar -xzf archive.tar.gz (extract), tar -tzf archive.tar.gz (list). For each: when to use, compression ratios, compatibility.",
                "count": 65,
                "associations": ["compression", "archive", "tar", "gzip", "Linux"]
            },
            {
                "name": "find_and_search",
                "prompt": "Generate atomic facts about Linux find and search commands. Cover: FIND (search filesystem, find -name pattern, -type f file/d dir/l symlink, -size +10M larger than 10MB, -mtime -7 modified in last 7 days, -exec command {} execute on results, -delete remove), LOCATE (fast filename search, updatedb to rebuild database, locate name), UPDATEDB (update locate database), WHICH (find command in PATH), WHEREIS (find executable/source/manual), TYPE (show command type, builtin/alias/function/file). Find regex and glob patterns: -iname case-insensitive, -regex full regex match, -path path pattern. Common find use: find . -name '*.log' -mtime +30 -delete (delete old logs). For each: syntax, performance considerations, practical examples.",
                "count": 65,
                "associations": ["find", "search", "locate", "Linux", "filesystem"]
            },
            {
                "name": "linux_boot_process",
                "prompt": "Generate atomic facts about Linux boot process and system startup. Cover: BIOS/UEFI (firmware, initializes hardware, locates bootloader, UEFI more modern with EFI System Partition), GRUB (Grand Unified Bootloader, menu to select kernel/OS, configuration in /etc/grub.d, regenerate with grub-mkconfig), Kernel loading (vmlinuz file, decompressed and executed by bootloader), INITRAMFS (initial ramdisk, minimal root filesystem, loads drivers/mounts real root), Init/systemd (first process, PID 1, starts other services, runs scripts in /etc/init.d on traditional or systemd units), Runlevels/targets (systemd targets: rescue.target single-user, multi-user.target no graphical, graphical.target X11), DMESG (kernel boot messages, dmesg | grep error find issues), /var/log/boot.log boot log). For each: what happens at each stage, troubleshooting boot issues, customizing boot.",
                "count": 65,
                "associations": ["boot", "kernel", "GRUB", "systemd", "Linux"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 17: IMAGE KNOWLEDGE
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "image_knowledge",
        "label": "Image Knowledge",
        "layer": 17,
        "topics": [
            {
                "name": "image_fundamentals",
                "prompt": "Generate atomic facts about digital image fundamentals. Cover: Pixels (smallest unit of image, color information), Resolution (width x height in pixels, e.g., 1920x1080), DPI/PPI (dots/pixels per inch, print vs screen, 72 PPI screen standard, 300 DPI print standard), Aspect ratio (width:height proportion, 16:9 widescreen, 4:3 standard, 1:1 square), Color depth (bits per pixel, 1-bit black/white, 8-bit grayscale 256 levels, 24-bit RGB 8 bits each, 32-bit with alpha), Bit depth (number of colors possible, 1-bit = 2 colors, 8-bit = 256 colors, 16-bit = 65536 colors), Image size on disk (resolution x color depth, 1920x1080x24bit = ~6MB uncompressed). For each: how it affects quality, storage, display.",
                "count": 70,
                "associations": ["image", "pixel", "resolution", "color depth", "digital"]
            },
            {
                "name": "color_models_and_spaces",
                "prompt": "Generate atomic facts about color models and color spaces. Cover: RGB (Red/Green/Blue, additive color, screen display, 0-255 per channel), CMYK (Cyan/Magenta/Yellow/Black, subtractive color, print, CMYK different from RGB due to ink), HSL/HSV (Hue/Saturation/Lightness or Value, more intuitive for humans, H 0-360 degrees, S 0-100%, L/V 0-100%), Lab (device-independent, L for lightness, a and b for color), sRGB (standard RGB, color space for internet, widely supported), Adobe RGB (larger color gamut for professional), Display P3 (Apple standard, wider gamut), Color gamut (range of colors that can be displayed), ICC profiles (color management, embed in files), Color conversion (from one space to another, loss possible). For each: use cases, gamut differences, when to use which.",
                "count": 70,
                "associations": ["color", "RGB", "CMYK", "color space", "gamut"]
            },
            {
                "name": "image_compression",
                "prompt": "Generate atomic facts about image compression techniques. Cover: Lossy compression (removes information, not recoverable, smaller file size, visible artifacts at high compression, used in JPEG/WebP lossy/HEIF), Lossless compression (all info preserved, larger file size, used in PNG/GIF/FLAC), JPEG compression (DCT Discrete Cosine Transform, divides image into 8x8 blocks, quality 1-100 setting, chroma subsampling 4:4:4 full color / 4:2:2 half horizontal / 4:2:0 half both, typical 75-85 quality for web), PNG deflate (dictionary-based, lossless, works best for graphics), WebP (developed by Google, both lossy and lossless, smaller than JPEG/PNG), Chroma subsampling (humans less sensitive to color detail, can store less color info), Quality settings (0-100%, higher = larger file, visual difference decreases at high quality). For each: compression ratio, use cases, quality tradeoffs.",
                "count": 70,
                "associations": ["compression", "lossy", "lossless", "JPEG", "PNG"]
            },
            {
                "name": "raster_vs_vector",
                "prompt": "Generate atomic facts about raster and vector graphics. Cover: Raster/bitmap (grid of pixels, fixed resolution, scales up shows pixelation, good for photos, formats PNG/JPEG/GIF), Vector (mathematical paths/curves, scale-independent, resolution-independent, formats SVG/EPS/PDF, good for logos/icons), Scaling (raster blurry when enlarged, vector crisp at any size), File size (raster larger for complex images, vector smaller for simple graphics), Editing (raster pixel-by-pixel, vector modify shapes), Rendering (raster direct to screen, vector rendered to raster for display), Vector format advantages (scalable, text remains searchable, small file size, animatable), Raster format advantages (natural for photos, predictable rendering, wide support), Conversion (vector to raster via rendering, raster to vector via tracing approximation). For each: when to use, advantages/disadvantages.",
                "count": 65,
                "associations": ["raster", "vector", "graphics", "SVG", "resolution"]
            },
            {
                "name": "image_metadata",
                "prompt": "Generate atomic facts about image metadata. Cover: EXIF (Exchangeable Image File Format, embedded in JPEG/TIFF, contains camera settings: aperture f-number, shutter speed, ISO sensitivity, focal length, flash status, white balance, metering mode, date/time taken, GPS coordinates, camera model), IPTC (International Press Telecommunications Council, keywords, caption, copyright, creator), XMP (Extensible Metadata Platform, XML-based, keywords, ratings, color labels, GPS, author), Tags (descriptive keywords), GPS coordinates (latitude/longitude, altitude in photos), Orientation tag (rotation flag, camera rotated when shooting), Creation date (when photo taken, embedded in EXIF), Camera info (make/model/lens), Processing history (edits applied). Tools (exiftool read/write, Properties dialog Windows, Preview macOS). For each: what it contains, privacy implications, how to view/edit, stripping metadata.",
                "count": 70,
                "associations": ["metadata", "EXIF", "GPS", "image information"]
            },
            {
                "name": "image_editing_concepts",
                "prompt": "Generate atomic facts about image editing concepts and techniques. Cover: Layers (stacked images, transparency support, blending, editing one without affecting others), Masks (define which parts of layer are visible, grayscale determines opacity), Selections (isolate area for editing, tools lasso/rectangle/magic wand), Blending modes (how layers combine: Normal, Multiply, Screen, Overlay, etc.), Histograms (distribution of tones from black to white, peak on left = dark, right = bright), Levels (adjust input/output range, correct exposure), Curves (fine-tune tone mapping, S-curve for contrast), Sharpening (enhance edges, unsharp mask technique), Noise reduction (reduce grain, particularly in shadows), Cropping (remove edges), Resizing (scale image, interpolation method affects quality), Interpolation (nearest neighbor blocky but fast, bilinear smooth, bicubic better, Lanczos best). For each: what it does, when to use, order of operations.",
                "count": 70,
                "associations": ["image editing", "Photoshop", "layer", "adjustment", "filter"]
            },
            {
                "name": "image_file_internals",
                "prompt": "Generate atomic facts about image file format internals and structure. Cover: PNG chunks (IHDR header, IDAT data, PLTE palette, tRNS transparency, ancillary chunks), JPEG markers (SOI start, APP0-APP15 application data, DQT quantization table, DHT Huffman table, SOF start of frame, SOS start of scan, EOI end), GIF frame structure (each frame separate, global color table, local color table, delay between frames, disposal method), TIFF tags (image directory, tags store metadata and image data, multiple images per file), Alpha channel (transparency layer, 0=fully transparent, 255=opaque, 8-bit or 1-bit), Color palettes (indexed color, palette stores actual RGB values, colors limited to palette), Interlacing (Adam7 in PNG, progressive in JPEG, allows preview while downloading), Chunks in PNG (critical vs ancillary, can ignore ancillary if unknown). For each: how data is organized, multi-image support, embedded metadata.",
                "count": 65,
                "associations": ["file format", "PNG", "JPEG", "structure", "chunks"]
            },
            {
                "name": "image_operations",
                "prompt": "Generate atomic facts about common image operations and transformations. Cover: Resize/Scale (change dimensions, aspect ratio preservation, quality loss if upscaling), Crop (remove edges, reframing, aspect ratio change possible), Rotate (90/180/270 degrees, arbitrary angle rotation), Flip (horizontal mirror, vertical mirror), Convert format (PNG to JPEG loses transparency, quality considerations), Adjust brightness (shift all tones), Adjust contrast (stretch tonal range), Adjust saturation (color intensity), Apply filters (blur, sharpen, distort, effects), Batch processing (apply operation to many files), Watermark (embed text/logo), Resize canvas (add borders), Extract region (copy part of image), Compose (combine multiple images). Tools (ImageMagick command line, GIMP GUI, Photoshop, Preview macOS, Paint.NET). For each: tool capability, batch options, automation.",
                "count": 65,
                "associations": ["image operations", "transformation", "ImageMagick", "GIMP"]
            },
            {
                "name": "screenshots_and_capture",
                "prompt": "Generate atomic facts about screenshots and screen capture. Cover: Screen resolution (monitor native resolution, typical 1920x1080/2560x1440/3840x2160), Display scaling (125%/150%/200% logical scaling on Windows/macOS, affects screenshot size), Retina/HiDPI displays (macOS Retina 2x pixel density, high DPI Windows displays), Screenshot tools (Print Screen button captures to clipboard, Shift+Print Screen selected window, Win+Shift+S snippet tool Windows, Cmd+Shift+3 full macOS, Cmd+Shift+4 selection macOS), Screen recording (video vs screenshot, built-in tools or third-party OBS), Screenshot format (BMP default, PNG/JPEG options), Multi-monitor (which display captured), Cursor inclusion (capture mouse cursor or not), Transparency (background color in web screenshots), Scaling considerations (scaling on HiDPI causes blurriness unless captured at native resolution). For each: tool availability by OS, image quality, capture modes.",
                "count": 60,
                "associations": ["screenshot", "screen capture", "resolution", "HiDPI"]
            },
            {
                "name": "images_for_web",
                "prompt": "Generate atomic facts about optimizing images for web. Cover: Responsive images (different sizes for different screen widths, srcset attribute, picture element), SRCSET (browser picks appropriate resolution/density, format: image.jpg 1x image-2x.jpg 2x, size: image.jpg 500w image-large.jpg 1000w), Picture element (art direction, different crops for different sizes), Lazy loading (load images when approaching viewport, native loading='lazy'), Format choice (JPEG for photos, PNG for graphics/transparency, WebP for better compression, AVIF newest), Compression tools (ImageMagick mogrify, PNGCrush, WebP conversion), Thumbnails (generate small previews), Progressive JPEG (load low-res first then refine), CDN (content delivery network for global distribution), Responsive images adoption (WebP fallback to JPEG), File size optimization (balance quality and size). For each: performance impact, browser support, implementation complexity.",
                "count": 65,
                "associations": ["web", "responsive", "optimization", "CDN", "lazy loading"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 18: VIDEO KNOWLEDGE
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "video_knowledge",
        "label": "Video Knowledge",
        "layer": 18,
        "topics": [
            {
                "name": "video_fundamentals",
                "prompt": "Generate atomic facts about video fundamentals. Cover: Frames (individual images in sequence), Frame rate (24fps film standard, 25fps PAL, 30fps NTSC, 60fps smooth motion, 120fps for slow-motion), Resolution (480p 852x480, 720p 1280x720, 1080p 1920x1080, 2K 2560x1440, 4K 3840x2160, 8K 7680x4320), Aspect ratio (16:9 widescreen standard, 4:3 older, 21:9 ultra-wide, 1:1 square), Interlaced vs progressive (interlaced alternates lines, progressive full frames, progressive better for computer), Bitrate (amount of data per second, 5 Mbps 720p acceptable quality, 10-15 Mbps 1080p good, 50+ Mbps 4K). Time and duration (fps determines playback speed, 1 second = frame_rate frames). For each: quality implications, storage requirements, display compatibility.",
                "count": 70,
                "associations": ["video", "frame rate", "resolution", "codec", "bitrate"]
            },
            {
                "name": "video_containers_codecs",
                "prompt": "Generate atomic facts about video containers and codecs. Cover: Container vs codec (container is the file format, codec is how video/audio is encoded, MP4 container can hold H.264 or H.265 video), MP4 container (MPEG-4 Part 14, supports H.264/H.265/VP9, AAC audio, widely compatible), MKV container (Matroska, flexible, supports any codec, chapter support, less universal), AVI container (older, Audio Video Interleave, limited codec support), MOV container (Apple QuickTime, similar to MP4), WebM container (Google, VP8/VP9 video, Vorbis/Opus audio), Muxing/demuxing (combining streams into container / extracting streams), Tracks (video track, audio track(s), subtitle track(s), metadata). Tools (ffmpeg identify/convert, MediaInfo show details). For each: codec compatibility, streaming capability, file size differences.",
                "count": 70,
                "associations": ["video", "container", "codec", "MP4", "MKV"]
            },
            {
                "name": "video_compression",
                "prompt": "Generate atomic facts about video compression and encoding. Cover: I-frames (intra, self-contained key frames, large file), P-frames (predicted, references previous frame, smaller), B-frames (bi-directional, references previous and next, smallest, not in fast playback), GOP (Group of Pictures, sequence of frames, typically I-frame every 30 frames, affects seeking accuracy and compression ratio), Motion estimation (comparing blocks between frames to find movement, more accurate=better compression but slower), Motion compensation (using motion vectors to reconstruct frames), Quantization (reduce precision of coefficients, aggressive = more compression but artifacts), Quality settings (CRF constant rate factor 0-51 in H.264, lower = better quality, 18-28 typical range), CBR vs VBR (constant bitrate vs variable, VBR adapts quality to content complexity), Two-pass encoding (first pass analyze, second encode with better allocation). For each: quality tradeoff, encoding time, seeking behavior.",
                "count": 70,
                "associations": ["compression", "encoding", "frame type", "GOP", "quality"]
            },
            {
                "name": "video_encoding_transcoding",
                "prompt": "Generate atomic facts about video encoding and transcoding. Cover: FFmpeg (command line tool, ffmpeg -i input.mp4 -c:v libx264 -crf 23 output.mp4 basic encode, -c:a aac audio codec, -preset ultrafast/superfast/fast/medium/slow encoding speed), Handbrake (GUI, presets for quality levels, batch conversion), Hardware encoding (NVIDIA NVENC GPU acceleration, Intel Quick Sync, AMD VCE, faster than CPU but less flexible), Presets (ultrafast/superfast/fast for quick encoding, medium balance, slow/slower/veryslow higher compression, trade speed for quality), Profile/level (constraints for device compatibility, Main profile typical, Level 4.1 for 1080p devices), 10-bit color (higher precision, smaller gamut cameras, not widely supported yet), Encoding speed vs quality (faster = worse quality at same bitrate, slower = better). Tools and techniques for transcoding, batch processing. For each: speed/quality tradeoff, hardware requirements, use cases.",
                "count": 70,
                "associations": ["encoding", "transcoding", "ffmpeg", "H.264", "bitrate"]
            },
            {
                "name": "streaming_protocols",
                "prompt": "Generate atomic facts about video streaming protocols. Cover: HLS (HTTP Live Streaming, Apple standard, works with plain HTTP, manifests .m3u8 playlists, segments .ts files, 10-30 second latency typical), DASH (Dynamic Adaptive Bitrate Streaming, manifests .mpd XML, segments .mp4, vendor-neutral, lower latency), RTMP (Real Time Messaging Protocol, flash/legacy, point-to-point, not HTTP friendly, deprecated), RTSP (Real Time Streaming Protocol, live streaming, not HTTP), WebRTC (peer-to-peer, ultra-low latency, used for video conferencing), Adaptive bitrate (client switches quality based on bandwidth, ladder of bitrates), Manifest files (m3u8 for HLS lists segments, mpd for DASH lists representations), Buffering (client buffers before playback, more buffering = more latency), Latency (delay from capture to playback, HLS 10-30s, DASH 4-8s, WebRTC <1s). For each: use cases, compatibility, latency/reliability tradeoff.",
                "count": 65,
                "associations": ["streaming", "HLS", "DASH", "protocol", "adaptive bitrate"]
            },
            {
                "name": "subtitles_and_captions",
                "prompt": "Generate atomic facts about subtitles and captions. Cover: SRT (SubRip, text-based, simple format, timing in hours:minutes:seconds, widely supported), ASS/SSA (Advanced SubStation Alpha, styled subtitles, colors/fonts/positions, complex format), VTT (WebVTT, web standard, similar to SRT but better), Closed captions (CC, includes sound descriptions [SOUND] [MUSIC], accessibility), Open captions (burned into video, always visible), Embedded vs external (embedded in video file, external .srt files), Subtitle rendering (positioning, font selection, background colors), Streaming subtitles (in manifest or separate track), Multi-language support (multiple subtitle tracks), Timing accuracy (sync with audio/video, drift over time in long videos). Subtitle tools (subedit, generate from speech-to-text). For each: use cases, compatibility, styling capabilities.",
                "count": 65,
                "associations": ["subtitles", "captions", "SRT", "accessibility", "timing"]
            },
            {
                "name": "video_editing",
                "prompt": "Generate atomic facts about video editing concepts and software. Cover: Timeline (nonlinear editor NLE, clips arranged on tracks, scrubbing playhead), Cuts (joining clips end-to-end), Transitions (fade, cross-dissolve, wipe between clips), Keyframes (setting animation at points in time, interpolated between), Rendering (final output generation from project, encodes to H.264/H.265), Non-linear editing (change/rearrange clips without re-encoding, unlike linear tape), Proxy editing (work with lower resolution for speed, render final at full resolution), Color grading (adjust color/tone, looks, LUTs), Video effects (distort, blur, composite), Audio mixing (multi-track audio, levels, EQ, compression), Export (render final video, format/codec/bitrate selection). Popular editors (Adobe Premiere Pro, DaVinci Resolve, Final Cut Pro, OpenShot). For each: workflow, rendering time, complexity.",
                "count": 65,
                "associations": ["video editing", "NLE", "timeline", "rendering", "effects"]
            },
            {
                "name": "hdr_video",
                "prompt": "Generate atomic facts about HDR (High Dynamic Range) video. Cover: SDR vs HDR (Standard Dynamic Range limited range, HDR extended brightness and color), HDR10 (standard for UHD Blu-ray, 10-bit color, PQ gamma curve, 1000 nits peak brightness), HDR10+ (dynamic metadata, tone mapping per-scene not per-picture, Samsung/Apple), Dolby Vision (proprietary HDR, higher peak brightness 4000 nits, requires licensing), HLG (Hybrid Log-Gamma, for broadcasting, no metadata), PQ curve (perceptual quantizer, more levels in shadows), Wide color gamut (Rec.2020 color space larger than Rec.709), Tone mapping (mapping HDR to SDR for compatibility), Peak brightness nits (1000 typical, 2000+ for professional displays), Hardware support (4K TV HDR support, gaming consoles, smartphones, limited streaming support). For each: quality advantages, equipment requirements, streaming support.",
                "count": 65,
                "associations": ["HDR", "video quality", "color", "brightness", "4K"]
            },
            {
                "name": "video_metadata",
                "prompt": "Generate atomic facts about video metadata and properties. Cover: MediaInfo tool (read and display all video information, streams, codecs, bitrates, duration), Container metadata (title, author, date, duration, chapter info), Video stream info (codec, bitrate, frame rate, resolution, aspect ratio, color space, profile/level), Audio stream info (codec, bitrate, sample rate 44.1/48/96 kHz, channels mono/stereo/5.1/7.1, language tag), Subtitle stream info (format, language tag, codec), Chapters (named sections, bookmarks in video), Thumbnails (embedded preview images), Rotation flag (for portrait videos), Creation date (when recorded), Total bitrate (all streams combined), Pixel format (YUV 4:2:0, RGB 4:4:4). Tools (ffprobe show stream info, MediaInfo GUI, mp4info for MP4 atoms). For each: what it contains, how to read/edit, importance for playback.",
                "count": 60,
                "associations": ["metadata", "video information", "codec", "bitrate", "streams"]
            },
            {
                "name": "video_for_web",
                "prompt": "Generate atomic facts about optimizing video for web delivery. Cover: HTML5 video element (native playback, controls, autoplay policies, preload attribute none/metadata/auto), Video formats and codecs (H.264 nearly universal, VP9/Chrome/Firefox, HEVC limited support), Media source extensions (JavaScript buffer management, adaptive bitrate), Video.js (JavaScript player with plugins, HLS/DASH support), Autoplay policies (chrome blocks video sound=muted, some browsers block all autoplay), Preload (metadata only = quick thumbnail/duration, none = don't preload), Poster image (thumbnail before play), Formats needed (provide multiple: MP4/WebM, browsers pick compatible), Codec compatibility (test browser support), Fallback (image or message for unsupported browsers), Streaming formats (HLS for Apple devices, DASH for others), CDN for video distribution. For each: browser support, user experience, bandwidth optimization.",
                "count": 65,
                "associations": ["web video", "HTML5", "streaming", "codec support", "player"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 19: AUDIO KNOWLEDGE
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "audio_knowledge",
        "label": "Audio Knowledge",
        "layer": 19,
        "topics": [
            {
                "name": "audio_fundamentals",
                "prompt": "Generate atomic facts about audio fundamentals and digital audio. Cover: Sound waves (mechanical waves, propagate through air), Frequency (Hz hertz, 20 Hz - 20 kHz human hearing range, lower = bass, higher = treble), Amplitude (loudness, larger wave = louder sound, measured in dB decibels), Sample rate (44.1 kHz CD standard, 48 kHz professional, 96 kHz high-resolution, 192 kHz mastering, Nyquist theorem sample rate > 2x frequency), Bit depth (16-bit CD standard 96 dB dynamic range, 24-bit 144 dB, 32-bit floating point), Channels (mono 1 channel, stereo 2 channels L/R, surround 5.1/7.1, Atmos object-based). Dynamic range (difference between quietest and loudest, 16-bit CD = 96 dB). For each: perception/quality tradeoffs, storage requirements, use cases.",
                "count": 70,
                "associations": ["audio", "sound", "frequency", "sample rate", "bit depth"]
            },
            {
                "name": "audio_codecs",
                "prompt": "Generate atomic facts about audio codecs and encoding. Cover: MP3 (MPEG-1 Layer 3, lossy, psychoacoustic model, 128-320 kbps bitrate, widely supported, frame-based), AAC (Advanced Audio Codec, lossy, better quality than MP3 at same bitrate, used in iTunes/Apple, standardized in MPEG-4), Opus (modern codec, lossy, excellent quality at low bitrate 64 kbps stereo, adaptive bitrate, WebRTC standard), FLAC (Free Lossless Audio Codec, lossless, decent compression, widely supported, metadata support), Vorbis (lossy, open source, used in OGG container, quality from 0-10 scale), WMA (Windows Media Audio, lossy proprietary), ALAC (Apple Lossless Audio Codec, lossless, proprietary), How codecs work (frequency domain analysis, psychoacoustic models). For each: compression ratio, quality levels, use cases, licensing.",
                "count": 70,
                "associations": ["audio codec", "MP3", "AAC", "FLAC", "compression"]
            },
            {
                "name": "audio_file_formats",
                "prompt": "Generate atomic facts about audio file formats and containers. Cover: WAV (waveform audio, PCM uncompressed or ADPCM, large files 10+ MB per minute, format: RIFF container with fmt and data chunks, professional audio), AIFF (Audio Interchange File Format, Apple standard, similar to WAV, big-endian), MP3 (file format for MPEG audio, ID3 tags for metadata, widely supported), FLAC (file format for FLAC codec, can store high resolution, metadata support), OGG (container for Vorbis codec, good for internet), M4A (MPEG-4 Audio, AAC codec in MP4 container, Apple iTunes default), WMA (Windows Media Audio file), OPUS in OGG/WebM (container for Opus codec), MIDI (Musical Instrument Digital Interface, not audio but note information). Containers vs codecs (WAV can contain multiple codecs, MP3 is both). For each: use cases, quality, portability.",
                "count": 70,
                "associations": ["audio format", "WAV", "FLAC", "MP3", "container"]
            },
            {
                "name": "audio_compression",
                "prompt": "Generate atomic facts about audio compression techniques. Cover: Lossy compression (removes information imperceptible to human ear, smaller files, MP3/AAC/Opus/Vorbis), Lossless compression (preserves all information, FLAC/ALAC/WMA Lossless, larger files than lossy), Psychoacoustic model (humans less sensitive to certain frequencies in presence of loud frequencies, simultaneous masking, frequency masking), MP3 compression (divide into frequency bands, apply psychoacoustic model, quantize coefficients), Bitrate (amount of data per second, 128 kbps MP3 phone quality, 192-256 kbps transparent quality, 320 kbps high quality, VBR variable bitrate adapts to complexity), CBR vs VBR (constant rate vs variable, VBR more efficient but less compatible), Quality settings (quality 0-9 in LAME MP3, Q0 best quality). For each: compression ratio, artifacts, listening quality perception.",
                "count": 65,
                "associations": ["compression", "lossy", "lossless", "psychoacoustic", "bitrate"]
            },
            {
                "name": "audio_editing",
                "prompt": "Generate atomic facts about audio editing concepts and techniques. Cover: Waveform (visual representation of audio, time on x-axis, amplitude on y-axis), Spectrogram (frequency content over time, darker = less energy, lighter = more energy), Amplitude (volume level, normalize to peak loudness), Trim (remove beginning/end), Fade in/out (gradually increase/decrease volume), Noise reduction (reduce background noise, estimate profile and subtract), Equalization EQ (adjust frequency response, boost/cut specific frequencies), Compression (dynamic range compression, louder parts made quieter, used to even out vocals), Limiter (prevent clipping/distortion by capping peaks), Reverb (add space/echo effect), Time stretch (change duration without changing pitch), Pitch shift (change pitch without changing speed). Tools (Audacity free, Adobe Audition professional, REAPER flexible). For each: effect type, when to use, adjustable parameters.",
                "count": 70,
                "associations": ["audio editing", "Audacity", "effect", "EQ", "compression"]
            },
            {
                "name": "midi_and_digital_music",
                "prompt": "Generate atomic facts about MIDI and digital music production. Cover: MIDI protocol (Musical Instrument Digital Interface, not audio but control messages, note on/off, velocity, CC continuous controller, program change), MIDI messages (Note On/Off 144-159, CC 176-191 volume/pan/modulation, Program Change 192-207 select instrument), MIDI channels (16 channels, each can have different instrument), MIDI file format (SMF Standard MIDI File, .mid extension, records MIDI messages with timestamps, Type 0 single track, Type 1 multiple tracks), General MIDI (standardized program numbers, instrument 0 = acoustic piano), Virtual instruments (software synthesizer, VST/AU/AAX plugin formats, can use soundfont), DAW (Digital Audio Workstation, Ableton/FL Studio/Logic/Reaper, record/edit MIDI and audio), Synthesizer (generates sound from waveforms, oscillators + envelopes + LFOs). For each: use in music production, expressiveness, MIDI sequencing workflow.",
                "count": 65,
                "associations": ["MIDI", "music production", "digital", "synthesizer", "DAW"]
            },
            {
                "name": "spatial_and_surround_audio",
                "prompt": "Generate atomic facts about spatial and surround audio. Cover: Stereo panning (left/right balance, creates width perception), Surround sound 5.1 (L/R front, Center, L/R surround, .1 subwoofer for LFE low frequency), 7.1 surround (adds back L/R), Dolby Atmos (object-based audio, each sound is object with 3D coordinates, not channel-based), Binaural audio (simulate 3D sound using HRTF, for headphones, creates illusion of spatial location), HRTF (Head Related Transfer Function, how ears perceive direction, individual differences), Ambisonics (spherical harmonics encoding of sound field, B-format first order, higher orders more accurate), Object-based audio (sounds placed in 3D space, renderer adapts to speaker configuration), Surround field mastering (mixing for various channel configurations), Spatial audio on headphones (Apple Spatial Audio uses head tracking and HRTF). For each: application domains, equipment needed, realism vs complexity.",
                "count": 65,
                "associations": ["spatial audio", "surround", "5.1", "Atmos", "binaural"]
            },
            {
                "name": "audio_streaming",
                "prompt": "Generate atomic facts about audio streaming and internet audio. Cover: Audio codecs for streaming (MP3, AAC, Opus, Vorbis, FLAC for high-res), Icecast/Shoutcast (streaming servers, broadcast audio), Podcast RSS feeds (feed with audio file links, directories Apple Podcasts/Spotify index), WebRTC audio (real-time communication, built-in codecs OPUS/PCMU/PCMA), Web Audio API (JavaScript audio playback/synthesis), HTTP progressive download (download file while playing), Adaptive streaming (HLS/DASH for audio too), Audio buffering (buffer before playback to avoid dropouts), Latency (delay from source to listener, live ~2-3s, on-demand negligible), Bandwidth requirements (128 kbps MP3 = 15.7 KB/s, 256 kbps AAC = 32 KB/s). For each: technology, use cases, latency/quality tradeoff.",
                "count": 60,
                "associations": ["streaming", "audio", "live", "podcast", "WebRTC"]
            },
            {
                "name": "audio_metadata",
                "prompt": "Generate atomic facts about audio metadata and tagging. Cover: ID3 tags (ID3v1 128 bytes at end of MP3, ID3v2 at beginning, variable size), ID3v2 frames (TIT2 title, TPE1 artist, TALB album, TRCK track number, TYER year, APIC attached picture), Vorbis comments (OGG Vorbis, FLAC metadata, human-readable key=value format, artist, album, date, etc.), ReplayGain (loudness normalization, prevents jarring volume differences between tracks, measure in dB), Loudness standard LUFS (Loudness Units relative to Full Scale, -14 LUFS Spotify, -13 LUFS Apple Music, -16 LUFS EBU R128), EBU R128 standard (loudness measurement for broadcast), Album art (embedded in ID3 or Vorbis comments, typically 200x200 pixels), Genre tags (standardized vs free-form). Tools (tag editors, MediaInfo read tags, ffprobe). For each: metadata format, use by players/services, loudness normalization importance.",
                "count": 65,
                "associations": ["metadata", "ID3", "tags", "ReplayGain", "loudness"]
            },
            {
                "name": "speech_and_voice",
                "prompt": "Generate atomic facts about speech and voice audio. Cover: Sample rate (16 kHz adequate for speech intelligibility, 8 kHz legacy phone, 48 kHz full bandwidth), Speech codecs (G.711 ulaw/alaw legacy phone quality 8 kHz, G.722 wideband 16 kHz, Opus excellent at 8-16 kHz with low latency), Voice over IP (VoIP uses speech codecs, echo cancellation important), Text-to-speech (synthesized speech, naturalness/latency tradeoff), Speech recognition (automatic speech recognition ASR, acoustic model + language model), Formants (resonant frequencies of vocal tract, F1/F2/F3 characterize vowels), Phonemes (smallest meaningful units of speech), Voice quality metrics (MOS Mean Opinion Score subjective testing, objective metrics like PESQ). Recording voice (minimize background noise, good microphone, proper gain), Noise reduction for speech (preserve speech clarity while removing noise). For each: quality/bandwidth tradeoff, codec selection, real-time constraints.",
                "count": 60,
                "associations": ["speech", "voice", "telephony", "VoIP", "quality"]
            },
        ]
    },
]


def count_curriculum_targets() -> dict:
    """Return dict of target fact counts per domain."""
    counts = {}
    total = 0
    for domain in CURRICULUM:
        domain_count = sum(t["count"] for t in domain["topics"])
        counts[domain["name"]] = {
            "label": domain["label"],
            "layer": domain["layer"],
            "topics": len(domain["topics"]),
            "target_facts": domain_count,
        }
        total += domain_count
    counts["__total__"] = total
    return counts


# --- Generation -------------------------------------------------------------

def generate_domain(domain: dict, model: str) -> dict:
    """Generate all facts for a single domain. Returns domain data dict."""
    name = domain["name"]
    label = domain["label"]
    layer = domain["layer"]
    topics = domain["topics"]

    print(f"\n{'='*70}")
    print(f"  LAYER {layer}: {label.upper()}")
    print(f"  {len(topics)} topics, target: {sum(t['count'] for t in topics)} facts")
    print(f"{'='*70}")

    domain_data = {
        "name": name,
        "label": label,
        "layer": layer,
        "topics": [],
        "total_facts": 0,
    }

    for i, topic in enumerate(topics, 1):
        topic_name = topic["name"]
        prompt = topic["prompt"]
        count = topic["count"]
        associations = topic.get("associations", [])

        print(f"\n  [{i}/{len(topics)}] {topic_name} (target: {count} facts)...", end=" ", flush=True)

        facts = generate_facts_for_topic(topic_name, prompt, model, count)

        # Convert to structured format with associations
        fact_objects = []
        for fact in facts:
            fact_objects.append({
                "content": fact,
                "associations": associations,
            })

        domain_data["topics"].append({
            "name": topic_name,
            "facts": fact_objects,
            "count": len(fact_objects),
        })
        domain_data["total_facts"] += len(fact_objects)

        print(f"got {len(facts)} facts")

    return domain_data


def generate_all(model: str, domain_filter: str = None, resume: bool = False) -> dict:
    """Generate the entire curriculum (or one domain). Returns full data dict."""

    # Load existing data if resuming
    existing = {}
    if resume and os.path.exists(OUTPUT_FILE):
        try:
            with open(OUTPUT_FILE, "r", encoding="utf-8") as f:
                old = json.load(f)
            for d in old.get("domains", []):
                existing[d["name"]] = d
            print(f"[resume] Loaded {len(existing)} existing domains from {OUTPUT_FILE}")
        except Exception as e:
            print(f"[resume] Could not load existing data: {e}")

    # Check Ollama is running
    print("[check] Testing Ollama connection...", end=" ", flush=True)
    try:
        req = urllib.request.Request(
            "http://localhost:11434/api/tags",
            headers={"Content-Type": "application/json"}
        )
        with urllib.request.urlopen(req, timeout=10) as resp:
            models_data = json.loads(resp.read().decode("utf-8"))
            available = [m["name"] for m in models_data.get("models", [])]
            print(f"OK. Available models: {', '.join(available) if available else 'none'}")

            # Check if requested model is available
            model_base = model.split(":")[0]
            if not any(model_base in m for m in available):
                print(f"[warn] Model '{model}' not found locally. Ollama will try to pull it.")
    except Exception as e:
        print(f"FAILED: {e}")
        print("[error] Is Ollama running? Start with: ollama serve")
        sys.exit(1)

    domains_to_generate = CURRICULUM
    if domain_filter:
        domains_to_generate = [d for d in CURRICULUM if d["name"] == domain_filter]
        if not domains_to_generate:
            valid = [d["name"] for d in CURRICULUM]
            print(f"[error] Unknown domain '{domain_filter}'. Valid: {', '.join(valid)}")
            sys.exit(1)

    all_domains = []
    total_facts = 0
    start_time = time.time()

    for domain in domains_to_generate:
        # Skip if resuming and already generated
        if resume and domain["name"] in existing:
            print(f"\n[skip] {domain['label']} — already generated ({existing[domain['name']]['total_facts']} facts)")
            all_domains.append(existing[domain["name"]])
            total_facts += existing[domain["name"]]["total_facts"]
            continue

        domain_data = generate_domain(domain, model)
        all_domains.append(domain_data)
        total_facts += domain_data["total_facts"]

        # Save after each domain (in case of crash)
        _save_progress(all_domains, model, total_facts)

    # If resuming, include domains that weren't in the generate list
    if resume:
        generated_names = {d["name"] for d in all_domains}
        for name, data in existing.items():
            if name not in generated_names:
                all_domains.append(data)
                total_facts += data["total_facts"]

    # Sort by layer
    all_domains.sort(key=lambda d: d.get("layer", 99))

    elapsed = time.time() - start_time
    result = {
        "version": "2.0",
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "model": model,
        "total_facts": total_facts,
        "domain_count": len(all_domains),
        "domains": all_domains,
    }

    # Final save
    with open(OUTPUT_FILE, "w", encoding="utf-8") as f:
        json.dump(result, f, indent=2, ensure_ascii=False)

    print(f"\n{'='*70}")
    print(f"  GENERATION COMPLETE")
    print(f"  Total facts: {total_facts}")
    print(f"  Domains: {len(all_domains)}")
    print(f"  Time: {elapsed:.0f}s ({elapsed/60:.1f}m)")
    print(f"  Output: {OUTPUT_FILE}")
    print(f"{'='*70}")

    return result


def _save_progress(domains: list, model: str, total_facts: int):
    """Save current progress to disk (crash recovery)."""
    data = {
        "version": "2.0",
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "model": model,
        "total_facts": total_facts,
        "domain_count": len(domains),
        "domains": sorted(domains, key=lambda d: d.get("layer", 99)),
        "_note": "partial — generation in progress",
    }
    with open(OUTPUT_FILE, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)


# --- Dry Run ----------------------------------------------------------------

def dry_run():
    """Print fact counts per domain without generating anything."""
    counts = count_curriculum_targets()
    total = counts.pop("__total__")

    print(f"\n{'='*70}")
    print(f"  LU AI CURRICULUM SYSTEMS — DRY RUN")
    print(f"{'='*70}\n")

    for name, info in sorted(counts.items(), key=lambda x: x[1]["layer"]):
        print(f"  Layer {info['layer']:2d}: {info['label']:<30s}  "
              f"{info['topics']:3d} topics  ->  {info['target_facts']:4d} facts")

    print(f"\n  {'-'*60}")
    print(f"  TOTAL: {sum(c['topics'] for c in counts.values())} topics -> {total} target facts")
    print()

    # Also check existing output
    if os.path.exists(OUTPUT_FILE):
        try:
            with open(OUTPUT_FILE, "r", encoding="utf-8") as f:
                data = json.load(f)
            existing_total = data.get("total_facts", 0)
            print(f"  Existing curriculum_systems_data.json: {existing_total} facts")
            for d in data.get("domains", []):
                print(f"    {d.get('label', d.get('name'))}: {d.get('total_facts', 0)} facts")
        except Exception:
            pass


# --- CLI --------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="Lu AI Curriculum Systems Training — generate knowledge facts using Ollama",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python train_curriculum_systems.py --dry-run                     Show target counts
  python train_curriculum_systems.py --generate                    Generate all domains
  python train_curriculum_systems.py --generate --domain filetypes  Generate one domain
  python train_curriculum_systems.py --generate --resume            Skip already-done domains
  python train_curriculum_systems.py --generate --model mistral     Use a different model

Domains: """ + ", ".join(d["name"] for d in CURRICULUM)
    )

    parser.add_argument("--generate", action="store_true",
                        help="Generate facts using Ollama and save to curriculum_systems_data.json")
    parser.add_argument("--dry-run", action="store_true",
                        help="Just show target fact counts per domain")
    parser.add_argument("--domain", type=str, default=None,
                        help="Generate only this domain (e.g. 'filetypes', 'windows_cli')")
    parser.add_argument("--model", type=str, default=DEFAULT_MODEL,
                        help=f"Ollama model to use (default: {DEFAULT_MODEL})")
    parser.add_argument("--resume", action="store_true",
                        help="Skip domains already present in curriculum_systems_data.json")

    args = parser.parse_args()

    if not args.generate and not args.dry_run:
        parser.print_help()
        print("\nSpecify --generate or --dry-run")
        sys.exit(1)

    if args.dry_run:
        dry_run()
        return

    if args.generate:
        generate_all(model=args.model, domain_filter=args.domain, resume=args.resume)


if __name__ == "__main__":
    main()
