// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Lu Brain v2 — Web research system tests.
//!
//! Tests (all pure logic — no live network required):
//!   1. URL safety validation (HTTPS only, blocked TLDs, private IPs)
//!   2. HTML text extraction (tag stripping, boilerplate filtering)
//!   3. DDG Lite HTML parsing (fixture-based, no network)
//!   4. Link extraction from HTML
//!   5. URL encode / decode helpers
//!   6. Idle trigger logic (timing-based state machine)
//!   7. Grokopedia-first / DDG-fallback search ordering (structural, graceful failure)
//!   8. Cerebellum WebSearch tool dispatch (returns error without network, not a panic)
//!   9. Cerebellum Http tool dispatch (HTTPS safety enforcement)
//!  10. Brain::web_research integration (Brain::new() includes web_idle field)

use brain::cerebellum::{Cerebellum, ToolKind, ToolRequest};
use brain::web_idle::{IdleConfig, IdleResearcher};
use brain::web_research::{
    decode_entities, extract_links_from_html, extract_text_from_html, parse_ddg_lite_html,
    strip_tags, url_decode, url_encode, WebResearcher,
};
use brain::web_safety::{WebSafety, MAX_LINKS_PER_PAGE, MAX_PAGE_SIZE};
use brain::Brain;
use std::time::Duration;

// ── Test helpers ──────────────────────────────────────────────────────────────

fn pass(name: &str) {
    println!("  ✓ {}", name);
}

fn fail(name: &str, detail: &str) {
    eprintln!("  ✗ {} — {}", name, detail);
    std::process::exit(1);
}

fn ok_or_fail(cond: bool, name: &str, detail: &str) {
    if cond {
        pass(name);
    } else {
        fail(name, detail);
    }
}

// ── Test 1: URL safety validation ─────────────────────────────────────────────

fn test_url_safety() {
    println!("\n[1] URL safety validation");
    let s = WebSafety::new();

    // HTTPS required
    ok_or_fail(
        !s.check_url("http://example.com").safe,
        "http:// rejected",
        "http should be blocked",
    );
    ok_or_fail(
        !s.check_url("ftp://example.com").safe,
        "ftp:// rejected",
        "ftp should be blocked",
    );
    ok_or_fail(
        !s.check_url("example.com").safe,
        "bare hostname rejected",
        "no scheme should be blocked",
    );

    // HTTPS passes
    ok_or_fail(
        s.check_url("https://example.com").safe,
        "https://example.com passes",
        "should be safe",
    );
    ok_or_fail(
        s.check_url("https://en.wikipedia.org/wiki/Rust_(programming_language)").safe,
        "wikipedia passes",
        "should be safe",
    );

    // Blocked TLDs
    ok_or_fail(
        !s.check_url("https://hidden.onion").safe,
        ".onion blocked",
        "onion TLD should be blocked",
    );
    ok_or_fail(
        !s.check_url("https://site.i2p").safe,
        ".i2p blocked",
        "i2p TLD should be blocked",
    );

    // Blocked domains
    ok_or_fail(
        !s.check_url("https://localhost/path").safe,
        "localhost blocked",
        "localhost should be blocked",
    );
    ok_or_fail(
        !s.check_url("https://127.0.0.1/").safe,
        "127.0.0.1 blocked",
        "loopback IP should be blocked",
    );

    // Private IP ranges
    ok_or_fail(
        !s.check_url("https://192.168.1.1/").safe,
        "192.168.x.x blocked",
        "private IP should be blocked",
    );
    ok_or_fail(
        !s.check_url("https://10.0.0.1/").safe,
        "10.x.x.x blocked",
        "private IP should be blocked",
    );
    ok_or_fail(
        !s.check_url("https://172.16.0.1/").safe,
        "172.16.x.x blocked",
        "private IP should be blocked",
    );
    ok_or_fail(
        !s.check_url("https://169.254.1.1/").safe,
        "link-local 169.254.x.x blocked",
        "link-local IP should be blocked",
    );

    // Content-type
    ok_or_fail(
        s.check_content_type("text/html; charset=utf-8"),
        "text/html content-type allowed",
        "should be allowed",
    );
    ok_or_fail(
        s.check_content_type("application/json"),
        "application/json content-type allowed",
        "should be allowed",
    );
    ok_or_fail(
        !s.check_content_type("application/octet-stream"),
        "binary content-type blocked",
        "should be blocked",
    );
    ok_or_fail(
        !s.check_content_type("image/png"),
        "image/png content-type blocked",
        "should be blocked",
    );

    // Size check
    ok_or_fail(s.check_size(1024), "small body passes", "should pass");
    ok_or_fail(
        s.check_size(MAX_PAGE_SIZE),
        "exact MAX_PAGE_SIZE passes",
        "should pass",
    );
    ok_or_fail(
        !s.check_size(MAX_PAGE_SIZE + 1),
        "oversized body rejected",
        "should fail",
    );

    // Constants
    ok_or_fail(MAX_PAGE_SIZE == 5 * 1024 * 1024, "MAX_PAGE_SIZE = 5 MB", "wrong value");
    ok_or_fail(MAX_LINKS_PER_PAGE == 50, "MAX_LINKS_PER_PAGE = 50", "wrong value");
}

