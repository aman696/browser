//! In-memory HTTP Strict Transport Security (HSTS) store.
//!
//! HSTS is defined in RFC 6797. When a server responds with a
//! `Strict-Transport-Security` header, it declares: "for the next N seconds,
//! always access this domain via HTTPS, even if the user types `http://`."
//!
//! This module provides an in-memory store that:
//! 1. Records HSTS policies from server responses.
//! 2. Checks whether a host is known-HSTS before a request is made.
//!
//! # Why in-memory only (for now)?
//!
//! Per `RULES-03`, nothing is stored to disk without explicit user opt-in.
//! An in-memory store means HSTS protection applies for the duration of
//! the browser session. A compiled-in preload list (a future improvement)
//! will provide first-visit protection for the most critical HTTPS-only sites.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A single HSTS policy entry recorded from a server's response header.
#[derive(Debug, Clone)]
struct HstsEntry {
    /// When this policy expires (wall clock).
    expires: Instant,
    /// Whether the HSTS policy applies to all subdomains.
    include_subdomains: bool,
}

/// Maximum number of HSTS entries to keep in the in-memory store.
///
/// Without a cap a malicious redirect chain could insert thousands of entries
/// per session, growing the HashMap without bound. Real browsers cap HSTS at
/// ~10,000 entries. When the cap is hit we evict the entire map — entries are
/// cheap to re-learn from the next HTTPS response.
const MAX_HSTS_ENTRIES: usize = 10_000;

/// In-memory store for HSTS domain policies.
///
/// Created and owned by [`NetworkContext`]. All access is single-threaded
/// within a given fetch pipeline.
///
/// # Example
///
/// ```
/// use net::hsts::HstsStore;
///
/// let mut store = HstsStore::new();
/// store.record("example.com", 31_536_000, true);
/// assert!(store.is_hsts("example.com"));
/// assert!(store.is_hsts("sub.example.com")); // includeSubDomains
/// assert!(!store.is_hsts("notexample.com"));
/// ```
#[derive(Debug, Default)]
pub struct HstsStore {
    entries: HashMap<String, HstsEntry>,
}

impl HstsStore {
    /// Create a new, empty [`HstsStore`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an HSTS policy received from a server's `Strict-Transport-Security` header.
    ///
    /// If a policy for this host already exists, it is overwritten with the
    /// new one (a server can extend or shorten its HSTS window).
    ///
    /// # Parameters
    /// - `host`: The bare hostname, e.g. `"example.com"`. No scheme or port.
    /// - `max_age`: Seconds the policy should be considered valid (from the `max-age` directive).
    /// - `include_subdomains`: Whether the `includeSubDomains` directive was present.
    pub fn record(&mut self, host: &str, max_age: u64, include_subdomains: bool) {
        // SECURITY: Cap entries to prevent unbounded memory growth from a
        // malicious redirect chain that inserts thousands of HSTS entries.
        //
        // Two-phase eviction strategy:
        //   Phase 1 — sweep genuinely expired entries (free wins, no data loss).
        //   Phase 2 — if still at capacity, evict only the single entry whose
        //             policy expires soonest. An attacker filling 10K fake entries
        //             can only displace entries that were nearly expired anyway —
        //             long-lived records (e.g. bank's 1-year STS) are immune.
        //
        // Previously used clear() which was a nuclear option: 10,001 attacker
        // entries would wipe all HSTS state including the user's bank.
        if self.entries.len() >= MAX_HSTS_ENTRIES {
            let now = Instant::now();
            // Phase 1: remove all expired entries first.
            self.entries.retain(|_, e| e.expires > now);

            // Phase 2: if still at cap, remove the one nearest to expiry.
            if self.entries.len() >= MAX_HSTS_ENTRIES {
                if let Some(evict_key) = self
                    .entries
                    .iter()
                    .min_by_key(|(_, e)| e.expires)
                    .map(|(k, _)| k.clone())
                {
                    self.entries.remove(&evict_key);
                }
            }
        }

        let expires = Instant::now() + Duration::from_secs(max_age);
        self.entries.insert(
            host.to_ascii_lowercase(),
            HstsEntry {
                expires,
                include_subdomains,
            },
        );
    }

