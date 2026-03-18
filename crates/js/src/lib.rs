//! `js` — JavaScript engine integration for Ferrum.
//!
//! # Engine Choice
//!
//! Ferrum uses [Boa](https://boajs.dev/) (`boa_engine` on crates.io) — a
//! pure-Rust ECMAScript bytecode compiler and VM. This avoids the C++ memory
//! safety issues that V8 and SpiderMonkey introduce.
//!
//! # Status: Audit complete, integration pending
//!
//! The Boa dependency audit has been completed and documented in
//! `docs/decisions/boa-audit.md`. Decision: **Integrate as-is** (v0.21.0).
//! Boa is pure Rust, makes no network calls, and carries only one low-risk
//! compile-time warning (unmaintained `paste` crate, no runtime impact).
//!
//! Next step: add `boa_engine` to this crate's `Cargo.toml` and implement
//! the `JsRuntime` type below.

/// Execute a JavaScript string in an isolated Boa context.
///
/// JavaScript is disabled by default in Ferrum and enabled per-site via
/// the permission system in `crates/security`.
///
/// # Errors
///
/// Returns [`JsError::NotImplemented`] until `boa_engine` is integrated.
pub fn execute(_script: &str) -> Result<(), JsError> {
    Err(JsError::NotImplemented)
}

/// Errors that can occur during JavaScript execution.
#[derive(Debug, thiserror::Error)]
pub enum JsError {
    /// The JS engine is not yet integrated (pending Boa audit).
    #[error("JavaScript engine not yet integrated — boa audit required first")]
    NotImplemented,
}
