// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Web Safety Layer
//!
//! Lu's street smarts for the internet. Every URL goes through here
//! before Lu visits it. HTTPS-only, domain blocklist, content-type
//! validation, size limits, and no executable downloads.
//!
//! PHILOSOPHY: Lu should browse like a cautious researcher.
//! Read text, follow links, learn knowledge. Never execute anything,
//! never download binaries, never visit known-bad domains.
//!
//! Created by John Dean Martin

use url::Url;
use std::collections::HashSet;

/// Maximum page size Lu will download (5MB)
pub const MAX_PAGE_SIZE: usize = 5 * 1024 * 1024;

/// Maximum number of links Lu will extract from a single page
pub const MAX_LINKS_PER_PAGE: usize = 50;

/// Maximum depth for following links (clicks deep from search results)
pub const MAX_FOLLOW_DEPTH: u32 = 2;

/// Maximum pages per research session
pub const MAX_PAGES_PER_SESSION: u32 = 5;

/// Maximum time per research session (seconds)
pub const MAX_SESSION_SECONDS: u64 = 300; // 5 minutes

/// Result of a safety check on a URL.
#[derive(Debug, Clone)]
pub struct SafetyVerdict {
    pub safe: bool,
    pub url: String,
    pub reason: String,
}

/// Blocked top-level domains (known spam/malware havens)
const BLOCKED_TLDS: &[&str] = &[
    ".tk", ".ml", ".ga", ".cf", ".gq",   // Free TLDs abused by malware
    ".top", ".xyz", ".buzz", ".club",     // High-spam TLDs
    ".zip", ".mov",                        // Deceptive file-extension TLDs
];

/// Blocked domains (known malware, phishing, ad farms)
const BLOCKED_DOMAINS: &[&str] = &[
    // Ad/tracking networks
    "doubleclick.net", "adnxs.com", "googlesyndication.com",
    "advertising.com", "clickbank.net",
    // Known malware distributors
    "malware-traffic-analysis.net",
    // Content farms / SEO spam
    "ehow.com", "answerbag.com",
    // Phishing aggregators
    "bit.ly", "tinyurl.com", "t.co", // URL shorteners (can't verify destination)
    "is.gd", "ow.ly", "buff.ly",
];

/// Allowed content types (Lu only reads text)
const SAFE_CONTENT_TYPES: &[&str] = &[
    "text/html",
    "text/plain",
    "application/xhtml+xml",
    "application/json",
    "text/xml",
    "application/xml",
];

/// Blocked file extensions in URLs (never download these)
const BLOCKED_EXTENSIONS: &[&str] = &[
    ".exe", ".msi", ".bat", ".cmd", ".ps1", ".sh", ".bash",
    ".dll", ".sys", ".scr", ".com", ".vbs", ".js",
    ".zip", ".rar", ".7z", ".tar", ".gz",
    ".iso", ".img", ".dmg",
    ".pdf", ".doc", ".docx", ".xls", ".xlsx", // Lu reads web pages, not downloads
    ".mp3", ".mp4", ".avi", ".mkv", ".wav",
    ".apk", ".deb", ".rpm",
];

/// The Web Safety Engine — Lu's internet street smarts.
pub struct WebSafety {
    /// Additional blocked domains (learned over time)
    custom_blocked: HashSet<String>,
    /// Trusted domains that bypass strict checks
    trusted_domains: HashSet<String>,
}

impl Default for WebSafety {
    fn default() -> Self {
        let mut trusted = HashSet::new();
        // Lu's home — always safe
        trusted.insert("dialectforge.com".to_string());
        trusted.insert("www.dialectforge.com".to_string());
        // Major knowledge sources
        trusted.insert("en.wikipedia.org".to_string());
        trusted.insert("en.m.wikipedia.org".to_string());
        trusted.insert("stackoverflow.com".to_string());
        trusted.insert("docs.rust-lang.org".to_string());
        trusted.insert("developer.mozilla.org".to_string());
        trusted.insert("github.com".to_string());
        trusted.insert("arxiv.org".to_string());
        trusted.insert("britannica.com".to_string());
        trusted.insert("www.britannica.com".to_string());

        Self {
            custom_blocked: HashSet::new(),
            trusted_domains: trusted,
        }
    }
}

