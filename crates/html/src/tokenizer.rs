//! HTML tokenizer — converts a raw HTML string into a list of [`Token`]s.
//!
//! This tokenizer follows the structure of the WHATWG HTML Living Standard
//! §13.2.5, though it is not yet a complete implementation. Deviations are
//! marked with `// SPEC DEVIATION:` comments.
//!
//! # Design
//!
//! The tokenizer operates as a single pass over the input string using byte
//! indices. It produces a `Vec<Token>` which the [`super::parser`] then
//! consumes to build the DOM tree. Tokens store owned `String` data so that
//! the token list can be passed to the parser without borrowing the source.

use crate::token::{Token, TokenKind};
use std::collections::HashMap;

/// Tokenizes a raw HTML string into a [`Vec<Token>`].
///
/// Tag names and attribute names are lowercased during tokenization so that
/// comparisons in the parser stage do not need to be case-insensitive.
///
/// # Spec note
///
/// The WHATWG tokenizer is a state machine with ~80 explicit states. This
/// implementation captures the core cases (start tags, end tags, self-closing
/// tags, comments, text, DOCTYPE, raw-text mode for `<script>` and `<style>`)
/// but does not yet handle edge cases like CDATA sections or the `<textarea>`
/// raw-text mode.
///
/// # SPEC DEVIATION: §13.2.5 — DOCTYPE parsing
///
/// DOCTYPE tokens are recognised and consumed but the name, public identifier,
/// and system identifier are not stored. The parser does not use them yet.
pub fn tokenize(html: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut i = 0;
    let bytes = html.as_bytes();
    let len = html.len();

    while i < len {
        if bytes[i] == b'<' {
            // Check for a comment: <!-- ... -->
            if html[i..].starts_with("<!--") {
                let content_start = i + 4;
                let end = html[content_start..]
                    .find("-->")
                    .map(|pos| content_start + pos)
                    .unwrap_or(len);

                tokens.push(Token::text_or_comment(
                    TokenKind::Comment,
                    &html[content_start..end],
                ));

                i = if end + 3 <= len { end + 3 } else { len };
                continue;
            }

            // DOCTYPE: <!DOCTYPE ...>
            // PERF: Previously `html[i..].to_ascii_lowercase().starts_with("<!doctype")`
            // allocated and lowercased the *entire remaining input* on every `<` character —
            // O(n²) behaviour on any non-trivial HTML file. Fix: slice to exactly 9 bytes
            // and use eq_ignore_ascii_case() which does a zero-allocation byte-by-byte
            // comparison, making this O(1) per `<` character.
            if html[i..].len() >= 9 && html[i..i + 9].eq_ignore_ascii_case("<!doctype") {
                let close = html[i..].find('>').map(|p| i + p).unwrap_or(len);
                tokens.push(Token::text_or_comment(TokenKind::Doctype, ""));
                i = close + 1;
                continue;
            }

            // Normal tag: find the closing '>'
            let tag_close = match html[i + 1..].find('>') {
                Some(pos) => i + 1 + pos,
                None => break, // Malformed: no closing '>' — stop tokenizing.
            };

            let inside = &html[i + 1..tag_close];

            // Determine tag type from leading/trailing slashes.
            let is_end_tag = inside.starts_with('/');
            let inner = if is_end_tag {
                inside[1..].trim()
            } else {
                inside.trim()
            };
            let is_self_closing = inner.ends_with('/');
            let inner = if is_self_closing {
                inner[..inner.len() - 1].trim()
            } else {
                inner
            };

            // Split "tagname attr=val attr2=val2" at the first whitespace.
            let (raw_tag_name, attr_str) = match inner.find(|c: char| c.is_ascii_whitespace()) {
                Some(sp) => (&inner[..sp], &inner[sp + 1..]),
                None => (inner, ""),
            };

            let tag_name = raw_tag_name.to_ascii_lowercase();

            let kind = if is_end_tag {
                TokenKind::EndTag
            } else if is_self_closing {
                TokenKind::SelfClosingTag
            } else {
                TokenKind::StartTag
            };

            let attributes = parse_attributes(attr_str);
            tokens.push(Token::tag(kind.clone(), &tag_name, attributes));

            i = tag_close + 1;

            // RAW-TEXT MODE: after <script> or <style> consume everything
            // until the matching end tag as a single TEXT token.
            // SPEC: §13.2.5.1 — script data state / RCDATA state
            if matches!(kind, TokenKind::StartTag) && (tag_name == "script" || tag_name == "style")
            {
                let end_tag = format!("</{}>", tag_name);
                let close_pos = find_case_insensitive(html, &end_tag, i);
                match close_pos {
                    Some(pos) => {
                        tokens.push(Token::text_or_comment(TokenKind::Text, &html[i..pos]));
                        // Emit the end tag token and advance past it.
                        tokens.push(Token::tag(TokenKind::EndTag, &tag_name, HashMap::new()));
                        i = pos + end_tag.len();
                    }
                    None => {
                        // No closing tag — take the rest as text.
                        tokens.push(Token::text_or_comment(TokenKind::Text, &html[i..]));
                        i = len;
                    }
                }
            }
        } else {
            // Text content: accumulate until the next '<'.
            let next_tag = html[i..].find('<').map(|p| i + p).unwrap_or(len);
            let text = &html[i..next_tag];
            if !text.is_empty() {
                tokens.push(Token::text_or_comment(TokenKind::Text, text));
            }
            i = next_tag;
        }
    }

    tokens
}

