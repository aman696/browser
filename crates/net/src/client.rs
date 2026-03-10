//! HTTP/HTTPS fetch implementation — the real network plumbing.
//!
//! # Request pipeline
//!
//! 1. [`parse_url`] — validate and normalise the URL.
//! 2. [`dns::resolve`] — DoH lookup via Cloudflare 1.1.1.1.
//! 3. [`tokio::net::TcpStream::connect`] — open a TCP connection.
//! 4. [`tls::make_connector`] — wrap in TLS (for HTTPS).
//! 5. [`http::build_request`] — send the HTTP/1.1 GET request.
//! 6. Read all bytes until the server closes the connection.
//! 7. [`http::parse_response`] — split headers from body, decode chunked.
//! 8. Follow redirects (HTTP 3xx) up to [`MAX_REDIRECTS`] times.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::{FetchError, ParsedUrl, parse_url};
use crate::{dns, http, tls};

/// Maximum number of HTTP redirects to follow before giving up.
const MAX_REDIRECTS: u8 = 5;

/// Fetch a URL and return the response body as a UTF-8 string.
pub async fn fetch(url: &str) -> Result<String, FetchError> {
    fetch_inner(url.to_owned(), 0).await
}

/// Internal fetch with redirect depth tracking.
///
/// Recursive async functions must return `Pin<Box<dyn Future>>` so that
/// the compiler can use a fixed-size heap allocation for the future rather
/// than an infinitely-nested type (E0733).
fn fetch_inner(
    url: String,
    depth: u8,
) -> Pin<Box<dyn Future<Output = Result<String, FetchError>> + Send>> {
    Box::pin(async move { fetch_step(url, depth).await })
}

/// One step of the fetch pipeline — split out of `fetch_inner` so the
/// main body can be written as a normal async block.
async fn fetch_step(url: String, depth: u8) -> Result<String, FetchError> {
    if depth > MAX_REDIRECTS {
        return Err(FetchError::TooManyRedirects);
    }

    let parsed = parse_url(&url)?;

    // ── 1. DNS resolution ─────────────────────────────────────────────────
    let ip = dns::resolve(&parsed.host).await?;

    // ── 2. TCP connection ─────────────────────────────────────────────────
    let port: u16 = parsed
        .port
        .parse()
        .map_err(|_| FetchError::Protocol(format!("invalid port '{}'", parsed.port)))?;

    let addr = SocketAddr::new(ip, port);
    let tcp = TcpStream::connect(addr)
        .await
        .map_err(|e| FetchError::Io(e.to_string()))?;

    // ── 3. Send request + read response ───────────────────────────────────
    let request = http::build_request(&parsed.host, &parsed.path);
    let raw_response = if parsed.is_https {
        send_https(tcp, &parsed, request.as_bytes()).await?
    } else {
        send_http(tcp, request.as_bytes()).await?
    };

    // ── 4. Parse response ─────────────────────────────────────────────────
    let response = http::parse_response(&raw_response)?;

    // ── 5. Handle redirects ───────────────────────────────────────────────
    match response.status {
        200..=299 => Ok(response.body),
        301 | 302 | 303 | 307 | 308 => {
            let location = response.location.ok_or_else(|| {
                FetchError::Protocol(format!(
                    "HTTP {} redirect from '{url}' had no Location header",
                    response.status
                ))
            })?;

            // Resolve relative redirects: if Location is a path (e.g. `/login`)
            // rather than an absolute URL, prepend the original origin.
            let next_url = if location.starts_with("http://") || location.starts_with("https://") {
                location
            } else if location.starts_with('/') {
                let scheme = if parsed.is_https { "https" } else { "http" };
                format!("{scheme}://{}:{}{}", parsed.host, parsed.port, location)
            } else {
                // Relative path — resolve against the current URL's directory.
                let base = url.rsplitn(2, '/').last().unwrap_or(url.as_str());
                format!("{base}/{location}")
            };

            eprintln!("[ferrum::net] redirect {} → {next_url}", response.status);
            fetch_inner(next_url, depth + 1).await
        }
        status => Err(FetchError::HttpStatus(status)),
    }
}

/// Send a GET request over a TLS-wrapped TCP stream and read the raw response.
async fn send_https(
    tcp: TcpStream,
    parsed: &ParsedUrl,
    request: &[u8],
) -> Result<Vec<u8>, FetchError> {
    let connector = tls::make_connector()?;

    // rustls requires an owned ServerName<'static> for the TLS SNI extension.
    let server_name = rustls::pki_types::ServerName::try_from(parsed.host.as_str())
        .map_err(|e| FetchError::Tls(format!("invalid server name '{}': {e}", parsed.host)))?
        .to_owned();

    let mut tls_stream = connector
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

/// Read all bytes from an async stream until it is closed by the remote end.
///
/// Uses a manual read loop instead of `read_to_end` to handle servers that
/// close the TCP connection without sending a TLS `close_notify` alert.
/// This is technically a protocol violation (RFC 8446 §6.1) but extremely
/// common in practice — Google, Cloudflare, and many CDNs do it.
///
/// We sent `Connection: close` in the request, so the TCP EOF is the
/// correct stream terminator regardless of whether close_notify was sent.
/// rustls surfaces the missing close_notify as `std::io::ErrorKind::UnexpectedEof`,
/// which we catch and treat as a clean end-of-stream.
async fn read_all<S: AsyncReadExt + Unpin>(stream: &mut S) -> Result<Vec<u8>, FetchError> {
    let mut buf = Vec::with_capacity(64 * 1024);
    let mut tmp = [0u8; 8192];

    loop {
        match stream.read(&mut tmp).await {
            Ok(0) => break, // clean EOF
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break, // no close_notify
            Err(e) => return Err(FetchError::Io(e.to_string())),
        }
    }

    Ok(buf)
}