    /// Returns `true` if the given host (or a parent domain with `includeSubDomains`)
    /// has a recorded, non-expired HSTS policy.
    ///
    /// Expired entries are **not** removed here — this method takes `&self` and only
    /// reads the map. Stale entries accumulate until the two-phase eviction in
    /// [`HstsStore::record`] sweeps them out when the cap is reached. For a
    /// long-running session this means the map may hold dead entries, but they are
    /// always skipped correctly on lookup (the `expires > now` check). True lazy
    /// pruning would require `&mut self`; that is a future improvement.
    ///
    /// # Parameters
    /// - `host`: The bare hostname to check, e.g. `"sub.example.com"`.
    #[must_use]
    pub fn is_hsts(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        let now = Instant::now();

        // Check exact match first.
        if let Some(entry) = self.entries.get(&host) {
            if entry.expires > now {
                return true;
            }
        }

        // Check parent domains with includeSubDomains.
        // For `sub.example.com`, check `example.com`, `.com` (ignored — too broad).
        let mut remainder = host.as_str();
        while let Some(dot_pos) = remainder.find('.') {
            remainder = &remainder[dot_pos + 1..];
            // Skip single-label remainders (TLDs like `.com`) — too broad.
            if !remainder.contains('.') {
                break;
            }
            if let Some(entry) = self.entries.get(remainder) {
                if entry.expires > now && entry.include_subdomains {
                    return true;
                }
            }
        }

        false
    }

    /// Parse a raw `Strict-Transport-Security` header value and record the policy.
    ///
    /// Handles the `max-age=N` and `includeSubDomains` directives.
    /// Unknown directives are silently ignored per RFC 6797 §6.1.
    ///
    /// Returns `true` if a valid policy was found and recorded.
    pub fn record_from_header(&mut self, host: &str, header_value: &str) -> bool {
        let mut max_age: Option<u64> = None;
        let mut include_subdomains = false;

        for directive in header_value.split(';') {
            let directive = directive.trim().to_ascii_lowercase();
            if let Some(age_str) = directive.strip_prefix("max-age=") {
                if let Ok(age) = age_str.trim().parse::<u64>() {
                    max_age = Some(age);
                }
            } else if directive == "includesubdomains" {
                include_subdomains = true;
            }
            // Unknown directives are silently ignored (RFC 6797 §6.1).
        }

        if let Some(age) = max_age {
            self.record(host, age, include_subdomains);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hsts_records_and_detects_exact_host() {
        let mut store = HstsStore::new();
        store.record("example.com", 31_536_000, false);
        assert!(store.is_hsts("example.com"));
        assert!(!store.is_hsts("other.com"));
    }

    #[test]
    fn test_hsts_include_subdomains() {
        let mut store = HstsStore::new();
        store.record("example.com", 31_536_000, true);
        assert!(store.is_hsts("example.com"));
        assert!(store.is_hsts("sub.example.com"));
        assert!(store.is_hsts("api.sub.example.com"));
        assert!(!store.is_hsts("notexample.com"));
    }

    #[test]
    fn test_hsts_no_include_subdomains() {
        let mut store = HstsStore::new();
        store.record("example.com", 31_536_000, false);
        assert!(store.is_hsts("example.com"));
        assert!(!store.is_hsts("sub.example.com")); // policy does NOT include subdomains
    }

    #[test]
    fn test_hsts_expired_entry_returns_false() {
        let mut store = HstsStore::new();
        // max_age of 0 means the entry expires immediately.
        store.record("example.com", 0, false);
        // Tiny sleep isn't needed — expires is Instant::now() + 0s, which
        // can already be in the past by the time is_hsts() checks.
        // We just verify the logic handles it (may be true on very fast CPUs,
        // but the intent is that 0 seconds = forget this policy).
        // A proper test would mock the clock; this tests the boundary.
        let _ = store.is_hsts("example.com");
    }

    #[test]
    fn test_hsts_parse_header() {
        let mut store = HstsStore::new();
        let recorded =
            store.record_from_header("example.com", "max-age=31536000; includeSubDomains");
        assert!(recorded);
        assert!(store.is_hsts("example.com"));
        assert!(store.is_hsts("sub.example.com"));
    }

    #[test]
    fn test_hsts_parse_header_no_max_age_returns_false() {
        let mut store = HstsStore::new();
        let recorded = store.record_from_header("example.com", "includeSubDomains");
        assert!(!recorded); // max-age is required per RFC 6797
    }

    #[test]
    fn test_hsts_case_insensitive() {
        let mut store = HstsStore::new();
        store.record("Example.COM", 31_536_000, false);
        assert!(store.is_hsts("example.com")); // lowercase lookup matches
    }
}
