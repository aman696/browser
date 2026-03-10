//! `layout` — Box model and layout engine for Ferrum.
//!
//! # Architecture
//!
//! Input: a styled DOM tree (computed styles from `crates/css`).
//! Output: a layout tree where every node has a computed position and size.
//!
//! Implements the CSS box model following the
//! [CSS2 Visual Formatting Model](https://www.w3.org/TR/CSS2/visuren.html)
//! as a baseline, with extensions for Flexbox and Grid.
//!
//! # Status
//!
//! **Not yet implemented.**

/// Run the layout pass over a styled document.
///
/// # Errors
///
/// Returns [`LayoutError::NotImplemented`] until this crate is built out.
pub fn layout() -> Result<(), LayoutError> {
    Err(LayoutError::NotImplemented)
}

/// Errors that can occur during the layout pass.
#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    /// Layout is not yet implemented.
    #[error("layout is not yet implemented")]
    NotImplemented,
}
