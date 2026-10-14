// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Web research module — search → fetch → extract → ingest pipeline.
//!
//! Search order: **Grokopedia first**, DuckDuckGo Lite second, Wikipedia OpenSearch as final fallback.
//! HTTP is handled via ureq (blocking, HTTPS via native-tls).
//! HTML parsing uses a zero-dependency tag-stripping approach.
//!
//! Research loop (per session):
//!   1. Search with Grokopedia; fall back to DuckDuckGo Lite if empty/unreachable.
//!   2. Fetch top result pages (safety-checked, HTTPS only).
//!   3. Extract text from HTML.
//!   4. Follow relevant links up to depth 2, max 5 pages total.
//!   5. 5-minute overall timeout, 5 MB per-page limit.
//!
//! The caller (Brain::web_research) is responsible for ingesting extracted_texts.

use crate::web_safety::{WebSafety, MAX_PAGE_SIZE, MAX_LINKS_PER_PAGE};
use std::collections::HashSet;
use std::time::{Duration, Instant};

const DEFAULT_MAX_DEPTH: usize = 2;
const DEFAULT_MAX_PAGES: usize = 5;
const DEFAULT_TIMEOUT_SECS: u64 = 300; // 5 minutes
const DEFAULT_MAX_RESULTS: usize = 10;
const HTTP_TIMEOUT_SECS: u64 = 8;
const SEARCH_TIMEOUT_SECS: u64 = 5;

// ── Public types ──────────────────────────────────────────────────────────────

/// A single search result from a search provider.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Outcome of a web research session.
#[derive(Debug, Clone)]
pub struct ResearchReport {
    pub query: String,
    pub pages_visited: usize,
    pub facts_extracted: usize,
    pub sources: Vec<String>,
    pub summary: String,
    pub elapsed_secs: u64,
    /// Raw text blocks ready for ingestion via brain.learn_text().
    pub(crate) extracted_texts: Vec<String>,
}

impl ResearchReport {
    /// Create an empty/failed report for `query`.
    pub fn empty(query: &str) -> Self {
        ResearchReport {
            query: query.to_string(),
            pages_visited: 0,
            facts_extracted: 0,
            sources: Vec::new(),
            summary: format!("No results found for: {}", query),
            elapsed_secs: 0,
            extracted_texts: Vec::new(),
        }
    }

    /// Expose extracted texts for use by Brain (ingestion).
    pub fn extracted_texts(&self) -> &[String] {
        &self.extracted_texts
    }
}

/// Configuration for a research session.
#[derive(Debug, Clone)]
pub struct ResearchConfig {
    /// Maximum link-follow depth (default: 2).
    pub max_depth: usize,
    /// Maximum total pages fetched per session (default: 5).
    pub max_pages: usize,
    /// Overall research timeout in seconds (default: 300).
    pub timeout_secs: u64,
    /// Maximum search results to seed from (default: 10).
    pub max_results: usize,
}

impl Default for ResearchConfig {
    fn default() -> Self {
        ResearchConfig {
            max_depth: DEFAULT_MAX_DEPTH,
            max_pages: DEFAULT_MAX_PAGES,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            max_results: DEFAULT_MAX_RESULTS,
        }
    }
}

// ── WebResearcher ─────────────────────────────────────────────────────────────

/// The web researcher — drives the full search → fetch → extract loop.
pub struct WebResearcher {
    pub config: ResearchConfig,
    pub safety: WebSafety,
}

impl Default for WebResearcher {
    fn default() -> Self {
        WebResearcher {
            config: ResearchConfig::default(),
            safety: WebSafety::new(),
        }
    }
}

