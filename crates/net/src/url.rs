//! URL parsing for the Ferrum networking layer.
//!
//! Implements a minimal URL parser sufficient for HTTP/HTTPS requests.
//! Only `http://` and `https://` schemes are supported. Other schemes
//! (e.g. `ftp://`, `file://`) are rejected with [`UrlError::UnsupportedScheme`].
//!
//! # Security hardening
//!
//! - **Userinfo** (`user:pass@host`) is rejected — it is a phishing vector per WHATWG URL spec §5.1.
//! - **Fragment identifiers** (`#anchor`) are stripped — fragments must not be sent to servers
//!   (RFC 9110 §4.2.4) and often contain OAuth tokens or private state.
//! - **IPv6 Zone IDs** (`[::1%eth0]`) are rejected — they can probe local network interfaces.

/// A parsed URL broken into its constituent parts.
///
/// `ParsedUrl` stores only the information the network layer needs to open
/// a TCP connection and send an HTTP request. It does not attempt to model
/// the full URL spec (query strings, credentials) — those will be added as
/// the HTTP implementation matures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrl {
    /// Whether TLS should be used for this connection.
    ///
    /// `true` for `https://` or when no scheme is specified (Ferrum defaults
    /// to HTTPS). `false` only for explicit `http://` to localhost.
    pub is_https: bool,

    /// `true` when the host is `localhost` or `127.0.0.1`.
    ///
    /// Localhost connections are exempt from the HTTPS enforcement rule
    /// so that developers can test without a local TLS cert.
    pub is_localhost: bool,

    /// The host portion of the URL, e.g. `"example.com"` or `"localhost"`.
    pub host: String,

    /// The TCP port to connect to, e.g. `443` or `80`.
    ///
    /// Derived from the URL's explicit port if present, otherwise inferred
    /// from the scheme: `443` for HTTPS, `80` for HTTP.
    pub port: u16,

    /// The path and query portion of the URL, always starting with `/`.
    ///
    /// Defaults to `"/"` when the URL contains no path component.
    /// Fragment identifiers (`#...`) are stripped before storage.
    pub path: String,
}

/// Errors that can occur when parsing a URL.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UrlError {
    /// The URL string is completely empty.
    #[error("URL must not be empty")]
    Empty,

    /// The scheme is present but is not `http` or `https`.
    ///
    /// Ferrum only handles HTTP/HTTPS. Other schemes (ftp, file, data, etc.)
    /// are not supported and will produce this error.
    #[error("unsupported scheme in URL (only http and https are supported)")]
    UnsupportedScheme,

    /// The URL contains a userinfo component (`user:pass@host`).
    ///
    /// Userinfo in URLs is rejected per the WHATWG URL specification because
    /// it is a well-documented phishing vector — `https://bank.com@evil.com/`
    /// looks like it visits `bank.com` but actually visits `evil.com`.
    #[error("URLs with userinfo (user:pass@host) are not allowed")]
    UserInfoNotAllowed,

    /// The host component is structurally invalid.
    ///
    /// Covers IPv6 Zone IDs (`[::1%eth0]`) which can probe local network
    /// interfaces, and other malformed host strings.
    #[error("invalid host in URL: {0}")]
    InvalidHost(String),
}

