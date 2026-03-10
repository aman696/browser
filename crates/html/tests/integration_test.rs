//! Integration tests for the HTML tokenizer and parser.
//!
//! These tests load fixture HTML files from `tests/fixtures/` and verify
//! that the tokenizer and parser handle real-world HTML correctly.
//! Fixture files represent common patterns found in production web pages.

use bumpalo::Bump;
use html::dom::NodeKind;
use html::parser::HtmlParser;
use html::tokenizer::tokenize;

/// Load a fixture file by name from the workspace-level tests/fixtures/ directory.
///
/// Uses `CARGO_MANIFEST_DIR` to find the crate root, then navigates to
/// the shared fixtures directory two levels up.
fn load_fixture(filename: &str) -> String {
    // Each crate's manifest dir is crates/<name>/.
    // The fixtures live at the workspace root: ../../tests/fixtures/.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest_dir)
        .join("../../tests/fixtures")
        .join(filename);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read fixture {filename}: {e}"))
}

// ─── Tokenizer integration tests ─────────────────────────────────────────────

#[test]
fn test_tokenize_simple_page_produces_tokens() {
    let html = load_fixture("simple_page.html");
    let tokens = tokenize(&html);

    // A real page should produce many tokens — at minimum DOCTYPE, html,
    // head, body, and several element and text tokens.
    assert!(
        tokens.len() > 10,
        "expected many tokens from a full HTML page, got {}",
        tokens.len()
    );
}

#[test]
fn test_tokenize_script_style_page_does_not_split_on_angle_brackets() {
    let html = load_fixture("script_style_page.html");
    let tokens = tokenize(&html);

    // The '<' inside the script body must NOT be treated as a tag opener.
    // If it is, we'd get spurious tokens like StartTag("2;") etc.
    let spurious = tokens
        .iter()
        .any(|t| t.tag_name.contains(';') || t.tag_name.contains('='));

    assert!(
        !spurious,
        "tokenizer incorrectly split script content on '<'"
    );
}

#[test]
fn test_tokenize_malformed_page_does_not_panic() {
    // The tokenizer must not panic on malformed HTML — it should produce
    // whatever tokens it can and stop gracefully.
    let html = load_fixture("malformed_page.html");
    let tokens = tokenize(&html);
    assert!(
        !tokens.is_empty(),
        "should produce at least some tokens from malformed HTML"
    );
}

// ─── Parser integration tests ─────────────────────────────────────────────────

#[test]
fn test_parse_simple_page_has_html_root() {
    let html = load_fixture("simple_page.html");
    let arena = Bump::new();
    let parser = HtmlParser::new(&arena);
    let root = parser.parse(&html);

    assert!(matches!(root.kind, NodeKind::Document));
    let root_children = root.children.borrow();
    assert!(
        root_children.iter().any(|n| n.tag_name == "html"),
        "parsed document should have an <html> child"
    );
}

#[test]
fn test_parse_simple_page_body_has_expected_elements() {
    let html = load_fixture("simple_page.html");
    let arena = Bump::new();
    let parser = HtmlParser::new(&arena);
    let root = parser.parse(&html);

    // Walk the tree to find the <h1> element anywhere in the document.
    assert!(
        find_element(root, "h1"),
        "parsed document should contain an <h1> element"
    );
    assert!(
        find_element(root, "ul"),
        "parsed document should contain a <ul> element"
    );
    assert!(
        find_element(root, "a"),
        "parsed document should contain an <a> element"
    );
}

#[test]
fn test_parse_malformed_page_does_not_panic() {
    // The parser must never panic on malformed HTML. It applies
    // the WHATWG error-recovery rules instead.
    let html = load_fixture("malformed_page.html");
    let arena = Bump::new();
    let parser = HtmlParser::new(&arena);
    let root = parser.parse(&html);
    // As long as we get a Document back without panicking, this passes.
    assert!(matches!(root.kind, NodeKind::Document));
}

#[test]
fn test_parse_script_style_page_script_node_has_text_content() {
    let html = load_fixture("script_style_page.html");
    let arena = Bump::new();
    let parser = HtmlParser::new(&arena);
    let root = parser.parse(&html);

    // There should be a <script> element whose text child contains the JS source.
    let script = find_node(root, "script");
    assert!(script.is_some(), "should have a <script> element");
    let script_node = script.unwrap();
    let children = script_node.children.borrow();
    let has_js_text = children
        .iter()
        .any(|n| matches!(n.kind, NodeKind::Text) && n.text_content.contains("var x"));
    assert!(has_js_text, "script element should contain raw JS text");
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Returns true if any descendant of `node` is an Element with the given tag name.
fn find_element<'a>(node: &'a html::dom::Node<'a>, tag: &str) -> bool {
    find_node(node, tag).is_some()
}

/// Returns the first descendant Element with the given tag name, or None.
fn find_node<'a>(node: &'a html::dom::Node<'a>, tag: &str) -> Option<&'a html::dom::Node<'a>> {
    if matches!(node.kind, NodeKind::Element) && node.tag_name == tag {
        return Some(node);
    }
    for child in node.children.borrow().iter() {
        if let Some(found) = find_node(child, tag) {
            return Some(found);
        }
    }
    None
}