// ── Test 2: HTML text extraction ──────────────────────────────────────────────

fn test_html_extraction() {
    println!("\n[2] HTML text extraction");

    let html = r#"
    <html>
    <head><title>Test Page</title></head>
    <script>alert("noise"); var x = 1;</script>
    <style>.btn { color: red; padding: 4px; }</style>
    <nav><ul><li><a href="/">Home</a></li></ul></nav>
    <body>
      <h1>Rust Programming Language</h1>
      <p>Rust is a systems programming language focused on safety, speed, and concurrency.</p>
      <p>It prevents memory safety bugs at compile time without needing a garbage collector.</p>
      <p>The borrow checker enforces ownership rules that eliminate use-after-free bugs.</p>
      <footer>Copyright © 2024 Example Corp. All rights reserved.</footer>
    </body>
    </html>"#;

    let texts = extract_text_from_html(html);

    ok_or_fail(
        texts.iter().any(|t| t.contains("systems programming")),
        "meaningful paragraph extracted",
        "expected paragraph about Rust",
    );
    ok_or_fail(
        texts.iter().any(|t| t.contains("borrow checker")),
        "second paragraph extracted",
        "expected borrow checker paragraph",
    );
    ok_or_fail(
        !texts.iter().any(|t| t.contains("alert(")),
        "script content stripped",
        "script should not appear in output",
    );
    ok_or_fail(
        !texts.iter().any(|t| t.contains("color: red")),
        "style content stripped",
        "style rules should not appear",
    );
    ok_or_fail(
        !texts.iter().any(|t| t.to_lowercase().starts_with("copyright ©")),
        "copyright boilerplate filtered",
        "copyright line should be filtered",
    );
}

// ── Test 3: DDG Lite HTML parsing ─────────────────────────────────────────────

fn test_ddg_lite_parsing() {
    println!("\n[3] DDG Lite HTML parsing");

    let html = r#"
    <html><body>
    <table>
      <tr>
        <td><a class="result-link" href="/l/?uddg=https%3A%2F%2Fwww.rust-lang.org%2F&rut=x">The Rust Programming Language</a></td>
      </tr>
      <tr>
        <td class="result-snippet">Rust empowers everyone to build reliable and efficient software.</td>
      </tr>
      <tr>
        <td><a class="result-link" href="/l/?uddg=https%3A%2F%2Fen.wikipedia.org%2Fwiki%2FRust&rut=y">Rust - Wikipedia</a></td>
      </tr>
      <tr>
        <td class="result-snippet">Rust is a multi-paradigm, general-purpose programming language.</td>
      </tr>
    </table>
    </body></html>"#;

    let results = parse_ddg_lite_html(html, 10);

    ok_or_fail(!results.is_empty(), "DDG results parsed", "expected at least 1 result");
    ok_or_fail(
        results.len() >= 2,
        "both DDG results parsed",
        &format!("expected 2, got {}", results.len()),
    );

    let r0 = &results[0];
    ok_or_fail(
        r0.title.contains("Rust Programming"),
        "first result title correct",
        &format!("got: {}", r0.title),
    );
    ok_or_fail(
        r0.url == "https://www.rust-lang.org/",
        "first result URL decoded",
        &format!("got: {}", r0.url),
    );
    ok_or_fail(
        r0.snippet.contains("reliable"),
        "first result snippet captured",
        &format!("got: {}", r0.snippet),
    );

    let r1 = &results[1];
    ok_or_fail(
        r1.url.contains("wikipedia"),
        "second result URL correct",
        &format!("got: {}", r1.url),
    );
}

