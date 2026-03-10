//! `html` — WHATWG-spec HTML tokenizer, parser and DOM for Ferrum.
//!
//! # Architecture
//!
//! The pipeline is: raw HTML bytes → [`tokenizer`] → `Vec<Token>` →
//! [`parser`] → arena-allocated [`dom`] tree.
//!
//! DOM nodes live inside a [`bumpalo::Bump`] arena provided by the caller.
//! This means the entire tree is freed in O(1) when the arena is dropped —
//! no per-node allocation overhead, no `Box` chains, and no reference cycles
//! that Rust's ownership model would struggle to express.
//!
//! # Spec Compliance
//!
//! The tokenizer and parser follow the
//! [WHATWG HTML Living Standard §13](https://html.spec.whatwg.org/multipage/parsing.html).
//! Any deliberate deviation is marked with a `// SPEC DEVIATION:` comment at
//! the exact point of divergence.
//!
//! # Example
//!
//! ```rust
//! use bumpalo::Bump;
//! use html::parser::HtmlParser;
//!
//! let arena = Bump::new();
//! let mut parser = HtmlParser::new(&arena);
//! let root = parser.parse("<html><body><p>Hello</p></body></html>");
//! // root is a &Node allocated inside `arena` — it lives as long as `arena`.
//! ```

pub mod dom;
pub mod parser;
pub mod token;
pub mod tokenizer;
