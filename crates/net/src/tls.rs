//! TLS client configuration for Ferrum using [`rustls`] with the
//! `aws-lc-rs` cryptographic backend.
//!

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

    // ── TODO(CT): Certificate Transparency verification ────────────────────
    //
    // What CT is:
    //   Certificate Transparency (RFC 9162) requires CAs to log every issued
    //   certificate to a public, append-only log. A Signed Certificate
    //   Timestamp (SCT) is proof that a cert was logged. Browsers that enforce
    //   CT will reject certificates without at least two valid SCTs.
    //
    // The gap today:
    //   `rustls` validates the cert chain against trusted roots (webpki-roots)
    //   but does NOT verify SCTs. A CA-misissued certificate — one that was
    //   never submitted to any CT log — would pass the current handshake with
    //   no error. This has happened in practice (DigiNotar 2011, Symantec 2017).
    //
    // Why it is left incomplete (intentionally):
    //   1. No production-ready Rust crate exists for SCT signature verification
    //      as of 2026-03. The `certificate-transparency` crate is abandoned.
    //   2. Requires consuming Google's CT log list JSON at compile/startup time:
    //      https://www.gstatic.com/ct/log_list/v3/all_logs_list.json
    //   3. `rustls` exposes SCT bytes via `PeerCertificate::cert.as_der()` and
    //      the TLS extension `SCT` (type 18) — parsing requires DER decoding.
    //
    // Implementation path when ready:
    //   1. Implement a custom `rustls::client::danger::ServerCertVerifier` that
    //      wraps `WebPkiServerVerifier` and adds SCT validation on top.
    //   2. Call `.with_custom_certificate_verifier(Arc::new(FerrumsCtVerifier))`
    //      instead of `.with_root_certificates(root_store).with_no_client_auth()`.
    //   3. Hard-fail (`FetchError::Tls`) if < 2 valid SCTs are present.
    //   4. Bundle the CT log list as a compiled-in constant (like `webpki-roots`).
    //
    // Reference: RFC 9162 — https://www.rfc-editor.org/rfc/rfc9162
    // ──────────────────────────────────────────────────────────────────────────

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