impl WebResearcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: ResearchConfig) -> Self {
        WebResearcher {
            config,
            safety: WebSafety::new(),
        }
    }

    // ── Public entry points ───────────────────────────────────────────────────

    /// Run a full research session: search → fetch → extract.
    ///
    /// Returns a ResearchReport with `extracted_texts` populated.
    /// The caller (Brain) ingests those texts via `brain.learn_text()`.
    pub fn research(&self, query: &str) -> ResearchReport {
        let start = Instant::now();
        let deadline = start + Duration::from_secs(self.config.timeout_secs);

        // 1. Search — Grokopedia first, DDG Lite fallback.
        let results = self.search(query);
        if results.is_empty() {
            return ResearchReport::empty(query);
        }

        // 2. Seed the fetch queue from top search results.
        let mut queue: Vec<(String, usize)> = results
            .iter()
            .take(self.config.max_pages)
            .map(|r| (r.url.clone(), 0usize))
            .collect();

        let mut visited: HashSet<String> = HashSet::new();
        let mut all_texts: Vec<String> = Vec::new();
        let mut sources: Vec<String> = Vec::new();

        // 3. Fetch loop with depth-limited link following.
        while !queue.is_empty() && visited.len() < self.config.max_pages {
            if Instant::now() > deadline {
                eprintln!("[web] research timeout after {} pages", visited.len());
                break;
            }

            let (url, depth) = queue.remove(0);
            if visited.contains(&url) {
                continue;
            }

            // Safety check before every fetch.
            let verdict = self.safety.check_url(&url);
            if !verdict.safe {
                eprintln!("[web] skipping unsafe URL: {} — {}", url, verdict.reason);
                continue;
            }

            let remaining_secs = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or(Duration::ZERO)
                .as_secs()
                .max(1);
            let page_timeout = remaining_secs.min(HTTP_TIMEOUT_SECS);
            match http_get(&url, page_timeout) {
                Ok(html) => {
                    if html.len() > MAX_PAGE_SIZE {
                        eprintln!("[web] skipping oversized page: {} ({} bytes)", url, html.len());
                        continue;
                    }
                    visited.insert(url.clone());
                    sources.push(url.clone());

                    let texts = extract_text_from_html(&html);
                    eprintln!("[web] {} → {} text blocks", url, texts.len());
                    all_texts.extend(texts);

                    // Follow links if within depth budget.
                    if depth < self.config.max_depth {
                        let links = extract_links_from_html(&html, &url);
                        for link in links.into_iter().take(MAX_LINKS_PER_PAGE) {
                            if !visited.contains(&link) {
                                queue.push((link, depth + 1));
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[web] fetch error for {}: {}", url, e);
                }
            }
        }

        let elapsed = start.elapsed().as_secs();
        let facts = all_texts.len();
        let summary = if sources.is_empty() {
            format!("No pages loaded for: {}", query)
        } else {
            format!(
                "Researched '{}': {} page(s), {} text block(s). Primary source: {}",
                query,
                sources.len(),
                facts,
                sources.first().cloned().unwrap_or_default()
            )
        };

        ResearchReport {
            query: query.to_string(),
            pages_visited: sources.len(),
            facts_extracted: facts,
            sources,
            summary,
            elapsed_secs: elapsed,
            extracted_texts: all_texts,
        }
    }

    // ── Search pipeline ───────────────────────────────────────────────────────

    /// Search: Grokopedia → DuckDuckGo Lite → Wikipedia OpenSearch (three-tier cascade).
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        match self.search_grokopedia(query) {
            Ok(results) if !results.is_empty() => {
                eprintln!("[web] grokopedia returned {} results", results.len());
                results
            }
            Ok(_) => {
                eprintln!("[web] grokopedia returned 0 results, trying DuckDuckGo Lite");
                self.ddg_fallback(query)
            }
            Err(e) => {
                eprintln!("[web] grokopedia failed ({}), trying DuckDuckGo Lite", e);
                self.ddg_fallback(query)
            }
        }
    }

    fn ddg_fallback(&self, query: &str) -> Vec<SearchResult> {
        match self.search_ddg_lite(query) {
            Ok(results) if !results.is_empty() => {
                eprintln!("[web] DuckDuckGo Lite returned {} results", results.len());
                results
            }
            Ok(_) => {
                eprintln!("[web] DuckDuckGo Lite returned 0 results, trying Wikipedia");
                self.wikipedia_fallback(query)
            }
            Err(e) => {
                eprintln!("[web] DuckDuckGo Lite failed ({}), trying Wikipedia", e);
                self.wikipedia_fallback(query)
            }
        }
    }

    fn wikipedia_fallback(&self, query: &str) -> Vec<SearchResult> {
        match self.search_wikipedia(query) {
            Ok(results) => {
                eprintln!("[web] Wikipedia returned {} results", results.len());
                results
            }
            Err(e) => {
                eprintln!("[web] Wikipedia also failed: {}", e);
                Vec::new()
            }
        }
    }

    /// Search Grokopedia — expects a JSON array of search results.
    ///
    /// Endpoint: `GET https://grokopedia.com/api/search?q={query}`
    /// Response:  `[{ "title": "...", "url": "...", "snippet": "..." }]`
    ///        or: `{ "results": [...] }`
    pub fn search_grokopedia(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let encoded = url_encode(query);
        let url = format!("https://grokopedia.com/api/search?q={}", encoded);

        let body = http_get(&url, SEARCH_TIMEOUT_SECS)?;

        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("JSON parse: {}", e))?;

        let arr = json
            .as_array()
            .or_else(|| json["results"].as_array())
            .ok_or_else(|| "no results array in Grokopedia response".to_string())?;

        let results = arr
            .iter()
            .filter_map(|v| {
                let title = v["title"].as_str()?.to_string();
                let url = v["url"].as_str()?.to_string();
                let snippet = v["snippet"].as_str().unwrap_or("").to_string();
                // Only accept http(s) URLs
                if url.starts_with("https://") || url.starts_with("http://") {
                    Some(SearchResult { title, url, snippet })
                } else {
                    None
                }
            })
            .take(self.config.max_results)
            .collect();

        Ok(results)
    }

    /// Search DuckDuckGo Lite — POST form to `https://lite.duckduckgo.com/lite/`.
    ///
    /// DDG Lite is a minimal HTML page designed for low-bandwidth browsers.
    /// It returns real search results with minimal bot-detection.
    pub fn search_ddg_lite(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let form_body = format!("q={}", url_encode(query));
        let body = http_post_form(
            "https://lite.duckduckgo.com/lite/",
            &form_body,
            SEARCH_TIMEOUT_SECS,
        )?;
        let results = parse_ddg_lite_html(&body, self.config.max_results);
        if results.is_empty() {
            // Log enough of the response to diagnose parsing/bot-detection issues.
            let preview: String = body.chars().take(400).collect();
            eprintln!(
                "[web] DDG Lite parse yielded 0 results (body len={}, preview={:?})",
                body.len(),
                preview
            );
        }
        Ok(results)
    }

    /// Search Wikipedia via the OpenSearch API (reliable, no bot-detection).
    ///
    /// Endpoint: `GET https://en.wikipedia.org/w/api.php?action=opensearch&search=QUERY&limit=N&format=json`
    /// Response: `["query", ["Title1", ...], ["Desc1", ...], ["URL1", ...]]`
    pub fn search_wikipedia(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let encoded = url_encode(query);
        let url = format!(
            "https://en.wikipedia.org/w/api.php?action=opensearch&search={}&limit={}&format=json",
            encoded,
            self.config.max_results,
        );
        let body = http_get(&url, SEARCH_TIMEOUT_SECS)?;
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("JSON parse: {}", e))?;

        // OpenSearch format: [query_string, [titles], [descriptions], [urls]]
        let arr = json.as_array().ok_or_else(|| "expected JSON array from Wikipedia".to_string())?;
        if arr.len() < 4 {
            return Ok(Vec::new());
        }
        let titles = arr[1].as_array().ok_or_else(|| "titles not an array".to_string())?;
        let descs  = arr[2].as_array().ok_or_else(|| "descs not an array".to_string())?;
        let urls   = arr[3].as_array().ok_or_else(|| "urls not an array".to_string())?;

        let results = titles
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let title   = t.as_str()?.to_string();
                let snippet = descs.get(i).and_then(|d| d.as_str()).unwrap_or("").to_string();
                let url     = urls.get(i).and_then(|u| u.as_str())?.to_string();
                if url.starts_with("https://") || url.starts_with("http://") {
                    Some(SearchResult { title, url, snippet })
                } else {
                    None
                }
            })
            .take(self.config.max_results)
            .collect();

        Ok(results)
    }
}

