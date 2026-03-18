//! DOM node types for the Ferrum HTML parser.
//!
//! All nodes are allocated inside a [`bumpalo::Bump`] arena. This means:
//!
//! - Zero per-node heap allocations — the arena is one contiguous memory block.
//! - O(1) deallocation — dropping the arena frees the entire tree at once.
//! - No `Rc`/`Arc` needed for parent/sibling pointers (arena lifetime covers
//!   all nodes in the same tree).
//!
//! The lifetime parameter `'arena` on every type ties the node to the arena
//! it was allocated in, preventing use-after-free at compile time.

use bumpalo::Bump;
use std::cell::{Cell, RefCell};

/// The kind of a DOM node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    /// The virtual root of the document.
    Document,
    /// An HTML element such as `<div>` or `<p>`.
    Element,
    /// A text run between or inside elements.
    Text,
    /// An HTML comment `<!-- ... -->`.
    Comment,
}

/// A single node in the DOM tree.
///
/// Nodes are arena-allocated and connected via raw pointers. This is
/// necessary because Rust's ownership model cannot naturally express
/// the parent↔child bidirectional graph that the DOM requires.
///
/// # Safety
///
/// All node pointers stored inside a `Node` point into the same `Bump`
/// arena. They are safe to dereference as long as the arena is alive.
/// The `'arena` lifetime ensures the arena outlives any reference to a node.
pub struct Node<'arena> {
    /// What kind of node this is.
    pub kind: NodeKind,

    /// The lowercased tag name for `Element` nodes (e.g. `"div"`, `"p"`).
    /// Empty string for all other node kinds.
    pub tag_name: &'arena str,

    /// Attributes for `Element` nodes, in source order.
    ///
    /// Stored as `(name, value)` pairs matching the token representation.
    /// Vec preserves attribute order per WHATWG §13.1.2.3 and avoids HashMap
    /// overhead for the typical case of 1–3 attributes per element.
    /// Empty for all other node kinds.
    pub attributes: Vec<(String, String)>,

    /// Text content for `Text` and `Comment` nodes.
    /// Empty for `Element` and `Document` nodes.
    pub text_content: String,

    /// The parent of this node, or `None` for the document root.
    pub parent: Cell<Option<&'arena Node<'arena>>>,

    /// The ordered list of child nodes.
    ///
    /// Wrapped in `RefCell` to allow safe interior mutability: the tree builder
    /// holds shared (`&`) references to nodes but needs to push children onto
    /// them. `RefCell` provides this without any `unsafe` code.
    pub children: RefCell<bumpalo::collections::Vec<'arena, &'arena Node<'arena>>>,
}

impl<'arena> Node<'arena> {
    /// Allocate a new `Document` root node in the given arena.
    pub fn document(arena: &'arena Bump) -> &'arena Self {
        arena.alloc(Node {
            kind: NodeKind::Document,
            tag_name: "",
            attributes: Vec::new(),
            text_content: String::new(),
            parent: Cell::new(None),
            children: RefCell::new(bumpalo::collections::Vec::new_in(arena)),
        })
    }

    /// Allocate a new `Element` node in the given arena.
    pub fn element(
        arena: &'arena Bump,
        tag_name: &'arena str,
        attributes: Vec<(String, String)>,
    ) -> &'arena Self {
        arena.alloc(Node {
            kind: NodeKind::Element,
            tag_name,
            attributes,
            text_content: String::new(),
            parent: Cell::new(None),
            children: RefCell::new(bumpalo::collections::Vec::new_in(arena)),
        })
    }

    /// Allocate a new `Text` node in the given arena.
    pub fn text(arena: &'arena Bump, content: impl Into<String>) -> &'arena Self {
        arena.alloc(Node {
            kind: NodeKind::Text,
            tag_name: "",
            attributes: Vec::new(),
            text_content: content.into(),
            parent: Cell::new(None),
            children: RefCell::new(bumpalo::collections::Vec::new_in(arena)),
        })
    }

    /// Allocate a new `Comment` node in the given arena.
    pub fn comment(arena: &'arena Bump, content: impl Into<String>) -> &'arena Self {
        arena.alloc(Node {
            kind: NodeKind::Comment,
            tag_name: "",
            attributes: Vec::new(),
            text_content: content.into(),
            parent: Cell::new(None),
            children: RefCell::new(bumpalo::collections::Vec::new_in(arena)),
        })
    }
}

impl std::fmt::Debug for Node<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            NodeKind::Document => write!(f, "[Document]"),
            NodeKind::Element => write!(f, "<{}>", self.tag_name),
            NodeKind::Text => write!(f, "Text({:?})", self.text_content),
            NodeKind::Comment => write!(f, "<!--{}-->", self.text_content),
        }
    }
}
