//! HTTP/HTTPS fetch implementation — the real network plumbing.
//!
//! # Request pipeline (per [`NetworkContext::fetch`])
//!
//! 1. [`parse_url`] — validate and sanitise the URL (rejects userinfo, Zone IDs,
//!    strips fragments, enforces HTTPS for remote hosts).
//! 2. HSTS check — if the host was seen with a `Strict-Transport-Security`
//!    header in a previous response, force HTTPS regardless of the URL scheme.
//! 3. [`dns::resolve`] — DoH lookup using the cached `TokioResolver`.
//! 4. [`tokio::net::TcpStream::connect`] — open a TCP connection (10 s timeout).
//! 5. TLS wrap — for HTTPS, perform the rustls handshake with explicit SNI.
//! 6. [`http::build_request`] — send the HTTP/1.1 GET request.
//! 7. [`read_all`] — read response bytes (30 s timeout, 50 MB size cap).
//! 8. [`http::parse_response`] — split headers, decode chunked body.
//! 9. Record HSTS from response `Strict-Transport-Security` header.
//! 10. Follow redirects (HTTP 3xx) up to [`MAX_REDIRECTS`] times.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::context::NetworkContext;
use crate::{FetchError, ParsedUrl, parse_url};
use crate::{dns, http};

/// Maximum number of HTTP redirects to follow before giving up.
const MAX_REDIRECTS: u8 = 5;

/// Maximum number of bytes to buffer from a response body.
/// Hard limit preventing RAM exhaustion from hostile or misconfigured servers.
const MAX_RESPONSE_BYTES: usize = 50 * 1024 * 1024; // 50 MB

/// Timeout for establishing a TCP connection to the remote server.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for the entire HTTP response read after the request is sent.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum permitted length of a `Location` redirect header value.
/// Oversized Location headers can be used in header-injection attacks.
const MAX_LOCATION_BYTES: usize = 4096;

/// Internal fetch entry point used by [`NetworkContext::fetch`].
///
/// Uses `Pin<Box<dyn Future>>` to allow recursive calls for redirect following
/// (E0733: recursive async functions require heap indirection).
pub fn fetch_with_context<'a>(
    ctx: &'a NetworkContext,
    url: String,
    depth: u8,
) -> Pin<Box<dyn Future<Output = Result<String, FetchError>> + Send + 'a>> {
    Box::pin(fetch_step(ctx, url, depth))
}

/// One step of the fetch pipeline.
async fn fetch_step(ctx: &NetworkContext, url: String, depth: u8) -> Result<String, FetchError> {
    if depth > MAX_REDIRECTS {
        return Err(FetchError::TooManyRedirects);
    }

    let mut parsed = parse_url(&url)?;

    // ── 1. HSTS enforcement ────────────────────────────────────────────────
    // If this host was previously seen with an HSTS header, force HTTPS
    // regardless of what the parsed URL says.
    {
        let hsts = ctx.hsts.lock().unwrap_or_else(|e| e.into_inner());
        if hsts.is_hsts(&parsed.host) && !parsed.is_https {
            parsed.is_https = true;
            parsed.port = "443".to_owned();
        }
    }

    // ── 2. DNS resolution ──────────────────────────────────────────────────
    let ip = dns::resolve(&ctx.resolver, &parsed.host).await?;

    // ── 3. TCP connection (with timeout) ───────────────────────────────────
    let port: u16 = parsed
        .port
        .parse()
        .map_err(|_| FetchError::Protocol(format!("invalid port '{}'", parsed.port)))?;

    let addr = SocketAddr::new(ip, port);
    let tcp = timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| FetchError::Timeout)?
        .map_err(|e| FetchError::Io(e.to_string()))?;

    // ── 4. Send request + read response (with timeout) ────────────────────
    let request = http::build_request(&parsed.host, &parsed.path);
    let raw_response = timeout(
        READ_TIMEOUT,
        send_and_read(ctx, tcp, &parsed, request.as_bytes()),
    )
    .await
    .map_err(|_| FetchError::Timeout)??;

    // ── 5. Parse response ─────────────────────────────────────────────────
    let response = http::parse_response(&raw_response)?;

    // ── 6. Record HSTS policy from response headers ───────────────────────
    if parsed.is_https {
        if let Some(sts_value) = &response.hsts_header {
            let mut hsts = ctx.hsts.lock().unwrap_or_else(|e| e.into_inner());
            hsts.record_from_header(&parsed.host, sts_value);
        }
    }

    // ── 7. Handle redirects ───────────────────────────────────────────────
    match response.status {
        200..=299 => Ok(response.body),
        301 | 302 | 303 | 307 | 308 => {
            let location = response.location.ok_or_else(|| {
                FetchError::Protocol(format!(
                    "HTTP {} redirect from '{url}' had no Location header",
                    response.status
                ))
            })?;

            // SECURITY: Cap Location header length to prevent header injection attacks.
            if location.len() > MAX_LOCATION_BYTES {
                return Err(FetchError::Protocol(format!(
                    "redirect Location header exceeds maximum length ({MAX_LOCATION_BYTES} bytes)"
                )));
            }

            // Resolve relative redirects against the current origin.
            let next_url =
                if location.starts_with("https://") || location.starts_with("http://") {
                    location
                } else if location.starts_with('/') {
                    let scheme = if parsed.is_https { "https" } else { "http" };
                    format!("{scheme}://{}:{}{}", parsed.host, parsed.port, location)
                } else {
                    let base = url.rsplitn(2, '/').last().unwrap_or(url.as_str());
                    format!("{base}/{location}")
                };

            // SECURITY: Re-parse the redirect URL through `parse_url` so that
            // HTTPS enforcement, userinfo rejection, and fragment stripping are
            // re-applied. This prevents a server from issuing an HTTP downgrade
            // via a redirect (e.g. `https://evil.com` → `http://evil.com/steal`).
            let next_parsed = parse_url(&next_url)?;
            if parsed.is_https && !next_parsed.is_https && !next_parsed.is_localhost {
                return Err(FetchError::Protocol(format!(
                    "redirect from HTTPS to HTTP is not allowed: '{next_url}'"
                )));
            }

            fetch_with_context(ctx, next_url, depth + 1).await
        }
        status => Err(FetchError::HttpStatus(status)),
    }
}