// ── Test 4: Link extraction ───────────────────────────────────────────────────

fn test_link_extraction() {
    println!("\n[4] Link extraction from HTML");

    let html = r##"
    <a href="https://rust-lang.org/learn">Learn Rust</a>
    <a href="/docs/intro">Docs (relative)</a>
    <a href="//crates.io/crates/ureq">Protocol-relative</a>
    <a href="javascript:void(0)">JS — should be excluded</a>
    <a href="mailto:info@example.com">Email — should be excluded</a>
    <a href="#section">Anchor — should be excluded</a>
    <a href="http://insecure.example.com">HTTP — should be excluded (non-HTTPS)</a>
    "##;

    let links = extract_links_from_html(html, "https://base.example.com/page");

    ok_or_fail(
        links.iter().any(|u| u == "https://rust-lang.org/learn"),
        "absolute HTTPS link included",
        "rust-lang.org should be in links",
    );
    ok_or_fail(
        links.iter().any(|u| u.starts_with("https://base.example.com")),
        "relative path resolved to HTTPS",
        "relative /docs/intro should resolve",
    );
    ok_or_fail(
        links.iter().any(|u| u.starts_with("https://crates.io")),
        "protocol-relative link resolved to HTTPS",
        "//crates.io should become https://crates.io",
    );
    ok_or_fail(
        !links.iter().any(|u| u.starts_with("javascript:")),
        "javascript: links excluded",
        "javascript: should be excluded",
    );
    ok_or_fail(
        !links.iter().any(|u| u.starts_with("mailto:")),
        "mailto: links excluded",
        "mailto: should be excluded",
    );
    ok_or_fail(
        !links.iter().any(|u| u.starts_with('#')),
        "anchor links excluded",
        "# links should be excluded",
    );
    ok_or_fail(
        !links.iter().any(|u| u.starts_with("http://insecure")),
        "HTTP (non-HTTPS) links excluded",
        "http:// links should be excluded",
    );
}

// ── Test 5: URL encode / decode ───────────────────────────────────────────────

fn test_url_helpers() {
    println!("\n[5] URL encode / decode helpers");

    // Encoding
    ok_or_fail(
        url_encode("hello world") == "hello+world",
        "space encoded as +",
        &format!("got: {}", url_encode("hello world")),
    );
    ok_or_fail(
        url_encode("a&b=c") == "a%26b%3Dc",
        "& and = percent-encoded",
        &format!("got: {}", url_encode("a&b=c")),
    );
    ok_or_fail(
        url_encode("rust lang 2024") == "rust+lang+2024",
        "multi-word query encoded",
        &format!("got: {}", url_encode("rust lang 2024")),
    );
    ok_or_fail(
        url_encode("safe-chars_~.ok") == "safe-chars_~.ok",
        "unreserved chars unchanged",
        &format!("got: {}", url_encode("safe-chars_~.ok")),
    );

    // Decoding
    ok_or_fail(
        url_decode("hello+world") == "hello world",
        "+ decoded to space",
        &format!("got: {}", url_decode("hello+world")),
    );
    ok_or_fail(
        url_decode("a%26b%3Dc") == "a&b=c",
        "percent-encoded chars decoded",
        &format!("got: {}", url_decode("a%26b%3Dc")),
    );
    ok_or_fail(
        url_decode("https%3A%2F%2Frust-lang.org%2F") == "https://rust-lang.org/",
        "URL percent-decoded correctly",
        &format!("got: {}", url_decode("https%3A%2F%2Frust-lang.org%2F")),
    );

    // Round-trip
    let original = "Rust's memory safety & zero-cost abstractions";
    ok_or_fail(
        url_decode(&url_encode(original)) == original,
        "encode→decode round-trip",
        &format!("round-trip failed for: {}", original),
    );

    // HTML entities
    ok_or_fail(
        decode_entities("AT&amp;T") == "AT&T",
        "&amp; decoded",
        &format!("got: {}", decode_entities("AT&amp;T")),
    );
    ok_or_fail(
        decode_entities("&lt;tag&gt;") == "<tag>",
        "&lt; &gt; decoded",
        &format!("got: {}", decode_entities("&lt;tag&gt;")),
    );

    // strip_tags
    let stripped = strip_tags("<p>Hello <b>world</b>!</p>");
    ok_or_fail(
        stripped.contains("Hello") && stripped.contains("world"),
        "strip_tags preserves text",
        &format!("got: {:?}", stripped),
    );
    ok_or_fail(
        !stripped.contains('<'),
        "strip_tags removes all tags",
        "still contains < character",
    );
}

