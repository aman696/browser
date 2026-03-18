//! HTML token types produced by the [`super::tokenizer`].
//!
//! A `Token` is a lightweight view into the original HTML source string —
//! it borrows slices rather than copying them wherever possible. The
//! lifetime `'src` ties each token to the HTML string it was parsed from.

/// The kind of HTML token recognised by the tokenizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// A `<!DOCTYPE ...>` declaration.
    Doctype,
    /// An opening tag such as `<div id="main">`.
    StartTag,
    /// A closing tag such as `</div>`.
    EndTag,
    /// A self-closing tag such as `<img />` or a void element like `<br>`.
    SelfClosingTag,
    /// An HTML comment `<!-- ... -->`.
    Comment,
    /// Raw text content between tags.
    Text,
}

/// A single HTML token produced by the tokenizer.
///
/// Strings are stored as owned `String` values so that the token list can
/// outlive the parse pass that produced them. Tag names are always
/// lowercased to simplify comparisons in the tree-construction stage.
#[derive(Debug, Clone)]
pub struct Token {
    /// The kind of this token.
    pub kind: TokenKind,

    /// The lowercased tag name for `StartTag`, `EndTag`, and `SelfClosingTag`.
    ///
    /// Empty for `Text`, `Comment`, and `Doctype` tokens.
    pub tag_name: String,

    /// Attributes for `StartTag` and `SelfClosingTag` tokens, in source order.
    ///
    /// Stored as `(name, value)` pairs. Names are lowercased; values have
    /// surrounding quotes stripped. Order is preserved per WHATWG §13.1.2.3.
    /// Using Vec instead of HashMap: HTML elements have 1–3 attributes on
    /// average — hashing is pure overhead, and HashMap does not preserve order.
    /// Empty for all other token kinds.
    pub attributes: Vec<(String, String)>,

    /// Text content for `Text`, `Comment`, and `Doctype` tokens.
    ///
    /// Empty for tag tokens.
    pub text: String,
}

impl Token {
    /// Construct a tag token (start, end, or self-closing).
    pub fn tag(
        kind: TokenKind,
        tag_name: impl Into<String>,
        attributes: Vec<(String, String)>,
    ) -> Self {
        Self {
            kind,
            tag_name: tag_name.into(),
            attributes,
            text: String::new(),
        }
    }

    /// Construct a text or comment token.
    pub fn text_or_comment(kind: TokenKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            tag_name: String::new(),
            attributes: Vec::new(),
            text: text.into(),
        }
    }
}
