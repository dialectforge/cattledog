// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Web Fetch Engine
//!
//! Lu's eyes on the internet. Uses reqwest for HTTP and scraper for
//! HTML parsing. Extracts clean text, links (with anchor text),
//! and image context (alt text, captions, surrounding text).
//!
//! Lu doesn't download images — it understands what they depict
//! from their alt text and surrounding context, creating knowledge
//! nodes about the visual content.
//!
//! ARCHITECTURE: This module is pure I/O. It fetches and parses.
//! The research module (web_research.rs) decides WHAT to fetch.
//! The safety module (web_safety.rs) decides IF it's safe to fetch.
//!
//! Created by John Dean Martin

use crate::web_safety::{WebSafety, MAX_PAGE_SIZE};
use scraper::{Html, Selector};
use url::Url;
use std::time::Duration;
use std::collections::HashSet;
use std::io::Write;

const WEB_DEBUG_LOG: &str = r"brain\web_debug.log";

fn web_log(msg: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true).append(true)
        .open(WEB_DEBUG_LOG)
    {
        let _ = writeln!(f, "{}", msg);
    }
}

/// A fully parsed web page — everything Lu can learn from it.
#[derive(Debug, Clone)]
pub struct ParsedPage {
    pub url: String,
    pub title: String,
    pub text_content: Vec<String>,    // Clean text paragraphs
    pub headings: Vec<PageHeading>,   // H1-H6 hierarchy
    pub links: Vec<PageLink>,         // Outbound links with context
    pub images: Vec<PageImage>,       // Image context (no binary data)
    pub meta_description: String,     // <meta name="description">
    pub fetch_time_ms: u64,
}

/// A heading extracted from the page.
#[derive(Debug, Clone)]
pub struct PageHeading {
    pub level: u8,        // 1-6
    pub text: String,
}

/// A link extracted from the page with context.
#[derive(Debug, Clone)]
pub struct PageLink {
    pub url: String,
    pub anchor_text: String,       // The clickable text
    pub surrounding_text: String,  // Text around the link for context
    pub is_internal: bool,         // Same domain?
}

/// Image context — Lu doesn't download the image, just understands it.
#[derive(Debug, Clone)]
pub struct PageImage {
    pub src_url: String,
    pub alt_text: String,          // What the image shows
    pub caption: String,           // <figcaption> or nearby text
    pub surrounding_text: String,  // Context around the image
}

/// A search result from DuckDuckGo.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// The Web Fetch Engine.
pub struct WebFetcher {
    client: reqwest::blocking::Client,
    safety: WebSafety,
}

