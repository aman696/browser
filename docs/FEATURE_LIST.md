# Ferrum Browser — Feature List

> Generated from RULES.md. Keep this file updated as features are completed.
> Last updated: 2026-03-10

---

## Core Rendering Pipeline
- [ ] HTTP/HTTPS networking (crates/net)
- [ ] URL parsing and validation
- [ ] DNS resolution via hickory-resolver
- [ ] DNS-over-HTTPS (DoH)
- [ ] HTML tokenizer (WHATWG spec)
- [ ] HTML parser and DOM tree construction (WHATWG spec)
- [ ] CSS parser
- [ ] CSS cascade and computed style resolution
- [ ] Box model and layout engine
- [ ] Paint and compositing (crates/render)
- [ ] Image decoding (PNG, JPEG, WebP, AVIF)
- [ ] Web fonts (WOFF2)

## JavaScript
- [ ] Boa audit (docs/decisions/boa-audit.md)
- [ ] Boa integration into crates/js
- [ ] DOM API bindings (document, window, element)
- [ ] Fetch API (routed through NetworkContext)
- [ ] Event system (click, input, scroll, etc.)
- [ ] Per-site JavaScript enable/disable toggle

## Browser UI
- [ ] Address bar with URL input
- [ ] Tab management (open, close, switch)
- [ ] Back / Forward / Reload navigation
- [ ] Start page (minimalist, local, no remote content)
- [ ] Privacy warning interstitial
- [ ] Per-site permission manager UI
- [ ] Settings page (search engine, JavaScript toggle, cookie policy)
- [ ] Bookmarks (local storage only)
- [ ] Download manager

## Privacy & Security
- [ ] HTTPS enforcement (hard fail on downgrade)
- [ ] HSTS preload list shipped with binary
- [ ] Third-party cookie blocking (default on)
- [ ] Tracker blocklist (EasyPrivacy / Disconnect.me)
- [ ] Privacy warning interstitial for flagged domains
- [ ] Canvas fingerprinting protection
- [ ] Font enumeration blocking
- [ ] Battery API sanitisation
- [ ] Referrer policy enforcement
- [ ] Certificate validation (hard fail)
- [ ] Content Security Policy (CSP) enforcement
- [ ] Per-site permission store (permissions.toml)

## Platform
- [ ] Linux x86_64 build
- [ ] Linux aarch64 build
- [ ] Windows x86_64 build
- [ ] Cross-platform CI (GitHub Actions or equivalent)

## Developer / Project Hygiene
- [ ] cargo fmt enforced
- [ ] cargo clippy -D warnings enforced
- [ ] cargo audit passing
- [ ] Fuzz targets for HTML tokenizer
- [ ] Fuzz targets for CSS parser
- [ ] Integration tests for networking
- [ ] Session log system (docs/sessions/)
- [ ] Architecture decision records (docs/decisions/)