/// Parse a raw URL string into a [`ParsedUrl`].
///
/// Handles three forms:
/// - `https://host[:port][/path][#fragment]`
/// - `http://host[:port][/path][#fragment]`
/// - `host[/path]` — no scheme, assumed HTTPS
///
/// # Security
///
/// - Fragments (`#...`) are stripped and never stored or sent to servers.
/// - Userinfo (`user:pass@host`) is rejected as a phishing risk.
/// - IPv6 Zone IDs (`%` inside `[]`) are rejected to prevent local network probing.
/// - Remote `http://` URLs are silently upgraded to HTTPS.
///
/// # Errors
///
/// | Error | Condition |
/// |-------|-----------|
/// | [`UrlError::Empty`] | Empty input |
/// | [`UrlError::UnsupportedScheme`] | Scheme other than `http`/`https` |
/// | [`UrlError::UserInfoNotAllowed`] | `@` found in the host portion |
/// | [`UrlError::InvalidHost`] | IPv6 Zone ID (`%` inside `[]`) |
pub fn parse_url(input: &str) -> Result<ParsedUrl, UrlError> {
    if input.is_empty() {
        return Err(UrlError::Empty);
    }

    // ── 1. Detect and validate scheme ─────────────────────────────────────
    let (is_https, rest) = if let Some(after) = input.strip_prefix("https://") {
        (true, after)
    } else if let Some(after) = input.strip_prefix("http://") {
        (false, after)
    } else if input.contains("://") {
        // Has :// but not http or https — e.g. ftp://, file://
        return Err(UrlError::UnsupportedScheme);
    } else if let Some(colon_pos) = input.find(':') {
        let after_colon = &input[colon_pos + 1..];
        if !after_colon.starts_with(|c: char| c.is_ascii_digit()) {
            // `data:...`, `javascript:...`, `mailto:...` — non-digit after ':'
            return Err(UrlError::UnsupportedScheme);
        }
        // Port detected — treat as a schemeless URL with an explicit port.
        (true, input)
    } else {
        // No colon at all — plain host or host/path, default to HTTPS.
        (true, input)
    };

    // ── 2. Strip fragment identifier (`#...`) ─────────────────────────────
    // Fragments are client-side only and MUST NOT be sent to servers
    // (RFC 9110 §4.2.4). They often contain OAuth tokens or private app state.
    let rest = rest.split('#').next().unwrap_or(rest);

    // ── 3. Split host[:port] from the path at the first '/' ───────────────
    let (host_and_port, path) = match rest.find('/') {
        Some(slash) => (&rest[..slash], rest[slash..].to_owned()),
        None => (rest, "/".to_owned()),
    };

    // ── 4. Security: reject userinfo (`user:pass@host`) ───────────────────
    // `@` in the host portion is a phishing vector. `bank.com@evil.com`
    // looks legitimate but routes to `evil.com`. All major browsers reject
    // this pattern per the WHATWG URL spec §5.1.
    if host_and_port.contains('@') {
        return Err(UrlError::UserInfoNotAllowed);
    }

    // ── 5. Security: reject IPv6 Zone IDs ─────────────────────────────────
    // `[fe80::1%eth0]` has a `%` inside square brackets, indicating a Zone ID.
    // Zone IDs specify a network interface and can probe local interfaces that
    // should not be accessible from the browser (RFC 6874).
    if host_and_port.starts_with('[') {
        // We are looking at an IPv6 literal address.
        if let Some(close) = host_and_port.find(']') {
            let ipv6_literal = &host_and_port[1..close];
            if ipv6_literal.contains('%') {
                return Err(UrlError::InvalidHost(
                    "IPv6 Zone IDs are not allowed".to_owned(),
                ));
            }
        } else {
            return Err(UrlError::InvalidHost(
                "unclosed IPv6 bracket in host".to_owned(),
            ));
        }
    }

    // ── 6. Split host from optional port ──────────────────────────────────
    let (host, port) = match host_and_port.find(':') {
        Some(colon) => (
            host_and_port[..colon].to_owned(),
            host_and_port[colon + 1..]
                .parse::<u16>()
                .map_err(|_| UrlError::InvalidHost(format!(
                    "invalid port '{}'", &host_and_port[colon + 1..]
                )))?,
        ),
        None => {
            let default_port: u16 = if is_https { 443 } else { 80 };
            (host_and_port.to_owned(), default_port)
        }
    };

    let is_localhost = host == "localhost" || host == "127.0.0.1";

    // ── 7. HTTPS enforcement ───────────────────────────────────────────────
    // Silently upgrade all remote HTTP to HTTPS. The privacy warning system
    // will surface this to the user when implemented in `crates/security`.
    let (is_https, port) = if !is_localhost && !is_https {
        (true, 443u16)
    } else {
        (is_https, port)
    };

    Ok(ParsedUrl {
        is_https,
        is_localhost,
        host,
        port,
        path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_url_https() {
        let result = parse_url("https://example.com/index.html").unwrap();
        assert_eq!(result.host, "example.com");
        assert_eq!(result.port, 443);
        assert_eq!(result.path, "/index.html");
        assert!(result.is_https);
        assert!(!result.is_localhost);
    }

    #[test]
    fn test_parse_url_http_remote_upgraded_to_https() {
        // SECURITY: HTTP to a remote host is silently upgraded to HTTPS.
        let result = parse_url("http://example.com").unwrap();
        assert!(result.is_https);
        assert_eq!(result.port, 443);
    }

    #[test]
    fn test_parse_localhost_http_not_upgraded() {
        // Localhost HTTP is allowed without upgrade for developer workflows.
        let result = parse_url("http://localhost:8080/api").unwrap();
        assert!(!result.is_https);
        assert!(result.is_localhost);
        assert_eq!(result.port, 8080);
        assert_eq!(result.path, "/api");
    }

    #[test]
    fn test_parse_url_no_scheme_defaults_to_https() {
        let result = parse_url("example.com/page").unwrap();
        assert!(result.is_https);
        assert_eq!(result.host, "example.com");
        assert_eq!(result.path, "/page");
    }

    #[test]
    fn test_parse_url_no_path_defaults_to_slash() {
        let result = parse_url("https://example.com").unwrap();
        assert_eq!(result.path, "/");
    }

    #[test]
    fn test_parse_url_empty_returns_error() {
        assert_eq!(parse_url(""), Err(UrlError::Empty));
    }

    #[test]
    fn test_parse_url_unsupported_scheme() {
        assert_eq!(
            parse_url("ftp://example.com"),
            Err(UrlError::UnsupportedScheme)
        );
    }

    #[test]
    fn test_parse_127_0_0_1_is_localhost() {
        let result = parse_url("http://127.0.0.1:3000/").unwrap();
        assert!(result.is_localhost);
        assert!(!result.is_https);
    }

    // ── Security hardening tests ───────────────────────────────────────────

    #[test]
    fn test_userinfo_is_rejected() {
        // SECURITY: `user:pass@host` is a phishing vector — looks like bank.com,
        // routes to evil.com. Must be rejected per WHATWG URL spec §5.1.
        assert_eq!(
            parse_url("https://user:pass@example.com"),
            Err(UrlError::UserInfoNotAllowed)
        );
        assert_eq!(
            parse_url("https://bank.com@evil.com/"),
            Err(UrlError::UserInfoNotAllowed)
        );
    }

    #[test]
    fn test_fragment_is_stripped() {
        // SECURITY: Fragments are client-side only and must never be sent
        // to the server. They often contain OAuth tokens.
        let result = parse_url("https://example.com/page#section").unwrap();
        assert_eq!(result.path, "/page");

        let result = parse_url("https://example.com/callback#access_token=secret").unwrap();
        assert_eq!(result.path, "/callback");
    }

    #[test]
    fn test_ipv6_zone_id_is_rejected() {
        // SECURITY: Zone IDs can probe local network interfaces.
        assert!(matches!(
            parse_url("https://[fe80::1%eth0]/"),
            Err(UrlError::InvalidHost(_))
        ));
    }

    #[test]
    fn test_unclosed_ipv6_bracket_is_rejected() {
        assert!(matches!(
            parse_url("https://[::1/path"),
            Err(UrlError::InvalidHost(_))
        ));
    }

    #[test]
    fn test_javascript_scheme_is_rejected() {
        assert_eq!(
            parse_url("javascript:void(0)"),
            Err(UrlError::UnsupportedScheme)
        );
    }

    #[test]
    fn test_data_url_is_rejected() {
        assert_eq!(
            parse_url("data:text/html,<h1>hi</h1>"),
            Err(UrlError::UnsupportedScheme)
        );
    }
}
