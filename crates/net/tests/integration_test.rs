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
    assert_eq!(url.port, "443");
    assert!(url.is_https);
    assert!(!url.is_localhost);
    // Path includes the query string as-is (we don't parse query params at this layer)
    assert!(url.path.starts_with('/'));
}

#[test]
fn test_parse_url_with_explicit_port_443() {
    let url = parse_url("https://example.com:443/page").unwrap();
    assert_eq!(url.host, "example.com");
    assert_eq!(url.port, "443");
    assert_eq!(url.path, "/page");
}

#[test]
fn test_parse_url_with_non_standard_https_port() {
    let url = parse_url("https://example.com:8443/api").unwrap();
    assert_eq!(url.port, "8443");
    assert!(url.is_https);
}

#[test]
fn test_parse_http_localhost_with_port() {
    let url = parse_url("http://localhost:3000/dev/test").unwrap();
    assert!(url.is_localhost);
    assert!(!url.is_https);
    assert_eq!(url.port, "3000");
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
    assert_eq!(url.port, "443", "port must be updated to 443 after upgrade");
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
