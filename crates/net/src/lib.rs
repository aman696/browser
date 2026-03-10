//! `net` — HTTP/HTTPS networking for the Ferrum browser.
//!
//! This crate owns all socket, TLS, and DNS code. No other crate in the
//! workspace is permitted to open a socket or make a network request directly.
//!
//! # Architecture
//!
//! All requests flow through a [`NetworkContext`], which owns TLS, DNS, and
//! HSTS state and enforces all privacy policies in a single, auditable place.
//!
//! ```text
//! NetworkContext::fetch(url)
//!    │
//!    ├─ parse_url        → validate + sanitise URL, enforce HTTPS
//!    │                     (rejects userinfo, Zone IDs, fragments)
//!    ├─ hsts::is_hsts    → force HTTPS for HSTS-known hosts
//!    ├─ dns::resolve     → DNS-over-HTTPS via hickory-resolver + Cloudflare
//!    ├─ TcpStream        → tokio async TCP socket (10s connect timeout)
//!    ├─ tls::connector   → rustls TLS 1.2+/TLS 1.3 (aws-lc-rs + webpki-roots)
//!    ├─ http::request    → HTTP/1.1 GET with Sec-Fetch-* metadata headers
//!    └─ http::response   → header parse (httparse) + chunked body decode
//!                          (50 MB response size cap, 30s read timeout)
//! ```
//!
//! # Privacy guarantees
//!
//! - System DNS resolver is never called (`hickory-resolver` with DoH only).
//! - All remote HTTP is silently upgraded to HTTPS in [`parse_url`].
//! - No speculative/prefetch DNS queries — lookups happen only inside `fetch`.
//! - Certificate errors hard-fail; there is no click-through.
//! - Userinfo URLs (`user:pass@host`) are rejected before any connection.
//! - Fragment identifiers are stripped and never sent to servers.
//! - HSTS state is tracked in memory and enforced on subsequent requests.
//! - Response bodies are capped at 50 MB.
//! - Connect timeout: 10 seconds. Read timeout: 30 seconds.
//!
//! # Dependency rationale
//!
//! | Purpose | Crate | Why |
//! |---------|-------|-----|
//! | TLS     | `rustls` (aws-lc-rs) | Pure-Rust TLS. OpenSSL banned. |
//! | DNS     | `hickory-resolver` | DoH support. System resolver banned. |
//! | Roots   | `webpki-roots` | Mozilla root CAs compiled-in, no OS dependency. |
//! | HTTP    | `httparse` | Minimal, no_std, zero-copy header parser. |

pub mod client;
pub mod context;
pub mod dns;
pub mod hsts;
pub mod http;
pub mod tls;
mod url;

pub use context::NetworkContext;
pub use url::{ParsedUrl, UrlError, parse_url};

/// Fetch the given URL and return the response body as a `String`.
///
/// This is a convenience wrapper that creates a one-shot [`NetworkContext`].
/// For production use (browser tab loading, repeated fetches) prefer creating
/// a `NetworkContext` once and calling its `fetch` method to benefit from
/// connection and DNS resolver caching.
///
/// # Errors
///
/// Returns a typed [`FetchError`] describing exactly what went wrong.
pub async fn fetch(url: &str) -> Result<String, FetchError> {
    let ctx = NetworkContext::new()?;
    ctx.fetch(url).await
}

/// Errors that can occur when fetching a URL.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// The URL could not be parsed or was rejected for security reasons.
    #[error("invalid URL: {0}")]
    InvalidUrl(#[from] UrlError),

    /// DNS resolution failed (DoH query to Cloudflare 1.1.1.1).
    #[error("DNS resolution failed: {0}")]
    Dns(String),

    /// TCP connection or I/O error.
    #[error("I/O error: {0}")]
    Io(String),

    /// TLS handshake failed or the server's certificate is invalid.
    ///
    /// Per Ferrum's privacy model, certificate errors hard-fail.
    /// There is no click-through mechanism.
    #[error("TLS error: {0}")]
    Tls(String),

    /// The HTTP response was structurally invalid (malformed headers, etc.).
    #[error("protocol error: {0}")]
    Protocol(String),

    /// The server responded with an HTTP error status code.
    #[error("HTTP error {0}")]
    HttpStatus(u16),

    /// More than 5 HTTP redirects were followed without reaching a final response.
    #[error("too many redirects (max 5)")]
    TooManyRedirects,

    /// A network operation (connect or read) exceeded its time limit.
    ///
    /// Connect timeout: 10 seconds.
    /// Read timeout: 30 seconds.
    #[error("network operation timed out")]
    Timeout,

    /// The server's response body exceeded the maximum permitted size (50 MB).
    ///
    /// Hard limit to prevent RAM exhaustion from hostile or misconfigured
    /// servers streaming unlimited response data.
    #[error("response too large (max 50 MB)")]
    ResponseTooLarge,
}
