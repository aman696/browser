//! Integration tests for the `net` crate URL parser.
//!
//! These tests verify URL parsing against a broader set of real-world URL
//! patterns than the unit tests in `src/url.rs`. The focus is on edge cases
//! encountered when processing URLs found in real HTML documents.

use net::parse_url;

// ─── Real-world URL patterns ───────────────────────────────────────────────────

#[test]
fn test_parse_google_url() {
    let url = parse_url("https://www.google.com/search?q=ferrum").unwrap();
    assert_eq!(url.host, "www.google.com");
    assert_eq!(url.port, 443);
    assert!(url.is_https);
    assert!(!url.is_localhost);
    // Path includes the query string as-is (we don't parse query params at this layer)
    assert!(url.path.starts_with('/'));
}

#[test]
fn test_parse_url_with_explicit_port_443() {
    let url = parse_url("https://example.com:443/page").unwrap();
    assert_eq!(url.host, "example.com");
    assert_eq!(url.port, 443);
    assert_eq!(url.path, "/page");
}

#[test]
fn test_parse_url_with_non_standard_https_port() {
    let url = parse_url("https://example.com:8443/api").unwrap();
    assert_eq!(url.port, 8443);
    assert!(url.is_https);
}

#[test]
fn test_parse_http_localhost_with_port() {
    let url = parse_url("http://localhost:3000/dev/test").unwrap();
    assert!(url.is_localhost);
    assert!(!url.is_https);
    assert_eq!(url.port, 3000);
    assert_eq!(url.path, "/dev/test");
}

#[test]
fn test_parse_url_deep_path() {
    let url = parse_url("https://example.com/a/b/c/d.html").unwrap();
    assert_eq!(url.path, "/a/b/c/d.html");
}

#[test]
fn test_parse_url_trailing_slash() {
    let url = parse_url("https://example.com/").unwrap();
    assert_eq!(url.path, "/");
}

#[test]
fn test_parse_http_remote_is_silently_upgraded() {
    // SECURITY: Any http:// URL to a non-localhost host must be silently
    // upgraded to HTTPS. This is the core privacy enforcement rule.
    let url = parse_url("http://example.com/page").unwrap();
    assert!(url.is_https, "remote HTTP must be upgraded to HTTPS");
    assert_eq!(url.port, 443, "port must be updated to 443 after upgrade");
}

#[test]
fn test_parse_url_ftp_is_rejected() {
    assert!(parse_url("ftp://files.example.com").is_err());
}

#[test]
fn test_parse_url_data_scheme_is_rejected() {
    assert!(parse_url("data:text/html,<h1>hi</h1>").is_err());
}

#[test]
fn test_parse_url_empty_is_rejected() {
    assert!(parse_url("").is_err());
}

// ─── Security hardening tests ──────────────────────────────────────────────────

#[test]
fn test_userinfo_is_rejected() {
    // SECURITY: `user:pass@host` is a phishing vector — looks like bank.com
    // but routes to evil.com. Must be rejected per WHATWG URL spec §5.1.
    use net::UrlError;
    assert_eq!(
        net::parse_url("https://user:pass@example.com").unwrap_err(),
        UrlError::UserInfoNotAllowed
    );
    assert_eq!(
        net::parse_url("https://bank.com@evil.com/steal").unwrap_err(),
        UrlError::UserInfoNotAllowed
    );
}

#[test]
fn test_fragment_is_stripped_from_path() {
    // SECURITY: Fragments (RFC 9110 §4.2.4) must never be sent to the server.
    // They frequently contain OAuth tokens or private application state.
    let url = net::parse_url("https://example.com/callback#access_token=secret123").unwrap();
    assert_eq!(url.path, "/callback", "fragment must be stripped from path");
    assert!(!url.path.contains('#'), "no # in stored path");
}

#[test]
fn test_fragment_only_url_path_stays_root() {
    let url = net::parse_url("https://example.com#top").unwrap();
    assert_eq!(url.path, "/");
}

#[test]
fn test_ipv6_zone_id_is_rejected() {
    // SECURITY: Zone IDs (RFC 6874) can probe local network interfaces.
    assert!(
        net::parse_url("https://[fe80::1%eth0]/").is_err(),
        "IPv6 Zone ID must be rejected"
    );
}

#[test]
fn test_javascript_scheme_is_rejected() {
    // SECURITY: `javascript:` URIs must never be treated as URLs.
    assert!(net::parse_url("javascript:void(0)").is_err());
    assert!(net::parse_url("javascript:alert(1)").is_err());
}

#[test]
fn test_hsts_store_records_and_enforces() {
    use net::hsts::HstsStore;

    let mut store = HstsStore::new();
    let recorded =
        store.record_from_header("secure.example.com", "max-age=31536000; includeSubDomains");
    assert!(recorded, "valid STS header should be recorded");
    assert!(store.is_hsts("secure.example.com"));
    assert!(store.is_hsts("api.secure.example.com")); // includeSubDomains
    assert!(!store.is_hsts("other.com"));
}

#[test]
fn test_hsts_without_max_age_is_not_recorded() {
    // max-age is required by RFC 6797 — a header without it must be ignored.
    use net::hsts::HstsStore;

    let mut store = HstsStore::new();
    let recorded = store.record_from_header("example.com", "includeSubDomains");
    assert!(!recorded);
    assert!(!store.is_hsts("example.com"));
}
