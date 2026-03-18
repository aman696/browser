# Ferrum

**A privacy-first browser engine written in Rust.**

Ferrum renders the modern web without surveilling you while doing it. No telemetry. No background network calls. No trackers. No AI features. Just a browser that gets out of the way.

---

## What Ferrum Is

Ferrum is a browser engine being rewritten from a C++ prototype into idiomatic Rust. The rewrite is not a line-for-line translation — it is a ground-up reimplementation using Rust's ownership model, arena allocation, and async I/O to eliminate the classes of bugs that make browsers a primary attack surface.

The privacy model is not a feature toggle. It is the architecture. DNS queries are encrypted by default via DNS-over-HTTPS. HTTPS is enforced and downgrades hard-fail. History and cache do not touch disk unless you explicitly opt in. Third-party cookie blocking, canvas fingerprinting noise, and the HSTS preload list are planned defaults — not yet implemented.

The browser is designed to work on the real modern web — not a sandboxed subset of it. If a mainstream site requires it, Ferrum supports it. Compatibility is not sacrificed for privacy; they are managed as a tradeoff, transparently, with the user in control.

---

## Why Rust

Browsers parse untrusted input constantly — malformed HTML, adversarial CSS, hostile JavaScript. A use-after-free or buffer overflow in a browser is not a minor bug; it is a security vulnerability.

The C++ prototype proved the architecture. Rust is how it becomes production-safe. Memory safety is enforced at compile time. There is no garbage collector introducing latency pauses. The codebase is auditable without needing to reason about pointer ownership by hand.

No OpenSSL. No system resolver. No C FFI except where strictly contained (the `aws-lc-rs` crypto backend inside `rustls`).

---

## Current Status

**Early development.** The project is pre-alpha — not yet useful as a browser. The Rust workspace is set up and functional:

| Component | Status |
|---|---|
| Workspace structure (`crates/`) | ✅ Complete |
| URL parser (`crates/net`) | ✅ Complete — HTTPS enforcement, full test suite |
| HTML tokenizer (`crates/html`) | ✅ Complete — raw-text mode, WHATWG-aligned |
| HTML parser + DOM (`crates/html`) | ✅ Complete — arena allocation, error recovery |
| HTTP/HTTPS fetch (`crates/net`) | ✅ Complete — TLS (rustls), DoH (hickory), HSTS, redirects, SSRF guard |
| CSS parsing (`crates/css`) | 🔲 Not started |
| Layout engine (`crates/layout`) | 🔲 Not started |
| Renderer (`crates/render`) | 🔲 Not started |
| JavaScript engine (`crates/js`) | 🔲 Pending — `boa_engine` audit complete, integration not yet started |
| Security / privacy policy (`crates/security`) | 🔲 Stub only — not yet wired to network layer |

---

## Architecture

```
crates/
├── net/        HTTP/HTTPS networking. Owns all sockets and TLS. Nothing else touches the network.
├── html/       WHATWG-spec tokenizer and DOM (arena allocation via bumpalo/typed-arena)
├── css/        CSS parsing and cascade
├── layout/     Box model and layout engine
├── render/     Painting and compositing
├── js/         JavaScript engine (Boa — pending audit)
├── security/   TLS policy, cookies, HSTS, content blocking, permission store
└── browser/    Top-level binary. Wires crates together. No business logic.
```

Each crate has a single responsibility. No circular dependencies. No global mutable state. The network stack is a chokepoint — all requests pass through `NetworkContext` where privacy policy is enforced. There is no way for a crate to make an arbitrary socket call that bypasses policy.

---

## Privacy Model

These are defaults, not options:

- **Third-party cookies blocked.** *(Planned)* Cross-site tracking via cookies will be off at the network layer.
- **HTTPS enforced.** ✅ Downgrade attempts hard-fail. No warn-and-proceed for HTTP on HTTPS pages.
- **HSTS preload list shipped with the binary.** *(Planned)* In-memory HSTS store is implemented; compiled-in preload list is not yet added.
- **DNS-over-HTTPS.** ✅ Plaintext DNS is banned. All queries go through Cloudflare 1.1.1.1 via DoH by default.
- **No DNS prefetch, no prefetch/preconnect.** These leak browsing intent to servers you never chose to contact.
- **Canvas fingerprinting returns noise** for cross-origin contexts.
- **Font enumeration blocked.** Your installed font list is a fingerprint.
- **Battery API returns null.** Battery level is a fingerprinting vector.
- **Referrer policy: `strict-origin-when-cross-origin`.** Pages cannot override this to expose full URLs to third parties.
- **No disk storage of history, cookies, or cache** without explicit user opt-in. Everything lives in memory and is gone on close.
- **Certificate errors hard-fail.** No click-through for invalid certificates.

