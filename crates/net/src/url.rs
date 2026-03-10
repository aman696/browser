//! URL parsing for the Ferrum networking layer.
//!
//! Implements a minimal URL parser sufficient for HTTP/HTTPS requests.
//! Only `http://` and `https://` schemes are supported. Other schemes
//! (e.g. `ftp://`, `file://`) are rejected with [`UrlError::UnsupportedScheme`].

/// A parsed URL broken into its constituent parts.
///
/// `ParsedUrl` stores only the information the network layer needs to open
/// a TCP connection and send an HTTP request. It does not attempt to model
/// the full URL spec (query strings, fragments, credentials) — those will
/// be added as the HTTP implementation matures.
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

    /// The TCP port to connect to, e.g. `"443"` or `"80"`.
    ///
    /// Derived from the URL's explicit port if present, otherwise inferred
    /// from the scheme: `"443"` for HTTPS, `"80"` for HTTP.
    pub port: String,

    /// The path portion of the URL, always starting with `/`.
    ///
    /// Defaults to `"/"` when the URL contains no path component.
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
}

/// Parse a raw URL string into a [`ParsedUrl`].
///
/// Handles three forms:
/// - `https://host[:port][/path]`
/// - `http://host[:port][/path]`
/// - `host[/path]` — no scheme, assumed HTTPS
///
/// # Errors
///
/// Returns [`UrlError::Empty`] if `input` is empty.
/// Returns [`UrlError::UnsupportedScheme`] if a scheme other than
/// `http://` or `https://` is present.
///
/// # HTTPS enforcement
///
/// Ferrum enforces HTTPS for all non-localhost traffic. If the caller
/// provides an `http://` URL and the host is not localhost, the returned
/// `ParsedUrl` will have `is_https = true` and `port = "443"`. This mirrors
/// the HTTPS-upgrade logic that `NetworkContext` will apply at the fetch layer.
pub fn parse_url(input: &str) -> Result<ParsedUrl, UrlError> {
    if input.is_empty() {
        return Err(UrlError::Empty);
    }

    // Detect scheme by looking for "://". If present, validate it.
    // If absent, default to HTTPS.
    let (is_https, rest) = if let Some(after) = input.strip_prefix("https://") {
        (true, after)
    } else if let Some(after) = input.strip_prefix("http://") {
        (false, after)
    } else if input.contains("://") {
        // Has :// but not http or https — e.g. ftp://, file://
        return Err(UrlError::UnsupportedScheme);
    } else if let Some(colon_pos) = input.find(':') {
        // Has ':' but not '://'. Distinguish two cases:
        //   - Unsupported scheme: `data:text/html,...`, `javascript:void(0)`, `mailto:user@host`
        //     → the character right after ':' is NOT a digit.
        //   - Host with explicit port: `localhost:8080`, `example.com:443`
        //     → the character right after ':' IS a digit.
        let after_colon = &input[colon_pos + 1..];
        if !after_colon.starts_with(|c: char| c.is_ascii_digit()) {
            return Err(UrlError::UnsupportedScheme);
        }
        // Port detected — treat as a schemeless URL with an explicit port.
        // Default to HTTPS; the is_localhost + upgrade logic below will adjust.
        (true, input)
    } else {
        // No colon at all — plain host or host/path, default to HTTPS.
        (true, input)
    };

    // Split host[:port] from the path at the first '/'.
    let (host_and_port, path) = match rest.find('/') {
        Some(slash) => (&rest[..slash], rest[slash..].to_owned()),
        None => (rest, "/".to_owned()),
    };

    // Split the optional port off the host at the last ':'.
    // We look for ':' to handle the common host:port pattern.
    let (host, port) = match host_and_port.find(':') {
        Some(colon) => (
            host_and_port[..colon].to_owned(),
            host_and_port[colon + 1..].to_owned(),
        ),
        None => {
            // No explicit port — infer from scheme.
            let default_port = if is_https { "443" } else { "80" };
            (host_and_port.to_owned(), default_port.to_owned())
        }
    };

    let is_localhost = host == "localhost" || host == "127.0.0.1";

    // SECURITY: Ferrum enforces HTTPS for all non-localhost connections.
    // If the user typed an http:// URL for a remote host, upgrade it silently
    // here. The privacy warning system will notify them when they visit an
    // HTTP-only site that cannot be upgraded.
    let (is_https, port) = if !is_localhost && !is_https {
        (true, "443".to_owned())
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
        assert_eq!(result.port, "443");
        assert_eq!(result.path, "/index.html");
        assert!(result.is_https);
        assert!(!result.is_localhost);
    }

    #[test]
    fn test_parse_url_http_remote_upgraded_to_https() {
        // SECURITY: HTTP to a remote host is silently upgraded to HTTPS.
        let result = parse_url("http://example.com").unwrap();
        assert!(result.is_https);
        assert_eq!(result.port, "443");
    }

    #[test]
    fn test_parse_localhost_http_not_upgraded() {
        // Localhost HTTP is allowed without upgrade for developer workflows.
        let result = parse_url("http://localhost:8080/api").unwrap();
        assert!(!result.is_https);
        assert!(result.is_localhost);
        assert_eq!(result.port, "8080");
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
}
