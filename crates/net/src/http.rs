//! HTTP/1.1 request formatting and response parsing for Ferrum.
//!
//! This module handles the text-level protocol: building GET request strings
//! and parsing response headers and bodies. It does not touch sockets or TLS.
//!
//! # Outbound headers
//!
//! Every request includes `Sec-Fetch-Site`, `Sec-Fetch-Mode`, and
//! `Sec-Fetch-Dest` headers per the W3C Fetch Metadata specification.
//! These declare that the request is a top-level browser navigation,
//! not a sub-resource or cross-origin script-triggered fetch. Servers
//! can use these to enforce their own CSRF-prevention policies.
//!
//! `Referer` is deliberately never included. Including it would leak
//! the user's previous URL to the server (see `RULES-03-privacy.md`).
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
    /// Value of the `Strict-Transport-Security` header, if present.
    ///
    /// Only present on HTTPS responses. Used by the caller to update
    /// the session [`HstsStore`][crate::hsts::HstsStore].
    pub hsts_header: Option<String>,
    /// The response body decoded to UTF-8 (lossily — invalid bytes become U+FFFD).
    pub body: String,
}

/// Format an HTTP/1.1 GET request string.
///
/// Uses `Connection: close` so the server shuts the connection after the
/// response, allowing [`super::client`] to read until EOF rather than
/// needing to parse `Content-Length` for every response.
///
/// # TODO(#7): Connection reuse
/// `Connection: close` makes every fetch pay full DNS+TCP+TLS cost.
/// Switch to `Connection: keep-alive` before sub-resource loading.
///
/// # TODO(#11): Sec-Fetch-Site context
/// `Sec-Fetch-Site: none` is correct only for top-level user navigation.
/// Sub-resource loads (CSS, JS, images) need `same-origin`, `same-site`,
/// or `cross-site`. Requires a FetchContext metadata struct passed through
/// `build_request` when the resource-loading layer is added.
///
/// # Security headers
///
/// - `Sec-Fetch-Site: none` — request is user-initiated (typed URL/bookmark),
///   not triggered by a third-party script.
/// - `Sec-Fetch-Mode: navigate` — this is a top-level navigation.
/// - `Sec-Fetch-Dest: document` — the destination is an HTML document.
/// - `Referer` is **never** included — its absence prevents URL leakage per
///   `RULES-03-privacy.md`.
pub fn build_request(host: &str, path: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n\
         Accept-Encoding: identity\r\n\
         Accept-Language: en-US,en;q=0.5\r\n\
         Connection: close\r\n\
         Sec-Fetch-Site: none\r\n\
         Sec-Fetch-Mode: navigate\r\n\
         Sec-Fetch-Dest: document\r\n\
         User-Agent: Ferrum/0.1 (privacy-first; +https://github.com/ferrum-browser)\r\n\
         \r\n"
    )
}

/// Parse a raw HTTP response (headers + body) into a [`Response`].
///
/// Uses [`httparse`] for robust header parsing. Handles both
/// `Content-Length` and `Transfer-Encoding: chunked` body framing.
///
/// Extracts `Location` (for redirect following) and `Strict-Transport-Security`
/// (for HSTS recording) from the response headers.
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

    // REASON: _header_bytes kept as a named slice for debugger inspection during development.
    // It makes the raw header section visible when stepping through parse_response() in a
    // debugger without needing to manually compute the offset into `bytes`.
    // Do not remove — zero runtime cost (no allocation, just a slice reference).
    let _header_bytes = &bytes[..header_end];
    let raw_body = &bytes[header_end + 4..];

    // SECURITY: Use 128 headers — 64 was the original cap. A server could
    // send 64 junk headers before the real Strict-Transport-Security header,
    // causing it to be silently dropped and HSTS never recorded. 128 raises
    // the bar significantly without unbounded memory use.
    let mut parsed_headers = [httparse::EMPTY_HEADER; 128];
    let mut response = httparse::Response::new(&mut parsed_headers);

    // BUGFIX: httparse returns Ok(Status::Partial) — not Err — when the input
    // is incomplete. We must check the Status enum, not just the Result.
    // Passing header_bytes without the trailing \r\n\r\n (which we already
    // stripped above) can cause Partial if httparse needs the terminator.
    // We pass the full buffer including the separator to avoid this.
    let header_buf = &bytes[..header_end + 4]; // include the \r\n\r\n
    match response
        .parse(header_buf)
        .map_err(|e| FetchError::Protocol(format!("HTTP response header parse error: {e}")))?
    {
        httparse::Status::Partial => {
            return Err(FetchError::Protocol(
                "HTTP response headers incomplete (Status::Partial)".into(),
            ));
        }
        httparse::Status::Complete(_) => {} // all good
    }

    let status = response
        .code
        .ok_or_else(|| FetchError::Protocol("HTTP response has no status code".into()))?;

    /// Extract the first header value matching `name` (case-insensitive) as a UTF-8 string.
    fn find_header<'a>(headers: &[httparse::Header<'a>], name: &str) -> Option<String> {
        headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .and_then(|h| std::str::from_utf8(h.value).ok())
            .map(str::trim)
            .map(ToOwned::to_owned)
    }

    let location = find_header(response.headers, "location");
    let hsts_header = find_header(response.headers, "strict-transport-security");

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
        hsts_header,
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
