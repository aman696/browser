//! `security` — TLS policy, cookies, HSTS, content blocking and permissions.
//!
//! # Architecture
//!
//! This crate is the single point of truth for all security and privacy
//! decisions in Ferrum. Every other crate queries `SecurityManager` before
//! making requests or enabling APIs. No other crate makes its own blocking
//! decisions.
//!
//! Responsibilities:
//! - TLS certificate validation policy (hard-fail on errors)
//! - HSTS preload list (ships with binary, no network updates)
//! - Cookie jar (third-party cookies blocked by default)
//! - Tracker blocklist (EasyPrivacy / Disconnect.me, shipped with binary)
//! - Per-origin permission store (`permissions.toml`)
//! - Privacy warning interstitial trigger logic
//!
//! # Status
//!
//! **Not yet implemented.**

/// The central security and privacy policy manager.
///
/// All crates obtain a reference to the single `SecurityManager` instance
/// from `crates/browser` (the top-level wiring crate). They call it before
/// any action that might leak information or require user consent.
pub struct SecurityManager;

impl SecurityManager {
    /// Create a new `SecurityManager` with default (strictest) settings.
    ///
    /// In the final implementation this will load `permissions.toml` from
    /// disk and initialise the HSTS preload list from the embedded binary data.
    pub fn new() -> Self {
        SecurityManager
    }

    /// Check whether a request to `host` is permitted under the current policy.
    ///
    /// Returns `true` if the request should proceed, `false` if it should be
    /// blocked and a privacy warning shown to the user.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::NotImplemented`] until this crate is built out.
    pub fn check_request(&self, _host: &str) -> Result<bool, SecurityError> {
        Err(SecurityError::NotImplemented)
    }
}

impl Default for SecurityManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors from the security subsystem.
#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    /// The security subsystem is not yet implemented.
    #[error("security subsystem not yet implemented")]
    NotImplemented,
}