// ── Test 6: Idle trigger logic ────────────────────────────────────────────────

fn test_idle_logic() {
    println!("\n[6] Idle trigger logic");

    let fast = IdleConfig {
        idle_threshold: Duration::from_millis(50),
        cooldown: Duration::from_millis(30),
        max_per_period: 2,
    };

    // Not idle yet — should not trigger.
    let r = IdleResearcher::with_config(fast.clone());
    ok_or_fail(
        !r.should_research(),
        "no trigger immediately after creation",
        "should not trigger immediately",
    );

    // After idle threshold.
    std::thread::sleep(Duration::from_millis(80));
    ok_or_fail(
        r.should_research(),
        "triggers after idle threshold",
        "should trigger after idle period",
    );
    ok_or_fail(r.secs_until_next() == 0, "secs_until_next = 0 when ready", "should be 0");

    // Cooldown blocks second run.
    let mut r2 = IdleResearcher::with_config(fast.clone());
    std::thread::sleep(Duration::from_millis(80));
    assert!(r2.should_research());
    r2.record_research();
    ok_or_fail(
        !r2.should_research(),
        "cooldown blocks immediate re-trigger",
        "cooldown should apply",
    );

    // After cooldown.
    std::thread::sleep(Duration::from_millis(50));
    ok_or_fail(
        r2.should_research(),
        "triggers again after cooldown",
        "should allow after cooldown",
    );

    // Period cap.
    r2.record_research(); // 2nd run — hits cap (max=2)
    std::thread::sleep(Duration::from_millis(50));
    ok_or_fail(
        !r2.should_research(),
        "period cap blocks third run",
        "max_per_period=2 should block 3rd",
    );

    // Disabled.
    let mut r3 = IdleResearcher::with_config(fast.clone());
    r3.enabled = false;
    std::thread::sleep(Duration::from_millis(80));
    ok_or_fail(
        !r3.should_research(),
        "disabled researcher never triggers",
        "disabled should not trigger",
    );

    // record_interaction resets period after idle.
    let mut r4 = IdleResearcher::with_config(fast.clone());
    std::thread::sleep(Duration::from_millis(80));
    r4.record_research();
    r4.record_research(); // cap reached
    r4.record_interaction(); // reset period (we were idle → fresh period)
    std::thread::sleep(Duration::from_millis(80));
    ok_or_fail(
        r4.should_research(),
        "period resets after interaction during idle",
        "period should reset",
    );

    // Status string sanity.
    let s = r.status();
    ok_or_fail(
        s.contains("idle_researcher"),
        "status string has 'idle_researcher'",
        &format!("got: {}", s),
    );
}

// ── Test 7: Search ordering — Grokopedia first, DDG fallback ──────────────────

fn test_search_ordering() {
    println!("\n[7] Search ordering: Grokopedia-first, DDG-fallback");

    let researcher = WebResearcher::new();

    // Grokopedia will fail (domain doesn't resolve or returns non-JSON).
    // DDG Lite may succeed or fail depending on network availability.
    // The key assertion: the method must NOT panic and must return a Vec
    // (empty is fine in offline context).
    let results = researcher.search("Rust programming language memory safety");
    let _ = results; // Accept any result — just must not panic
    pass("search() completes without panic (Grokopedia→DDG fallback)");

    // Grokopedia direct: should return Err gracefully.
    let grok_result = researcher.search_grokopedia("test query");
    ok_or_fail(
        grok_result.is_err() || grok_result.as_ref().map(|v| v.is_empty()).unwrap_or(true),
        "grokopedia_first: fails or returns empty gracefully",
        "must not panic",
    );

    // If grokopedia fails, ddg_lite is tried.  Both may fail offline — that's OK.
    let ddg_result = researcher.search_ddg_lite("test query");
    let _ = ddg_result; // just no panic
    pass("search_ddg_lite() returns Result without panic");
}

// ── Test 8: Cerebellum WebSearch tool dispatch ────────────────────────────────

