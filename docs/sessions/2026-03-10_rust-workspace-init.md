# Session: Rust Workspace Initialisation

**Date:** 2026-03-10
**Crates modified:** `crates/net`, `crates/html`, `crates/css`, `crates/layout`, `crates/render`, `crates/js`, `crates/security`, `crates/browser`
**Files changed:**
- Added: `Cargo.toml` (workspace root)
- Added: `crates/net/Cargo.toml`, `crates/net/src/lib.rs`, `crates/net/src/url.rs`
- Added: `crates/html/Cargo.toml`, `crates/html/src/lib.rs`, `crates/html/src/token.rs`, `crates/html/src/tokenizer.rs`, `crates/html/src/dom.rs`, `crates/html/src/parser.rs`
- Added: `crates/css/Cargo.toml`, `crates/css/src/lib.rs`
- Added: `crates/layout/Cargo.toml`, `crates/layout/src/lib.rs`
- Added: `crates/render/Cargo.toml`, `crates/render/src/lib.rs`
- Added: `crates/js/Cargo.toml`, `crates/js/src/lib.rs`
- Added: `crates/security/Cargo.toml`, `crates/security/src/lib.rs`
- Added: `crates/browser/Cargo.toml`, `crates/browser/src/main.rs`
- Added: `docs/FEATURE_LIST.md`
- Added: `docs/sessions/2026-03-10_rust-workspace-init.md`
- Deleted: `src/` (entire tree — `main.cpp`, `Network/HttpClient.cpp`, `Network/URLParser.cpp`)
- Deleted: `include/` (entire tree — `Network/HttpClient.h`, `Network/URLParser.h`, `Parser/DOMNode.h`, `Parser/HtmlParser.h`, `Parser/HtmlParser.cpp`, `Parser/Token.h`, `Parser/Tokenizer.h`, `Parser/Tokenizer.cpp`)

---

## What Was Built

The project was migrated from a Windows-only C++ prototype using OpenSSL and WinSock2 into a Rust Cargo workspace with the crate structure mandated by the project rules. No C++ files remain.

**`crates/net`** contains a URL parser (`url.rs`) that is a ground-up Rust rewrite of the C++ `URLParser.cpp`. It handles `https://`, `http://`, and schemeless URLs. It enforces HTTPS silently for any non-localhost remote host, which is the privacy policy required by RULES-03. The `fetch()` function is a stub returning `Err(FetchError::NotImplemented)` — real networking with `rustls` + `tokio` + `hickory-resolver` is a separate session once the workspace structure is stable.

**`crates/html`** contains four modules that together replace the C++ tokenizer, parser, and DOM:
- `token.rs` — `TokenKind` enum and `Token` struct (owned strings instead of raw C-string pointers)
- `tokenizer.rs` — single-pass byte-index tokenizer rewriting `Tokenizer.cpp`, with raw-text mode for `<script>` and `<style>`, and `// SPEC DEVIATION:` comments on known gaps
- `dom.rs` — `Node<'arena>` allocated in a `bumpalo::Bump` arena; replaces manual `new`/`delete` with arena-lifetime-managed allocation; `Cell<Option<&Node>>` for parent pointers provides safe interior mutability
- `parser.rs` — stack-based tree builder rewriting `HtmlParser.cpp`; all `unsafe` blocks carry `// SAFETY:` comments explaining the pointer cast invariant

**Six stub crates** (`css`, `layout`, `render`, `js`, `security`, `browser/main.rs`) are scaffolded with `//!` crate-level doc comments describing each subsystem's role. The `js` crate explicitly documents that `boa_engine` must not be added until `docs/decisions/boa-audit.md` exists.

**`crates/browser`** is the thin top-level binary (`ferrum`). It parses a URL, calls the (stub) fetch, and falls back to inline sample HTML so the parsing pipeline can be demonstrated before real networking exists.

---

## Why These Decisions Were Made

**Arena allocation (`bumpalo`) for DOM nodes.** The C++ prototype used raw `new`/`delete` with a recursive destructor. In Rust, the natural equivalent would be `Box<Node>` with `Rc` for parent pointers, but that introduces per-node heap allocations and reference-counting overhead. A `bumpalo::Bump` arena allocates all nodes from a single contiguous block and frees them all in O(1) when the arena is dropped. This is both faster and simpler to reason about. The WHATWG spec says the parser discards the token stream and the source bytes once the tree is built — arena allocation matches this lifetime exactly.