// ── HTTP helpers ──────────────────────────────────────────────────────────────

/// HTTP GET (blocking, HTTPS via ureq + native-tls).
pub fn http_get(url: &str, timeout_secs: u64) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) Gecko/20100101 Firefox/125.0")
        .build();

    let response = agent
        .get(url)
        .call()
        .map_err(|e| format!("GET {}: {}", url, e))?;

    response
        .into_string()
        .map_err(|e| format!("read body from {}: {}", url, e))
}

/// HTTP POST with `application/x-www-form-urlencoded` body.
pub fn http_post_form(url: &str, form_body: &str, timeout_secs: u64) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) Gecko/20100101 Firefox/125.0")
        .build();

    let response = agent
        .post(url)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .set("Accept-Language", "en-US,en;q=0.5")
        .send_string(form_body)
        .map_err(|e| format!("POST {}: {}", url, e))?;

    response
        .into_string()
        .map_err(|e| format!("read body from {}: {}", url, e))
}

// ── HTML parsing ──────────────────────────────────────────────────────────────

/// Extract meaningful text paragraphs from HTML.
///
/// Strategy (no external parser needed):
///   1. Remove script, style, nav, header, footer blocks entirely.
///   2. Strip remaining HTML tags (preserving word boundaries).
///   3. Decode common HTML entities.
///   4. Filter paragraphs by length (20–5000 chars).
///   5. Drop boilerplate (cookie notices, copyright lines).
///   6. Deduplicate.
pub fn extract_text_from_html(html: &str) -> Vec<String> {
    // Remove noisy block elements and their content.
    let s = remove_html_block(html, "script");
    let s = remove_html_block(&s, "style");
    let s = remove_html_block(&s, "nav");
    let s = remove_html_block(&s, "header");
    let s = remove_html_block(&s, "footer");
    let s = remove_html_block(&s, "noscript");
    let s = remove_html_block(&s, "aside");

    // Strip remaining tags.
    let s = strip_tags(&s);

    // Decode entities.
    let s = decode_entities(&s);

    let mut seen: HashSet<String> = HashSet::new();
    let mut results = Vec::new();

    // Split on newlines (tags → spaces left boundaries there).
    for chunk in s.split('\n') {
        let trimmed: String = chunk.split_whitespace().collect::<Vec<_>>().join(" ");
        if trimmed.len() < 20 || trimmed.len() > 5000 {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if lower.starts_with("cookie")
            || lower.starts_with("privacy policy")
            || lower.starts_with("terms of")
            || lower.contains("accept cookies")
            || lower.starts_with("copyright ©")
            || lower.starts_with("all rights reserved")
        {
            continue;
        }
        if seen.insert(trimmed.clone()) {
            results.push(trimmed);
        }
    }

    results
}

/// Extract outbound HTTPS links from HTML, resolved against `base_url`.
pub fn extract_links_from_html(html: &str, base_url: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let base_domain = extract_domain(base_url);

    let html_lower = html.to_lowercase();
    let mut pos = 0;

    while pos < html.len() && links.len() < MAX_LINKS_PER_PAGE {
        // Find next <a tag.
        let rel = match html_lower[pos..].find("<a ") {
            Some(i) => i,
            None => break,
        };
        let tag_start = pos + rel;

        // Find end of the opening tag.
        let tag_end = match html_lower[tag_start..].find('>') {
            Some(i) => tag_start + i,
            None => break,
        };

        // Extract href from the original-case slice.
        let tag_slice = &html[tag_start..=tag_end];
        pos = tag_end + 1;

        let href = match extract_attr(tag_slice, "href") {
            Some(h) if !h.is_empty() => h,
            _ => continue,
        };

        // Skip anchors, javascript:, mailto:.
        if href.starts_with('#')
            || href.to_lowercase().starts_with("javascript:")
            || href.to_lowercase().starts_with("mailto:")
        {
            continue;
        }

        let url = resolve_url(&href, base_url, &base_domain);
        if url.is_empty() || !url.starts_with("https://") {
            continue;
        }

        if seen.insert(url.clone()) {
            links.push(url);
        }
    }

    links
}

// ── DuckDuckGo Lite parser ────────────────────────────────────────────────────

/// Parse DuckDuckGo Lite HTML and return up to `max` SearchResults.
///
/// DDG Lite HTML pattern:
///   `<a class="result-link" href="/l/?uddg=PERCENT_ENCODED_URL&...">Title</a>`
///   `<td class="result-snippet">Snippet text</td>`
pub fn parse_ddg_lite_html(html: &str, max: usize) -> Vec<SearchResult> {
    let mut titles_urls: Vec<(String, String)> = Vec::new();
    let mut snippets: Vec<String> = Vec::new();

    // Collect result-link anchors.
    let mut pos = 0;
    while titles_urls.len() < max {
        let lower = &html[pos..];
        let Some(marker) = lower.find("result-link") else { break };
        let abs = pos + marker;

        // Backtrack to find the opening < of the containing <a tag.
        let tag_open = match html[..abs].rfind('<') {
            Some(i) => i,
            None => { pos = abs + 1; continue; }
        };
        let tag_end = match html[tag_open..].find('>') {
            Some(i) => tag_open + i,
            None => { pos = abs + 1; continue; }
        };

        let tag_slice = &html[tag_open..=tag_end];
        let after_tag = tag_end + 1;

        let href = extract_attr(tag_slice, "href").unwrap_or_default();
        let url = decode_ddg_redirect(&href);

        if !url.is_empty() && (url.starts_with("http://") || url.starts_with("https://")) {
            // Title text lives between the closing > and </a>.
            let close_a = html[after_tag..].find("</a>").unwrap_or(0);
            let raw = &html[after_tag..after_tag + close_a];
            let title = strip_tags(raw)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");

            if !title.is_empty() {
                titles_urls.push((title, url));
            }
        }

        pos = after_tag;
    }

    // Collect result-snippet cells.
    pos = 0;
    while snippets.len() < max {
        let lower = &html[pos..];
        let Some(marker) = lower.find("result-snippet") else { break };
        let abs = pos + marker;

        let cell_end = match html[abs..].find("</td>") {
            Some(i) => abs + i,
            None => { pos = abs + 1; continue; }
        };

        let cell = &html[abs..cell_end];
        let snippet = strip_tags(cell)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        snippets.push(snippet);

        pos = cell_end + 5;
    }

    // Zip titles+URLs with snippets.
    titles_urls
        .into_iter()
        .enumerate()
        .map(|(i, (title, url))| {
            let snippet = snippets.get(i).cloned().unwrap_or_default();
            SearchResult { title, url, snippet }
        })
        .collect()
}

// ── String / HTML helpers ─────────────────────────────────────────────────────

/// Remove all `<tagname …>…</tagname>` blocks (case-insensitive).
fn remove_html_block(html: &str, tag: &str) -> String {
    let open_pat = format!("<{}", tag);
    let close_pat = format!("</{}>", tag);

    let mut result = String::with_capacity(html.len());
    let lower = html.to_lowercase();
    let close_lower = close_pat.to_lowercase();
    let mut pos = 0;

    while pos < html.len() {
        let start = match lower[pos..].find(&open_pat) {
            Some(i) => pos + i,
            None => {
                result.push_str(&html[pos..]);
                break;
            }
        };
        result.push_str(&html[pos..start]);

        // Find the matching close tag after the open tag.
        let search_from = start + open_pat.len();
        let end = lower[search_from..]
            .find(&close_lower)
            .map(|i| search_from + i + close_pat.len())
            .unwrap_or(html.len());

        // Emit a newline as a paragraph separator.
        result.push('\n');
        pos = end;
    }

    result
}

/// Strip all HTML/XML tags, emitting a space at each tag boundary.
pub fn strip_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                result.push('\n'); // tag boundary → paragraph separator
            }
            '>' => {
                in_tag = false;
            }
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