/// Send the request bytes and read back the full raw response.
///
/// Dispatches to the TLS or plain-TCP path depending on `parsed.is_https`.
async fn send_and_read(
    ctx: &NetworkContext,
    tcp: TcpStream,
    parsed: &ParsedUrl,
    request: &[u8],
) -> Result<Vec<u8>, FetchError> {
    if parsed.is_https {
        send_https(ctx, tcp, parsed, request).await
    } else {
        send_http(tcp, request).await
    }
}

/// Send a GET request over a TLS-wrapped TCP stream and read the raw response.
async fn send_https(
    ctx: &NetworkContext,
    tcp: TcpStream,
    parsed: &ParsedUrl,
    request: &[u8],
) -> Result<Vec<u8>, FetchError> {
    // rustls requires an owned ServerName<'static> for the TLS SNI extension.
    let server_name = rustls::pki_types::ServerName::try_from(parsed.host.as_str())
        .map_err(|e| FetchError::Tls(format!("invalid server name '{}': {e}", parsed.host)))?
        .to_owned();

    let mut tls_stream = ctx
        .connector
        .connect(server_name, tcp)
        .await
        .map_err(|e| FetchError::Tls(e.to_string()))?;

    tls_stream
        .write_all(request)
        .await
        .map_err(|e| FetchError::Io(e.to_string()))?;

    read_all(&mut tls_stream).await
}

/// Send a GET request over a plain TCP stream and read the raw response.
///
/// Only reachable for `http://localhost` URLs — all remote HTTP is upgraded
/// to HTTPS by [`parse_url`] before this point.
async fn send_http(mut tcp: TcpStream, request: &[u8]) -> Result<Vec<u8>, FetchError> {
    tcp.write_all(request)
        .await
        .map_err(|e| FetchError::Io(e.to_string()))?;

    read_all(&mut tcp).await
}

/// Read all bytes from an async stream until closed, with a hard size cap.
///
/// Uses a manual read loop instead of `read_to_end` to:
/// - Handle servers that close the TCP connection without sending a TLS
///   `close_notify` alert (common in practice; technically RFC 8446 §6.1
///   violation but used by Google, Cloudflare, and many CDNs).
/// - Enforce [`MAX_RESPONSE_BYTES`] to prevent RAM exhaustion from hostile
///   or misconfigured servers streaming unlimited data.
///
/// rustls surfaces missing `close_notify` as `std::io::ErrorKind::UnexpectedEof`,
/// which we catch and treat as a clean end-of-stream.
async fn read_all<S: AsyncReadExt + Unpin>(stream: &mut S) -> Result<Vec<u8>, FetchError> {
    let mut buf = Vec::with_capacity(64 * 1024);
    let mut tmp = [0u8; 8192];

    loop {
        match stream.read(&mut tmp).await {
            Ok(0) => break, // clean EOF
            Ok(n) => {
                // SECURITY: Enforce response size cap to prevent RAM exhaustion.
                if buf.len() + n > MAX_RESPONSE_BYTES {
                    return Err(FetchError::ResponseTooLarge);
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break, // no close_notify
            Err(e) => return Err(FetchError::Io(e.to_string())),
        }
    }

    Ok(buf)
}