impl WebSafety {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a URL is safe for Lu to visit.
    pub fn check_url(&self, raw_url: &str) -> SafetyVerdict {
        // Parse the URL
        let url = match Url::parse(raw_url) {
            Ok(u) => u,
            Err(e) => return SafetyVerdict {
                safe: false,
                url: raw_url.to_string(),
                reason: format!("Invalid URL: {}", e),
            },
        };

        // Rule 1: HTTPS only (with exception for localhost)
        let scheme = url.scheme();
        let host = url.host_str().unwrap_or("");
        if scheme != "https" && host != "localhost" && host != "127.0.0.1" {
            return SafetyVerdict {
                safe: false,
                url: raw_url.to_string(),
                reason: format!("Insecure protocol '{}'. Lu only visits HTTPS sites.", scheme),
            };
        }

        // Rule 2: Must have a host
        if host.is_empty() {
            return SafetyVerdict {
                safe: false,
                url: raw_url.to_string(),
                reason: "No host in URL.".to_string(),
            };
        }

        let host_lower = host.to_lowercase();

        // Rule 3: Check blocked TLDs
        for tld in BLOCKED_TLDS {
            if host_lower.ends_with(tld) {
                return SafetyVerdict {
                    safe: false,
                    url: raw_url.to_string(),
                    reason: format!("Blocked TLD '{}' — high malware risk.", tld),
                };
            }
        }

        // Rule 4: Check blocked domains
        for domain in BLOCKED_DOMAINS {
            if host_lower == *domain || host_lower.ends_with(&format!(".{}", domain)) {
                return SafetyVerdict {
                    safe: false,
                    url: raw_url.to_string(),
                    reason: format!("Blocked domain '{}' — known unsafe.", domain),
                };
            }
        }

        // Rule 5: Check custom blocked domains
        if self.custom_blocked.contains(&host_lower) {
            return SafetyVerdict {
                safe: false,
                url: raw_url.to_string(),
                reason: "Domain blocked by Lu (learned unsafe).".to_string(),
            };
        }

        // Rule 6: Check blocked file extensions in the path
        let path = url.path().to_lowercase();
        for ext in BLOCKED_EXTENSIONS {
            if path.ends_with(ext) {
                return SafetyVerdict {
                    safe: false,
                    url: raw_url.to_string(),
                    reason: format!("Blocked file type '{}' — Lu doesn't download files.", ext),
                };
            }
        }

        // Rule 7: No IP-address-only hosts (except localhost)
        if host.parse::<std::net::IpAddr>().is_ok()
            && host != "127.0.0.1" && host != "::1"
        {
            return SafetyVerdict {
                safe: false,
                url: raw_url.to_string(),
                reason: "Raw IP address — Lu needs a real domain name.".to_string(),
            };
        }

        // Rule 8: No absurdly long URLs (potential buffer overflow attempts)
        if raw_url.len() > 2048 {
            return SafetyVerdict {
                safe: false,
                url: raw_url.to_string(),
                reason: "URL too long (>2048 chars) — suspicious.".to_string(),
            };
        }

        // All checks passed
        SafetyVerdict {
            safe: true,
            url: raw_url.to_string(),
            reason: if self.trusted_domains.contains(&host_lower) {
                "Trusted domain.".to_string()
            } else {
                "Passed all safety checks.".to_string()
            },
        }
    }

    /// Check if a content-type header is safe for Lu to process.
    pub fn check_content_type(&self, content_type: &str) -> bool {
        let ct_lower = content_type.to_lowercase();
        SAFE_CONTENT_TYPES.iter().any(|safe| ct_lower.contains(safe))
    }

    /// Add a domain to the blocklist (Lu learned it's unsafe).
    pub fn block_domain(&mut self, domain: &str) {
        self.custom_blocked.insert(domain.to_lowercase());
    }

    /// Add a domain to the trusted list.
    pub fn trust_domain(&mut self, domain: &str) {
        self.trusted_domains.insert(domain.to_lowercase());
    }

    /// Check if a domain is trusted (for prioritizing search results).
    pub fn is_trusted(&self, domain: &str) -> bool {
        self.trusted_domains.contains(&domain.to_lowercase())
    }
}
