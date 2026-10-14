// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Web safety layer — URL validation, blocked TLDs/domains, content-type whitelist.
//!
//! Safety rules enforced:
//!   - HTTPS only (http:// rejected)
//!   - Blocked TLDs: .onion, .i2p, .bit, .exit, .zkey
//!   - Blocked domains: localhost, 127.0.0.1, 0.0.0.0
//!   - Blocked IP ranges: RFC-1918 private space, link-local
//!   - Content-type whitelist: text/html, application/json, text/plain, application/xhtml+xml
//!   - Max response size: 5 MB
//!   - Max links to follow per page: 50

pub const MAX_PAGE_SIZE: usize = 5 * 1024 * 1024; // 5 MB
pub const MAX_LINKS_PER_PAGE: usize = 50;

// ── SafetyVerdict ─────────────────────────────────────────────────────────────

/// Decision returned by the safety engine for a given URL.
#[derive(Debug, Clone)]
pub struct SafetyVerdict {
    pub safe: bool,
    pub reason: String,
}

// ── WebSafety ─────────────────────────────────────────────────────────────────

/// Web safety configuration and enforcement.
#[derive(Debug, Clone)]
pub struct WebSafety {
    /// Blocked top-level domains (e.g. ".onion").
    pub blocked_tlds: Vec<String>,
    /// Blocked hostnames / IP addresses (exact or prefix match).
    pub blocked_domains: Vec<String>,
    /// Allowed content-type values (substring match against the header).
    pub allowed_content_types: Vec<String>,
}

impl Default for WebSafety {
    fn default() -> Self {
        WebSafety {
            blocked_tlds: vec![
                ".onion".into(),
                ".i2p".into(),
                ".bit".into(),
                ".exit".into(),
                ".zkey".into(),
            ],
            blocked_domains: vec![
                "localhost".into(),
                "127.0.0.1".into(),
                "0.0.0.0".into(),
                "::1".into(),
            ],
            allowed_content_types: vec![
                "text/html".into(),
                "application/json".into(),
                "text/plain".into(),
                "application/xhtml+xml".into(),
            ],
        }
    }
}

impl WebSafety {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check whether a URL is safe to fetch.
    ///
    /// Enforces HTTPS-only, blocked TLDs, blocked domains, and private IP ranges.
    pub fn check_url(&self, url: &str) -> SafetyVerdict {
        // HTTPS only.
        if !url.starts_with("https://") {
            return SafetyVerdict {
                safe: false,
                reason: format!(
                    "HTTPS required — URL must start with https:// (got: {})",
                    url.chars().take(50).collect::<String>()
                ),
            };
        }

        let lower = url.to_lowercase();

        // Extract the host portion (strip "https://", path, port, query).
        let after_scheme = &lower[8..];
        let host = after_scheme
            .split('/')
            .next()
            .unwrap_or(after_scheme)
            .split('?')
            .next()
            .unwrap_or(after_scheme)
            .split(':')
            .next()
            .unwrap_or(after_scheme);

        // Blocked TLDs.
        for tld in &self.blocked_tlds {
            if host.ends_with(tld.as_str()) {
                return SafetyVerdict {
                    safe: false,
                    reason: format!("Blocked TLD: {}", tld),
                };
            }
        }

        // Blocked exact domains / prefixes.
        for blocked in &self.blocked_domains {
            if host == blocked.as_str() || host.starts_with(blocked.as_str()) {
                return SafetyVerdict {
                    safe: false,
                    reason: format!("Blocked domain/IP: {}", blocked),
                };
            }
        }

        // Block private / internal IP ranges.
        if is_private_ip(host) {
            return SafetyVerdict {
                safe: false,
                reason: format!("Private/internal IP blocked: {}", host),
            };
        }

        SafetyVerdict {
            safe: true,
            reason: "OK".into(),
        }
    }

    /// Check whether a content-type header value is acceptable.
    pub fn check_content_type(&self, content_type: &str) -> bool {
        let ct_lower = content_type.to_lowercase();
        self.allowed_content_types
            .iter()
            .any(|allowed| ct_lower.contains(allowed.as_str()))
    }

    /// Check whether a response body size is within the configured limit.
    pub fn check_size(&self, bytes: usize) -> bool {
        bytes <= MAX_PAGE_SIZE
    }

    /// Validate that a URL string is structurally usable (safe + parseable).
    pub fn validate_url(&self, url: &str) -> Result<(), String> {
        let verdict = self.check_url(url);
        if verdict.safe {
            Ok(())
        } else {
            Err(verdict.reason)
        }
    }
}

/// Returns `true` if the host looks like a private/internal/loopback IP address.
fn is_private_ip(host: &str) -> bool {
    // RFC 1918 private ranges
    if host.starts_with("10.") { return true; }
    if host.starts_with("192.168.") { return true; }
    // 172.16.0.0 – 172.31.255.255
    for second in 16u8..=31 {
        if host.starts_with(&format!("172.{}.", second)) {
            return true;
        }
    }
    // Link-local
    if host.starts_with("169.254.") { return true; }
    // IPv6 loopback
    if host == "[::1]" { return true; }
    false
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_https_required() {
        let s = WebSafety::new();
        assert!(!s.check_url("http://example.com").safe);
        assert!(!s.check_url("ftp://example.com").safe);
        assert!(!s.check_url("example.com").safe);
    }

    #[test]
    fn test_valid_https() {
        let s = WebSafety::new();
        assert!(s.check_url("https://example.com").safe);
        assert!(s.check_url("https://en.wikipedia.org/wiki/Rust").safe);
    }

    #[test]
    fn test_blocked_tlds() {
        let s = WebSafety::new();
        assert!(!s.check_url("https://evil.onion").safe);
        assert!(!s.check_url("https://hidden.i2p").safe);
    }

    #[test]
    fn test_blocked_domains() {
        let s = WebSafety::new();
        assert!(!s.check_url("https://localhost/path").safe);
        assert!(!s.check_url("https://127.0.0.1/api").safe);
        assert!(!s.check_url("https://0.0.0.0/").safe);
    }

    #[test]
    fn test_private_ips() {
        let s = WebSafety::new();
        assert!(!s.check_url("https://192.168.1.1/").safe);
        assert!(!s.check_url("https://10.0.0.1/").safe);
        assert!(!s.check_url("https://172.16.0.1/").safe);
        assert!(!s.check_url("https://172.31.255.255/").safe);
        assert!(!s.check_url("https://169.254.0.1/").safe);
    }

    #[test]
    fn test_content_type() {
        let s = WebSafety::new();
        assert!(s.check_content_type("text/html; charset=utf-8"));
        assert!(s.check_content_type("application/json"));
        assert!(s.check_content_type("text/plain"));
        assert!(!s.check_content_type("application/octet-stream"));
        assert!(!s.check_content_type("image/png"));
    }

    #[test]
    fn test_size_check() {
        let s = WebSafety::new();
        assert!(s.check_size(1024));
        assert!(s.check_size(MAX_PAGE_SIZE));
        assert!(!s.check_size(MAX_PAGE_SIZE + 1));
    }
}
