//! DNS resolution for Ferrum using [`hickory_resolver`] over HTTPS (DoH).
//!
//! The system resolver (`getaddrinfo`) is banned per `RULES-04-networking.md`
//! because it uses plaintext UDP/TCP DNS, leaking every domain you visit to
//! your ISP and any observer on the local network.
//!
//! This module configures a [`hickory_resolver::TokioResolver`] to use
//! Cloudflare's 1.1.1.1 DNS-over-HTTPS endpoint as the default. All DNS
//! queries are encrypted inside TLS, indistinguishable from normal HTTPS
//! traffic to a network observer.
//!
//! # Why Cloudflare 1.1.1.1?
//!
//! - Lowest global latency of any public DoH resolver (avg ~14 ms).
//! - Audited annually by KPMG; does not sell query logs.
//! - Future versions of Ferrum will allow the user to configure their own resolver.
//!
//! # hickory-resolver 0.25 API note
//!
//! In 0.25 the constructor changed from `TokioResolver::tokio(config, opts)`
//! to a builder pattern: `TokioResolver::builder_with_config(config, provider).build()`.
//! `TokioResolver` is now a type alias for `Resolver<TokioConnectionProvider>`.

use std::net::IpAddr;
use std::time::Duration;

use hickory_resolver::TokioResolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use hickory_resolver::name_server::TokioConnectionProvider;

use crate::FetchError;

/// Maximum number of resolved hosts to keep in the in-memory DNS cache.
const DNS_CACHE_SIZE: usize = 32;

/// Timeout for each individual DNS query (not the entire lookup chain).
const DNS_QUERY_TIMEOUT: Duration = Duration::from_secs(5);

/// Build a [`TokioResolver`] configured for DNS-over-HTTPS via Cloudflare.
///
/// The resolver is built once and stored in [`NetworkContext`] for reuse
/// across all fetch calls. Reusing the resolver means:
/// - Previously resolved hostnames are served from the in-memory cache.
/// - The DoH TLS session to 1.1.1.1 can be kept alive across requests.
///
/// # Privacy guarantees
///
/// - `ResolverConfig::cloudflare_https()` sends queries only to 1.1.1.1 over DoH.
/// - No system resolver fallback is configured.
/// - No speculative prefetch queries are issued.
#[must_use]
pub fn build_resolver() -> TokioResolver {
    let mut opts = ResolverOpts::default();
    // Keep at most 32 recently-resolved hosts in memory. This avoids
    // re-resolving stable CDN/server addresses on every page load.
    opts.cache_size = DNS_CACHE_SIZE;
    // Hard timeout per DNS query. Prevents a slow DoH server from stalling
    // the browser indefinitely.
    opts.timeout = DNS_QUERY_TIMEOUT;
    // Drop intermediate CNAME chain records — only the final address matters.
    opts.preserve_intermediates = false;

    TokioResolver::builder_with_config(
        ResolverConfig::cloudflare_https(),
        TokioConnectionProvider::default(),
    )
    .with_options(opts)
    .build()
}