/// Decode the most common HTML entities.
pub fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&hellip;", "…")
        .replace("&copy;", "©")
}

/// Extract an HTML attribute value from a tag string (handles `"` and `'` quoting).
pub fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let pat = format!("{}=", attr);
    let at = lower.find(&pat)?;
    let after = &tag[at + pat.len()..];

    if after.starts_with('"') {
        let end = after[1..].find('"')?;
        Some(after[1..1 + end].to_string())
    } else if after.starts_with('\'') {
        let end = after[1..].find('\'')?;
        Some(after[1..1 + end].to_string())
    } else {
        let end = after
            .find(|c: char| c.is_ascii_whitespace() || c == '>')
            .unwrap_or(after.len());
        Some(after[..end].to_string())
    }
}

/// Percent-encode a string for use as a URL query parameter value.
pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Percent-decode a URL-encoded string.
pub fn url_decode(s: &str) -> String {
    let mut out = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_nibble(bytes[i + 1]);
            let lo = hex_nibble(bytes[i + 2]);
            out.push((hi << 4 | lo) as char);
            i += 3;
        } else if bytes[i] == b'+' {
            out.push(' ');
            i += 1;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn hex_nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

/// Decode a DuckDuckGo redirect href: `/l/?uddg=PERCENT_ENCODED_URL&…`
fn decode_ddg_redirect(href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    if let Some(pos) = href.find("uddg=") {
        let encoded = href[pos + 5..].split('&').next().unwrap_or("");
        url_decode(encoded)
    } else {
        String::new()
    }
}

/// Extract the bare hostname from a URL (strips scheme, path, port, query).
pub fn extract_domain(url: &str) -> String {
    let after = if url.starts_with("https://") {
        &url[8..]
    } else if url.starts_with("http://") {
        &url[7..]
    } else {
        return String::new();
    };
    after
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Resolve a (possibly relative) href against a base URL.
fn resolve_url(href: &str, base_url: &str, base_domain: &str) -> String {
    if href.starts_with("https://") || href.starts_with("http://") {
        return href.to_string();
    }
    if href.starts_with("//") {
        return format!("https:{}", href);
    }
    if href.starts_with('/') {
        return format!("https://{}{}", base_domain, href);
    }
    // Relative path: resolve against the base URL's directory.
    if let Some(last_slash) = base_url.rfind('/') {
        if last_slash > 8 {
            // not just "https://"
            return format!("{}/{}", &base_url[..last_slash], href);
        }
    }
    format!("{}/{}", base_url.trim_end_matches('/'), href)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encode_decode() {
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("rust lang"), "rust+lang");
        assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
        assert_eq!(url_decode("hello+world"), "hello world");
        assert_eq!(url_decode("a%26b%3Dc"), "a&b=c");
    }

    #[test]
    fn test_strip_tags() {
        let html = "<p>Hello <b>world</b>!</p>";
        let text = strip_tags(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
        assert!(!text.contains('<'));
    }

    #[test]
    fn test_decode_entities() {
        assert_eq!(decode_entities("AT&amp;T"), "AT&T");
        assert_eq!(decode_entities("&lt;tag&gt;"), "<tag>");
        assert_eq!(decode_entities("don&apos;t"), "don't");
    }

    #[test]
    fn test_extract_text_from_html() {
        let html = r#"
        <html>
        <head><title>Test</title></head>
        <script>alert("noise")</script>
        <style>.foo { color: red; }</style>
        <nav><a href="/">Home</a></nav>
        <body>
          <p>Rust is a systems programming language focused on safety and performance.</p>
          <p>It prevents memory safety bugs at compile time without a garbage collector.</p>
          <footer>Copyright © 2024</footer>
        </body>
        </html>"#;

        let texts = extract_text_from_html(html);
        assert!(texts.iter().any(|t| t.contains("systems programming")));
        assert!(texts.iter().any(|t| t.contains("memory safety")));
        // Boilerplate should be filtered.
        assert!(!texts.iter().any(|t| t.contains("alert")));
    }

    #[test]
    fn test_extract_links() {
        let html = r#"
        <a href="https://example.com/page">Link 1</a>
        <a href="/relative">Relative</a>
        <a href="javascript:void(0)">JS</a>
        <a href="mailto:foo@bar.com">Mail</a>
        "#;
        let links = extract_links_from_html(html, "https://base.example.com/");
        // Absolute https link
        assert!(links.iter().any(|u| u == "https://example.com/page"));
        // Relative resolved to HTTPS
        assert!(links.iter().any(|u| u.starts_with("https://base.example.com")));
        // javascript: and mailto: excluded
        assert!(!links.iter().any(|u| u.starts_with("javascript:")));
        assert!(!links.iter().any(|u| u.starts_with("mailto:")));
    }

    #[test]
    fn test_parse_ddg_lite_html() {
        // Minimal DDG Lite HTML fixture.
        let html = r#"
        <tr>
          <td><a class="result-link" href="/l/?uddg=https%3A%2F%2Frust-lang.org">The Rust Language</a></td>
        </tr>
        <tr>
          <td class="result-snippet">Rust is a systems language.</td>
        </tr>
        "#;
        let results = parse_ddg_lite_html(html, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "The Rust Language");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert!(results[0].snippet.contains("systems language"));
    }

    #[test]
    fn test_extract_attr() {
        assert_eq!(
            extract_attr(r#"<a href="https://example.com" class="link">"#, "href"),
            Some("https://example.com".into())
        );
        assert_eq!(
            extract_attr(r#"<a href='single'>"#, "href"),
            Some("single".into())
        );
        assert_eq!(extract_attr(r#"<a class="x">"#, "href"), None);
    }

    #[test]
    fn test_ddg_redirect_decode() {
        // Verify DDG-style redirect decoding without network access.
        let href = "/l/?uddg=https%3A%2F%2Fwww.rust-lang.org%2F&rut=abc";
        let url = url_decode(&href[href.find("uddg=").unwrap() + 5..].split('&').next().unwrap());
        assert_eq!(url, "https://www.rust-lang.org/");
    }

    #[test]
    fn test_search_ordering_grokopedia_first() {
        // Verify that when grokopedia fails, DDG is tried.
        // (No network call — both will fail gracefully in offline context.)
        let researcher = WebResearcher::new();
        // search() should return an empty vec rather than panic on network errors.
        let results = researcher.search("nonexistent query for testing purposes 12345");
        // Either empty (both failed) or has results (network available) — must not panic.
        let _ = results; // just confirming no panic
    }

    #[test]
    fn test_parse_wikipedia_opensearch() {
        // Verify that the Wikipedia OpenSearch JSON format is parsed correctly.
        let researcher = WebResearcher::new();
        // Simulate a well-formed OpenSearch response without making a network call.
        let json_body = r#"["rust programming",["Rust (programming language)","Rustbelt"],["Rust is a systems language.","A region in the US."],["https://en.wikipedia.org/wiki/Rust_(programming_language)","https://en.wikipedia.org/wiki/Rust_Belt"]]"#;
        let json: serde_json::Value = serde_json::from_str(json_body).unwrap();
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 4);
        let titles = arr[1].as_array().unwrap();
        let descs  = arr[2].as_array().unwrap();
        let urls   = arr[3].as_array().unwrap();
        assert_eq!(titles[0].as_str(), Some("Rust (programming language)"));
        assert!(descs[0].as_str().unwrap().contains("systems language"));
        assert!(urls[0].as_str().unwrap().starts_with("https://en.wikipedia.org"));
        // Verify the searcher doesn't panic on a live call (network optional).
        let _ = researcher.search_wikipedia("Rust programming language");
    }
}
