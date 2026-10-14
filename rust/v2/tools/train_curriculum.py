#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Lu AI — Curriculum Training Script v2

Generates a comprehensive knowledge base for Lu's brain using Ollama.
Produces a structured JSON file (curriculum_data.json) that can be batch-loaded
into the brain via MCP tools.

Target: 8,000-10,000 atomic facts across 13 knowledge domains.

Usage:
    python train_curriculum.py --generate                    # Generate all domains
    python train_curriculum.py --generate --domain networking  # One domain only
    python train_curriculum.py --dry-run                      # Count facts per domain
    python train_curriculum.py --generate --model mistral     # Use different model
    python train_curriculum.py --generate --resume            # Skip domains already in JSON

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
OUTPUT_FILE = os.path.join(SCRIPT_DIR, "curriculum_data.json")

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
    # LAYER 0: ENGLISH LANGUAGE — Lu needs this to communicate
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "english",
        "label": "English Language",
        "layer": 0,
        "topics": [
            # Parts of speech
            {
                "name": "nouns_definition_and_types",
                "prompt": "Generate atomic facts about nouns in English grammar. Cover: what a noun is, common nouns, proper nouns, abstract nouns, collective nouns, countable vs uncountable nouns. Give specific examples for each type.",
                "count": 55,
                "associations": ["noun", "parts of speech", "grammar", "english"]
            },
            {
                "name": "verbs_definition_and_types",
                "prompt": "Generate atomic facts about verbs in English grammar. Cover: what a verb is, action verbs, linking verbs, auxiliary/helping verbs, modal verbs, transitive vs intransitive verbs, regular vs irregular verbs. Give specific examples.",
                "count": 60,
                "associations": ["verb", "parts of speech", "grammar", "english"]
            },
            {
                "name": "adjectives_and_adverbs",
                "prompt": "Generate atomic facts about adjectives and adverbs in English. Cover: what adjectives are, comparative and superlative forms, adjective order rules, what adverbs are, types of adverbs (manner, time, place, frequency, degree). Give specific examples of each.",
                "count": 55,
                "associations": ["adjective", "adverb", "parts of speech", "grammar"]
            },
            {
                "name": "prepositions_conjunctions_articles",
                "prompt": "Generate atomic facts about prepositions, conjunctions, and articles in English. Cover: common prepositions (in, on, at, by, for, with, etc.) and when to use each, coordinating conjunctions (FANBOYS), subordinating conjunctions, correlative conjunctions, definite vs indefinite articles (the, a, an), when to omit articles.",
                "count": 55,
                "associations": ["preposition", "conjunction", "article", "grammar"]
            },
            {
                "name": "pronouns_and_interjections",
                "prompt": "Generate atomic facts about pronouns and interjections in English. Cover: personal pronouns (I/me, he/him, she/her, they/them), possessive pronouns, reflexive pronouns, demonstrative pronouns (this/that/these/those), relative pronouns (who/which/that), interrogative pronouns, common interjections and their uses.",
                "count": 50,
                "associations": ["pronoun", "interjection", "parts of speech", "grammar"]
            },
            # Sentence structure
            {
                "name": "sentence_structure_basics",
                "prompt": "Generate atomic facts about English sentence structure. Cover: subject-verb-object order, subjects and predicates, direct and indirect objects, simple sentences, compound sentences (joined by conjunctions), complex sentences (with dependent clauses), compound-complex sentences. Give clear examples of each pattern.",
                "count": 55,
                "associations": ["sentence structure", "syntax", "grammar", "english"]
            },
            {
                "name": "clauses_and_phrases",
                "prompt": "Generate atomic facts about clauses and phrases in English. Cover: independent clauses, dependent/subordinate clauses, noun clauses, adjective clauses, adverb clauses, noun phrases, verb phrases, prepositional phrases, participial phrases, gerund phrases, infinitive phrases. Give a specific example for each.",
                "count": 55,
                "associations": ["clause", "phrase", "syntax", "grammar"]
            },
            # Punctuation
            {
                "name": "punctuation_rules",
                "prompt": "Generate atomic facts about punctuation in English. Cover rules for: periods, commas (serial comma, after introductory elements, in compound sentences, with nonrestrictive clauses), semicolons, colons, apostrophes (possession, contractions), quotation marks, parentheses, em dashes, en dashes, hyphens, ellipses, exclamation marks, question marks. Give specific usage examples.",
                "count": 60,
                "associations": ["punctuation", "writing", "grammar", "english"]
            },
            # Verb tenses
            {
                "name": "verb_tenses_simple",
                "prompt": "Generate atomic facts about simple verb tenses in English. Cover: simple past (walked, ran, ate), simple present (walk, run, eat), simple future (will walk, will run). Include irregular verb forms. Show how to form each tense and give examples in sentences.",
                "count": 55,
                "associations": ["verb tense", "past tense", "present tense", "future tense", "grammar"]
            },
            {
                "name": "verb_tenses_progressive_perfect",
                "prompt": "Generate atomic facts about progressive and perfect verb tenses in English. Cover: present progressive (am walking), past progressive (was walking), future progressive (will be walking), present perfect (have walked), past perfect (had walked), future perfect (will have walked), present perfect progressive (have been walking). Show formation and usage for each.",
                "count": 55,
                "associations": ["verb tense", "progressive", "perfect tense", "grammar"]
            },
            {
                "name": "conditional_and_subjunctive",
                "prompt": "Generate atomic facts about conditional and subjunctive mood in English. Cover: zero conditional (if water boils...), first conditional (if it rains...), second conditional (if I were...), third conditional (if I had known...), mixed conditionals, subjunctive mood uses (I wish, if I were, it's important that he be). Give clear examples.",
                "count": 50,
                "associations": ["conditional", "subjunctive", "verb mood", "grammar"]
            },
            # Vocabulary
            {
                "name": "common_vocabulary_actions",
                "prompt": "Generate atomic facts defining common English action words and their meanings. Each fact should be like 'To accomplish means to successfully complete a task' or 'To acquire means to obtain or gain possession of something'. Cover everyday verbs that an intelligent being needs to know. Be precise about meaning distinctions.",
                "count": 75,
                "associations": ["vocabulary", "word meaning", "definition", "english"]
            },
            {
                "name": "common_vocabulary_descriptors",
                "prompt": "Generate atomic facts defining common English adjectives and their meanings. Each fact should be like 'Abundant means existing in large quantities' or 'Ambiguous means having more than one possible meaning'. Cover descriptive words an intelligent being needs. Be precise.",
                "count": 75,
                "associations": ["vocabulary", "adjective", "definition", "english"]
            },
            {
                "name": "common_vocabulary_concepts",
                "prompt": "Generate atomic facts defining common English nouns for abstract concepts. Each fact should be like 'Integrity means the quality of being honest and having strong moral principles' or 'Hypothesis means a proposed explanation for a phenomenon that can be tested'. Cover important conceptual words.",
                "count": 75,
                "associations": ["vocabulary", "concept", "definition", "english"]
            },
            {
                "name": "common_vocabulary_technical",
                "prompt": "Generate atomic facts defining common technical and academic English words. Each fact should be like 'To analyze means to examine something in detail to understand its structure' or 'A paradigm is a typical example or model of something'. Cover words used in science, technology, and academic discussion.",
                "count": 75,
                "associations": ["vocabulary", "technical", "definition", "english"]
            },
            {
                "name": "common_vocabulary_everyday",
                "prompt": "Generate atomic facts defining common everyday English words that people use frequently. Cover words for time (immediately, eventually, meanwhile), quantity (approximately, substantial, scarce), quality (reliable, fragile, versatile), relationships (mutual, reciprocal, compatible). Format: 'X means Y'.",
                "count": 75,
                "associations": ["vocabulary", "everyday", "definition", "english"]
            },
            {
                "name": "common_vocabulary_emotions",
                "prompt": "Generate atomic facts defining English words for emotions, feelings, and mental states. Each fact like 'Anxious means feeling worried or uneasy about something uncertain' or 'Euphoria is an intense feeling of happiness and excitement'. Cover the full range of human emotional vocabulary.",
                "count": 60,
                "associations": ["vocabulary", "emotion", "feeling", "definition"]
            },
            {
                "name": "common_vocabulary_more_words",
                "prompt": "Generate atomic facts defining more common English words. Cover: words for communication (articulate, convey, imply, infer), words for change (transform, evolve, fluctuate, deteriorate), words for thinking (contemplate, speculate, deduce, rationalize), words for creation (innovate, fabricate, synthesize, devise). Format: 'X means Y'.",
                "count": 75,
                "associations": ["vocabulary", "definition", "english"]
            },
            {
                "name": "common_vocabulary_advanced",
                "prompt": "Generate atomic facts defining advanced but commonly encountered English words. Cover: pragmatic, empirical, ubiquitous, ephemeral, dichotomy, nuance, catalyst, precedent, paradox, arbitrary, inherent, implicit, explicit, comprehensive, pertinent, redundant, succinct, verbose, feasible, viable, tangible, abstract, correlate, extrapolate, interpolate. Format: 'X means Y'.",
                "count": 60,
                "associations": ["vocabulary", "advanced", "definition", "english"]
            },
            # Grammar rules
            {
                "name": "grammar_rules_agreement",
                "prompt": "Generate atomic facts about subject-verb agreement rules in English. Cover: singular subjects take singular verbs, plural subjects take plural verbs, compound subjects with 'and', subjects with 'or/nor', collective nouns, indefinite pronouns (everyone, each, neither), inverted sentences, amounts and distances. Give right and wrong examples.",
                "count": 50,
                "associations": ["grammar", "subject-verb agreement", "english"]
            },
            {
                "name": "grammar_rules_misc",
                "prompt": "Generate atomic facts about English grammar rules. Cover: pronoun-antecedent agreement, parallel structure in lists and comparisons, dangling modifiers, misplaced modifiers, active vs passive voice and when to use each, double negatives, who vs whom, that vs which, less vs fewer, affect vs effect, lie vs lay. Give specific examples.",
                "count": 55,
                "associations": ["grammar", "grammar rules", "english"]
            },
            # Transitions and connectors
            {
                "name": "transition_words",
                "prompt": "Generate atomic facts about transition words and phrases in English. Cover: addition (furthermore, moreover, in addition), contrast (however, nevertheless, on the other hand), cause/effect (therefore, consequently, as a result), sequence (first, next, finally), example (for instance, specifically), summary (in conclusion, to summarize), comparison (similarly, likewise). State what each transition signals and give usage examples.",
                "count": 55,
                "associations": ["transition", "connector", "writing", "english"]
            },
            # Questions
            {
                "name": "question_formation",
                "prompt": "Generate atomic facts about forming questions in English. Cover: question words (who, what, where, when, why, how, which, whose), yes/no questions (auxiliary verb inversion), tag questions, indirect questions, embedded questions, negative questions, rhetorical questions. Show how each type is formed with examples.",
                "count": 50,
                "associations": ["question", "interrogative", "grammar", "english"]
            },
            # Plurals and possessives
            {
                "name": "plural_and_possessive_rules",
                "prompt": "Generate atomic facts about plural and possessive rules in English. Cover: regular plurals (-s, -es), plurals ending in -y, -f/-fe, -o, irregular plurals (child/children, mouse/mice, person/people, goose/geese, tooth/teeth, ox/oxen), Latin/Greek plurals (datum/data, criterion/criteria), possessive with 's, possessive with s', possessive pronouns vs contractions (its/it's, their/they're, your/you're).",
                "count": 50,
                "associations": ["plural", "possessive", "grammar", "english"]
            },
            # Idioms
            {
                "name": "common_idioms",
                "prompt": "Generate atomic facts about common English idioms and their meanings. Each fact should explain what the idiom means. Format: 'The idiom X means Y'. Cover: break the ice, hit the nail on the head, let the cat out of the bag, burn the midnight oil, cost an arm and a leg, piece of cake, once in a blue moon, bite the bullet, under the weather, a blessing in disguise, the ball is in your court, break a leg, cutting corners, and more.",
                "count": 60,
                "associations": ["idiom", "expression", "figurative language", "english"]
            },
            {
                "name": "more_idioms_and_expressions",
                "prompt": "Generate atomic facts about English expressions, phrasal verbs, and figurative language. Cover: common phrasal verbs (look up, break down, give in, carry out, turn out, come across, figure out), common expressions (at the end of the day, by and large, for the time being, in the long run), and figurative meanings. Format: 'X means Y'.",
                "count": 55,
                "associations": ["idiom", "phrasal verb", "expression", "english"]
            },
            # Active/passive voice and reported speech
            {
                "name": "voice_and_reported_speech",
                "prompt": "Generate atomic facts about active/passive voice and reported speech in English. Cover: active voice (subject performs action: 'The dog bit the man'), passive voice (subject receives action: 'The man was bitten by the dog'), how to form passive (be + past participle), when passive is preferred, reported/indirect speech (He said that..., She asked if...), backshift of tenses in reported speech, reporting verbs (said, told, asked, claimed, suggested).",
                "count": 50,
                "associations": ["voice", "passive", "active", "reported speech", "grammar"]
            },
            # Word formation
            {
                "name": "word_formation",
                "prompt": "Generate atomic facts about English word formation. Cover: prefixes (un-, re-, pre-, mis-, over-, under-, dis-, non-, anti-, inter-, multi-) and their meanings, suffixes (-tion/-sion, -ment, -ness, -ful, -less, -able/-ible, -ous, -ive, -ly, -er/-or, -ist, -ism) and their meanings, compound words, root words from Latin and Greek, how affixes change word class (happy -> happiness, create -> creation). Give examples for each.",
                "count": 55,
                "associations": ["word formation", "prefix", "suffix", "morphology", "english"]
            },
            # Commonly confused words
            {
                "name": "commonly_confused_words",
                "prompt": "Generate atomic facts about commonly confused English words. Cover the difference between: their/there/they're, your/you're, its/it's, affect/effect, then/than, to/too/two, accept/except, advice/advise, lose/loose, principle/principal, complement/compliment, stationary/stationery, who's/whose, weather/whether, breath/breathe, conscience/conscious, desert/dessert, emigrate/immigrate, farther/further, imply/infer, lay/lie, precede/proceed.",
                "count": 55,
                "associations": ["confused words", "homophones", "spelling", "english"]
            },
            # Conversation patterns
            {
                "name": "conversation_patterns",
                "prompt": "Generate atomic facts about English conversation patterns and pragmatics. Cover: greetings and farewells (formal vs informal), making requests (could you, would you mind, I was wondering if), giving opinions (I think, in my opinion, it seems to me), agreeing and disagreeing politely, apologizing, expressing gratitude, hedging language (sort of, kind of, a bit, somewhat), formal vs informal register, turn-taking in conversation, discourse markers (well, so, anyway, actually, basically).",
                "count": 50,
                "associations": ["conversation", "pragmatics", "register", "english"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 1: NETWORKING & PROTOCOLS
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "networking",
        "label": "Networking & Protocols",
        "layer": 1,
        "topics": [
            {
                "name": "osi_model",
                "prompt": "Generate atomic facts about the OSI model layers. Cover all 7 layers: Physical, Data Link, Network, Transport, Session, Presentation, Application. For each layer state: its number, its name, what it does, what protocols operate at that layer, and what data unit it uses (bits, frames, packets, segments, data).",
                "count": 55,
                "associations": ["OSI model", "networking", "protocol layers"]
            },
            {
                "name": "tcp_ip_stack",
                "prompt": "Generate atomic facts about the TCP/IP protocol stack. Cover: the 4-layer model (Link, Internet, Transport, Application), how it maps to OSI, IP addressing (IPv4 and IPv6), IP packet structure (header fields: TTL, source/dest IP, protocol field), subnet masks, CIDR notation, how packets are routed between networks.",
                "count": 60,
                "associations": ["TCP/IP", "IP", "networking", "internet protocol"]
            },
            {
                "name": "tcp_protocol",
                "prompt": "Generate atomic facts about TCP (Transmission Control Protocol). Cover: three-way handshake (SYN, SYN-ACK, ACK), connection teardown (FIN), TCP header fields (sequence number, acknowledgment number, flags, window size, checksum), flow control, congestion control (slow start, congestion avoidance), TCP states (LISTEN, ESTABLISHED, TIME_WAIT, etc.), reliable delivery, retransmission.",
                "count": 60,
                "associations": ["TCP", "transport layer", "networking", "reliable delivery"]
            },
            {
                "name": "udp_protocol",
                "prompt": "Generate atomic facts about UDP (User Datagram Protocol). Cover: connectionless nature, UDP header structure (source port, dest port, length, checksum), why UDP is faster than TCP, use cases (DNS, VoIP, gaming, streaming, QUIC), no guaranteed delivery, no ordering, no flow control. Compare specific differences with TCP.",
                "count": 40,
                "associations": ["UDP", "transport layer", "networking", "datagram"]
            },
            {
                "name": "dns_protocol",
                "prompt": "Generate atomic facts about DNS (Domain Name System). Cover: what DNS does, DNS record types (A, AAAA, CNAME, MX, NS, TXT, PTR, SOA, SRV), DNS resolution process (recursive vs iterative), DNS hierarchy (root servers, TLD servers, authoritative servers), DNS caching, TTL, DNSSEC, DNS over HTTPS (DoH), DNS over TLS (DoT), DNS poisoning, DNS amplification attacks.",
                "count": 60,
                "associations": ["DNS", "domain name", "networking", "name resolution"]
            },
            {
                "name": "dhcp_arp_icmp",
                "prompt": "Generate atomic facts about DHCP, ARP, and ICMP protocols. Cover: DHCP process (DORA: Discover, Offer, Request, Acknowledge), DHCP lease time, DHCP relay, ARP (Address Resolution Protocol) mapping IP to MAC addresses, ARP cache, ARP spoofing, gratuitous ARP, ICMP (Internet Control Message Protocol), ping, traceroute, ICMP types (echo request/reply, destination unreachable, time exceeded).",
                "count": 55,
                "associations": ["DHCP", "ARP", "ICMP", "networking"]
            },
            {
                "name": "http_https",
                "prompt": "Generate atomic facts about HTTP and HTTPS. Cover: HTTP methods (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS), status codes (200, 201, 301, 302, 304, 400, 401, 403, 404, 500, 502, 503), HTTP headers (Content-Type, Authorization, Cache-Control, Cookie, Set-Cookie), HTTP/1.1 vs HTTP/2 vs HTTP/3, HTTPS and TLS, request/response structure, cookies, sessions, CORS.",
                "count": 60,
                "associations": ["HTTP", "HTTPS", "web protocol", "networking"]
            },
            {
                "name": "tls_ssl_handshake",
                "prompt": "Generate atomic facts about TLS/SSL. Cover: what TLS does, TLS 1.2 handshake steps (ClientHello, ServerHello, Certificate, Key Exchange, Change Cipher Spec, Finished), TLS 1.3 improvements (1-RTT handshake, removed RSA key exchange), cipher suites, certificate authorities, certificate chains, certificate pinning, forward secrecy, HSTS, SNI (Server Name Indication), OCSP stapling.",
                "count": 55,
                "associations": ["TLS", "SSL", "encryption", "HTTPS", "handshake"]
            },
            {
                "name": "routing_protocols",
                "prompt": "Generate atomic facts about network routing. Cover: static vs dynamic routing, routing tables, BGP (Border Gateway Protocol) and how the internet routes between autonomous systems, OSPF (Open Shortest Path First) for internal routing, RIP (Routing Information Protocol), distance vector vs link state, default gateway, longest prefix match, route aggregation, routing metrics.",
                "count": 55,
                "associations": ["routing", "BGP", "OSPF", "networking"]
            },
            {
                "name": "nat_firewalls_proxies",
                "prompt": "Generate atomic facts about NAT, firewalls, and proxies. Cover: NAT (Network Address Translation) types (SNAT, DNAT, PAT), why NAT exists (IPv4 exhaustion), NAT traversal problems, firewall types (packet filter, stateful inspection, application-level gateway), firewall rules (allow/deny), DMZ, forward proxy vs reverse proxy, SOCKS proxy, load balancers.",
                "count": 55,
                "associations": ["NAT", "firewall", "proxy", "network security"]
            },
            {
                "name": "wifi_bluetooth",
                "prompt": "Generate atomic facts about WiFi and Bluetooth protocols. Cover: WiFi standards (802.11a/b/g/n/ac/ax/be), WiFi frequency bands (2.4GHz, 5GHz, 6GHz), WPA/WPA2/WPA3 security, SSID, WiFi channels, WiFi encryption types, Bluetooth versions, Bluetooth Low Energy (BLE), Bluetooth pairing, Bluetooth profiles (A2DP, HFP, HID), WiFi Direct, mesh networking.",
                "count": 50,
                "associations": ["WiFi", "Bluetooth", "wireless", "networking"]
            },
            {
                "name": "ethernet_switching",
                "prompt": "Generate atomic facts about Ethernet and network switching. Cover: Ethernet frame structure, MAC addresses, switches vs hubs, VLAN (Virtual LAN), spanning tree protocol (STP), link aggregation (LACP), PoE (Power over Ethernet), Ethernet speeds (10/100/1000/10G/25G/40G/100G), full duplex vs half duplex, jumbo frames, ARP table on switches.",
                "count": 50,
                "associations": ["Ethernet", "switch", "LAN", "data link layer"]
            },
            {
                "name": "network_troubleshooting",
                "prompt": "Generate atomic facts about network troubleshooting tools and techniques. Cover: ping (ICMP echo, testing reachability), traceroute/tracert (showing path to destination, TTL), nslookup/dig (DNS queries), netstat/ss (showing connections, listening ports), tcpdump (packet capture, filters), Wireshark (GUI packet analysis, display filters, follow TCP stream), nmap (port scanning, service detection, OS fingerprinting), iperf (bandwidth testing), MTR (combines ping and traceroute), common network issues and diagnosis.",
                "count": 55,
                "associations": ["troubleshooting", "ping", "traceroute", "tcpdump", "networking"]
            },
            {
                "name": "network_protocols_misc",
                "prompt": "Generate atomic facts about additional network protocols. Cover: FTP (file transfer, port 21, active vs passive mode), SSH (secure shell, port 22, key-based auth, tunneling), SMTP (email sending, port 25/587), IMAP vs POP3 (email retrieval), NTP (time synchronization), SNMP (network monitoring), LDAP (directory services), SIP (VoIP signaling), RTP (real-time media), QUIC (HTTP/3, UDP-based, built-in TLS), mDNS/Bonjour (local service discovery).",
                "count": 55,
                "associations": ["protocol", "SSH", "FTP", "SMTP", "networking"]
            },
            {
                "name": "ipv6",
                "prompt": "Generate atomic facts about IPv6. Cover: why IPv6 exists (IPv4 exhaustion, only 4.3 billion addresses), IPv6 address format (128-bit, 8 groups of 4 hex digits), address types (unicast, multicast, anycast, link-local fe80::), IPv6 header (simpler than IPv4, no checksum, flow label), NDP (Neighbor Discovery Protocol, replaces ARP), SLAAC (Stateless Address Autoconfiguration), dual-stack (running IPv4 and IPv6 together), transition mechanisms (6to4, Teredo, NAT64).",
                "count": 50,
                "associations": ["IPv6", "IP address", "networking", "internet protocol"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 2: CRYPTOGRAPHY & ENCRYPTION
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "cryptography",
        "label": "Cryptography & Encryption",
        "layer": 2,
        "topics": [
            {
                "name": "symmetric_encryption",
                "prompt": "Generate atomic facts about symmetric encryption. Cover: what symmetric encryption is (same key for encrypt/decrypt), AES (key sizes 128/192/256, block cipher, modes: ECB/CBC/CTR/GCM), DES and 3DES (history, 56-bit key weakness), ChaCha20 (stream cipher, used in TLS), Blowfish, Twofish, block ciphers vs stream ciphers, initialization vectors (IV), padding (PKCS7).",
                "count": 60,
                "associations": ["symmetric encryption", "AES", "cryptography"]
            },
            {
                "name": "asymmetric_encryption",
                "prompt": "Generate atomic facts about asymmetric (public-key) encryption. Cover: public/private key pairs, RSA (how it works conceptually — prime factorization, key sizes 2048/4096), ECC (Elliptic Curve Cryptography, smaller keys, same security), Diffie-Hellman key exchange (how two parties agree on a shared secret over insecure channel), ECDH, ElGamal, key generation, why asymmetric is slower than symmetric.",
                "count": 60,
                "associations": ["asymmetric encryption", "RSA", "ECC", "public key", "cryptography"]
            },
            {
                "name": "hash_functions",
                "prompt": "Generate atomic facts about cryptographic hash functions. Cover: properties (deterministic, fixed output size, avalanche effect, pre-image resistance, collision resistance), SHA-256 (output size, usage), SHA-3, MD5 (broken, collision vulnerabilities), bcrypt (password hashing, salt, work factor), scrypt, Argon2 (memory-hard), HMAC (keyed hashing), hash vs encryption (one-way vs reversible), Merkle trees.",
                "count": 55,
                "associations": ["hash function", "SHA-256", "cryptography", "hashing"]
            },
            {
                "name": "digital_signatures_pki",
                "prompt": "Generate atomic facts about digital signatures and PKI. Cover: how digital signatures work (sign with private key, verify with public key), certificate authorities (CA), X.509 certificates, certificate chains (root CA, intermediate CA, leaf), certificate revocation (CRL, OCSP), self-signed certificates, Let's Encrypt, code signing, PGP/GPG, Web of Trust vs CA model, certificate transparency logs.",
                "count": 55,
                "associations": ["digital signature", "PKI", "certificate", "cryptography"]
            },
            {
                "name": "key_exchange_protocols",
                "prompt": "Generate atomic facts about key exchange protocols and advanced cryptography. Cover: Diffie-Hellman in detail, ECDHE (Ephemeral Elliptic Curve Diffie-Hellman), perfect forward secrecy (PFS), key derivation functions (KDF, HKDF, PBKDF2), key wrapping, key agreement vs key transport, session keys, pre-shared keys, authenticated key exchange.",
                "count": 50,
                "associations": ["key exchange", "Diffie-Hellman", "forward secrecy", "cryptography"]
            },
            {
                "name": "advanced_cryptography",
                "prompt": "Generate atomic facts about advanced cryptographic concepts. Cover: zero-knowledge proofs (proving knowledge without revealing it, zk-SNARKs, zk-STARKs), homomorphic encryption (computing on encrypted data, FHE), secure multi-party computation (MPC), secret sharing (Shamir's Secret Sharing), oblivious transfer, garbled circuits, post-quantum cryptography (lattice-based, hash-based, code-based), quantum key distribution (QKD).",
                "count": 55,
                "associations": ["zero-knowledge proof", "homomorphic encryption", "quantum cryptography", "advanced cryptography"]
            },
            {
                "name": "cryptographic_attacks",
                "prompt": "Generate atomic facts about cryptographic attacks and weaknesses. Cover: brute force attacks, birthday attack (on hash functions), man-in-the-middle attacks on key exchange, side-channel attacks (timing, power analysis, cache), padding oracle attacks, chosen-plaintext and chosen-ciphertext attacks, replay attacks, known weaknesses in MD5 and SHA-1, why ECB mode is insecure (patterns visible), rainbow tables and why salting defeats them, length extension attacks.",
                "count": 55,
                "associations": ["cryptographic attack", "security", "cryptanalysis", "cryptography"]
            },
            {
                "name": "real_world_crypto_applications",
                "prompt": "Generate atomic facts about real-world cryptography applications. Cover: HTTPS/TLS certificate ecosystem, end-to-end encryption in messaging (Signal Protocol, double ratchet), disk encryption (LUKS, BitLocker, FileVault), password managers (how they store passwords, master key derivation), blockchain and cryptocurrency (proof of work, proof of stake, Merkle trees, digital signatures for transactions), secure boot chain, code signing, SSH key authentication.",
                "count": 55,
                "associations": ["applied cryptography", "TLS", "encryption", "blockchain"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 3: VPN & PRIVACY TECHNOLOGY
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "vpn_privacy",
        "label": "VPN & Privacy Technology",
        "layer": 3,
        "topics": [
            {
                "name": "vpn_fundamentals",
                "prompt": "Generate atomic facts about how VPNs work. Cover: tunneling (encapsulating packets inside other packets), VPN types (site-to-site, remote access, client-to-site), split tunneling, full tunnel, VPN concentrators, VPN endpoints, encryption in transit, VPN overhead and MTU issues, VPN reconnection, kill switches, DNS leak protection.",
                "count": 55,
                "associations": ["VPN", "tunneling", "privacy", "encryption"]
            },
            {
                "name": "vpn_protocols",
                "prompt": "Generate atomic facts about VPN protocols. Cover: WireGuard (modern, fast, small codebase, uses ChaCha20/Poly1305, Curve25519), OpenVPN (SSL-based, uses OpenSSL, UDP or TCP, highly configurable), IPSec (IKEv2/IPSec, ESP, AH, transport vs tunnel mode), L2TP/IPSec, PPTP (deprecated, insecure), SSTP, specific strengths and weaknesses of each protocol.",
                "count": 60,
                "associations": ["VPN protocol", "WireGuard", "OpenVPN", "IPSec"]
            },
            {
                "name": "dpi_and_traffic_analysis",
                "prompt": "Generate atomic facts about deep packet inspection and traffic analysis. Cover: what DPI is (inspecting packet contents beyond headers), how DPI identifies VPN traffic, traffic fingerprinting (protocol signatures, packet size/timing patterns), statistical traffic analysis, flow analysis, how the Great Firewall of China uses DPI, how ISPs use DPI for throttling, encrypted SNI (ESNI/ECH), traffic classification techniques.",
                "count": 55,
                "associations": ["DPI", "traffic analysis", "censorship", "privacy"]
            },
            {
                "name": "obfuscation_techniques",
                "prompt": "Generate atomic facts about traffic obfuscation techniques. Cover: obfs4 (pluggable transport for Tor), Shadowsocks (SOCKS5 proxy with encryption), V2Ray/Xray (multiple protocols, VMESS, VLESS), domain fronting (hiding destination behind CDN), traffic padding, protocol mimicry (making VPN look like HTTPS), stunnel, Cloak, meek transport, CDN-based circumvention, encrypted DNS for circumvention.",
                "count": 55,
                "associations": ["obfuscation", "censorship circumvention", "Shadowsocks", "pluggable transport"]
            },
            {
                "name": "anonymity_networks",
                "prompt": "Generate atomic facts about anonymity networks. Cover: Tor (onion routing, three-hop circuits, entry/middle/exit relays, hidden services/.onion, Tor Browser, guard nodes, bridge relays), I2P (garlic routing, unidirectional tunnels, eepsites), mix networks (mixing messages to prevent correlation), Freenet, HORNET, metadata resistance, timing attacks on Tor, Tor's threat model.",
                "count": 55,
                "associations": ["Tor", "I2P", "anonymity", "onion routing", "privacy"]
            },
            {
                "name": "metadata_and_privacy",
                "prompt": "Generate atomic facts about metadata and digital privacy. Cover: what metadata is (data about data — who, when, where, how long, not content), why metadata matters (reveals patterns, relationships, habits), metadata from phone calls, emails, web browsing, how metadata enables surveillance, minimizing metadata leaks, VPN metadata (connection timestamps, data volume), IP address as identifier, browser fingerprinting.",
                "count": 50,
                "associations": ["metadata", "privacy", "surveillance", "digital privacy"]
            },
            {
                "name": "censorship_circumvention_techniques",
                "prompt": "Generate atomic facts about internet censorship circumvention. Cover: how China's Great Firewall works (DNS poisoning, IP blocking, DPI, keyword filtering, connection reset), how Iran/Russia censor (throttling, SNI filtering, protocol blocking), circumvention strategies (domain fronting, CDN-based proxies, encrypted DNS, pluggable transports), Psiphon, Lantern, Geneva (genetic algorithm for evading censorship), decoy routing, refraction networking, steganography for communication.",
                "count": 55,
                "associations": ["censorship", "circumvention", "Great Firewall", "privacy"]
            },
            {
                "name": "vpn_security_analysis",
                "prompt": "Generate atomic facts about VPN security and trust analysis. Cover: VPN logging policies (no-log claims, audit verification), VPN jurisdiction (Five Eyes, Fourteen Eyes countries), VPN trust model (you shift trust from ISP to VPN provider), VPN IP leaks (WebRTC leak, DNS leak, IPv6 leak), VPN kill switch mechanisms, multi-hop VPN (traffic through multiple servers), VPN over Tor vs Tor over VPN, warrant canaries, transparency reports, VPN protocol vulnerabilities.",
                "count": 50,
                "associations": ["VPN security", "VPN trust", "privacy", "logging"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 4: AI & MACHINE LEARNING
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "ai_ml",
        "label": "AI & Machine Learning",
        "layer": 4,
        "topics": [
            {
                "name": "neural_network_basics",
                "prompt": "Generate atomic facts about neural network fundamentals. Cover: what a neuron/perceptron is (weighted sum + activation), weights and biases, activation functions (sigmoid, ReLU, tanh, softmax, leaky ReLU), layers (input, hidden, output), feedforward networks, how a neural network makes predictions, universal approximation theorem, loss functions (MSE, cross-entropy).",
                "count": 60,
                "associations": ["neural network", "perceptron", "activation function", "AI"]
            },
            {
                "name": "backpropagation_training",
                "prompt": "Generate atomic facts about training neural networks. Cover: backpropagation (chain rule to compute gradients), gradient descent (SGD, mini-batch, learning rate), optimizers (Adam, RMSprop, AdaGrad, momentum), learning rate schedules, batch normalization, dropout (regularization), weight initialization (Xavier, He), overfitting vs underfitting, training/validation/test splits, early stopping, epochs and iterations.",
                "count": 60,
                "associations": ["backpropagation", "gradient descent", "training", "optimization", "AI"]
            },
            {
                "name": "cnns_computer_vision",
                "prompt": "Generate atomic facts about Convolutional Neural Networks and computer vision. Cover: convolution operation (filters/kernels sliding over input), feature maps, pooling (max pooling, average pooling), stride and padding, famous architectures (LeNet, AlexNet, VGG, ResNet, Inception), transfer learning, image classification, object detection (YOLO, R-CNN), semantic segmentation, data augmentation.",
                "count": 55,
                "associations": ["CNN", "convolutional neural network", "computer vision", "deep learning"]
            },
            {
                "name": "rnns_sequence_models",
                "prompt": "Generate atomic facts about Recurrent Neural Networks and sequence models. Cover: RNN structure (hidden state fed back), vanishing/exploding gradients in RNNs, LSTM (Long Short-Term Memory — forget gate, input gate, output gate, cell state), GRU (Gated Recurrent Unit), bidirectional RNNs, sequence-to-sequence models, encoder-decoder architecture, attention mechanism (the key innovation before transformers).",
                "count": 55,
                "associations": ["RNN", "LSTM", "GRU", "sequence model", "deep learning"]
            },
            {
                "name": "transformers_and_attention",
                "prompt": "Generate atomic facts about the Transformer architecture. Cover: self-attention mechanism (Query, Key, Value matrices), multi-head attention, positional encoding (why transformers need it — no inherent sequence order), encoder and decoder stacks, layer normalization, residual connections, the original 'Attention Is All You Need' paper (2017, Vaswani et al), why transformers replaced RNNs (parallelizable, long-range dependencies), context window, tokenization.",
                "count": 60,
                "associations": ["transformer", "attention", "self-attention", "deep learning", "NLP"]
            },
            {
                "name": "nlp_fundamentals",
                "prompt": "Generate atomic facts about Natural Language Processing. Cover: tokenization (word-level, subword, BPE, WordPiece), word embeddings (Word2Vec, GloVe — words as vectors, similar words near each other), BERT (bidirectional, masked language model, pre-training + fine-tuning), GPT (autoregressive, next-token prediction), language model perplexity, named entity recognition, sentiment analysis, text classification, prompt engineering.",
                "count": 60,
                "associations": ["NLP", "language model", "BERT", "GPT", "embeddings"]
            },
            {
                "name": "reinforcement_learning",
                "prompt": "Generate atomic facts about reinforcement learning. Cover: agent, environment, state, action, reward, policy, value function, Markov Decision Process (MDP), Q-learning, Deep Q-Networks (DQN), policy gradient methods, actor-critic, reward shaping, exploration vs exploitation (epsilon-greedy), discount factor (gamma), RLHF (Reinforcement Learning from Human Feedback), PPO (Proximal Policy Optimization), AlphaGo.",
                "count": 55,
                "associations": ["reinforcement learning", "Q-learning", "reward", "policy", "AI"]
            },
            {
                "name": "generative_ai",
                "prompt": "Generate atomic facts about generative AI models. Cover: GANs (generator vs discriminator, adversarial training, mode collapse), VAEs (Variational Autoencoders — encoder, decoder, latent space, KL divergence), diffusion models (forward process adds noise, reverse process removes it, DDPM, stable diffusion), autoregressive generation, text-to-image models, large language models (LLMs), fine-tuning, LoRA, quantization.",
                "count": 55,
                "associations": ["generative AI", "GAN", "diffusion model", "VAE", "LLM"]
            },
            {
                "name": "ai_ethics_safety",
                "prompt": "Generate atomic facts about AI ethics and safety. Cover: bias in training data, fairness metrics, AI alignment (ensuring AI goals match human goals), interpretability/explainability (why a model made a decision), hallucinations in language models, deepfakes, AI in surveillance, responsible AI development, AI regulation approaches, existential risk debate, AI safety research, adversarial attacks on ML models.",
                "count": 50,
                "associations": ["AI ethics", "AI safety", "bias", "alignment"]
            },
            {
                "name": "ml_practical",
                "prompt": "Generate atomic facts about practical machine learning. Cover: data preprocessing (normalization, standardization, one-hot encoding, handling missing values), feature engineering, feature selection, cross-validation (k-fold), bias-variance tradeoff, ensemble methods (random forest, gradient boosting, XGBoost, bagging), hyperparameter tuning (grid search, random search, Bayesian optimization), confusion matrix (precision, recall, F1-score, accuracy), ROC curve and AUC, underfitting vs overfitting.",
                "count": 60,
                "associations": ["machine learning", "training", "evaluation", "practical ML"]
            },
            {
                "name": "knowledge_graphs_embeddings",
                "prompt": "Generate atomic facts about knowledge representation in AI. Cover: knowledge graphs (nodes, edges, triples: subject-predicate-object), ontologies, semantic web (RDF, OWL), word embeddings (Word2Vec skip-gram and CBOW, cosine similarity, analogy tasks like king-man+woman=queen), sentence embeddings (SBERT), embedding spaces, dimensionality reduction (PCA, t-SNE, UMAP), vector databases (similarity search, approximate nearest neighbors), retrieval-augmented generation (RAG).",
                "count": 55,
                "associations": ["knowledge graph", "embedding", "vector", "semantic", "AI"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 5: COMPUTER SCIENCE FUNDAMENTALS
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "cs_fundamentals",
        "label": "Computer Science Fundamentals",
        "layer": 5,
        "topics": [
            {
                "name": "data_structures_linear",
                "prompt": "Generate atomic facts about linear data structures. Cover: arrays (contiguous memory, O(1) access, O(n) insert), dynamic arrays (amortized doubling), linked lists (singly linked, doubly linked, O(1) insert at head, O(n) search), stacks (LIFO, push/pop, uses: function calls, undo, bracket matching), queues (FIFO, enqueue/dequeue, uses: BFS, task scheduling), deques (double-ended queue), circular buffers.",
                "count": 55,
                "associations": ["data structure", "array", "linked list", "stack", "queue"]
            },
            {
                "name": "data_structures_trees_graphs",
                "prompt": "Generate atomic facts about trees and graphs. Cover: binary trees (at most 2 children per node), binary search trees (left < parent < right, O(log n) operations), balanced BSTs (AVL, Red-Black), B-trees (used in databases, multiple keys per node), heaps (min-heap, max-heap, priority queues), tries (prefix trees), graphs (directed, undirected, weighted, adjacency list vs adjacency matrix), graph terminology (vertices, edges, degree, path, cycle).",
                "count": 60,
                "associations": ["data structure", "tree", "graph", "BST", "heap"]
            },
            {
                "name": "data_structures_hash_advanced",
                "prompt": "Generate atomic facts about hash tables and advanced data structures. Cover: hash tables (hash function, O(1) average lookup, collision handling: chaining, open addressing), load factor, rehashing, bloom filters (probabilistic set membership, no false negatives), skip lists, union-find (disjoint sets), LRU cache implementation, consistent hashing.",
                "count": 50,
                "associations": ["hash table", "bloom filter", "data structure"]
            },
            {
                "name": "algorithms_sorting_searching",
                "prompt": "Generate atomic facts about sorting and searching algorithms. Cover: bubble sort (O(n²), simple, stable), insertion sort (O(n²), good for small/nearly-sorted), merge sort (O(n log n), stable, divide-and-conquer), quicksort (O(n log n) average, O(n²) worst, in-place, pivot selection), heapsort (O(n log n), in-place), counting sort (O(n+k), non-comparison), radix sort, binary search (O(log n), requires sorted data), comparison sort lower bound (O(n log n)).",
                "count": 60,
                "associations": ["algorithm", "sorting", "searching", "complexity"]
            },
            {
                "name": "algorithms_graph",
                "prompt": "Generate atomic facts about graph algorithms. Cover: BFS (breadth-first search, uses queue, shortest path in unweighted graphs), DFS (depth-first search, uses stack/recursion, cycle detection), Dijkstra's algorithm (shortest path with non-negative weights), Bellman-Ford (handles negative weights), A* search (heuristic-guided), topological sort (DAGs, dependency ordering), Kruskal's and Prim's (minimum spanning tree), Floyd-Warshall (all-pairs shortest path).",
                "count": 55,
                "associations": ["graph algorithm", "BFS", "DFS", "shortest path", "algorithm"]
            },
            {
                "name": "algorithms_dynamic_programming",
                "prompt": "Generate atomic facts about dynamic programming and algorithm design. Cover: what dynamic programming is (breaking into overlapping subproblems, memoization), top-down vs bottom-up DP, classic DP problems (Fibonacci, knapsack, longest common subsequence, edit distance, coin change), greedy algorithms (locally optimal choices, when they work), divide and conquer, backtracking, amortized analysis.",
                "count": 50,
                "associations": ["dynamic programming", "memoization", "greedy", "algorithm"]
            },
            {
                "name": "complexity_theory",
                "prompt": "Generate atomic facts about computational complexity theory. Cover: Big-O notation (upper bound), Big-Omega (lower bound), Big-Theta (tight bound), common complexities (O(1), O(log n), O(n), O(n log n), O(n²), O(2^n), O(n!)), space complexity, P (solvable in polynomial time), NP (verifiable in polynomial time), NP-hard, NP-complete, P vs NP problem, examples of NP-complete problems (SAT, traveling salesman, graph coloring), reduction.",
                "count": 50,
                "associations": ["complexity", "Big-O", "P vs NP", "algorithm analysis"]
            },
            {
                "name": "operating_systems",
                "prompt": "Generate atomic facts about operating system concepts. Cover: processes vs threads, context switching, process scheduling (round-robin, priority, FCFS, multilevel feedback queue), virtual memory (paging, page tables, page faults, TLB), memory management (heap, stack, malloc/free), deadlock (conditions: mutual exclusion, hold-and-wait, no preemption, circular wait), file systems (inodes, FAT, ext4, NTFS), system calls, kernel space vs user space, interrupts.",
                "count": 60,
                "associations": ["operating system", "process", "memory", "scheduling", "file system"]
            },
            {
                "name": "databases",
                "prompt": "Generate atomic facts about databases. Cover: relational databases (tables, rows, columns, SQL), SQL operations (SELECT, INSERT, UPDATE, DELETE, JOIN types), ACID properties (Atomicity, Consistency, Isolation, Durability), indexing (B-tree index, hash index, why indexes speed up reads but slow writes), normalization (1NF, 2NF, 3NF), NoSQL types (document, key-value, column-family, graph), CAP theorem, transactions, query optimization, connection pooling.",
                "count": 60,
                "associations": ["database", "SQL", "NoSQL", "ACID", "indexing"]
            },
            {
                "name": "distributed_systems",
                "prompt": "Generate atomic facts about distributed systems. Cover: CAP theorem (Consistency, Availability, Partition tolerance — pick 2), eventual consistency, consensus algorithms (Raft, Paxos), distributed hash tables (DHT), replication (leader-follower, multi-leader), sharding/partitioning, vector clocks, two-phase commit, microservices vs monolith, message queues (Kafka, RabbitMQ), service discovery, load balancing strategies.",
                "count": 55,
                "associations": ["distributed systems", "CAP theorem", "consensus", "replication"]
            },
            {
                "name": "compilers_interpreters",
                "prompt": "Generate atomic facts about compilers and interpreters. Cover: compilation pipeline (lexing, parsing, semantic analysis, optimization, code generation), tokens and lexical analysis, AST (Abstract Syntax Tree), context-free grammars, parser types (recursive descent, LR, LL), type checking, intermediate representation (IR), JIT compilation (Just-In-Time), interpreted vs compiled languages, bytecode (JVM, Python bytecode), linking (static vs dynamic).",
                "count": 50,
                "associations": ["compiler", "interpreter", "AST", "parsing", "compilation"]
            },
            {
                "name": "concurrency_parallelism",
                "prompt": "Generate atomic facts about concurrency and parallelism. Cover: concurrency vs parallelism (interleaving vs simultaneous), threads vs processes, race conditions, mutexes and locks, semaphores, deadlock prevention, readers-writer locks, condition variables, atomic operations, lock-free data structures, actor model, CSP (Communicating Sequential Processes), async/await vs threads, green threads/coroutines, thread pools, work stealing.",
                "count": 55,
                "associations": ["concurrency", "parallelism", "thread", "mutex", "deadlock"]
            },
            {
                "name": "computer_architecture",
                "prompt": "Generate atomic facts about computer architecture. Cover: CPU components (ALU, control unit, registers, cache), instruction cycle (fetch-decode-execute), pipelining, branch prediction, cache hierarchy (L1, L2, L3, cache line, cache miss), RAM (DRAM vs SRAM), virtual memory and TLB, memory hierarchy, bus architecture, I/O (interrupts, DMA), RISC vs CISC (ARM vs x86), multi-core processors, SIMD instructions, GPU architecture (massive parallelism, shader cores).",
                "count": 55,
                "associations": ["computer architecture", "CPU", "cache", "memory", "hardware"]
            },
            {
                "name": "version_control_git",
                "prompt": "Generate atomic facts about version control and Git. Cover: what version control is (tracking changes over time), Git's data model (snapshots, not diffs; content-addressable objects: blobs, trees, commits), branches (lightweight pointers to commits), merging (fast-forward, three-way merge), rebasing, HEAD pointer, staging area (index), common commands (commit, push, pull, fetch, merge, rebase, cherry-pick, stash), merge conflicts, remote repositories, pull requests, Git internals (SHA-1 hashes, packfiles).",
                "count": 55,
                "associations": ["Git", "version control", "branch", "merge", "commit"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 6: PROGRAMMING LANGUAGES
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "programming",
        "label": "Programming Languages",
        "layer": 6,
        "topics": [
            {
                "name": "rust_ownership_borrowing",
                "prompt": "Generate atomic facts about Rust's ownership and borrowing system. Cover: ownership rules (each value has one owner, when owner goes out of scope value is dropped), move semantics, the borrow checker, immutable references (&T), mutable references (&mut T), only one mutable reference OR multiple immutable references at a time, lifetimes ('a annotations), lifetime elision rules, the 'static lifetime, dangling reference prevention.",
                "count": 55,
                "associations": ["Rust", "ownership", "borrowing", "lifetime", "memory safety"]
            },
            {
                "name": "rust_types_traits",
                "prompt": "Generate atomic facts about Rust types and traits. Cover: primitive types (i8-i128, u8-u128, f32, f64, bool, char), String vs &str, Vec<T>, Option<T> (Some/None), Result<T,E> (Ok/Err), enums with data, pattern matching (match), structs, traits (like interfaces), trait bounds, generics, impl blocks, derive macros (Debug, Clone, PartialEq), Box<T>, Rc<T>, Arc<T>, RefCell<T>.",
                "count": 60,
                "associations": ["Rust", "type system", "trait", "generic", "enum"]
            },
            {
                "name": "rust_async_error_handling",
                "prompt": "Generate atomic facts about Rust async programming and error handling. Cover: async/await syntax, Future trait, tokio runtime, async fn returns impl Future, .await suspends execution, error handling with Result and ? operator, unwrap vs expect, custom error types, thiserror and anyhow crates, panic vs recoverable errors, unsafe blocks (what they allow), Send and Sync traits, channels (mpsc).",
                "count": 55,
                "associations": ["Rust", "async", "error handling", "tokio", "concurrency"]
            },
            {
                "name": "python_fundamentals",
                "prompt": "Generate atomic facts about Python programming. Cover: dynamic typing, indentation-based syntax, basic types (int, float, str, bool, None), lists, tuples, dictionaries, sets, list comprehensions, f-strings, slicing, unpacking, *args/**kwargs, type hints, duck typing, GIL (Global Interpreter Lock), pip and virtual environments, __init__, __str__, __repr__, dunder methods.",
                "count": 60,
                "associations": ["Python", "programming", "dynamic typing"]
            },
            {
                "name": "python_advanced",
                "prompt": "Generate atomic facts about advanced Python features. Cover: decorators (@decorator syntax, functools.wraps), generators (yield, lazy evaluation, memory efficient), context managers (with statement, __enter__/__exit__), iterators (__iter__/__next__), lambda functions, map/filter/reduce, dataclasses, ABC (Abstract Base Classes), metaclasses, descriptors, slots, asyncio, multiprocessing vs threading.",
                "count": 55,
                "associations": ["Python", "decorator", "generator", "advanced"]
            },
            {
                "name": "javascript_fundamentals",
                "prompt": "Generate atomic facts about JavaScript fundamentals. Cover: let vs const vs var (block scope, hoisting), primitive types (string, number, boolean, null, undefined, symbol, bigint), objects and arrays, arrow functions, template literals, destructuring, spread/rest operators, truthy/falsy values, == vs === (loose vs strict equality), prototype chain, this keyword binding rules, closures, event loop (call stack, task queue, microtasks).",
                "count": 60,
                "associations": ["JavaScript", "programming", "web", "closure", "prototype"]
            },
            {
                "name": "typescript_and_async_js",
                "prompt": "Generate atomic facts about TypeScript and async JavaScript. Cover TypeScript: type annotations, interfaces, type aliases, union types (|), intersection types (&), generics, enums, type narrowing, type guards, utility types (Partial, Required, Pick, Omit, Record). Cover async JS: Promises (resolve, reject, then, catch), async/await, Promise.all, Promise.race, fetch API, callbacks, callback hell.",
                "count": 55,
                "associations": ["TypeScript", "JavaScript", "async", "Promise", "type system"]
            },
            {
                "name": "programming_paradigms",
                "prompt": "Generate atomic facts about programming paradigms and concepts. Cover: OOP (encapsulation, inheritance, polymorphism, abstraction, SOLID principles), functional programming (pure functions, immutability, higher-order functions, first-class functions, referential transparency, monads), imperative vs declarative, type systems (static vs dynamic, strong vs weak, structural vs nominal), garbage collection (mark-and-sweep, reference counting), memory management (stack vs heap).",
                "count": 55,
                "associations": ["programming paradigm", "OOP", "functional programming", "type system"]
            },
            {
                "name": "rust_ecosystem",
                "prompt": "Generate atomic facts about the Rust ecosystem and tooling. Cover: Cargo (package manager and build system), crates.io (package registry), Cargo.toml (dependencies, features), modules and visibility (pub, mod, use), common crates (serde for serialization, tokio for async, reqwest for HTTP, clap for CLI args, tracing for logging), error handling patterns (anyhow for applications, thiserror for libraries), testing (#[test], #[cfg(test)], integration tests), documentation (/// doc comments, cargo doc), clippy (linter), rustfmt.",
                "count": 55,
                "associations": ["Rust", "Cargo", "crates", "ecosystem", "tooling"]
            },
            {
                "name": "web_development_patterns",
                "prompt": "Generate atomic facts about web development patterns and practices. Cover: MVC (Model-View-Controller), SPA (Single Page Application), SSR (Server-Side Rendering), SSG (Static Site Generation), hydration, component-based architecture (React, Vue, Svelte), state management (Redux, Zustand, signals), routing (client-side vs server-side), bundlers (webpack, Vite, esbuild), package managers (npm, yarn, pnpm), REST API design patterns, middleware pattern, authentication flows.",
                "count": 55,
                "associations": ["web development", "SPA", "React", "design pattern"]
            },
            {
                "name": "design_patterns",
                "prompt": "Generate atomic facts about software design patterns. Cover: creational patterns (Singleton, Factory, Builder, Prototype), structural patterns (Adapter, Decorator, Facade, Proxy, Composite), behavioral patterns (Observer, Strategy, Command, Iterator, State, Template Method), SOLID principles in detail (Single Responsibility, Open/Closed, Liskov Substitution, Interface Segregation, Dependency Inversion), DRY, KISS, YAGNI, composition over inheritance.",
                "count": 55,
                "associations": ["design pattern", "SOLID", "software engineering", "architecture"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 7: MATHEMATICS
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "mathematics",
        "label": "Mathematics",
        "layer": 7,
        "topics": [
            {
                "name": "linear_algebra",
                "prompt": "Generate atomic facts about linear algebra. Cover: vectors (magnitude, direction, addition, scalar multiplication, dot product, cross product), matrices (addition, multiplication, transpose, inverse, determinant), eigenvalues and eigenvectors (Av = λv), linear transformations (rotation, scaling, projection), matrix decompositions (LU, QR, SVD), vector spaces, basis, dimension, rank, null space, linear independence.",
                "count": 60,
                "associations": ["linear algebra", "vector", "matrix", "eigenvalue", "math"]
            },
            {
                "name": "calculus",
                "prompt": "Generate atomic facts about calculus. Cover: limits (approaching a value), derivatives (rate of change, slope of tangent), derivative rules (power rule, product rule, quotient rule, chain rule), common derivatives (d/dx of x^n, sin x, cos x, e^x, ln x), integrals (area under curve, antiderivative), integration techniques (substitution, integration by parts), fundamental theorem of calculus, partial derivatives, gradient (vector of partial derivatives), multivariable chain rule.",
                "count": 60,
                "associations": ["calculus", "derivative", "integral", "gradient", "math"]
            },
            {
                "name": "probability_statistics",
                "prompt": "Generate atomic facts about probability and statistics. Cover: probability axioms (0 ≤ P ≤ 1, P(sample space) = 1), conditional probability P(A|B), Bayes' theorem (P(A|B) = P(B|A)P(A)/P(B)), distributions (normal/Gaussian, Bernoulli, binomial, Poisson, uniform, exponential), mean, median, mode, variance, standard deviation, central limit theorem, hypothesis testing (null hypothesis, p-value, significance level), confidence intervals, correlation vs causation.",
                "count": 60,
                "associations": ["probability", "statistics", "Bayes", "distribution", "math"]
            },
            {
                "name": "information_theory",
                "prompt": "Generate atomic facts about information theory. Cover: entropy (H = -Σ p(x) log p(x), measures uncertainty/information content), bits (unit of information), mutual information, KL divergence (measures difference between distributions), cross-entropy (used as loss function in ML), data compression (Shannon's source coding theorem), channel capacity (Shannon's noisy channel theorem), lossless vs lossy compression, Huffman coding, information gain.",
                "count": 50,
                "associations": ["information theory", "entropy", "Shannon", "KL divergence", "math"]
            },
            {
                "name": "graph_theory",
                "prompt": "Generate atomic facts about graph theory. Cover: vertices and edges, directed vs undirected graphs, weighted graphs, degree of a vertex, paths and cycles, connected graphs, trees (connected acyclic graphs), Euler paths/circuits, Hamiltonian paths/circuits, planarity (Kuratowski's theorem), graph coloring (chromatic number), bipartite graphs, complete graphs (Kn), adjacency matrix, shortest path problems, network flow.",
                "count": 50,
                "associations": ["graph theory", "vertex", "edge", "path", "math"]
            },
            {
                "name": "logic_and_proofs",
                "prompt": "Generate atomic facts about mathematical logic. Cover: propositional logic (AND, OR, NOT, implication, biconditional), truth tables, tautology and contradiction, predicate logic (for all ∀, there exists ∃), proof techniques (direct proof, proof by contradiction, proof by induction, proof by contrapositive), modus ponens, modus tollens, De Morgan's laws, Boolean algebra, logical equivalences, satisfiability.",
                "count": 50,
                "associations": ["logic", "proof", "propositional", "predicate", "math"]
            },
            {
                "name": "number_theory_basics",
                "prompt": "Generate atomic facts about number theory. Cover: prime numbers (definition, infinitely many, Fundamental Theorem of Arithmetic), divisibility rules, GCD and LCM (Euclidean algorithm), modular arithmetic (congruence, modular inverse, Fermat's little theorem), prime factorization, Sieve of Eratosthenes, RSA relies on difficulty of factoring large semiprimes, Euler's totient function, coprime/relatively prime numbers, Chinese Remainder Theorem.",
                "count": 50,
                "associations": ["number theory", "prime", "modular arithmetic", "math"]
            },
            {
                "name": "set_theory_combinatorics",
                "prompt": "Generate atomic facts about set theory and combinatorics. Cover: sets (union, intersection, complement, difference, subset, power set), Venn diagrams, cardinality, countable vs uncountable sets, permutations (n!), combinations (n choose k), binomial theorem, pigeonhole principle, inclusion-exclusion principle, generating functions, Catalan numbers, recurrence relations, counting principles (multiplication, addition).",
                "count": 50,
                "associations": ["set theory", "combinatorics", "permutation", "combination", "math"]
            },
            {
                "name": "optimization",
                "prompt": "Generate atomic facts about mathematical optimization. Cover: convex optimization (convex functions have one global minimum), gradient descent (follow negative gradient), stochastic gradient descent, Newton's method, constraint optimization (Lagrange multipliers), linear programming (simplex method), convex vs non-convex problems, local vs global optima, saddle points, learning rate importance, convergence, loss landscapes in neural networks.",
                "count": 50,
                "associations": ["optimization", "gradient descent", "convex", "math"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 8: PHYSICS
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "physics",
        "label": "Physics",
        "layer": 8,
        "topics": [
            {
                "name": "classical_mechanics",
                "prompt": "Generate atomic facts about classical mechanics. Cover: Newton's three laws of motion (inertia, F=ma, action-reaction), mass vs weight, gravitational force (F = Gm₁m₂/r²), kinetic energy (½mv²), potential energy (mgh), conservation of energy, conservation of momentum, work (W = Fd cos θ), power (P = W/t), friction (static, kinetic), simple harmonic motion, projectile motion, circular motion, torque, angular momentum.",
                "count": 60,
                "associations": ["mechanics", "Newton", "force", "energy", "physics"]
            },
            {
                "name": "electromagnetism",
                "prompt": "Generate atomic facts about electromagnetism. Cover: electric charge (positive, negative), Coulomb's law (F = kq₁q₂/r²), electric field, voltage (potential difference), current (I = Q/t), resistance (Ohm's law V = IR), circuits (series, parallel), capacitance, inductance, magnetic fields, Faraday's law (electromagnetic induction), Maxwell's equations (four equations unifying electricity and magnetism), electromagnetic waves (light, radio, X-rays — all same phenomenon, different wavelengths), speed of light.",
                "count": 60,
                "associations": ["electromagnetism", "Maxwell", "electric", "magnetic", "physics"]
            },
            {
                "name": "thermodynamics",
                "prompt": "Generate atomic facts about thermodynamics. Cover: temperature (average kinetic energy of molecules), heat vs temperature, zeroth law (thermal equilibrium is transitive), first law (energy conservation, ΔU = Q - W), second law (entropy always increases in isolated system, heat flows from hot to cold), third law (absolute zero unattainable), entropy (measure of disorder), heat engines, Carnot efficiency, specific heat capacity, phase transitions.",
                "count": 50,
                "associations": ["thermodynamics", "entropy", "heat", "energy", "physics"]
            },
            {
                "name": "quantum_mechanics",
                "prompt": "Generate atomic facts about quantum mechanics. Cover: wave-particle duality (light and matter behave as both), Planck's constant (h), quantization of energy, photoelectric effect (Einstein), Heisenberg uncertainty principle (can't know both position and momentum precisely), Schrödinger equation, wave function and probability, superposition (quantum system in multiple states simultaneously), quantum entanglement (correlated particles), measurement problem (wave function collapse), quantum tunneling, spin.",
                "count": 55,
                "associations": ["quantum mechanics", "superposition", "entanglement", "physics"]
            },
            {
                "name": "relativity",
                "prompt": "Generate atomic facts about Einstein's theory of relativity. Cover: special relativity (speed of light is constant for all observers, time dilation, length contraction, E=mc², simultaneity is relative, no faster-than-light travel), general relativity (gravity is curvature of spacetime, massive objects bend spacetime, gravitational time dilation, gravitational lensing, prediction of black holes, gravitational waves), equivalence principle.",
                "count": 50,
                "associations": ["relativity", "Einstein", "spacetime", "physics"]
            },
            {
                "name": "waves_and_optics",
                "prompt": "Generate atomic facts about waves and optics. Cover: wave properties (wavelength, frequency, amplitude, speed, v=f*lambda), transverse vs longitudinal waves, electromagnetic spectrum (radio, microwave, infrared, visible, UV, X-ray, gamma — in order of increasing frequency), speed of light (299,792,458 m/s), reflection, refraction (Snell's law), diffraction, interference (constructive, destructive), polarization, fiber optics (total internal reflection), sound waves (compression waves in medium).",
                "count": 55,
                "associations": ["waves", "optics", "electromagnetic spectrum", "physics"]
            },
            {
                "name": "atomic_physics_nuclear",
                "prompt": "Generate atomic facts about atomic and nuclear physics. Cover: atom structure (protons, neutrons, electrons), atomic number and mass number, isotopes, electron orbitals and shells, periodic table organization, chemical bonds (ionic, covalent, metallic), radioactive decay (alpha, beta, gamma), half-life, nuclear fission (splitting heavy atoms, nuclear power), nuclear fusion (combining light atoms, powers the sun), strong and weak nuclear forces, standard model particles (quarks, leptons, bosons).",
                "count": 55,
                "associations": ["atomic physics", "nuclear", "atom", "particle", "physics"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 9: CYBERSECURITY
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "cybersecurity",
        "label": "Cybersecurity",
        "layer": 9,
        "topics": [
            {
                "name": "threat_models_attack_surfaces",
                "prompt": "Generate atomic facts about threat modeling and attack surfaces. Cover: what a threat model is (identifying assets, threats, mitigations), attack surface (all points where an attacker can try to enter), STRIDE threat model (Spoofing, Tampering, Repudiation, Information Disclosure, Denial of Service, Elevation of Privilege), attack vectors, threat actors (nation-state, organized crime, insider, hacktivist), defense in depth, principle of least privilege, zero trust architecture.",
                "count": 55,
                "associations": ["threat model", "attack surface", "cybersecurity", "defense"]
            },
            {
                "name": "common_vulnerabilities",
                "prompt": "Generate atomic facts about common software vulnerabilities. Cover: SQL injection (unsanitized input in SQL queries), XSS (cross-site scripting — injecting scripts into web pages, stored vs reflected), CSRF (cross-site request forgery — tricking user's browser to make requests), buffer overflow (writing beyond allocated memory), path traversal (../), command injection, SSRF (server-side request forgery), insecure deserialization, XXE (XML external entities), IDOR (insecure direct object references), race conditions.",
                "count": 60,
                "associations": ["vulnerability", "OWASP", "injection", "XSS", "cybersecurity"]
            },
            {
                "name": "authentication_authorization",
                "prompt": "Generate atomic facts about authentication and authorization. Cover: authentication (proving identity) vs authorization (proving permission), passwords (hashing with salt, bcrypt/Argon2), multi-factor authentication (something you know/have/are), OAuth 2.0 (authorization framework, access tokens, refresh tokens, grant types), JWT (JSON Web Token — header.payload.signature, stateless), session-based auth, API keys, RBAC (Role-Based Access Control), SSO (Single Sign-On), SAML, OpenID Connect.",
                "count": 55,
                "associations": ["authentication", "authorization", "OAuth", "JWT", "security"]
            },
            {
                "name": "network_security",
                "prompt": "Generate atomic facts about network security. Cover: firewalls (packet filtering, stateful inspection, next-gen), IDS vs IPS (Intrusion Detection vs Prevention — signature-based, anomaly-based), SIEM (Security Information and Event Management — log aggregation, correlation, alerting), honeypots, network segmentation, VLANs for security, DMZ, port scanning (nmap), packet sniffing, man-in-the-middle attacks, ARP spoofing defense, network access control (NAC).",
                "count": 55,
                "associations": ["network security", "firewall", "IDS", "IPS", "SIEM"]
            },
            {
                "name": "malware_types",
                "prompt": "Generate atomic facts about malware types and techniques. Cover: viruses (self-replicating, attach to programs), worms (self-propagating across networks), trojans (disguised as legitimate software), ransomware (encrypts files, demands payment), spyware (monitors activity), adware, rootkits (hide deep in OS), RATs (Remote Access Trojans — give attacker control), keyloggers, botnets, fileless malware, polymorphic malware (changes signature), zero-day exploits.",
                "count": 55,
                "associations": ["malware", "virus", "ransomware", "trojan", "cybersecurity"]
            },
            {
                "name": "incident_response_forensics",
                "prompt": "Generate atomic facts about incident response and digital forensics. Cover: incident response phases (preparation, identification, containment, eradication, recovery, lessons learned), NIST incident response framework, chain of custody, disk imaging, memory forensics, log analysis, indicators of compromise (IoC), YARA rules, reverse engineering malware, sandbox analysis, threat intelligence, CVE (Common Vulnerabilities and Exposures), penetration testing vs red teaming.",
                "count": 50,
                "associations": ["incident response", "forensics", "NIST", "cybersecurity"]
            },
            {
                "name": "web_security",
                "prompt": "Generate atomic facts about web application security. Cover: OWASP Top 10 in detail, Content Security Policy (CSP), CORS (Cross-Origin Resource Sharing), HTTP security headers (X-Frame-Options, X-Content-Type-Options, Strict-Transport-Security), input validation and sanitization, parameterized queries (preventing SQL injection), output encoding (preventing XSS), secure cookie flags (HttpOnly, Secure, SameSite), rate limiting, WAF (Web Application Firewall), security testing tools (Burp Suite, OWASP ZAP).",
                "count": 55,
                "associations": ["web security", "OWASP", "XSS prevention", "cybersecurity"]
            },
            {
                "name": "social_engineering",
                "prompt": "Generate atomic facts about social engineering attacks. Cover: phishing (email-based, spear phishing targets specific people, whaling targets executives), vishing (voice phishing), smishing (SMS phishing), pretexting (creating false scenarios), baiting (leaving infected USB drives), tailgating (following authorized person through door), watering hole attacks, credential stuffing, typosquatting, business email compromise (BEC), how to recognize social engineering, security awareness training.",
                "count": 50,
                "associations": ["social engineering", "phishing", "attack", "cybersecurity"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 10: INTERNET & WEB
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "web",
        "label": "Internet & Web",
        "layer": 10,
        "topics": [
            {
                "name": "how_the_web_works",
                "prompt": "Generate atomic facts about how the web works. Cover: client-server model, what happens when you type a URL (DNS lookup, TCP connection, TLS handshake, HTTP request, server processing, HTTP response, browser rendering), web servers (Apache, Nginx), reverse proxies, request/response cycle, stateless nature of HTTP, cookies for state, web sockets for persistent connections, same-origin policy.",
                "count": 55,
                "associations": ["web", "HTTP", "client-server", "browser", "internet"]
            },
            {
                "name": "apis_and_protocols",
                "prompt": "Generate atomic facts about web APIs and protocols. Cover: REST (resources, HTTP methods, stateless, JSON responses, status codes), GraphQL (single endpoint, query language, client specifies data shape, resolvers), WebSocket (full-duplex persistent connection, real-time), gRPC (Protocol Buffers, HTTP/2, streaming), SOAP (XML-based, WSDL), API versioning, rate limiting, pagination, API authentication (API keys, OAuth, bearer tokens), webhook.",
                "count": 55,
                "associations": ["API", "REST", "GraphQL", "WebSocket", "web"]
            },
            {
                "name": "web_infrastructure",
                "prompt": "Generate atomic facts about web infrastructure. Cover: CDN (Content Delivery Network — edge servers, caching closer to users, Cloudflare, Akamai), load balancing (round-robin, least connections, consistent hashing), caching (browser cache, server cache, Redis, Memcached, cache invalidation), DNS infrastructure, SSL/TLS certificates for websites, HTTP/2 features (multiplexing, server push, header compression), HTTP/3 (QUIC, UDP-based).",
                "count": 50,
                "associations": ["CDN", "load balancing", "caching", "web infrastructure"]
            },
            {
                "name": "cloud_computing",
                "prompt": "Generate atomic facts about cloud computing. Cover: IaaS (Infrastructure as a Service — virtual machines, storage), PaaS (Platform as a Service — managed runtime), SaaS (Software as a Service — end-user applications), major providers (AWS, Azure, GCP), containers (Docker — package app with dependencies, Dockerfile, images, containers), Kubernetes (container orchestration, pods, services, deployments), serverless (AWS Lambda, functions-as-a-service), microservices architecture, CI/CD pipelines.",
                "count": 55,
                "associations": ["cloud", "AWS", "Docker", "Kubernetes", "serverless"]
            },
            {
                "name": "html_css_basics",
                "prompt": "Generate atomic facts about HTML and CSS. Cover HTML: semantic elements (header, nav, main, article, section, footer), forms, tables, links, images, lists, accessibility (ARIA). Cover CSS: selectors (element, class, ID, descendant, child), box model (margin, border, padding, content), flexbox (justify-content, align-items), CSS grid, positioning (static, relative, absolute, fixed, sticky), responsive design (media queries, viewport units), specificity.",
                "count": 55,
                "associations": ["HTML", "CSS", "web development", "frontend"]
            },
            {
                "name": "browser_internals",
                "prompt": "Generate atomic facts about how web browsers work. Cover: rendering engine (parsing HTML into DOM tree, CSSOM, render tree, layout, paint, compositing), JavaScript engine (V8, SpiderMonkey, JIT compilation), event loop (single-threaded, task queue, microtask queue), Web APIs (setTimeout, fetch, DOM manipulation), service workers (offline caching, push notifications), Web Workers (background threads), WebAssembly (near-native performance in browser), IndexedDB, localStorage, sessionStorage.",
                "count": 50,
                "associations": ["browser", "rendering", "V8", "DOM", "web"]
            },
            {
                "name": "devops_practices",
                "prompt": "Generate atomic facts about DevOps practices. Cover: CI/CD (Continuous Integration/Continuous Deployment, automated testing and deployment), infrastructure as code (Terraform, Ansible, CloudFormation), monitoring and observability (Prometheus, Grafana, alerting), logging (structured logging, log aggregation, ELK stack), containerization workflow (build, push, deploy), blue-green deployment, canary deployment, rolling updates, feature flags, GitOps, SRE (Site Reliability Engineering, SLOs, SLIs, error budgets).",
                "count": 55,
                "associations": ["DevOps", "CI/CD", "infrastructure", "deployment"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 11: PRIVACY & SURVEILLANCE
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "privacy_surveillance",
        "label": "Privacy & Surveillance",
        "layer": 11,
        "topics": [
            {
                "name": "mass_surveillance",
                "prompt": "Generate atomic facts about mass surveillance programs and history. Cover: NSA programs revealed by Snowden (PRISM — collecting data from tech companies, XKeyscore — searching intercepted communications, Upstream — tapping fiber optic cables), Five Eyes alliance (US, UK, Canada, Australia, New Zealand), GCHQ Tempora, room 641A (AT&T/NSA), Stingray/IMSI catchers (fake cell towers), facial recognition surveillance, CCTV networks, social credit systems.",
                "count": 55,
                "associations": ["surveillance", "NSA", "PRISM", "Snowden", "privacy"]
            },
            {
                "name": "tracking_technologies",
                "prompt": "Generate atomic facts about tracking technologies. Cover: cookies (first-party vs third-party, tracking across sites), browser fingerprinting (canvas fingerprint, WebGL, audio context, fonts, screen resolution — unique combination identifies users), device fingerprinting, tracking pixels (1x1 invisible images that phone home), supercookies, ETag tracking, link decoration (adding tracking parameters to URLs), cross-device tracking, ad networks, data brokers.",
                "count": 55,
                "associations": ["tracking", "fingerprinting", "cookies", "privacy"]
            },
            {
                "name": "privacy_laws_and_rights",
                "prompt": "Generate atomic facts about privacy laws and digital rights. Cover: GDPR (EU, right to erasure, consent requirements, data portability, 72-hour breach notification, DPO, fines up to 4% global revenue), CCPA/CPRA (California, right to know, right to delete, right to opt-out of sale), HIPAA (US health data), FERPA (US education data), Fourth Amendment (US, unreasonable search and seizure), Electronic Communications Privacy Act, warrant requirements for data, data localization laws.",
                "count": 50,
                "associations": ["GDPR", "CCPA", "privacy law", "data protection", "rights"]
            },
            {
                "name": "anonymity_and_tools",
                "prompt": "Generate atomic facts about anonymity tools and techniques. Cover: Tor Browser (routes through 3+ relays, prevents ISP seeing destination), VPN limitations for anonymity (VPN provider can see traffic), encrypted email (ProtonMail, PGP), encrypted messaging (Signal protocol, end-to-end encryption), Tails OS (amnesic live system, routes everything through Tor), secure deletion, metadata stripping from files, cryptocurrency privacy (Bitcoin is pseudonymous not anonymous, Monero ring signatures).",
                "count": 50,
                "associations": ["anonymity", "Tor", "encryption", "privacy tools"]
            },
        ]
    },

    # ════════════════════════════════════════════════════════════════════════
    # LAYER 12: GENERAL KNOWLEDGE
    # ════════════════════════════════════════════════════════════════════════
    {
        "name": "general_knowledge",
        "label": "General Knowledge",
        "layer": 12,
        "topics": [
            {
                "name": "world_geography",
                "prompt": "Generate atomic facts about world geography. Cover: continents (7) and their largest countries, oceans (5), major mountain ranges (Himalayas, Andes, Rockies, Alps), major rivers (Nile, Amazon, Mississippi, Yangtze, Danube), most populous countries, capital cities of major nations, geographic features (deserts: Sahara, Gobi; rainforests: Amazon; plains, tundra, islands).",
                "count": 60,
                "associations": ["geography", "world", "continent", "country"]
            },
            {
                "name": "major_historical_events",
                "prompt": "Generate atomic facts about major historical events. Cover: ancient civilizations (Mesopotamia, Egypt, Greece, Rome), the Renaissance, the Scientific Revolution, the Industrial Revolution, French Revolution (1789), American Revolution (1776), World War I (1914-1918, causes, key events), World War II (1939-1945, Holocaust, atomic bombs), Cold War, fall of the Berlin Wall (1989), invention of the internet, key dates and what happened.",
                "count": 60,
                "associations": ["history", "world history", "civilization", "war"]
            },
            {
                "name": "scientific_method",
                "prompt": "Generate atomic facts about the scientific method and scientific thinking. Cover: observation, hypothesis formation, experimentation, data collection, analysis, conclusion, peer review, reproducibility, control groups, independent/dependent variables, placebo effect, double-blind studies, correlation vs causation, falsifiability (Karl Popper), paradigm shifts (Thomas Kuhn), scientific consensus, Occam's razor.",
                "count": 50,
                "associations": ["scientific method", "science", "experiment", "hypothesis"]
            },
            {
                "name": "economics_fundamentals",
                "prompt": "Generate atomic facts about economics fundamentals. Cover: supply and demand, equilibrium price, scarcity, opportunity cost, marginal utility, GDP (Gross Domestic Product), inflation (general increase in prices, measured by CPI), deflation, interest rates, monetary policy (central banks, Federal Reserve), fiscal policy (government spending, taxation), market types (perfect competition, monopoly, oligopoly), comparative advantage, trade.",
                "count": 55,
                "associations": ["economics", "supply and demand", "GDP", "inflation"]
            },
            {
                "name": "philosophy_of_mind",
                "prompt": "Generate atomic facts about philosophy of mind and consciousness. Cover: the hard problem of consciousness (David Chalmers — why is there subjective experience?), dualism (Descartes — mind and body are separate), physicalism (mind is physical), functionalism (mental states defined by their function), Chinese Room argument (John Searle — syntax isn't semantics), Turing test, philosophical zombies, qualia, free will debate (determinism, compatibilism, libertarianism), emergence, intentionality.",
                "count": 50,
                "associations": ["philosophy of mind", "consciousness", "AI philosophy", "thought"]
            },
            {
                "name": "biology_basics",
                "prompt": "Generate atomic facts about biology fundamentals. Cover: cells (prokaryotic vs eukaryotic, organelles: nucleus, mitochondria, ribosomes, cell membrane), DNA (double helix, base pairs A-T G-C, genes encode proteins), RNA (mRNA, tRNA, transcription and translation), evolution (natural selection, mutation, adaptation, speciation), photosynthesis (plants convert CO2 + water + light into glucose + oxygen), cellular respiration (glucose + oxygen -> ATP + CO2 + water), taxonomy (domain, kingdom, phylum, class, order, family, genus, species).",
                "count": 55,
                "associations": ["biology", "cell", "DNA", "evolution", "science"]
            },
            {
                "name": "chemistry_basics",
                "prompt": "Generate atomic facts about chemistry. Cover: elements and the periodic table (groups, periods, metals/nonmetals/metalloids), chemical bonds (ionic: electron transfer, covalent: electron sharing, hydrogen bonds), chemical reactions (reactants -> products, balancing equations), acids and bases (pH scale, 0-14, acid < 7 < base), moles and Avogadro's number (6.022 x 10^23), states of matter, gas laws (Boyle's, Charles's, ideal gas PV=nRT), oxidation and reduction (electron transfer), common elements and their properties.",
                "count": 55,
                "associations": ["chemistry", "element", "reaction", "periodic table", "science"]
            },
            {
                "name": "critical_thinking",
                "prompt": "Generate atomic facts about critical thinking and logical reasoning. Cover: logical fallacies (ad hominem, strawman, false dichotomy, slippery slope, appeal to authority, appeal to emotion, circular reasoning, hasty generalization, red herring, tu quoque, no true Scotsman, bandwagon), cognitive biases (confirmation bias, anchoring, availability heuristic, Dunning-Kruger effect, sunk cost fallacy, hindsight bias), argument structure (premise, conclusion, valid vs sound).",
                "count": 55,
                "associations": ["critical thinking", "logic", "fallacy", "bias", "reasoning"]
            },
            {
                "name": "units_measurements",
                "prompt": "Generate atomic facts about units and measurements. Cover: SI base units (meter, kilogram, second, ampere, kelvin, mole, candela), metric prefixes (kilo, mega, giga, tera, milli, micro, nano, pico), unit conversions, speed of light (3 x 10^8 m/s), absolute zero (0 K = -273.15 C), Celsius vs Fahrenheit vs Kelvin, bits and bytes (8 bits = 1 byte), KB/MB/GB/TB (powers of 1000 vs 1024), bandwidth vs latency, common physical constants.",
                "count": 50,
                "associations": ["units", "measurement", "SI", "metric", "science"]
            },
        ]
    },
]


# --- Fact Counting ----------------------------------------------------------

def count_curriculum_targets() -> dict:
    """Count target facts per domain and total."""
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
    print(f"  LU AI CURRICULUM — DRY RUN")
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
            print(f"  Existing curriculum_data.json: {existing_total} facts")
            for d in data.get("domains", []):
                print(f"    {d.get('label', d.get('name'))}: {d.get('total_facts', 0)} facts")
        except Exception:
            pass


# --- CLI --------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="Lu AI Curriculum Training — generate knowledge facts using Ollama",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python train_curriculum.py --dry-run                     Show target counts
  python train_curriculum.py --generate                    Generate all domains
  python train_curriculum.py --generate --domain networking  Generate one domain
  python train_curriculum.py --generate --resume            Skip already-done domains
  python train_curriculum.py --generate --model mistral     Use a different model

Domains: """ + ", ".join(d["name"] for d in CURRICULUM)
    )

    parser.add_argument("--generate", action="store_true",
                        help="Generate facts using Ollama and save to curriculum_data.json")
    parser.add_argument("--dry-run", action="store_true",
                        help="Just show target fact counts per domain")
    parser.add_argument("--domain", type=str, default=None,
                        help="Generate only this domain (e.g. 'networking', 'cryptography')")
    parser.add_argument("--model", type=str, default=DEFAULT_MODEL,
                        help=f"Ollama model to use (default: {DEFAULT_MODEL})")
    parser.add_argument("--resume", action="store_true",
                        help="Skip domains already present in curriculum_data.json")

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
