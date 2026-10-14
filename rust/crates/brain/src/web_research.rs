// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Web Research Engine
//!
//! Lu's research brain. This is where Lu THINKS while browsing.
//! Not just fetching pages — evaluating, deciding, following links,
//! building understanding, and knowing when to stop.
//!
//! THE RESEARCH LOOP:
//! 1. Query Formation — turn a knowledge gap into smart search keywords
//! 2. Search — find candidate pages via DuckDuckGo
//! 3. Evaluate — score each result BEFORE visiting (title/snippet relevance)
//! 4. Read — fetch and parse the best page
//! 5. Think — did this fill the gap? What links look promising?
//! 6. Follow or Stop — follow a promising link, refine search, or stop
//! 7. Ingest — feed everything learned through the ingest pipeline
//!
//! DEPTH CONTROL:
//! - Max pages per session: configurable (default 5)
//! - Max link-follow depth: 2 clicks from search results
//! - Max time: 5 minutes per research session
//! - Lu stops early if the gap is filled
//!
//! Created by John Dean Martin

use crate::web_fetch::{WebFetcher, ParsedPage, SearchResult, PageLink};
use crate::web_safety;
use std::time::Instant;
use std::collections::HashSet;

/// A complete research session result.
#[derive(Debug, Clone)]
pub struct ResearchResult {
    pub query: String,
    pub pages_visited: Vec<VisitedPage>,
    pub knowledge_extracted: Vec<ExtractedKnowledge>,
    pub images_found: Vec<ImageKnowledge>,
    pub gap_filled: bool,
    pub summary: String,
    pub total_time_ms: u64,
}

/// A page Lu visited during research.
#[derive(Debug, Clone)]
pub struct VisitedPage {
    pub url: String,
    pub title: String,
    pub relevance_score: f32,
    pub paragraphs_extracted: usize,
    pub links_found: usize,
    pub depth: u32,  // 0 = from search, 1 = one click deep, etc.
}

/// A piece of knowledge extracted from web research.
#[derive(Debug, Clone)]
pub struct ExtractedKnowledge {
    pub content: String,
    pub source_url: String,
    pub confidence: f32,  // How relevant this fact is to the query
}

/// An image Lu understood from context (not downloaded).
#[derive(Debug, Clone)]
pub struct ImageKnowledge {
    pub image_url: String,
    pub description: String,  // What Lu thinks the image shows
    pub source_url: String,
    pub relevance: f32,
}

/// Research configuration.
#[derive(Debug, Clone)]
pub struct ResearchConfig {
    pub max_pages: u32,
    pub max_depth: u32,
    pub max_time_secs: u64,
    pub max_results_per_search: usize,
    pub min_relevance: f32,  // Minimum relevance to bother reading a page
}

impl Default for ResearchConfig {
    fn default() -> Self {
        Self {
            max_pages: web_safety::MAX_PAGES_PER_SESSION,
            max_depth: web_safety::MAX_FOLLOW_DEPTH,
            max_time_secs: web_safety::MAX_SESSION_SECONDS,
            max_results_per_search: 8,
            min_relevance: 0.2,
        }
    }
}

/// The Web Research Engine — Lu's research brain.
pub struct WebResearcher {
    fetcher: WebFetcher,
    config: ResearchConfig,
}

impl WebResearcher {
    pub fn new(fetcher: WebFetcher) -> Self {
        Self {
            fetcher,
            config: ResearchConfig::default(),
        }
    }

    pub fn with_config(fetcher: WebFetcher, config: ResearchConfig) -> Self {
        Self { fetcher, config }
    }

