//! `css` — CSS parsing and cascade resolution for Ferrum.
//!
//! # Architecture
//!
//! Input: raw CSS text + a styled DOM tree.
//! Output: computed style values for each node.
//!
//! This crate follows [CSS Syntax Level 3](https://www.w3.org/TR/css-syntax-3/)
//! for tokenization and [CSS Cascade Level 4](https://www.w3.org/TR/css-cascade-4/)
//! for property resolution.
//!
//! # Status
//!
//! **Not yet implemented.** The stub below documents the planned public API
//! so that other crates can depend on this crate's types from the start.

/// Parse a CSS stylesheet string.
///
/// Returns a parsed stylesheet handle that can be used with the cascade
/// resolver to compute per-element styles.
///
/// # Errors
///
/// Returns a [`CssError::NotImplemented`] until this crate is built out.
pub fn parse_css(_input: &str) -> Result<(), CssError> {
    Err(CssError::NotImplemented)
}

/// Errors that can occur during CSS parsing or cascade resolution.
#[derive(Debug, thiserror::Error)]
pub enum CssError {
    /// CSS parsing is not yet implemented.
    #[error("CSS parsing is not yet implemented")]
    NotImplemented,
}
