# Session: 2026-03-10 — Network Security Hardening (`crates/net`)

## Objective

Harden `crates/net` against security and privacy threats, as mandated by
`RULES-03-privacy.md` and `RULES-04-networking.md`, and as designed in
`docs/decisions/network-hardening.md`.

## Changes Made

### New Files
| File | Purpose |
|------|---------|
| `crates/net/src/hsts.rs` | In-memory HSTS store — parses `Strict-Transport-Security` headers and enforces HTTPS on repeat visits |
| `crates/net/src/context.rs` | `NetworkContext` — single policy chokepoint owning cached TLS, DNS, and HSTS state |

### Modified Files
| File | Change Summary |
|------|---------------|
| `crates/net/Cargo.toml` | Added `tokio/time` feature for timeout support |
| `crates/net/src/lib.rs` | Added `FetchError::Timeout`, `FetchError::ResponseTooLarge`; re-exports `NetworkContext` and `hsts` |
| `crates/net/src/url.rs` | Reject userinfo (`@`), strip fragments (`#`), reject IPv6 Zone IDs (`%` in `[]`) |
| `crates/net/src/tls.rs` | Explicitly pin TLS versions to `[TLS13, TLS12]` via `builder_with_protocol_versions`; return `Arc<ClientConfig>` |
| `crates/net/src/dns.rs` | `resolve()` accepts `&TokioResolver` (not building a new one per call); `build_resolver()` factory with cache + timeout opts |
| `crates/net/src/http.rs` | Add `Sec-Fetch-Site/Mode/Dest` headers; extract `Strict-Transport-Security` header in `Response` |
| `crates/net/src/client.rs` | 10s connect timeout; 30s read timeout; 50MB response cap; HSTS recording from response; redirect HTTPS-downgrade blocking; Location header length cap at 4096 bytes |
| `crates/net/tests/integration_test.rs` | 7 new security tests: userinfo, fragment, Zone ID, javascript scheme, HSTS store |

## Hardening Summary

| # | Threat | Mitigation |
|---|--------|------------|
| 1 | Phishing via `user:pass@host` URL | Rejected in `parse_url` with `UrlError::UserInfoNotAllowed` |
| 2 | OAuth token leakage via fragment forwarding | Fragment stripped before any DNS/TCP work |
| 3 | Local network probing via IPv6 Zone IDs | Rejected in `parse_url` with `UrlError::InvalidHost` |
| 4 | `javascript:` / `data:` URI injection | Rejected with `UrlError::UnsupportedScheme` |
| 5 | TLS version downgrade (BEAST, POODLE) | Explicit version pin `[TLS13, TLS12]` |
| 6 | DNS surveillance (ISP/LAN) | DoH-only via cached `TokioResolver` (Cloudflare 1.1.1.1) |
| 7 | Slowloris / infinite-hang | 10s connect + 30s read timeout |
| 8 | RAM exhaustion via large responses | 50MB cap in `read_all` |
| 9 | Redirect-based HTTPS downgrade | Re-parse via `parse_url` after every 3xx; hard-fail on `https→http` |
| 10 | HSTS not tracked across requests | In-memory `HstsStore` recording and enforcing HSTS policies |
| 11 | Redirect header injection | `Location` capped at 4096 bytes |
| 12 | Sec-Fetch metadata missing | `Sec-Fetch-Site/Mode/Dest` added to all outbound requests |

## Design Notes

- `NetworkContext` constructs the TLS config and DoH resolver once at startup.
  This amortises the cost of root CA store construction and preserves the DNS
  resolver's internal cache across all fetch calls in a session.
- `HstsStore` is behind `std::sync::Mutex<>` inside `NetworkContext` to allow
  multi-task access. In practice accesses are single-threaded per tab context.
- No `unwrap()` or `expect()` in library code. All errors return typed `FetchError` variants.

## Test Coverage

- All previous 18 URL parser tests continue to pass.
- 7 new integration tests covering each new security rule.
- All `hsts.rs` unit tests (7 cases) verify the store's record/enforce/expire logic.