/// Resolve a hostname to an IP address using the provided DoH resolver.
///
/// If `host` is already a numeric IP address (v4 or v6), it is returned
/// directly without a DNS query. Otherwise a DoH query is made via the
/// resolver (which may be served from cache on repeat lookups).
///
/// # Errors
///
/// Returns [`FetchError::Dns`] if the hostname cannot be resolved.
pub async fn resolve(resolver: &TokioResolver, host: &str) -> Result<IpAddr, FetchError> {
    let is_localhost_name = crate::url::is_localhost_host(host);

    // Short-circuit for literal IP addresses — no DNS needed.
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(ip) && !is_localhost_name {
            return Err(FetchError::Dns(
                "DNS resolved to private/reserved IP address — possible SSRF".into(),
            ));
        }
        return Ok(ip);
    }

    let lookup = resolver
        .lookup_ip(host)
        .await
        .map_err(|e| FetchError::Dns(e.to_string()))?;

    // Prefer IPv4 to maximise compatibility with servers that do not yet
    // support IPv6 dual-stack. Fall back to any address if no IPv4 is found.
    let ip = lookup
        .iter()
        .find(|ip: &IpAddr| ip.is_ipv4())
        .or_else(|| lookup.iter().next())
        .ok_or_else(|| FetchError::Dns(format!("no IP addresses found for '{host}'")))?;

    if is_private_ip(ip) && !is_localhost_name {
        return Err(FetchError::Dns(
            "DNS resolved to private/reserved IP address — possible SSRF".into(),
        ));
    }

    Ok(ip)
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            ipv4.is_loopback()   // 127.0.0.0/8
                || ipv4.is_unspecified() // 0.0.0.0/8
                || octets[0] == 0        // 0.0.0.0/8 (RFC 1122 — "this" network)
                || octets[0] == 10       // 10.0.0.0/8 (RFC 1918 private)
                || (octets[0] == 172 && (16..=31).contains(&octets[1])) // 172.16.0.0/12 (RFC 1918)
                || (octets[0] == 192 && octets[1] == 168) // 192.168.0.0/16 (RFC 1918)
                || (octets[0] == 169 && octets[1] == 254) // 169.254.0.0/16 link-local (RFC 3927)
                // SECURITY: RFC 6598 — Carrier-Grade NAT (100.64.0.0/10). ISPs use this range
                // internally; a DNS record resolving here could reach ISP infrastructure.
                || (octets[0] == 100 && (64..=127).contains(&octets[1])) // 100.64.0.0/10 CGNAT
                // SECURITY: IETF protocol/documentation reserved ranges (RFC 5737, RFC 6890).
                // These should never appear as real server addresses; block to prevent confusion.
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0) // 192.0.0.0/24
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2) // 192.0.2.0/24 TEST-NET-1
                || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100) // 198.51.100.0/24 TEST-NET-2
                || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113) // 203.0.113.0/24 TEST-NET-3
        }
        IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();
            // SECURITY: IPv4-mapped IPv6 addresses (::ffff:x.x.x.x, i.e. ::ffff:0:0/96)
            // are returned as IpAddr::V6 by hickory_resolver but the Linux kernel
            // transparently routes them to the IPv4 host when IPV6_V6ONLY=0 (the default).
            // Without this check an attacker can publish an AAAA record of ::ffff:192.168.1.1
            // and bypass the IPv4 private-range guard entirely — classic SSRF via protocol confusion.
            // We normalise to IPv4 and recurse so the full IPv4 range-table is applied.
            if let Some(ipv4) = ipv6.to_ipv4_mapped() {
                return is_private_ip(IpAddr::V4(ipv4));
            }
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || (segments[0] & 0xfe00) == 0xfc00 // ULA fc00::/7
                || (segments[0] & 0xffc0) == 0xfe80 // link-local fe80::/10
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_private_ip() {
        assert!(is_private_ip("127.0.0.1".parse().unwrap()));
        assert!(is_private_ip("10.0.0.1".parse().unwrap()));
        assert!(is_private_ip("192.168.1.1".parse().unwrap()));
        assert!(is_private_ip("169.254.169.254".parse().unwrap()));
        assert!(is_private_ip("172.16.0.1".parse().unwrap()));
        assert!(is_private_ip("172.31.255.255".parse().unwrap()));

        // Edge cases around the 172.16/12 block
        assert!(!is_private_ip("172.15.255.255".parse().unwrap())); // Just outside
        assert!(!is_private_ip("172.32.0.0".parse().unwrap())); // Just outside

        assert!(!is_private_ip("1.1.1.1".parse().unwrap()));
        assert!(!is_private_ip("93.184.216.34".parse().unwrap()));
        assert!(is_private_ip("::1".parse().unwrap()));

        // CGNAT 100.64.0.0/10 (RFC 6598) — ISP-internal range, must be blocked.
        assert!(is_private_ip("100.64.0.1".parse().unwrap()));
        assert!(is_private_ip("100.127.255.255".parse().unwrap()));
        assert!(!is_private_ip("100.63.255.255".parse().unwrap())); // just outside
        assert!(!is_private_ip("100.128.0.0".parse().unwrap())); // just outside

        // IETF reserved / documentation ranges (RFC 5737, RFC 6890).
        assert!(is_private_ip("192.0.0.1".parse().unwrap()));
        assert!(is_private_ip("192.0.2.1".parse().unwrap()));
        assert!(is_private_ip("198.51.100.1".parse().unwrap()));
        assert!(is_private_ip("203.0.113.1".parse().unwrap()));

        // SECURITY: IPv4-mapped IPv6 addresses must be treated as their IPv4 equivalent.
        // An attacker can serve ::ffff:192.168.1.1 as an AAAA record; without this check
        // it would bypass the IPv4 private-range guard and allow SSRF to internal hosts.
        assert!(is_private_ip("::ffff:192.168.1.1".parse().unwrap()));
        assert!(is_private_ip("::ffff:10.0.0.1".parse().unwrap()));
        assert!(is_private_ip("::ffff:127.0.0.1".parse().unwrap()));
        assert!(!is_private_ip("::ffff:1.1.1.1".parse().unwrap()));
    }

    #[tokio::test]
    async fn test_resolve_localhost() {
        let resolver = build_resolver();
        let ip = resolve(&resolver, "localhost").await.unwrap();
        assert_eq!(ip, "127.0.0.1".parse::<IpAddr>().unwrap());
    }
}