### Privacy Warnings

When a site is on a tracker blocklist, uses forced third-party cookies, has no HTTPS, or presents an invalid certificate, Ferrum shows a full-page interstitial before loading anything. The warning is rendered locally — no network calls to generate it. It shows:

1. The domain being visited
2. The specific reason it was flagged (not a generic "this may be unsafe")
3. Exactly what will be enabled if you proceed
4. Two buttons: **Go back** (default focus) and **I understand — continue to [domain]**

Permissions granted on the warning page apply to the current session only unless you check "Remember this for [domain]". They are stored in a plain `permissions.toml` you can inspect and edit. Permissions are per-origin — granting `example.com` does not grant `tracker.example.com`.

---

## JavaScript Engine

Ferrum uses [Boa](https://boajs.dev/) — a pure-Rust ECMAScript bytecode compiler and VM. Current target version: v0.21 (94.12% test262 conformance, October 2025). Boa has no C++ dependencies, no network calls, and its full source is auditable.

No V8. No SpiderMonkey. No JavaScriptCore. Embedding a C++ JS engine would be a memory-safety regression that undermines the entire reason for using Rust.

JavaScript is disabled by default and enabled per-site via the permission system.

### Bytecode VM, not JIT — and why

Most browsers use a JIT (Just-In-Time) compiler: JavaScript is compiled to native machine code on the fly as hot code paths are detected, then executed directly by the CPU. This is fast. It is also a significant attack surface.

Ferrum uses a bytecode VM: JavaScript is compiled to bytecode, which the VM interprets instruction by instruction. No native machine code is generated at runtime.

The speed tradeoff is real and worth being honest about:

**Sites that will work fine** — the vast majority of the web. News, social media, email, banking, GitHub, YouTube, shopping. The JavaScript on these pages is event-driven: respond to a click, fetch data, update the DOM. A bytecode VM handles this without issue.

**Sites that will be noticeably slower** — compute-heavy workloads: browser-based 3D games (Unity WebGL, Three.js), Figma, large Google Docs/Sheets with complex formulas, in-browser crypto key derivation. These rely on tight loops running at high throughput. The difference between a JIT and a bytecode VM is perceptible here.

**Sites that may not work acceptably** — real-time WebGL at 60fps, heavy in-browser video processing. These genuinely need JIT-level throughput.

The reasoning for accepting this tradeoff:

1. **JIT spraying is a documented browser attack class.** An attacker can craft JavaScript that causes the JIT compiler to write predictable patterns of machine code into executable memory, which they then exploit. For a browser whose identity is security and privacy, accepting this attack surface contradicts the core design.

2. **The sites that need JIT speed are often the same sites with the most aggressive tracking.** Figma, Google Docs, WebGL-heavy games — these are not where privacy-conscious users spend most of their time.

3. **The JIT compiler itself is the largest source of complexity and CVEs in V8 and SpiderMonkey.** Ferrum does not have the engineering resources to audit and maintain a JIT safely.

4. **This is revisitable.** Boa's roadmap includes JIT work. If a well-audited JIT becomes available within the Boa ecosystem, the decision can be reconsidered. But "fast enough for everyday web use" is the correct bar to clear first.

The Boa dependency audit is complete and documented in `docs/decisions/boa-audit.md`. Decision: integrate as-is (v0.21.0). Integration into `crates/js` is the next step.

---

## Networking Stack

| Component | Library | Reason |
|-----------|---------|--------|
| TLS | `rustls` 0.23.36 | Pure Rust TLS. No OpenSSL. No Heartbleed-class vulnerabilities. |
| DNS | `hickory-resolver` 0.25.2 | No system resolver (plaintext DNS = ISP surveillance). Supports DoH. |
| Async runtime | `tokio` | Standard Rust async I/O. |
| HTTP | HTTP/1.1 first, HTTP/2 planned | Build the foundation right before adding complexity. |

Note: `rustls` uses `aws-lc-rs` as its default crypto backend, which links a C library. This is an accepted and documented tradeoff — the C is contained entirely within the crypto provider. The protocol implementation is pure Rust.

---

## Web Compatibility

Ferrum targets the real modern web:

- HTML5 (WHATWG Living Standard)
- CSS3: Grid, Flexbox, custom properties, media queries, responsive images
- ES2022 JavaScript
- WebSockets, Fetch API, Web Workers, Canvas (with fingerprinting protection)
- WOFF2 web fonts
- gzip, deflate, brotli compression
- Chunked transfer encoding
- HTTP redirects

Graceful degradation for unimplemented features — the browser must not crash or show a blank page because one CSS feature is missing. Compatibility regressions are tracked against real sites in `docs/compatibility-notes.md`.

---

## Browser UI

The UI gets out of the way. The user came to visit a website, not to interact with the browser.

**Start page:** Rendered entirely locally. No remote content, no news feed, no search suggestions loaded from a server, no "top sites" that phone home. A search bar and nothing else. Fast.

**Address bar:** Shows the full URL. Never hides the protocol. HTTP sites display a persistent "Not secure" label — not a popup, always visible. Autocomplete from local history and bookmarks only. No keystrokes sent to a search engine before you press Enter.

**Toolbar:** Back, forward, reload, address bar, privacy indicator, menu. Nothing else by default. The privacy indicator shows at a glance whether JavaScript is on, whether a tracker was blocked, and whether exceptions are active — one icon, expandable on click.

**No onboarding flow.** No setup wizard. No tour. A first-time user should be able to navigate immediately.

**Error pages** are honest and human-readable. "This site's certificate has expired" — not `ERR_CERT_DATE_INVALID`. Technical detail is available in a collapsed section for those who want it.

Everything is keyboard navigable. `Ctrl+L` for the address bar. `Ctrl+T` for a new tab. Every action reachable without a mouse.

---

## What Ferrum Will Never Have

- Telemetry, analytics, or crash reporting — not even opt-in
- Auto-updates that phone home
- AI features of any kind
- Smart address bar suggestions that call external APIs
- Sponsored content or partner integrations
- DRM (DRM systems are identification systems by design)
- Bundled tracking SDKs

---

## Building

```bash
# Requires Rust stable ≥ 1.85 (project MSRV)
rustup update stable

# Check the workspace compiles
cargo check --workspace

# Run all tests
cargo test --workspace

# Lint (must pass before committing)
cargo fmt --check
cargo clippy --workspace -- -D warnings

# Security audit
cargo audit
```

Tier-1 targets: Linux x86_64, Linux aarch64, Windows x86_64. macOS: best effort.

---

## Contributing

Read `RULES.md` before writing any code. It is the single source of truth for how this codebase is written, structured, and maintained.

Key rules in brief:

- Rust 2024 edition. No C, no C++.
- No `unwrap()` in library code.
- Every public item needs a `///` doc comment.
- Every commit needs a session log in `docs/sessions/`.
- `cargo fmt` and `cargo clippy -D warnings` before every commit.
- No new dependency without justification in the PR.

The AI assistant working on this project reads `RULES.md` first, every time, before touching any code. On first load it checks whether `docs/FEATURE_LIST.md` exists and creates it if not.

---

## License

Ferrum is licensed under the **Mozilla Public License Version 2.0 (MPL 2.0)**.

MPL 2.0 is a simple, file-level copyleft license. This means:
- You are free to use Ferrum's browser engine crates as a library inside your own closed-source, proprietary application.
- However, if you modify Ferrum's *original source files* directly to improve the engine, you must publish those specific files under the same MPL 2.0 license so the community benefits.
- It strikes a balance: it protects the open-source nature of the browser engine itself, but does not aggressively "infect" external proprietary apps that merely embed it.

See [LICENSE](LICENSE) for the full text.

---

*Ferrum. It renders pages. It does not think about them, summarize them, or phone home about them.*