    /// The main research loop. Give Lu a question and it will
    /// search, browse, think, and learn.
    pub fn research(&self, query: &str) -> ResearchResult {
        let start = Instant::now();
        let mut visited: Vec<VisitedPage> = Vec::new();
        let mut knowledge: Vec<ExtractedKnowledge> = Vec::new();
        let mut images: Vec<ImageKnowledge> = Vec::new();
        let mut visited_urls: HashSet<String> = HashSet::new();
        let mut gap_filled = false;

        // Phase 1: Query formation — turn the gap into search keywords
        let search_query = self.form_search_query(query);

        // Phase 2: Search
        let search_results = match self.fetcher.search(&search_query, self.config.max_results_per_search) {
            Ok(results) => results,
            Err(e) => {
                return ResearchResult {
                    query: query.to_string(),
                    pages_visited: visited,
                    knowledge_extracted: knowledge,
                    images_found: images,
                    gap_filled: false,
                    summary: format!("Search failed: {}", e),
                    total_time_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        // Phase 3: Evaluate and rank results before visiting
        let ranked = self.rank_search_results(&search_results, query);

        // Phase 4-6: Read → Think → Follow loop
        let mut pages_visited_count = 0u32;

        for (result, relevance, depth) in ranked {
            // Time and page budget checks
            if start.elapsed().as_secs() >= self.config.max_time_secs { break; }
            if pages_visited_count >= self.config.max_pages { break; }
            if visited_urls.contains(&result.url) { continue; }
            if relevance < self.config.min_relevance { continue; }

            // Phase 4: Read the page
            match self.fetcher.fetch_page(&result.url) {
                Ok(page) => {
                    visited_urls.insert(result.url.clone());
                    pages_visited_count += 1;

                    // Phase 5: Think — extract knowledge, evaluate gap fill
                    let page_knowledge = self.extract_knowledge_from_page(&page, query);
                    let page_images = self.extract_image_knowledge(&page, query);

                    let page_relevance = if page_knowledge.is_empty() {
                        0.0
                    } else {
                        page_knowledge.iter().map(|k| k.confidence).sum::<f32>()
                            / page_knowledge.len() as f32
                    };

                    visited.push(VisitedPage {
                        url: page.url.clone(),
                        title: page.title.clone(),
                        relevance_score: page_relevance,
                        paragraphs_extracted: page_knowledge.len(),
                        links_found: page.links.len(),
                        depth,
                    });

                    knowledge.extend(page_knowledge);
                    images.extend(page_images);

                    // Did we fill the gap? (3+ relevant facts = probably yes)
                    let relevant_facts = knowledge.iter()
                        .filter(|k| k.confidence > 0.4)
                        .count();
                    if relevant_facts >= 3 {
                        gap_filled = true;
                    }

                    // Phase 6: Follow promising internal links
                    if depth < self.config.max_depth && !gap_filled {
                        let follow_links = self.pick_links_to_follow(&page.links, query, &visited_urls);
                        for link in follow_links {
                            if start.elapsed().as_secs() >= self.config.max_time_secs { break; }
                            if pages_visited_count >= self.config.max_pages { break; }
                            if visited_urls.contains(&link.url) { continue; }

                            if let Ok(follow_page) = self.fetcher.fetch_page(&link.url) {
                                visited_urls.insert(link.url.clone());
                                pages_visited_count += 1;

                                let follow_knowledge = self.extract_knowledge_from_page(&follow_page, query);
                                let follow_images = self.extract_image_knowledge(&follow_page, query);

                                visited.push(VisitedPage {
                                    url: follow_page.url.clone(),
                                    title: follow_page.title.clone(),
                                    relevance_score: follow_knowledge.iter()
                                        .map(|k| k.confidence).sum::<f32>()
                                        / follow_knowledge.len().max(1) as f32,
                                    paragraphs_extracted: follow_knowledge.len(),
                                    links_found: follow_page.links.len(),
                                    depth: depth + 1,
                                });

                                knowledge.extend(follow_knowledge);
                                images.extend(follow_images);
                            }
                        }
                    }
                }
                Err(_e) => {
                    // Page failed to load — skip it, try next
                    continue;
                }
            }
        }

        // Build summary
        let summary = if gap_filled {
            format!(
                "Research complete. Visited {} pages, extracted {} facts and {} images.",
                visited.len(), knowledge.len(), images.len()
            )
        } else if knowledge.is_empty() {
            format!("Research found no relevant information for '{}'.", query)
        } else {
            format!(
                "Partial research. Visited {} pages, extracted {} facts. Gap not fully filled.",
                visited.len(), knowledge.len()
            )
        };

        ResearchResult {
            query: query.to_string(),
            pages_visited: visited,
            knowledge_extracted: knowledge,
            images_found: images,
            gap_filled,
            summary,
            total_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    // === THINKING METHODS ===

    /// Turn a knowledge gap into effective search keywords.
    /// "What is DialectForge?" → "DialectForge protocol AI identity"
    fn form_search_query(&self, query: &str) -> String {
        let keywords: Vec<String> = query.split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
            .filter(|w| w.len() > 1)
            .collect();

        if keywords.is_empty() {
            // Fallback: use the whole query
            query.to_string()
        } else {
            keywords.join(" ")
        }
    }

    /// Rank search results by relevance to the query BEFORE visiting.
    /// Returns (result, relevance_score, depth=0) sorted by relevance.
    fn rank_search_results(&self, results: &[SearchResult], query: &str) -> Vec<(SearchResult, f32, u32)> {
        let query_lower = query.to_lowercase();
        let query_words: HashSet<String> = query_lower.split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() > 2)
            .collect();

        let mut scored: Vec<(SearchResult, f32, u32)> = results.iter().map(|r| {
            let mut score = 0.0f32;
            let title_lower = r.title.to_lowercase();
            let snippet_lower = r.snippet.to_lowercase();
            let combined = format!("{} {}", title_lower, snippet_lower);

            // Keyword hits in title (strong signal)
            let title_hits = query_words.iter()
                .filter(|w| title_lower.contains(w.as_str()))
                .count();
            score += 0.4 * (title_hits as f32 / query_words.len().max(1) as f32);

            // Keyword hits in snippet
            let snippet_hits = query_words.iter()
                .filter(|w| snippet_lower.contains(w.as_str()))
                .count();
            score += 0.3 * (snippet_hits as f32 / query_words.len().max(1) as f32);

            // Trusted domain bonus
            if let Ok(url) = url::Url::parse(&r.url) {
                if let Some(host) = url.host_str() {
                    if self.fetcher.safety().is_trusted(host) {
                        score += 0.15;
                    }
                }
            }

            // Definition-like content bonus
            if combined.contains(" is ") || combined.contains(" are ")
                || combined.contains("definition") || combined.contains("overview")
            {
                score += 0.1;
            }

            // Penalize likely non-content pages
            if title_lower.contains("login") || title_lower.contains("sign up")
                || title_lower.contains("buy") || title_lower.contains("subscribe")
                || title_lower.contains("download")
            {
                score -= 0.3;
            }

            (r.clone(), score.clamp(0.0, 1.0), 0)
        }).collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }

    /// Extract knowledge from a parsed page.
    /// Scores each paragraph by relevance to the original query.
    fn extract_knowledge_from_page(&self, page: &ParsedPage, query: &str) -> Vec<ExtractedKnowledge> {
        // File-based debug log (same file as web_fetch diagnostics).
        let wlog = |msg: &str| {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true).append(true)
                .open(r"brain\web_debug.log")
            {
                let _ = writeln!(f, "{}", msg);
            }
        };

        let query_words: HashSet<String> = query.to_lowercase()
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| !w.is_empty())
            .collect();

        let mut knowledge = Vec::new();

        wlog(&format!(
            "[knowledge] url={} query={:?} query_words={:?} text_content_count={} headings={}",
            page.url, query, query_words, page.text_content.len(), page.headings.len()
        ));

        // Extract from headings (often definitional)
        for heading in &page.headings {
            let lower = heading.text.to_lowercase();
            let hits = query_words.iter()
                .filter(|w| lower.contains(w.as_str()))
                .count();
            if hits > 0 {
                let confidence = 0.3 + 0.4 * (hits as f32 / query_words.len().max(1) as f32);
                knowledge.push(ExtractedKnowledge {
                    content: heading.text.clone(),
                    source_url: page.url.clone(),
                    confidence,
                });
            }
        }

        let title_lower = page.title.to_lowercase();
        let title_has_query_word = query_words.iter().any(|w| title_lower.contains(w.as_str()));

        // If query produced no meaningful keywords (e.g. was a raw URL or all stop-words),
        // treat every paragraph as relevant — no keyword gate.
        let no_keywords = query_words.is_empty();

        wlog(&format!(
            "[knowledge] effective_query={:?} query_words={} no_keywords={}",
            query, query_words.len(), no_keywords
        ));

        // Extract from text paragraphs
        let mut paras_passed_keyword_gate = 0usize;
        let mut total_sentences_kept = 0usize;
        for (para_idx, paragraph) in page.text_content.iter().enumerate() {
            let lower = paragraph.to_lowercase();
            let hits = query_words.iter()
                .filter(|w| lower.contains(w.as_str()))
                .count();

            // Keep paragraph if: no keywords to filter on (open browse), it has keyword hits,
            // the page title matched (whole page is relevant), or it's in the first 10 paragraphs.
            if !no_keywords && hits == 0 && !title_has_query_word && para_idx >= 10 { continue; }
            paras_passed_keyword_gate += 1;

            let keyword_ratio = hits as f32 / query_words.len().max(1) as f32;

            // Break long paragraphs into sentences for atomic knowledge
            let sentences: Vec<&str> = paragraph.split(|c: char| c == '.' || c == '!' || c == '?')
                .map(|s| s.trim())
                .filter(|s| s.len() > 15 && s.len() < 800)
                .collect();

            wlog(&format!(
                "[knowledge]   para[{}] hits={} keyword_ratio={:.2} len={} sentences={}",
                para_idx, hits, keyword_ratio, paragraph.len(), sentences.len()
            ));

            if sentences.is_empty() {
                // Short paragraph — use as-is
                if paragraph.len() > 15 && paragraph.len() < 500 {
                    knowledge.push(ExtractedKnowledge {
                        content: paragraph.clone(),
                        source_url: page.url.clone(),
                        confidence: 0.2 + 0.5 * keyword_ratio,
                    });
                }
            } else {
                for sentence in &sentences {
                    let sent_lower = sentence.to_lowercase();
                    let sent_hits = query_words.iter()
                        .filter(|w| sent_lower.contains(w.as_str()))
                        .count();
                    let sent_ratio = sent_hits as f32 / query_words.len().max(1) as f32;

                    if sent_hits > 0 || keyword_ratio > 0.5 {
                        total_sentences_kept += 1;
                        knowledge.push(ExtractedKnowledge {
                            content: format!("{}.", sentence),
                            source_url: page.url.clone(),
                            confidence: 0.2 + 0.5 * sent_ratio.max(keyword_ratio * 0.5),
                        });
                    }
                }
            }
        }

        // Cap knowledge per page to prevent flooding
        knowledge.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        knowledge.truncate(20);

        wlog(&format!(
            "[knowledge] DONE url={} paras_passed={} sentences_kept={} final_facts={}",
            page.url, paras_passed_keyword_gate, total_sentences_kept, knowledge.len()
        ));

        knowledge
    }

    /// Extract image knowledge — what images depict based on context.
    fn extract_image_knowledge(&self, page: &ParsedPage, query: &str) -> Vec<ImageKnowledge> {
        let query_words: HashSet<String> = query.to_lowercase()
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() > 2)
            .collect();

        page.images.iter().filter_map(|img| {
            // Build a description from available context
            let description = if !img.alt_text.is_empty() && !img.caption.is_empty() {
                format!("{} — {}", img.alt_text, img.caption)
            } else if !img.alt_text.is_empty() {
                img.alt_text.clone()
            } else if !img.caption.is_empty() {
                img.caption.clone()
            } else if img.surrounding_text.len() > 20 {
                format!("Image in context: {}", &img.surrounding_text[..img.surrounding_text.len().min(150)])
            } else {
                return None; // No useful context
            };

            // Score relevance
            let desc_lower = description.to_lowercase();
            let hits = query_words.iter()
                .filter(|w| desc_lower.contains(w.as_str()))
                .count();
            let relevance = hits as f32 / query_words.len().max(1) as f32;

            Some(ImageKnowledge {
                image_url: img.src_url.clone(),
                description,
                source_url: page.url.clone(),
                relevance,
            })
        }).collect()
    }

    /// Pick the most promising links to follow from a page.
    /// Prioritizes internal links with relevant anchor text.
    fn pick_links_to_follow(&self, links: &[PageLink], query: &str, visited: &HashSet<String>) -> Vec<PageLink> {
        let query_words: HashSet<String> = query.to_lowercase()
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() > 2)
            .collect();

        let mut scored: Vec<(PageLink, f32)> = links.iter()
            .filter(|l| !visited.contains(&l.url))
            .filter(|l| {
                // Safety check the link before considering it
                self.fetcher.safety().check_url(&l.url).safe
            })
            .map(|l| {
                let mut score = 0.0f32;
                let anchor_lower = l.anchor_text.to_lowercase();

                // Keyword hits in anchor text
                let hits = query_words.iter()
                    .filter(|w| anchor_lower.contains(w.as_str()))
                    .count();
                score += 0.6 * (hits as f32 / query_words.len().max(1) as f32);

                // Internal link bonus (same domain = more likely relevant)
                if l.is_internal { score += 0.2; }

                // Penalize generic navigation links
                if anchor_lower == "home" || anchor_lower == "contact"
                    || anchor_lower == "login" || anchor_lower == "about"
                    || anchor_lower.contains("privacy") || anchor_lower.contains("terms")
                {
                    score -= 0.5;
                }

                (l.clone(), score)
            })
            .filter(|(_, s)| *s > 0.1)
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Return top 2 links max
        scored.into_iter().take(2).map(|(l, _)| l).collect()
    }

