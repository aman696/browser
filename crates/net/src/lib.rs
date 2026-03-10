//! `net` — HTTP/HTTPS networking for the Ferrum browser.
//!
//! This crate owns all socket, TLS, and DNS code. No other crate in the
//! workspace is permitted to open a socket or make a network request directly.
//!
//! # Architecture
//!
//! ```text
//! fetch(url)
//!    │
//!    ├─ parse_url        → validate + normalise URL, enforce HTTPS
//!    ├─ dns::resolve     → DNS-over-HTTPS via hickory-resolver + Cloudflare
//!    ├─ TcpStream        → tokio async TCP socket
//!    ├─ tls::connector   → rustls TLS handshake (aws-lc-rs + webpki-roots)
//!    ├─ http::request    → HTTP/1.1 GET request string
//!    └─ http::response   → header parse (httparse) + chunked body decode
//! ```
//!
//! # Privacy guarantees
//!
//! - System DNS resolver is never called (`hickory-resolver` with DoH only).
//! - All remote HTTP is silently upgraded to HTTPS in [`parse_url`].
//! - No speculative/prefetch DNS queries — lookups happen only inside `fetch`.
//! - Certificate errors hard-fail; there is no click-through.
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
pub mod dns;
pub mod http;
pub mod tls;
mod url;

pub use url::{ParsedUrl, UrlError, parse_url};

/// Fetch the given URL and return the response body as a `String`.
///
/// This is the single public entry point for all network requests in Ferrum.
/// All privacy policy enforcement (HTTPS upgrade, DoH DNS, cert verification)
/// happens inside this function before any bytes leave the machine.
///
/// # Errors
///
/// Returns a typed [`FetchError`] describing exactly what went wrong.
/// The caller (`crates/browser`) maps these to appropriate error pages or
/// privacy warning interstitials.
pub async fn fetch(url: &str) -> Result<String, FetchError> {
    client::fetch(url).await
}

/// Errors that can occur when fetching a URL.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// The URL could not be parsed.
    #[error("invalid URL: {0}")]
    InvalidUrl(#[from] UrlError),

    /// DNS resolution failed (DoH query to Cloudflare 1.1.1.1).
    #[error("DNS resolution failed: {0}")]
    Dns(String),

    /// TCP connection to the remote server failed.
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
}
