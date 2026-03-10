//! `js` — JavaScript engine integration for Ferrum.
//!
//! # Engine Choice
//!
//! Ferrum uses [Boa](https://boajs.dev/) (`boa_engine` on crates.io) — a
//! pure-Rust ECMAScript bytecode compiler and VM. This avoids the C++ memory
//! safety issues that V8 and SpiderMonkey introduce.
//!
//! # Blocked: Boa audit required
//!
//! Per `RULES-04-networking.md`, `boa_engine` MUST NOT be added to
//! `Cargo.toml` until a full dependency audit has been completed and
//! documented in `docs/decisions/boa-audit.md`. That file does not yet exist.
//!
//! When the audit is complete, add `boa_engine` to this crate's `Cargo.toml`
//! and implement the `JsRuntime` type below.
//!
//! # Status
//!
//! **Blocked on audit.** See `docs/decisions/boa-audit.md`.

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
