//! [`NetworkContext`] — the central policy chokepoint for all Ferrum network requests.
//!
//! Per `RULES-04-networking.md`, all network requests must flow through a single
//! `NetworkContext` that enforces privacy and security policy. No other crate
//! may open a raw socket or bypass this chokepoint.
//!
//! # What `NetworkContext` owns
//!
//! | Resource | Why cached here |
//! |----------|----------------|
//! | `TlsConnector` | Wraps a shared `Arc<ClientConfig>` built once at startup |
//! | `TokioResolver` | DoH resolver with in-memory DNS cache — rebuilt per request is wasteful |
//! | `HstsStore` | Tracks HSTS policies from server responses across all fetches |
//!
//! # Thread safety
//!
//! `NetworkContext` is `Send` and `Sync`. The `TlsConnector` and `TokioResolver`
//! are both `Arc`-backed. `HstsStore` is protected by interior mutability via
//! `std::sync::Mutex` for multi-task access.

use std::sync::{Arc, Mutex};

use hickory_resolver::TokioResolver;
use tokio_rustls::TlsConnector;

use crate::hsts::HstsStore;
use crate::{FetchError, dns, tls};

/// The central networking state container for Ferrum.
///
/// Build one `NetworkContext` at application startup and pass it (or an
/// `Arc<NetworkContext>`) wherever network requests need to be made.
/// This amortises the cost of TLS config and DNS resolver construction
/// across the lifetime of the process.
pub struct NetworkContext {
    pub(crate) connector: TlsConnector,
    pub(crate) resolver: Arc<TokioResolver>,
    pub(crate) hsts: Mutex<HstsStore>,
}

impl NetworkContext {
    /// Create a new [`NetworkContext`], installing the `aws-lc-rs` crypto
    /// provider for `rustls` and building the TLS config and DoH resolver.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError::Tls`] if TLS configuration fails (extremely
    /// unlikely with static webpki-roots, but typed rather than panicked).
    pub fn new() -> Result<Self, FetchError> {
        // rustls 0.23 requires the crypto provider to be installed once per
        // process before any TLS operation. `.ok()` ignores the error if it
        // has already been installed (e.g., in tests where multiple contexts
        // might be created).
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .ok();

        let tls_config = tls::build_config()?;
        let connector = tls::make_connector(tls_config);
        let resolver = Arc::new(dns::build_resolver());
        let hsts = Mutex::new(HstsStore::new());

        Ok(Self {
            connector,
            resolver,
            hsts,
        })
    }

    /// Fetch a URL and return the response body.
    ///
    /// This is the main entry point. Calls [`crate::client::fetch_with_context`]
    /// internally, passing the cached TLS connector, DNS resolver, and HSTS store.
    pub async fn fetch(&self, url: &str) -> Result<String, FetchError> {
        crate::client::fetch_with_context(self, url.to_owned(), 0).await
    }
}
