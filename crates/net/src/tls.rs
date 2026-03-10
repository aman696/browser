//! TLS client configuration for Ferrum using [`rustls`] with the
//! `aws-lc-rs` cryptographic backend.
//!
//! Per `RULES-04-networking.md`:
//! - OpenSSL is **banned**. `rustls` is mandatory.
//! - The `aws-lc-rs` backend is mandatory. `ring` is not used.
//! - Certificate errors **hard-fail**. There is no click-through.
//!
//! Root certificate trust is provided by [`webpki_roots`], which ships
//! Mozilla's curated list of trusted CA certificates as a compiled-in
//! Rust constant. There is no dependency on the operating system's
//! certificate store, ensuring consistent behaviour across platforms and
//! eliminating the attack surface of system-level cert injection.

use std::sync::Arc;

use rustls::ClientConfig;
use rustls::RootCertStore;
use tokio_rustls::TlsConnector;

use crate::FetchError;

/// Build an [`Arc<ClientConfig>`] configured with Ferrum's TLS policy.
///
/// - Protocol versions are explicitly pinned to **TLS 1.2 and TLS 1.3**.
///   TLS 1.0 and 1.1 are deprecated by RFC 8996 and are never offered.
///   The pin is explicit — not relying on `rustls` defaults — so a future
///   library version change cannot silently re-enable older protocols.
/// - TLS 1.3 is negotiated first; TLS 1.2 is the minimum fallback.
/// - Client authentication is disabled (servers do not require it for
///   normal browser activity).
/// - Certificate verification is strict — any error hard-fails the
///   connection rather than presenting a click-through warning.
///
/// # Errors
///
/// Returns [`FetchError::Tls`] if the TLS configuration cannot be built
/// (should never happen with static webpki-roots, but errors are typed
/// rather than panicked to uphold the no-`unwrap` rule).
pub fn build_config() -> Result<Arc<ClientConfig>, FetchError> {
    // Directly assign the roots field rather than using extend() to avoid
    // any API ambiguity between rustls and webpki-roots versions.
    let root_store = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };

    // SECURITY: Explicitly pin the allowed TLS versions to 1.2 and 1.3.
    // Do NOT use `ClientConfig::builder()` without a version pin, as that
    // relies on rustls defaults which may change across releases.
    // TLS 1.0 and 1.1 are deprecated by RFC 8996 (March 2021) and banned here.
    let config = ClientConfig::builder_with_protocol_versions(&[
        &rustls::version::TLS13,
        &rustls::version::TLS12,
    ])
    .with_root_certificates(root_store)
    .with_no_client_auth();

    Ok(Arc::new(config))
}

/// Wrap an [`Arc<ClientConfig>`] into a [`TlsConnector`] for use in connections.
///
/// The config is passed in (rather than built here) so that [`NetworkContext`]
/// can build it once and reuse it across all connections, amortising the cost
/// of root CA store construction.
#[must_use]
pub fn make_connector(config: Arc<ClientConfig>) -> TlsConnector {
    TlsConnector::from(config)
}
