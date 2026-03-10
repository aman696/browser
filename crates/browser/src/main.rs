//! `browser` (ferrum) — top-level binary that wires all Ferrum crates together.
//!
//! This binary contains no business logic. Its only job is to:
//! 1. Accept a URL (hard-wired for now — CLI arg support comes later).
//! 2. Fetch it via `crates/net` (DoH DNS + TLS + HTTP/1.1).
//! 3. Parse the response body via `crates/html`.
//! 4. Print the DOM tree to demonstrate the full pipeline.

use bumpalo::Bump;
use html::parser::HtmlParser;
use net::parse_url;

#[tokio::main]
async fn main() {
    // ── 1. Target URL ──────────────────────────────────────────────────────
    // TODO: accept as a CLI argument in a later session.
    let target = "https://google.com/";

    let parsed = match parse_url(target) {
        Ok(url) => url,
        Err(e) => {
            eprintln!("[ferrum] URL parse error: {e}");
            std::process::exit(1);
        }
    };

    println!(
        "[ferrum] Fetching https://{}:{}{}",
        parsed.host, parsed.port, parsed.path
    );

    // ── 2. Fetch (real network) ────────────────────────────────────────────
    let html_body = match net::fetch(target).await {
        Ok(body) => {
            println!("[ferrum] Received {} bytes", body.len());
            body
        }
        Err(e) => {
            eprintln!("[ferrum] Fetch error: {e}");
            std::process::exit(1);
        }
    };

    // ── 3. Parse HTML ──────────────────────────────────────────────────────
    let arena = Bump::new();
    let parser = HtmlParser::new(&arena);
    let root = parser.parse(&html_body);

    // ── 4. Print DOM ───────────────────────────────────────────────────────
    println!("\n--- DOM Tree ---");
    print_node(root, 0);
    println!(
        "--- End of DOM ({} top-level children) ---",
        root.children.borrow().len()
    );
}

/// Recursively print a DOM node and its descendants with indentation.
fn print_node(node: &html::dom::Node<'_>, depth: usize) {
    use html::dom::NodeKind;

    let indent = "  ".repeat(depth);

    match node.kind {
        NodeKind::Document => println!("{indent}[Document]"),
        NodeKind::Element => {
            if node.attributes.is_empty() {
                println!("{indent}<{}>", node.tag_name);
            } else {
                let attrs: String = node
                    .attributes
                    .iter()
                    .map(|(k, v)| format!(" {k}=\"{v}\""))
                    .collect();
                println!("{indent}<{}{attrs}>", node.tag_name);
            }
        }
        NodeKind::Text => {
            let trimmed = node.text_content.trim();
            if !trimmed.is_empty() {
                // Truncate very long text nodes to keep output readable.
                if trimmed.len() > 80 {
                    println!("{indent}Text: {:?}…", &trimmed[..80]);
                } else {
                    println!("{indent}Text: {trimmed:?}");
                }
            }
        }
        NodeKind::Comment => {
            // Skip comments in output to reduce noise.
        }
    }

    for child in node.children.borrow().iter() {
        print_node(child, depth + 1);
    }
}