    /// Research a specific URL (user-provided). Skips search, goes direct.
    pub fn research_url(&self, url: &str, query: &str) -> ResearchResult {
        let start = Instant::now();
        let mut visited = Vec::new();
        let mut knowledge = Vec::new();
        let mut images = Vec::new();
        let mut visited_urls: HashSet<String> = HashSet::new();

        match self.fetcher.fetch_page(url) {
            Ok(page) => {
                visited_urls.insert(url.to_string());

                // When the caller passes no query (or passes the raw URL as query), derive
                // meaningful keywords from the page title so the keyword gate has real terms.
                // A URL like "https://rust-lang.org" matches zero words in any paragraph.
                let effective_query: String = if query.trim().is_empty() || query.starts_with("http") {
                    page.title.clone()
                } else {
                    query.to_string()
                };

                let page_knowledge = self.extract_knowledge_from_page(&page, &effective_query);
                let page_images = self.extract_image_knowledge(&page, &effective_query);

                visited.push(VisitedPage {
                    url: page.url.clone(),
                    title: page.title.clone(),
                    relevance_score: page_knowledge.iter()
                        .map(|k| k.confidence).sum::<f32>()
                        / page_knowledge.len().max(1) as f32,
                    paragraphs_extracted: page_knowledge.len(),
                    links_found: page.links.len(),
                    depth: 0,
                });

                // Follow promising links
                let follow_links = self.pick_links_to_follow(&page.links, &effective_query, &visited_urls);
                for link in follow_links {
                    if let Ok(follow_page) = self.fetcher.fetch_page(&link.url) {
                        visited_urls.insert(link.url.clone());
                        let fk = self.extract_knowledge_from_page(&follow_page, &effective_query);
                        let fi = self.extract_image_knowledge(&follow_page, &effective_query);
                        visited.push(VisitedPage {
                            url: follow_page.url.clone(),
                            title: follow_page.title.clone(),
                            relevance_score: fk.iter().map(|k| k.confidence).sum::<f32>()
                                / fk.len().max(1) as f32,
                            paragraphs_extracted: fk.len(),
                            links_found: follow_page.links.len(),
                            depth: 1,
                        });
                        knowledge.extend(fk);
                        images.extend(fi);
                    }
                }

                knowledge.extend(page_knowledge);
                images.extend(page_images);
            }
            Err(e) => {
                return ResearchResult {
                    query: query.to_string(),
                    pages_visited: visited,
                    knowledge_extracted: knowledge,
                    images_found: images,
                    gap_filled: false,
                    summary: format!("Failed to fetch {}: {}", url, e),
                    total_time_ms: start.elapsed().as_millis() as u64,
                };
            }
        }

        let gap_filled = knowledge.iter().filter(|k| k.confidence > 0.4).count() >= 3;

        ResearchResult {
            query: query.to_string(),
            pages_visited: visited,
            knowledge_extracted: knowledge,
            images_found: images,
            gap_filled,
            summary: format!("Direct research on {}. {} facts extracted.", url, 0),
            total_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Get a reference to the fetcher (for safety engine access).
    pub fn fetcher(&self) -> &WebFetcher {
        &self.fetcher
    }
}