impl WebFetcher {
    pub fn new(safety: WebSafety) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) Gecko/20100101 Firefox/125.0")
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, safety }
    }

    /// Search DuckDuckGo using the Lite HTML endpoint.
    ///
    /// DDG Lite (lite.duckduckgo.com/lite/) is a minimal text-only page
    /// designed for low-bandwidth browsers. It returns real search results
    /// and has minimal bot-detection, unlike the full HTML or JSON API
    /// endpoints which either block bots or only handle instant-answer
    /// lookups (not research queries).
    pub fn search(&self, query: &str, max_results: usize) -> Result<Vec<SearchResult>, String> {
        // DDG Lite requires a POST request with form data — GET returns the homepage.
        let search_url = "https://lite.duckduckgo.com/lite/";

        web_log(&format!("[search] query={:?} url={}", query, search_url));

        let response = self.client.post(search_url)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "en-US,en;q=0.5")
            .form(&[("q", query)])
            .send()
            .map_err(|e| {
                web_log(&format!("[search] HTTP error: {}", e));
                format!("Search request failed: {}", e)
            })?;

        let status = response.status();
        web_log(&format!("[search] status={}", status));

        if !status.is_success() {
            web_log(&format!("[search] HTTP_ERROR status={}", status));
            return Err(format!("Search returned status {}", status));
        }

        let body = response.text()
            .map_err(|e| format!("Failed to read search response: {}", e))?;

        web_log(&format!("[search] body_len={} first_500={:?}",
            body.len(),
            body.chars().take(500).collect::<String>()));

        let results = Self::parse_ddg_lite_results(&body, max_results);
        web_log(&format!("[search] parsed {} results", results.len()));
        Ok(results)
    }

    /// Parse DuckDuckGo Lite HTML into SearchResults.
    ///
    /// DDG Lite structure:
    ///   <a class="result-link" href="/l/?uddg=ENCODED_URL&...">Title</a>
    ///   <td class="result-snippet">Snippet text</td>
    ///
    /// Results and snippets are in parallel order, so we zip them by index.
    fn parse_ddg_lite_results(html: &str, max_results: usize) -> Vec<SearchResult> {
        let document = Html::parse_document(html);
        let mut results = Vec::new();

        let link_sel    = Selector::parse("a.result-link").unwrap();
        let snippet_sel = Selector::parse("td.result-snippet").unwrap();

        let links:   Vec<_> = document.select(&link_sel).collect();
        let snippets: Vec<_> = document.select(&snippet_sel).collect();

        web_log(&format!("[search] ddg_lite links={} snippets={}", links.len(), snippets.len()));

        for (i, link_elem) in links.iter().enumerate() {
            if results.len() >= max_results { break; }

            let title = link_elem.text().collect::<String>().trim().to_string();
            let href  = link_elem.value().attr("href").unwrap_or("").trim();

            if title.is_empty() || href.is_empty() { continue; }

            // DDG lite redirects through /l/?uddg=PERCENT_ENCODED_URL
            let url = if let Some(pos) = href.find("uddg=") {
                let encoded = href[pos + 5..].split('&').next().unwrap_or("");
                urlencoding::decode(encoded)
                    .map(|s| s.into_owned())
                    .unwrap_or_else(|_| href.to_string())
            } else if href.starts_with("http") {
                href.to_string()
            } else {
                format!("https://lite.duckduckgo.com{}", href)
            };

            if url.is_empty() { continue; }

            let snippet = snippets.get(i)
                .map(|s| s.text().collect::<String>()
                    .split_whitespace().collect::<Vec<_>>().join(" "))
                .unwrap_or_default();

            web_log(&format!("[search]   result[{}] title={:?} url={:?}", i, title, url));
            results.push(SearchResult { title, url, snippet });
        }

        results
    }

    /// Fetch and parse a web page. Safety-checked before fetching.
    pub fn fetch_page(&self, url: &str) -> Result<ParsedPage, String> {
        let start = std::time::Instant::now();

        // Safety check first
        let verdict = self.safety.check_url(url);
        if !verdict.safe {
            return Err(format!("BLOCKED: {}", verdict.reason));
        }

        // Fetch
        let response = self.client.get(url)
            .send()
            .map_err(|e| {
                web_log(&format!("[fetch] REQUEST_ERROR url={} error={}", url, e));
                format!("Fetch failed: {}", e)
            })?;

        // Check content type
        if let Some(ct) = response.headers().get("content-type") {
            let ct_str = ct.to_str().unwrap_or("");
            if !self.safety.check_content_type(ct_str) {
                web_log(&format!("[fetch] BAD_CONTENT_TYPE url={} ct={}", url, ct_str));
                return Err(format!("Unsafe content-type: {}", ct_str));
            }
        }

        let status = response.status();
        if !status.is_success() {
            web_log(&format!("[fetch] HTTP_ERROR url={} status={}", url, status));
            return Err(format!("HTTP {}", status));
        }

        let body = response.text()
            .map_err(|e| format!("Failed to read body: {}", e))?;

        web_log(&format!("[fetch] url={} body_bytes={} first_100={:?}",
            url, body.len(), body.chars().take(100).collect::<String>()));

        if body.len() > MAX_PAGE_SIZE {
            return Err(format!("Page too large: {} bytes (max {})", body.len(), MAX_PAGE_SIZE));
        }

        let fetch_time_ms = start.elapsed().as_millis() as u64;

        // Parse
        let base_url = Url::parse(url).ok();
        Ok(Self::parse_html(&body, url, base_url.as_ref(), fetch_time_ms))
    }

    /// Parse HTML into a structured ParsedPage.
    fn parse_html(html: &str, page_url: &str, base_url: Option<&Url>, fetch_time_ms: u64) -> ParsedPage {
        let document = Html::parse_document(html);

        // Title
        let title_sel = Selector::parse("title").unwrap();
        let title = document.select(&title_sel).next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        // Meta description
        let meta_sel = Selector::parse("meta[name=description]").unwrap();
        let meta_description = document.select(&meta_sel).next()
            .and_then(|e| e.value().attr("content"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        // Headings (H1-H6)
        let headings = Self::extract_headings(&document);

        // Text content (paragraphs, list items, blockquotes)
        let text_content = Self::extract_text(&document);

        // Links
        let links = Self::extract_links(&document, base_url);

        // Images
        let images = Self::extract_images(&document, base_url);

        ParsedPage {
            url: page_url.to_string(),
            title,
            text_content,
            headings,
            links,
            images,
            meta_description,
            fetch_time_ms,
        }
    }

    /// Extract heading hierarchy from the page.
    fn extract_headings(document: &Html) -> Vec<PageHeading> {
        let mut headings = Vec::new();
        for level in 1..=6u8 {
            let sel_str = format!("h{}", level);
            if let Ok(sel) = Selector::parse(&sel_str) {
                for elem in document.select(&sel) {
                    let text = elem.text().collect::<String>().trim().to_string();
                    if !text.is_empty() && text.len() < 200 {
                        headings.push(PageHeading { level, text });
                    }
                }
            }
        }
        headings
    }

    /// Extract clean text paragraphs from the page.
    /// Skips nav, footer, sidebar, script, style elements.
    fn extract_text(document: &Html) -> Vec<String> {
        let mut texts = Vec::new();
        let mut seen = HashSet::new();

        // Ancestor-exclusion selectors: skip content inside nav/header/footer/etc.
        let skip_ancestor_strs = ["nav", "header", "footer", ".navbox", ".sidebar", "#toc"];
        let skip_ancestors: Vec<Selector> = skip_ancestor_strs.iter()
            .filter_map(|s| Selector::parse(s).ok())
            .collect();

        // Leaf-level content selectors only (no broad containers like "article").
        let content_selectors = ["p", "li", "blockquote", "td"];

        let mut raw_count = 0usize;
        let mut skip_short = 0usize;
        let mut skip_long = 0usize;
        let mut skip_nav_ancestor = 0usize;
        let mut skip_boilerplate = 0usize;
        let mut skip_dupe = 0usize;

        for sel_str in &content_selectors {
            let Ok(sel) = Selector::parse(sel_str) else { continue };

            // --- DIAGNOSTIC: count raw hits before ANY filtering ---
            let all_elems: Vec<_> = document.select(&sel).collect();
            web_log(&format!("[extract] selector={:?} found {} elements", sel_str, all_elems.len()));
            for (i, elem) in all_elems.iter().enumerate().take(3) {
                let raw_text: String = elem.text().collect::<String>()
                    .split_whitespace().collect::<Vec<_>>().join(" ");
                let preview: String = raw_text.chars().take(200).collect();
                web_log(&format!("[extract]   elem[{}] len={} text={:?}", i, raw_text.len(), preview));
            }

            for elem in all_elems {
                raw_count += 1;

                let text: String = elem.text().collect::<String>()
                    .split_whitespace().collect::<Vec<_>>().join(" ");
                let trimmed = text.trim().to_string();

                if trimmed.len() < 20 { skip_short += 1; continue; }
                if trimmed.len() > 5000 { skip_long += 1; continue; }

                // Skip elements inside nav/header/footer/etc. by walking ancestors.
                let in_skip_zone = {
                    let mut found = false;
                    let mut node = elem.parent();
                    'outer: while let Some(n) = node {
                        if let Some(parent_ref) = scraper::ElementRef::wrap(n) {
                            let tag = parent_ref.value().name();
                            if tag == "html" || tag == "body" { break; }
                            for anc_sel in &skip_ancestors {
                                // A matching ancestor = this element is inside a skip zone.
                                // We compare by checking if parent_ref matches the selector
                                // by re-selecting from document and checking node identity.
                                if document.select(anc_sel).any(|e| std::ptr::eq(
                                    e.value() as *const _,
                                    parent_ref.value() as *const _,
                                )) {
                                    found = true;
                                    break 'outer;
                                }
                            }
                        }
                        node = n.parent();
                    }
                    found
                };
                if in_skip_zone { skip_nav_ancestor += 1; continue; }

                let lower = trimmed.to_lowercase();
                if lower.starts_with("cookie") || lower.starts_with("privacy policy")
                    || lower.starts_with("terms of") || lower.contains("accept cookies")
                    || lower.starts_with("copyright ©")
                {
                    skip_boilerplate += 1;
                    continue;
                }

                if seen.contains(&trimmed) { skip_dupe += 1; continue; }

                seen.insert(trimmed.clone());
                texts.push(trimmed);
            }
        }

        web_log(&format!(
            "[extract] DONE raw={} short={} long={} nav_ancestor={} boilerplate={} dupe={} => kept={}",
            raw_count, skip_short, skip_long, skip_nav_ancestor, skip_boilerplate, skip_dupe, texts.len()
        ));
        // Log first 3 kept blocks so we can confirm content quality
        for (i, t) in texts.iter().enumerate().take(3) {
            web_log(&format!("[extract]   kept[{}] len={} text={:?}", i, t.len(),
                t.chars().take(200).collect::<String>()));
        }

        texts
    }

    /// Extract links with anchor text and context.
    fn extract_links(document: &Html, base_url: Option<&Url>) -> Vec<PageLink> {
        let mut links = Vec::new();
        let sel = Selector::parse("a[href]").unwrap();
        let mut seen_urls = HashSet::new();

        for elem in document.select(&sel) {
            if links.len() >= crate::web_safety::MAX_LINKS_PER_PAGE { break; }

            let href = match elem.value().attr("href") {
                Some(h) => h,
                None => continue,
            };

            // Resolve relative URLs
            let resolved = if href.starts_with("http") {
                href.to_string()
            } else if let Some(base) = base_url {
                base.join(href).map(|u| u.to_string()).unwrap_or_default()
            } else {
                continue;
            };

            if resolved.is_empty() || seen_urls.contains(&resolved) { continue; }
            // Skip anchor-only links, javascript:, mailto:
            if resolved.starts_with('#') || resolved.starts_with("javascript:")
                || resolved.starts_with("mailto:") { continue; }

            seen_urls.insert(resolved.clone());

            let anchor_text = elem.text().collect::<String>().trim().to_string();
            if anchor_text.is_empty() || anchor_text.len() > 200 { continue; }

            // Get surrounding context (parent text minus the link text)
            let surrounding = elem.parent()
                .and_then(|p| {
                    scraper::ElementRef::wrap(p).map(|parent| {
                        parent.text().collect::<String>()
                            .split_whitespace().collect::<Vec<_>>().join(" ")
                    })
                })
                .unwrap_or_default();

            let is_internal = if let Some(base) = base_url {
                Url::parse(&resolved).ok()
                    .and_then(|u| u.host_str().map(|h| h == base.host_str().unwrap_or("")))
                    .unwrap_or(false)
            } else {
                false
            };

            links.push(PageLink {
                url: resolved,
                anchor_text,
                surrounding_text: surrounding.chars().take(300).collect(),
                is_internal,
            });
        }
        links
    }

    /// Extract image context — alt text, captions, surrounding text.
    /// Lu doesn't download images, just understands what they depict.
    fn extract_images(document: &Html, base_url: Option<&Url>) -> Vec<PageImage> {
        let mut images = Vec::new();
        let sel = Selector::parse("img").unwrap();
        let figcaption_sel = Selector::parse("figcaption").unwrap();

        for elem in document.select(&sel) {
            if images.len() >= 20 { break; } // Cap images

            let src = elem.value().attr("src").unwrap_or("");
            let alt = elem.value().attr("alt").unwrap_or("").trim().to_string();

            // Skip tiny tracking pixels and icons
            if alt.is_empty() && src.contains("pixel") { continue; }
            if alt.is_empty() && src.contains("icon") { continue; }

            // Resolve src URL
            let src_url = if src.starts_with("http") {
                src.to_string()
            } else if let Some(base) = base_url {
                base.join(src).map(|u| u.to_string()).unwrap_or_default()
            } else {
                src.to_string()
            };

            // Look for figcaption in parent <figure>
            let caption = elem.parent()
                .and_then(|p| scraper::ElementRef::wrap(p))
                .map(|parent| {
                    parent.select(&figcaption_sel).next()
                        .map(|fc| fc.text().collect::<String>().trim().to_string())
                        .unwrap_or_default()
                })
                .unwrap_or_default();

            // Get surrounding text for context
            let surrounding = elem.parent()
                .and_then(|p| scraper::ElementRef::wrap(p))
                .map(|parent| {
                    parent.text().collect::<String>()
                        .split_whitespace().collect::<Vec<_>>().join(" ")
                })
                .unwrap_or_default();

            // Only keep images that have SOME context
            if alt.is_empty() && caption.is_empty() && surrounding.len() < 20 {
                continue;
            }

            images.push(PageImage {
                src_url,
                alt_text: alt,
                caption,
                surrounding_text: surrounding.chars().take(300).collect(),
            });
        }
        images
    }

    /// Get a reference to the safety engine.
    pub fn safety(&self) -> &WebSafety {
        &self.safety
    }

    /// Get a mutable reference to the safety engine.
    pub fn safety_mut(&mut self) -> &mut WebSafety {
        &mut self.safety
    }
}
