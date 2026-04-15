# Ferrum Browser — Feature List

> Last updated: 2026-04-15

---

## Core Rendering Pipeline
- ✅ HTTP/HTTPS networking (crates/net) — `client.rs`; full fetch pipeline with redirect handling, timeouts, size cap
- ✅ URL parsing and validation — `url.rs`; HTTPS enforcement, userinfo/fragment/zone-id rejection, CRLF guard, port 0 rejection
- ✅ DNS resolution via hickory-resolver — `dns.rs`
- ✅ DNS-over-HTTPS (DoH) — `dns.rs`; Cloudflare upstream, SSRF guard covering RFC 1918, CGNAT, loopback `127.0.0.0/8`, IPv4-mapped IPv6
- [ ] HTML tokenizer (WHATWG spec) — **partial**; raw-text mode for `<script>`/`<style>`, DOCTYPE, basic start/end/self-closing/comment tokens. `SPEC DEVIATION:` comments in source. Boolean attributes, CDATA, `<textarea>` raw-text, and ~50 WHATWG states not yet implemented.
- [ ] HTML parser and DOM tree construction (WHATWG spec) — **partial**; stack-based builder with bumpalo arena allocation, auto-close, `Cell<Option<&Node>>` parent pointers. Not the full WHATWG tree-construction algorithm.
- [ ] CSS parser — **stub only** — `parse_css()` returns `CssError::NotImplemented`; roadmap in `docs/sessions/2026-03-19_css-parser-roadmap.md`
- [ ] CSS cascade and computed style resolution — not started
- [ ] Box model and layout engine — not started
- [ ] Paint and compositing (crates/render) — not started
- [ ] Image decoding (PNG, JPEG, WebP, AVIF) — not started
- [ ] Web fonts (WOFF2) — not started

## JavaScript
- ✅ Boa audit (docs/decisions/boa-audit.md) — complete; decision: integrate as-is (`boa_engine v0.21.0` is pure Rust, bytecode VM, zero network capability)
- [ ] Boa integration into crates/js — not started; `crates/js` is a stub pending integration
- [ ] DOM API bindings (document, window, element) — not started
- [ ] Fetch API (routed through NetworkContext) — not started
- [ ] Event system (click, input, scroll, etc.) — not started
- [ ] Per-site JavaScript enable/disable toggle — not started

## Browser UI
- [ ] Address bar with URL input — not started
- [ ] Tab management (open, close, switch) — not started
- [ ] Back / Forward / Reload navigation — not started
- [ ] Start page (minimalist, local, no remote content) — not started
- [ ] Privacy warning interstitial — not started
- [ ] Per-site permission manager UI — not started
- [ ] Settings page (search engine, JavaScript toggle, cookie policy) — not started
- [ ] Bookmarks (local storage only) — not started
- [ ] Download manager — not started

## Privacy & Security
- ✅ HTTPS enforcement (hard fail on downgrade) — `url.rs` silent upgrade + `client.rs` redirect downgrade guard
- ✅ HSTS in-memory store (session-scoped) — `hsts.rs`; two-phase eviction, `record_from_header`, subdomain matching
- [ ] Third-party cookie blocking (default on) — not started; no cookie jar implemented yet
- [ ] Tracker blocklist (EasyPrivacy / Disconnect.me) — not started
- [ ] Privacy warning interstitial for flagged domains — not started
- [ ] Canvas fingerprinting protection — not started
- [ ] Font enumeration blocking — not started
- [ ] Battery API sanitisation — not started
- [ ] Referrer policy enforcement — not started; `Sec-Fetch-*` metadata headers are sent on requests but no general `Referer` stripping/policy applied
- ✅ Certificate validation (hard fail) — rustls strict validation in `tls.rs`; TLS 1.2 minimum version pinned; no custom `ServerCertVerifier` bypasses
- [ ] Content Security Policy (CSP) enforcement — not started
- [ ] Per-site permission store (permissions.toml) — not started

## Platform
- ✅ Linux x86_64 build — gated in CI (`ubuntu-latest`; fmt + clippy + tests + audit on every push/PR)
- [ ] Linux aarch64 build — not started
- ✅ Windows x86_64 build — gated in CI (`windows-latest`; build + tests on every push/PR)
- ✅ Cross-platform CI (GitHub Actions) — `.github/workflows/CI.yml`; Linux + Windows on every push/PR

## Developer / Project Hygiene
- ✅ cargo fmt enforced — gated in CI (`cargo fmt --check` on every push/PR)
- ✅ cargo clippy -D warnings enforced — gated in CI (`cargo clippy --workspace -- -D warnings`)
- ✅ cargo audit passing — gated in CI; 1 allowed warning (`paste` crate unmaintained, compile-time only, zero runtime impact; see `docs/decisions/boa-audit.md`)
- [ ] Fuzz targets for HTML tokenizer — not started
- [ ] Fuzz targets for CSS parser — not started
- [ ] Integration tests for networking — not started; unit tests exist in `crates/net` and `crates/html`
- ✅ Session log system (docs/sessions/) — 5 session logs to date
- ✅ Architecture decision records (docs/decisions/) — 2 ADRs: `boa-audit.md`, `network-hardening.md`
