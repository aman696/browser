//! `render` — Painting and compositing for Ferrum.
//!
//! # Architecture
//!
//! Input: a completed layout tree from `crates/layout`.
//! Output: pixels on screen via the platform graphics API.
//!
//! The renderer is responsible for: painting background colours, borders,
//! and text; compositing layers; and presenting the final frame.
//!
//! # Status
//!
//! **Not yet implemented.**

/// Render a layout tree to screen.
///
/// # Errors
///
/// Returns [`RenderError::NotImplemented`] until this crate is built out.
pub fn render() -> Result<(), RenderError> {
    Err(RenderError::NotImplemented)
}

/// Errors that can occur during the render pass.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// Rendering is not yet implemented.
    #[error("rendering is not yet implemented")]
    NotImplemented,
}
