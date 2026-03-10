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
//! # hickory-resolver 0.25 API note
//!
//! In 0.25 the constructor changed from `TokioResolver::tokio(config, opts)`
//! to a builder pattern: `TokioResolver::builder_with_config(config, provider).build()`.
//! `TokioResolver` is now a type alias for `Resolver<TokioConnectionProvider>`.

use std::net::IpAddr;

use hickory_resolver::TokioResolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use hickory_resolver::name_server::TokioConnectionProvider;

use crate::FetchError;

/// Resolve a hostname to an IP address using DNS-over-HTTPS.
///
/// If `host` is already a numeric IP address (v4 or v6), it is returned
/// directly without a DNS query. Otherwise a DoH query is made to
/// Cloudflare 1.1.1.1.
///
/// # Errors
///
/// Returns [`FetchError::Dns`] if the hostname cannot be resolved.
pub async fn resolve(host: &str) -> Result<IpAddr, FetchError> {
    // Short-circuit for literal IP addresses — no DNS needed.
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(ip);
    }

    // hickory-resolver 0.25 builder API:
    //   TokioResolver::builder_with_config(ResolverConfig, ConnectionProvider)
    //     -> ResolverBuilder  ->  .build() -> Resolver<TokioConnectionProvider>
    //
    // ResolverConfig::cloudflare_https() requires the `https-aws-lc-rs` feature.
    // TokioConnectionProvider::default() wires up the tokio async runtime.
    let resolver: TokioResolver = TokioResolver::builder_with_config(
        ResolverConfig::cloudflare_https(),
        TokioConnectionProvider::default(),
    )
    .with_options(ResolverOpts::default())
    .build();

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