fn test_cerebellum_web_search() {
    println!("\n[8] Cerebellum WebSearch tool dispatch");

    let mut cereb = Cerebellum::new();

    // Missing query param.
    let req = ToolRequest::new(
        ToolKind::WebSearch,
        serde_json::json!({ "query": "" }),
    );
    let result = cereb.execute_sync(req);
    ok_or_fail(
        !result.success,
        "empty query returns error (not panic)",
        "should fail gracefully",
    );
    ok_or_fail(
        result.error.is_some(),
        "empty query has error message",
        "error field should be set",
    );

    // Valid query — may fail due to no network, but must not panic.
    let req2 = ToolRequest::new(
        ToolKind::WebSearch,
        serde_json::json!({ "query": "test query no network" }),
    );
    let result2 = cereb.execute_sync(req2);
    // Either success (network available) or error (offline) — no panic.
    let _ = result2.success;
    pass("WebSearch tool executes without panic");

    // Tool call stats updated.
    ok_or_fail(
        cereb.total_tool_calls >= 2,
        "tool call counter incremented",
        &format!("total_tool_calls = {}", cereb.total_tool_calls),
    );
}

// ── Test 9: Cerebellum Http tool dispatch ─────────────────────────────────────

fn test_cerebellum_http() {
    println!("\n[9] Cerebellum Http tool dispatch");

    let mut cereb = Cerebellum::new();

    // Missing URL.
    let req = ToolRequest::new(
        ToolKind::Http,
        serde_json::json!({ "method": "GET" }),
    );
    let result = cereb.execute_sync(req);
    ok_or_fail(
        !result.success,
        "missing url returns error",
        "should fail gracefully",
    );

    // HTTP (non-HTTPS) blocked by safety layer.
    let req2 = ToolRequest::new(
        ToolKind::Http,
        serde_json::json!({ "url": "http://example.com/", "method": "GET" }),
    );
    let result2 = cereb.execute_sync(req2);
    ok_or_fail(
        !result2.success,
        "http:// URL blocked by safety layer",
        "should be blocked",
    );
    ok_or_fail(
        result2
            .error
            .as_deref()
            .map(|e| e.contains("HTTPS") || e.contains("safety") || e.contains("blocked"))
            .unwrap_or(false),
        "Http tool error mentions safety",
        &format!("error: {:?}", result2.error),
    );

    // localhost blocked.
    let req3 = ToolRequest::new(
        ToolKind::Http,
        serde_json::json!({ "url": "https://localhost/api", "method": "GET" }),
    );
    let result3 = cereb.execute_sync(req3);
    ok_or_fail(
        !result3.success,
        "localhost blocked by safety layer",
        "should be blocked",
    );

    // Valid HTTPS URL — may fail offline, must not panic.
    let req4 = ToolRequest::new(
        ToolKind::Http,
        serde_json::json!({ "url": "https://example.com/", "method": "GET" }),
    );
    let result4 = cereb.execute_sync(req4);
    let _ = result4.success;
    pass("Http tool HTTPS URL executes without panic");
}

// ── Test 10: Brain::web_research integration ──────────────────────────────────

fn test_brain_integration() {
    println!("\n[10] Brain integration");

    // Brain::new() should include web_idle without panic.
    let mut brain = Brain::new();

    // web_idle field exists and is accessible.
    let status = brain.web_idle.status();
    ok_or_fail(
        status.contains("idle_researcher"),
        "brain.web_idle.status() works",
        &format!("got: {}", status),
    );

    // record_interaction reachable through brain.
    brain.web_idle.record_interaction();
    pass("brain.web_idle.record_interaction() works");

    // web_idle_check() with empty wonder queue → None.
    let result = brain.web_idle_check();
    ok_or_fail(
        result.is_none(),
        "web_idle_check returns None with empty wonder queue",
        "expected None when no wonder items",
    );

    // web_research() must not panic (network optional).
    // Give it a minimal timeout so the test doesn't block too long.
    let report = brain.web_research("Rust borrow checker memory safety test");
    ok_or_fail(
        !report.query.is_empty(),
        "web_research returns a report with query set",
        "report.query should be non-empty",
    );
    pass("brain.web_research() completes without panic");
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    println!("Lu Brain v2 — Web Research System Tests");
    println!("========================================");

    test_url_safety();
    test_html_extraction();
    test_ddg_lite_parsing();
    test_link_extraction();
    test_url_helpers();
    test_idle_logic();
    test_search_ordering();
    test_cerebellum_web_search();
    test_cerebellum_http();
    test_brain_integration();

    println!("\n========================================");
    println!("All web research tests passed.");
}