/// Parse a run of HTML attribute key=value pairs into a `HashMap`.
///
/// Handles both quoted (`attr="value"` and `attr='value'`) and unquoted
/// (`attr=value`) forms. Boolean attributes (no `=`) are not yet handled.
///
/// # SPEC DEVIATION: §13.2.5 attribute parsing
///
/// Boolean attributes (e.g. `<input disabled>`) are silently dropped.
/// This will be corrected in the next HTML tokenizer session.
fn parse_attributes(attr_str: &str) -> HashMap<String, String> {
    let mut attributes = HashMap::new();
    let mut remaining = attr_str.trim();

    while !remaining.is_empty() {
        // Skip leading whitespace.
        remaining = remaining.trim_start();
        if remaining.is_empty() {
            break;
        }

        // Find the '=' that separates name from value.
        let eq_pos = match remaining.find('=') {
            Some(pos) => pos,
            None => break, // SPEC DEVIATION: boolean attributes not yet handled.
        };

        let name = remaining[..eq_pos].trim().to_ascii_lowercase();
        remaining = &remaining[eq_pos + 1..];

        // Extract the value, stripping surrounding quotes if present.
        let (value, after) = if let Some(stripped) = remaining.strip_prefix('"') {
            match stripped.find('"') {
                Some(end) => (&stripped[..end], &stripped[end + 1..]),
                None => (stripped, ""), // Unclosed quote — consume the rest
            }
        } else if let Some(stripped) = remaining.strip_prefix('\'') {
            match stripped.find('\'') {
                Some(end) => (&stripped[..end], &stripped[end + 1..]),
                None => (stripped, ""), // Unclosed quote — consume the rest
            }
        } else {
            // Unquoted: value ends at the next whitespace.
            let end = remaining
                .find(|c: char| c.is_ascii_whitespace())
                .unwrap_or(remaining.len());
            (&remaining[..end], &remaining[end..])
        };

        if !name.is_empty() {
            attributes.insert(name, value.to_owned());
        }
        remaining = after;
    }

    attributes
}

/// Find `needle` inside `haystack` starting at `from`, case-insensitively.
///
/// Returns the byte index of the start of the match, or `None` if not found.
fn find_case_insensitive(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    let haystack_lower = haystack[from..].to_ascii_lowercase();
    let needle_lower = needle.to_ascii_lowercase();
    haystack_lower.find(&needle_lower).map(|pos| from + pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple_start_and_end_tag() {
        let tokens = tokenize("<p>Hello</p>");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0].kind, TokenKind::StartTag));
        assert_eq!(tokens[0].tag_name, "p");
        assert!(matches!(tokens[1].kind, TokenKind::Text));
        assert_eq!(tokens[1].text, "Hello");
        assert!(matches!(tokens[2].kind, TokenKind::EndTag));
        assert_eq!(tokens[2].tag_name, "p");
    }

    #[test]
    fn test_tokenize_self_closing_tag() {
        let tokens = tokenize("<br />");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].kind, TokenKind::SelfClosingTag));
        assert_eq!(tokens[0].tag_name, "br");
    }

    #[test]
    fn test_tokenize_comment() {
        let tokens = tokenize("<!-- a comment -->");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].kind, TokenKind::Comment));
        assert_eq!(tokens[0].text.trim(), "a comment");
    }

    #[test]
    fn test_tokenize_attribute_parsing() {
        let tokens = tokenize(r#"<a href="https://example.com" class="link">"#);
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0].attributes.get("href").map(|s| s.as_str()),
            Some("https://example.com")
        );
        assert_eq!(
            tokens[0].attributes.get("class").map(|s| s.as_str()),
            Some("link")
        );
    }

    #[test]
    fn test_tokenize_tag_names_are_lowercased() {
        let tokens = tokenize("<DIV></DIV>");
        assert_eq!(tokens[0].tag_name, "div");
        assert_eq!(tokens[1].tag_name, "div");
    }

    #[test]
    fn test_tokenize_script_raw_text_mode() {
        let tokens = tokenize("<script>var x = 1 < 2;</script>");
        // Should produce: StartTag(script), Text(var x = 1 < 2;), EndTag(script)
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[1].kind, TokenKind::Text));
        assert!(tokens[1].text.contains("var x"));
    }
}
