# Ferrum

**A privacy-first browser engine written in Rust.** In developement not yet useful as a browser.

---

## What Is This

Ferrum is a browser engine built from scratch in Rust. The goal is a privacy-first browser that treats privacy as an architectural constraint   
This started as a C++ prototype to learn how browsers work internally. That prototype proved the architecture; it has since been rewritten into a Rust Cargo workspace.

See [`docs/FEATURE_LIST.md`](docs/FEATURE_LIST.md) for detailed implementation status of every planned feature.

---

## Current Status

Early development. The networking layer is the most mature crate. The HTML tokenizer and parser work for common documents. Everything else is a stub.

| Crate | What It Does | Status |
|---|---|---|
| `crates/net` | HTTP/HTTPS, TLS, DoH DNS, HSTS, SSRF protection | ✅ Working |
| `crates/html` | HTML tokenizer + DOM tree builder | Partial (not full WHATWG state machine) |
| `crates/css` | CSS parsing and cascade | Stub |
| `crates/layout` | Box model and layout engine | Stub |
| `crates/render` | Painting and compositing | Stub |
| `crates/js` | JavaScript engine (Boa) | Stub — audit complete, integration not started |
| `crates/security` | Privacy/security policy module | Stub |
| `crates/browser` | Top-level binary | Minimal — fetches a URL, falls back to sample HTML |

---

## Building

Requires **Rust stable ≥ 1.85**.

```bash
# Check everything compiles
cargo check --workspace

# Run all tests
cargo test --workspace

# Lint
cargo fmt --check
cargo clippy --workspace -- -D warnings

# Security audit
cargo audit
```

CI runs `fmt`, `clippy`, `test`, and `audit` on every push against Linux x86_64 and Windows x86_64.

---

## Crate Structure

```
crates/
├── net/        All networking. The only crate that touches sockets or TLS.
├── html/       HTML tokenizer and DOM (arena allocation via bumpalo)
├── css/        CSS parsing and cascade (stub)
├── layout/     Box model and layout engine (stub)
├── render/     Painting and compositing (stub)
├── js/         JavaScript engine — Boa (stub)
├── security/   Privacy/security policy (stub)
└── browser/    Top-level binary. Wires crates together.
```

All network requests pass through `NetworkContext` in `crates/net`. No other crate makes socket calls. This is the central privacy enforcement chokepoint.

---

## What `crates/net` Implements

The networking layer is production-hardened against real threat classes:

- **HTTPS enforcement** — silent upgrade; downgrades hard-fail (no warn-and-proceed)
- **DNS-over-HTTPS** — system DNS never used; Cloudflare 1.1.1.1 via hickory-resolver
- **SSRF protection** — RFC 1918, CGN (`100.64.0.0/10`), loopback (`127.0.0.0/8`), IPv4-mapped IPv6 all blocked
- **TLS** — rustls with TLS 1.2 minimum pin; no OpenSSL; no system cert bypass
- **HSTS** — in-memory store; records and enforces on repeat requests within a session
- **Redirect hardening** — max 5 hops; downgrade guard; loopback redirect blocked
- **Request hardening** — CRLF injection blocked; port 0 rejected; `Sec-Fetch-*` headers sent
- **Response hardening** — 50 MB cap; 10s connect timeout; 30s read timeout

Not yet implemented in `crates/net`: response decompression (gzip/brotli), connection pooling, cookie jar, sub-resource loading. Roadmap: [`docs/sessions/2026-03-19_network-stack-roadmap.md`](docs/sessions/2026-03-19_network-stack-roadmap.md).

---

## JavaScript

The plan is to integrate [Boa](https://boajs.dev/) — a pure-Rust ECMAScript bytecode VM. No V8, no SpiderMonkey, no C++ JS engine. The audit is complete (`docs/decisions/boa-audit.md`); integration into `crates/js` has not started.

---

## Privacy Goals

These are the intended defaults once the engine reaches a usable state. Items marked ✅ are enforced now; others are planned.

- ✅ HTTPS enforced, downgrades hard-fail
- ✅ DNS-over-HTTPS — no plaintext DNS
- ✅ TLS certificate errors hard-fail
- Planned: Third-party cookies blocked at network layer
- Planned: HSTS preload list compiled into the binary
- Planned: Canvas fingerprinting — return noise for cross-origin contexts
- Planned: Font enumeration blocked
- Planned: No disk storage of history, cookies, or cache without explicit opt-in
- Planned: JavaScript disabled by default, enabled per-site

---

## Contributing

Read `.agents/Rules.md` before touching any code — it is the source of truth for coding standards, commit discipline, and dependency policy.

In brief:
- Rust 2024 edition. No C, no C++.
- No `unwrap()` or `expect()` in library code.
- Every public item needs a `///` doc comment.
- Session log in `docs/sessions/` for every meaningful work session.
- `cargo fmt` and `cargo clippy -D warnings` must pass before every commit.
- No new dependency without justification.

---

## License

[Mozilla Public License Version 2.0](LICENSE) — file-level copyleft. Modified source files must be published under MPL 2.0; embedding the engine in a closed-source application is permitted.

---

## AI Usage

This project uses AI assistance. See [`AI_Usage_Disclosure.md`](AI_Usage_Disclosure.md) for a full breakdown of what was AI-assisted and what was done manually.