**`Cell<Option<&Node>>` for parent pointers.** The DOM is a bidirectional graph: a child knows its parent and the parent knows its children. Rust's ownership model prevents holding `&mut` references to parent and child simultaneously. `Cell` provides interior mutability without `unsafe` at the call site, allowing the parent pointer to be set after the child is already referenced in the children list.

**`unsafe` in `append_child`.** Pushing into `parent.children` requires a mutable reference to the parent, but the tree-builder holds `&'arena Node` (shared reference). A raw pointer cast is used here with a documented `// SAFETY:` invariant: the tree builder is single-threaded, no other code holds a reference to `parent.children` simultaneously, and all nodes live for `'arena`. This is the standard pattern for arena-backed graph construction in Rust (used by `rustc` itself for its own arena-allocated IR).

**Owned `String` in tokens rather than `&'src str` slices.** The original plan considered borrowing slices from the source string, but the tokenizer's lowercasing of tag names means the token cannot point to the original bytes — the lowercased version requires a new allocation anyway. Owned strings keep the design simple at the cost of some allocation; this is revisable once the tokenizer is fuzz-tested and spec-complete.

**`FetchError::NotImplemented` stub.** Rather than `panic!()` or `todo!()`, the stub returns a typed error. This means `main.rs` can match on it and fall back to sample HTML, demonstrating the full pipeline compiles and runs without a network.

---

## What Was Tried and Didn't Work

**Lifetime threading for `Token<'src>`.** The initial design in the plan used `Token<'src>` with `&'src str` slices to avoid allocation. This was abandoned during implementation because tag name lowercasing forces an allocation regardless — returning a `&str` pointing into a locally-owned `String` would not compile. Owned `String` is the correct choice for now.

---

## Spec References

- WHATWG HTML Living Standard §13.2.5 — Tokenization: https://html.spec.whatwg.org/multipage/parsing.html#tokenization
- WHATWG HTML Living Standard §13.2.6 — Tree construction: https://html.spec.whatwg.org/multipage/parsing.html#tree-construction
- WHATWG HTML §13.1.2 — Void elements list: https://html.spec.whatwg.org/multipage/syntax.html#void-elements

---

## Known Limitations and TODOs

- `fetch()` in `crates/net` always returns `Err`. Real HTTP/1.1 over TLS requires `rustls` + `tokio` + `hickory-resolver` — planned for the next networking session.
- Boolean HTML attributes (e.g. `<input disabled>`) are silently dropped by the tokenizer. Documented with `// SPEC DEVIATION:` comment in `tokenizer.rs`.
- The tokenizer is not a full WHATWG state machine — ~80 states are required for full spec compliance; only the most common cases are handled.
- DOCTYPE tokens are recognised but not stored in the DOM.
- `<textarea>` raw-text mode is not yet handled (only `<script>` and `<style>`).
- CDATA sections are not handled.
- `boa_engine` is explicitly excluded from `crates/js` pending `docs/decisions/boa-audit.md`.

---

## Test Coverage

Unit tests are in `#[cfg(test)]` modules in the source files:

**`crates/net/src/url.rs`** (8 tests):
- `test_parse_url_https` — standard HTTPS URL
- `test_parse_url_http_remote_upgraded_to_https` — HTTPS enforcement
- `test_parse_localhost_http_not_upgraded` — localhost exemption
- `test_parse_url_no_scheme_defaults_to_https` — schemeless input
- `test_parse_url_no_path_defaults_to_slash` — missing path
- `test_parse_url_empty_returns_error` — error case
- `test_parse_url_unsupported_scheme` — ftp:// rejected
- `test_parse_127_0_0_1_is_localhost` — IP localhost detection

**`crates/html/src/tokenizer.rs`** (5 tests):
- `test_tokenize_simple_start_and_end_tag`
- `test_tokenize_self_closing_tag`
- `test_tokenize_comment`
- `test_tokenize_attribute_parsing`
- `test_tokenize_tag_names_are_lowercased`
- `test_tokenize_script_raw_text_mode`

**`crates/html/src/parser.rs`** (4 tests):
- `test_parse_simple_document`
- `test_parse_void_element_has_no_children`
- `test_parse_unclosed_element_auto_closes`
- `test_parse_comment_node`
