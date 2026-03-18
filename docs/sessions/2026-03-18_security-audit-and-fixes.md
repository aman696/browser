# Session: 2026-03-18 — Security Audit and Code Quality Fixes

## Objective

Full codebase audit across all source files: security vulnerabilities,
rules violations, documentation drift, and code quality issues. All
identified security bugs fixed in sequence, each committed and pushed
independently.

---

## Audit Findings (Phase 1–4)

A structured four-phase audit was performed:
1. Bootstrap — all RULES files, FEATURE_LIST, session logs, decision records
2. Documentation sync check — stale docs, untracked claims
3. Security and quality scan — every source file
4. Report — 20 numbered findings across SECURITY, BUG, RULES-VIOLATION, DOC-DRIFT, QUALITY

---

## Security Fixes

### 1. IPv4-mapped IPv6 SSRF bypass (`crates/net/src/dns.rs`)

`is_private_ip()` did not handle the `::ffff:0:0/96` prefix.
`hickory_resolver` returns AAAA records as `IpAddr::V6` without normalising
the `::ffff:` prefix — an attacker-controlled record of `::ffff:192.168.1.1`
passed the private-IP check and allowed SSRF to internal IPv4 hosts.

**Fix:** Call `to_ipv4_mapped()` at the top of the IPv6 arm and recurse into
the IPv4 branch, so the full RFC 1918 / loopback / link-local range table
applies regardless of address encoding.

**Commits:** `ba40b70`, `781d749` (fmt fix)

---

### 2. SSRF via redirect to non-`127.0.0.1` loopback addresses (`crates/net/src/url.rs`, `crates/net/src/client.rs`)

`parse_url()` used a narrow localhost check (`localhost` | `127.0.0.1` only).
A server-issued redirect to `https://127.0.0.2:6379/` bypassed HTTPS port
enforcement (already HTTPS, so no upgrade fired), bypassed the downgrade
guard (`is_https=true`), and bypassed the private-IP check in `dns::resolve`
because `is_localhost_host("127.0.0.2")=true`.

**Fix:**
- `parse_url()`: widen `is_localhost` to `is_localhost_host()` covering full `127.0.0.0/8`
- `client.rs`: add explicit guard blocking any redirect from a non-loopback origin to a loopback address, regardless of scheme

**Commit:** `f4a6e34`

---

### 3. Missing RFC-reserved IP ranges in SSRF guard (`crates/net/src/dns.rs`)

`is_private_ip()` was missing several address ranges:
- `100.64.0.0/10` — Carrier-Grade NAT (RFC 6598), used internally by ISPs
- `192.0.0.0/24` — IETF protocol assignments (RFC 6890)
- `192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24` — TEST-NET ranges (RFC 5737)

Also replaced manual range comparison with `(16..=31).contains()` to satisfy
`clippy -D warnings`.

**Commit:** `3afce7f`

---

### 4. CRLF injection via URL path (`crates/net/src/url.rs`)

`\r` or `\n` in the path were injected verbatim into the HTTP/1.1 request line,
allowing an attacker-controlled URL to terminate the request line early and
inject arbitrary headers.

**Fix:** Added `UrlError::InvalidPath` variant and a check in `parse_url()`
rejecting any path containing `\r` or `\n`. Covers path and query string.

**Commit:** `63edb3b`

---

### 5. Chunked decoder OOB panic on missing trailing CRLF (`crates/net/src/http.rs`)

`chunk_end > data.len()` was checked, but `chunk_end + 2 > data.len()` was not.
A hostile server omitting the `\r\n` after chunk data caused `pos` to overshoot,
potentially panicking on the next iteration's slice index.

**Fix:** Added explicit bounds check and byte verification that `data[chunk_end..chunk_end+2] == b"\r\n"` before advancing.

**Commit:** `6420c73`

---

## Bug Fixes

### 6. Port 0 rejection (`crates/net/src/url.rs`)

