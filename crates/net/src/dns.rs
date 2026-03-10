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
    // Short-circuit for literal IP addresses — no DNS needed.
    if let Ok(ip) = host.parse::<IpAddr>() {
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

    Ok(ip)
}
