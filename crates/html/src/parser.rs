//! HTML tree-construction parser — converts a [`Vec<Token>`] into a DOM tree.
//!
//! Implements a stack-based tree construction algorithm following the WHATWG
//! HTML Living Standard §13.2.6. The parser operates in a single pass over
//! the token list, maintaining an open-element stack and attaching completed
//! subtrees to their parents.
//!
//! All nodes are allocated inside the `bumpalo::Bump` arena passed to
//! [`HtmlParser::new`]. The returned root node lives as long as that arena.

use std::collections::HashSet;

use bumpalo::Bump;

use crate::dom::Node;
use crate::token::TokenKind;
use crate::tokenizer::tokenize;

/// The set of HTML void elements — elements that cannot have children and
/// therefore must never be pushed onto the open-element stack.
///
/// Defined in WHATWG HTML §13.1.2.
static VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// HTML tree-construction parser.
///
/// Accepts an arena that all DOM nodes will be allocated into. The arena
/// must outlive any use of the returned node tree.
pub struct HtmlParser<'arena> {
    /// The bump arena that owns all DOM node allocations.
    arena: &'arena Bump,
}

impl<'arena> HtmlParser<'arena> {
    /// Create a new parser that will allocate nodes into `arena`.
    pub fn new(arena: &'arena Bump) -> Self {
        Self { arena }
    }

    /// Parse a raw HTML string into a DOM tree.
    ///
    /// Returns a reference to the virtual `Document` root node. The root
    /// node and all its descendants are owned by the arena supplied to
    /// [`HtmlParser::new`].
    ///
    /// # Error handling
    ///
    /// Following the WHATWG spec philosophy, this parser never fails —
    /// malformed HTML is corrected using the same error-recovery rules
    /// browsers apply in practice:
    ///
    /// - Unclosed elements are auto-closed when the document ends.
    /// - Mismatched end tags cause intermediate open elements to be closed
    ///   until the matching open element is found or the stack is exhausted.
    pub fn parse(&self, html: &str) -> &'arena Node<'arena> {
        let tokens = tokenize(html);
        self.build_tree(tokens)
    }

    /// Build the DOM tree from a token list using a stack-based approach.
    ///
    /// The open-element stack tracks the current nesting context. When a
    /// start tag is seen, a new node is created and pushed. When a matching
    /// end tag is seen, the stack is unwound to find and close it.
    fn build_tree(&self, tokens: Vec<crate::token::Token>) -> &'arena Node<'arena> {
        let root = Node::document(self.arena);

        // The open-element stack. We use indices rather than pointers because
        // Rust's borrowing rules prevent holding mutable references alongside
        // the immutable ones stored in node children.
        // We store references into the arena directly — these are stable
        // because the arena never moves memory.
        let mut stack: Vec<&'arena Node<'arena>> = vec![root];

        let void_set: HashSet<&str> = VOID_ELEMENTS.iter().copied().collect();

        for token in tokens {
            match token.kind {
                TokenKind::StartTag => {
                    let tag_name = self.arena.alloc_str(&token.tag_name);
                    let node = Node::element(self.arena, tag_name, token.attributes);

                    // Attach to the current top of the stack.
                    // Fall back to root if the stack is somehow empty (which it shouldn't be).
                    let parent = stack.last().copied().unwrap_or(root);
                    append_child(parent, node);

                    // Void elements cannot have children — do not push them.
                    if !void_set.contains(token.tag_name.as_str()) {
                        stack.push(node);
                    }
                }

                TokenKind::EndTag => {
                    // Search the stack (from top) for the matching open element.
                    let close_name = &token.tag_name;
                    let match_pos = stack.iter().rposition(|n| n.tag_name == close_name);

                    match match_pos {
                        Some(pos) => {
                            // SPEC: pop everything above the matched element too
                            // (auto-closing implicitly opened elements).
                            stack.truncate(pos);
                        }
                        None => {
                            // Orphan end tag: no matching open element on the stack.
                            // SPEC: §13.2.6 — ignore it.
                            eprintln!("[html::parser] orphan end tag </{close_name}> — ignoring");
                        }
                    }
                }

                TokenKind::SelfClosingTag => {
                    let tag_name = self.arena.alloc_str(&token.tag_name);
                    let node = Node::element(self.arena, tag_name, token.attributes);
                    let parent = stack.last().copied().unwrap_or(root);
                    append_child(parent, node);
                    // Self-closing: never pushed onto the stack.
                }

                TokenKind::Text => {
                    let node = Node::text(self.arena, token.text);
                    let parent = stack.last().copied().unwrap_or(root);
                    append_child(parent, node);
                }

                TokenKind::Comment => {
                    let node = Node::comment(self.arena, token.text);
                    let parent = stack.last().copied().unwrap_or(root);
                    append_child(parent, node);
                }

                TokenKind::Doctype => {
                    // Doctype tokens are consumed but not represented in the
                    // DOM in this implementation.
                }
            }
        }

        // At end-of-document, any remaining open elements are auto-closed.
        // The stack bottom (index 0) is always the Document root.
        if stack.len() > 1 {
            eprintln!(
                "[html::parser] {} unclosed element(s) at end of document — auto-closing",
                stack.len() - 1
            );
        }

        root
    }
}

/// Append `child` to `parent`'s children list and set the child's parent pointer.
///
/// Uses `RefCell::borrow_mut()` for safe interior mutability — no `unsafe` needed.
/// The `RefCell` on `Node::children` allows the tree builder to hold shared
/// (`&'arena Node`) references to multiple nodes simultaneously while still
/// being able to mutate their children lists one at a time.
fn append_child<'arena>(parent: &'arena Node<'arena>, child: &'arena Node<'arena>) {
    child.parent.set(Some(parent));
    parent.children.borrow_mut().push(child);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::NodeKind;
    use bumpalo::Bump;

    #[test]
    fn test_parse_simple_document() {
        let arena = Bump::new();
        let parser = HtmlParser::new(&arena);
        let root = parser.parse("<html><body><p>Hello</p></body></html>");
        assert!(matches!(root.kind, NodeKind::Document));
        let children = root.children.borrow();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].tag_name, "html");
    }

    #[test]
    fn test_parse_void_element_has_no_children() {
        let arena = Bump::new();
        let parser = HtmlParser::new(&arena);
        let root = parser.parse("<div><br><span>after</span></div>");
        // root -> div -> [br, span]
        let root_children = root.children.borrow();
        let div = root_children[0];
        let div_children = div.children.borrow();
        assert_eq!(div_children.len(), 2);
        // br must have no children (void element)
        assert_eq!(div_children[0].children.borrow().len(), 0);
        assert_eq!(div_children[0].tag_name, "br");
    }

    #[test]
    fn test_parse_unclosed_element_auto_closes() {
        let arena = Bump::new();
        let parser = HtmlParser::new(&arena);
        // <p> is never closed — should still appear as a child of root
        let root = parser.parse("<p>Unclosed");
        assert_eq!(root.children.borrow().len(), 1);
        assert_eq!(root.children.borrow()[0].tag_name, "p");
    }

    #[test]
    fn test_parse_comment_node() {
        let arena = Bump::new();
        let parser = HtmlParser::new(&arena);
        let root = parser.parse("<!-- hello -->");
        assert_eq!(root.children.borrow().len(), 1);
        assert!(matches!(root.children.borrow()[0].kind, NodeKind::Comment));
    }
}