Port 0 tells the OS to assign an ephemeral port — it is never a valid
destination for an outbound HTTP request. Added rejection with `InvalidHost`.

**Commit:** `5997dce`

---

## Performance Fixes

### 7. O(n²) DOCTYPE detection in tokenizer (`crates/html/src/tokenizer.rs`)

`html[i..].to_ascii_lowercase().starts_with("<!doctype")` allocated and
lowercased the entire remaining input on every `<` character. Fixed by slicing
to exactly 9 bytes and using `eq_ignore_ascii_case()` — zero allocation, O(1).

**Commit:** `65e4fad`

---

## Refactors

### 8. `HashMap` → `Vec` for DOM and token attributes (`crates/html/`)

`HashMap<String, String>` was wrong for two reasons:
- WHATWG §13.1.2.3 requires attribute order preservation for serialization
- HTML elements have 1–3 attributes on average — hashing is pure overhead

Replaced with `Vec<(String, String)>` in `Token`, `Node`, and `parse_attributes()`.

**Commit:** `b17bea7`

---

### 9. Arena-allocate `text_content` in `Node` (`crates/html/src/dom.rs`)

`text_content: String` bypassed the bumpalo arena, creating a separate heap
allocation per Text and Comment node. Changed to `text_content: &'arena str`
using `arena.alloc_str()` to keep all node data inside the arena.

**Commit:** `a214202`

---

## Rules / Documentation Fixes

### 10. Chore batch (`crates/html/`, `crates/js/`, `crates/net/`)

- `html/parser.rs`: commented out two `eprintln!` calls banned in library crates (RULES-02)
- `js/lib.rs`: updated stale doc — boa-audit.md now exists, decision is integrate as-is
- `net/tls.rs`: removed stray blank line splitting the module doc block
- `net/http.rs`: added proper `REASON:` comment to `_header_bytes` (RULES-09)
- `net/hsts.rs`: corrected `is_hsts()` doc — entries are not pruned on lookup, only read past

**Commits:** `c6776dd`, `b7fc284`

---

## Housekeeping

- `.gitignore`: added `.agent/` and `Rules` patterns
- `Rules.md`: deleted from root (superseded by `.agents/rules/`)
- `net/url.rs`: `is_localhost()` commented out (dead code, clippy -D error, RULES-09)

**Commits:** `e063c7f`, `49e57aa`

---

## Test Coverage

All existing tests continue to pass. New regression tests added:
- IPv4-mapped IPv6 private ranges (`::ffff:192.168.1.1`, etc.)
- CGNAT and IETF reserved ranges
- CRLF in URL path
- Chunked decoder with missing/wrong trailing CRLF
- Port 0 rejection
- `127.x.x.x` loopback block as localhost
- DOCTYPE tokenizer correctness after slice fix
- Attribute order preservation via Vec

---

## Commits in Order

| Commit | Description |
|--------|-------------|
| `ba40b70` | fix(net/dns): patch SSRF bypass via IPv4-mapped IPv6 addresses |
| `781d749` | style: apply cargo fmt to dns.rs |
| `e063c7f` | chore: housekeeping and expose localhost helper utilities |
| `49e57aa` | fix(net/url): comment out unused is_localhost() |
| `f4a6e34` | fix(net): block redirects to internal loopback addresses |
| `3afce7f` | fix(net/dns): extend SSRF guard to cover CGNAT and IETF reserved ranges |
| `c6776dd` | chore: fix rules violations and stale documentation |
| `b7fc284` | docs(net/hsts): correct is_hsts() doc comment |
| `63edb3b` | fix(net/url): reject CRLF characters in URL path |
| `6420c73` | fix(net/http): prevent OOB panic in chunked decoder |
| `5997dce` | fix(net/url): reject port 0 in URL parsing |
| `65e4fad` | perf(html/tokenizer): fix O(n²) DOCTYPE detection |
| `b17bea7` | refactor(html): replace HashMap with Vec for attributes |
| `a214202` | refactor(html/dom): arena-allocate text_content in Node |
