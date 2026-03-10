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

/// Build a [`TlsConnector`] configured with Ferrum's TLS policy.
///
/// - TLS 1.2 and TLS 1.3 are both enabled (1.2 for compatibility,
///   1.3 preferred by rustls's default cipher suite ordering).
/// - Client authentication is disabled (servers do not require it for
///   normal browser activity).
/// - Certificate verification is strict — any error hard-fails the
///   connection rather than presenting a click-through warning.
///
/// # Errors
///
/// Returns [`FetchError::Tls`] if the TLS configuration cannot be built
/// (should never happen with static webpki-roots, but errors are typed
/// rather than paniced to uphold the no-`unwrap` rule).
pub fn make_connector() -> Result<TlsConnector, FetchError> {
    // rustls 0.23 requires explicitly installing a CryptoProvider when
    // using non-default features. We built with `aws-lc-rs` instead of the
    // default `ring` backend, so install_default() must be called before any
    // TLS handshake. Without this, cert verification silently fails with
    // UnknownIssuer even for perfectly valid certificates.
    // .ok() ignores the error if a provider is already installed (e.g. in tests).
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok();

    // Directly assign the roots field rather than using extend() to avoid
    // any API ambiguity between rustls and webpki-roots versions.
    let root_store = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };

    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(TlsConnector::from(Arc::new(config)))
}
