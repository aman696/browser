//! HTTP/1.1 request formatting and response parsing for Ferrum.
//!
//! This module handles the text-level protocol: building GET request strings
//! and parsing response headers and bodies. It does not touch sockets or TLS.
//!
//! # Chunked transfer encoding
//!
//! Many production servers (including Google, GitHub, Cloudflare CDN) respond
//! with `Transfer-Encoding: chunked` when the content length is not known
//! upfront. This module decodes chunked bodies correctly before returning them
//! to the caller as a plain `String`.

use crate::FetchError;

/// A parsed HTTP response.
pub struct Response {
    /// HTTP status code, e.g. 200, 301, 404.
    pub status: u16,
    /// Value of the `Location` header, if present (used for redirects).
    pub location: Option<String>,
    /// The response body decoded to UTF-8 (lossily — invalid bytes become U+FFFD).
    pub body: String,
}

/// Format an HTTP/1.1 GET request string.
///
/// Uses `Connection: close` so the server shuts the connection after the
/// response, allowing [`super::client`] to read until EOF rather than
/// having to parse Content-Length for every response.
///
/// `Accept-Encoding: identity` disables compression, so we receive the
/// raw body bytes without needing a decompressor. Compression support
/// will be added in a later session with `brotli` / `flate2`.
pub fn build_request(host: &str, path: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n\
         Accept-Encoding: identity\r\n\
         Accept-Language: en-US,en;q=0.5\r\n\
         Connection: close\r\n\
         User-Agent: Ferrum/0.1 (privacy-first; +https://github.com/ferrum-browser)\r\n\
         \r\n"
    )
}

/// Parse a raw HTTP response (headers + body) into a [`Response`].
///
/// Uses [`httparse`] for robust header parsing. Handles both
/// `Content-Length` and `Transfer-Encoding: chunked` body framing.
///
/// # Errors
///
/// Returns [`FetchError::Protocol`] if the response is structurally invalid
/// (no header/body separator, unrecognisable status line, etc.).
pub fn parse_response(bytes: &[u8]) -> Result<Response, FetchError> {
    // Find the blank line separating headers from body.
    let header_end = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| FetchError::Protocol("HTTP response has no header/body separator".into()))?;

    let header_bytes = &bytes[..header_end];
    let raw_body = &bytes[header_end + 4..];

    // Parse headers with httparse.
    let mut parsed_headers = [httparse::EMPTY_HEADER; 64];
    let mut response = httparse::Response::new(&mut parsed_headers);
    response
        .parse(header_bytes)
        .map_err(|e| FetchError::Protocol(format!("HTTP response header parse error: {e}")))?;

    let status = response
        .code
        .ok_or_else(|| FetchError::Protocol("HTTP response has no status code".into()))?;

    // Extract the Location header (lowercase search per RFC 7230 §3.2).
    let location = response
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("location"))
        .and_then(|h| std::str::from_utf8(h.value).ok())
        .map(str::trim)
        .map(ToOwned::to_owned);

    // Check for chunked transfer encoding.
    let is_chunked = response.headers.iter().any(|h| {
        h.name.eq_ignore_ascii_case("transfer-encoding")
            && std::str::from_utf8(h.value)
                .map(|v| v.to_ascii_lowercase().contains("chunked"))
                .unwrap_or(false)
    });

    let body_bytes = if is_chunked {
        decode_chunked(raw_body)?
    } else {
        raw_body.to_vec()
    };

    let body = String::from_utf8_lossy(&body_bytes).into_owned();

    Ok(Response {
        status,
        location,
        body,
    })
}

/// Decode a chunked transfer-encoded body into a flat byte vector.
///
/// Each chunk is `<hex-size>\r\n<data>\r\n`. The stream ends with `0\r\n\r\n`.
/// Chunk extensions (`;name=value` after the size) are ignored per §4.1.1.
///
/// # Errors
///
/// Returns [`FetchError::Protocol`] on a malformed chunk structure.
fn decode_chunked(data: &[u8]) -> Result<Vec<u8>, FetchError> {
    let mut result = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        // Find the end of the chunk-size line.
        let line_end = data[pos..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| {
                FetchError::Protocol("chunked encoding: missing CRLF after chunk size".into())
            })?;

        let size_line = std::str::from_utf8(&data[pos..pos + line_end]).map_err(|_| {
            FetchError::Protocol("chunked encoding: chunk size is not valid UTF-8".into())
        })?;

        // Strip optional chunk extensions (e.g. `a;name=value`).
        let size_hex = size_line.split(';').next().unwrap_or("").trim();

        let chunk_size = usize::from_str_radix(size_hex, 16).map_err(|_| {
            FetchError::Protocol(format!("chunked encoding: invalid hex size '{size_hex}'"))
        })?;

        pos += line_end + 2; // skip size line + CRLF

        if chunk_size == 0 {
            // Terminating chunk — ignore optional trailers.
            break;
        }

        let chunk_end = pos + chunk_size;
        if chunk_end > data.len() {
            return Err(FetchError::Protocol(format!(
                "chunked encoding: chunk of {chunk_size} bytes extends past end of data"
            )));
        }

        result.extend_from_slice(&data[pos..chunk_end]);
        pos = chunk_end + 2; // skip chunk data + CRLF
    }

    Ok(result)
}
