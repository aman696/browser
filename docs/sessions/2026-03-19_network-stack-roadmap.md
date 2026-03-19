# Network Stack Roadmap — Ferrum Browser

> A complete implementation plan for `crates/net`. Covers what a privacy-first
> Rust browser shipping in 2026 must support, split into a prototype milestone
> (loads real pages end-to-end) and a production milestone (handles the full
> modern web reliably).

---

## Current State

The net crate is the most mature crate in the workspace. Already implemented:

| Feature | Status | File |
|---|---|---|
| URL parsing + validation | Done | `url.rs` |
| HTTPS enforcement (silent upgrade) | Done | `url.rs` |
| Userinfo/fragment/zone-id rejection | Done | `url.rs` |
| CRLF injection guard | Done | `url.rs` |
| Port 0 rejection | Done | `url.rs` |
| TLS via rustls (aws-lc-rs) | Done | `tls.rs` |
| TLS 1.2+ version pin | Done | `tls.rs` |
| DNS-over-HTTPS (hickory + Cloudflare) | Done | `dns.rs` |
| SSRF guard (private IP blocking) | Done | `dns.rs` |
| IPv4-mapped IPv6 SSRF guard | Done | `dns.rs` |
| HTTP/1.1 request building | Done | `http.rs` |
| HTTP/1.1 response parsing (httparse) | Done | `http.rs` |
| Chunked transfer decoding | Done | `http.rs` |
| Sec-Fetch-* metadata headers | Done | `http.rs` |
| HSTS store (in-memory, two-phase eviction) | Done | `hsts.rs` |
| NetworkContext (central policy chokepoint) | Done | `context.rs` |
| Redirect following (max 5, HTTPS-downgrade guard) | Done | `client.rs` |
| Loopback redirect SSRF guard | Done | `client.rs` |
| Connect timeout (10s) | Done | `client.rs` |
| Read timeout (30s) | Done | `client.rs` |
| Response size cap (50MB) | Done | `client.rs` |

**What is NOT done** is everything below. The current implementation can fetch a
single HTML page. It cannot load sub-resources, handle cookies, decompress
responses, speak HTTP/2, or manage connections.

---

## How a Browser Network Stack Works — Full Pipeline

```
  User types URL / clicks link
         │
  ┌──────▼──────────┐
  │ URL Parse +     │  Validate, enforce HTTPS, strip fragments
  │ Security Check  │  Check blocklist, permissions, HSTS
  └──────┬──────────┘
         │
  ┌──────▼──────────┐
  │ Cookie Attach   │  Match cookies for this origin, attach Cookie header
  └──────┬──────────┘
         │
  ┌──────▼──────────┐
  │ DNS Resolution  │  DoH lookup (with cache)
  └──────┬──────────┘
         │
  ┌──────▼──────────┐
  │ Connection Pool  │  Reuse existing TCP+TLS connection or open new one
  └──────┬──────────┘
         │
  ┌──────▼──────────┐
  │ TLS Handshake   │  rustls — certificate validation, OCSP, CT
  └──────┬──────────┘
         │
  ┌──────▼──────────┐
  │ HTTP Request    │  Build request with headers (Accept, Cookie, etc.)
  └──────┬──────────┘
         │
  ┌──────▼──────────┐
  │ HTTP Response   │  Parse status + headers, decompress body
  │ Processing      │  Record HSTS, record cookies, follow redirects
  └──────┬──────────┘
         │
  ┌──────▼──────────┐
  │ Sub-resource    │  Parse HTML → discover CSS, JS, images, fonts
  │ Discovery       │  Queue all sub-resource fetches
  └──────┬──────────┘
         │
  ┌──────▼──────────┐
  │ Parallel Fetch  │  Fetch sub-resources with connection pooling
  │ + Priority      │  CSS > JS > fonts > images (render-blocking first)
  └──────────────────┘
```

---

## Prototype Milestone — "Loads Real Pages End-to-End"

The goal is to load a real webpage — HTML, CSS, images — and hand the resources
to the rendering pipeline. Single-page loads with basic sub-resources. No
persistent state across sessions.

### Phase P1: Response Decompression

**Why now:** Most web servers send compressed responses by default. Without
decompression, many servers return garbled data or refuse to serve content
without `Accept-Encoding`.

| Encoding | Prevalence | Crate |
|---|---|---|
| gzip | ~95% of servers | `flate2` |
| deflate | ~30% of servers | `flate2` |
| brotli | ~80% of HTTPS servers | `brotli` |
| zstd | Growing (~10%) | `zstd` (defer to production) |

**Implementation:**
1. Add `Accept-Encoding: gzip, deflate, br` to `build_request` in `http.rs`
2. Read `Content-Encoding` response header
3. Decompress body before returning from `read_all`
4. Handle `Transfer-Encoding: chunked` + `Content-Encoding: gzip` stacking

**Privacy note:** Not sending `Accept-Encoding` is itself a fingerprint — Ferrum
is currently identifiable as "the browser that never requests compression."
Adding this fixes both a compatibility and a privacy issue.

**Deliverable:** Transparent decompression. Callers of `fetch()` get the
decompressed body. Tests for gzip, deflate, and brotli responses.

---

### Phase P2: Content-Length Aware Buffer Sizing

The `read_all` function currently allocates a fixed 64KB buffer regardless of
response size. When `Content-Length` is present in headers (it usually is for
static resources), pre-size the buffer to avoid repeated reallocations.

```rust
let capacity = content_length.unwrap_or(64 * 1024).min(MAX_RESPONSE_SIZE);
let mut buf = Vec::with_capacity(capacity);
```

Small change, significant impact on memory allocation patterns for image and
font downloads.

---

### Phase P3: Sub-resource Loading

**Why it matters:** A bare HTML page without CSS, images, or JS is useless.
The browser must discover and fetch sub-resources referenced in the HTML.

**Resources to discover from HTML:**
| Tag | Attribute | Resource type |
|---|---|---|
| `<link rel="stylesheet">` | `href` | CSS |
| `<script>` | `src` | JavaScript |
| `<img>` | `src`, `srcset` | Image |
| `<link rel="icon">` | `href` | Favicon |
| `<style>` | (inline) | CSS (no fetch needed) |
| `<link rel="preload">` | `href` | Any (defer to production) |

**Loading order / priority:**
1. **Render-blocking:** `<link rel="stylesheet">` — must complete before first paint
2. **Parser-blocking:** `<script>` without `async`/`defer` — blocks HTML parsing
3. **Non-blocking:** `<img>`, `<script async>`, `<script defer>` — parallel

**Implementation:**
1. Walk the DOM tree after HTML parse
2. Collect all resource URLs with their type and priority
3. Resolve relative URLs against the document's base URL
4. Fetch through `NetworkContext` (all privacy policies apply)
5. Return resources keyed by URL for the rendering pipeline to consume

**Base URL resolution:** `<base href="...">` tag changes the base URL for all
relative URLs in the document. Must handle this correctly.

**Deliverable:** `ResourceLoader` struct that takes a DOM tree and returns a map
of URL → fetched resource bytes, with correct priority ordering.

---

### Phase P4: Connection Pooling (HTTP/1.1 Keep-Alive)

**Why it matters:** Opening a new TCP + TLS connection for every sub-resource
on a page is catastrophically slow. A typical page loads 30–80 sub-resources.
Without connection reuse, that's 30–80 TLS handshakes (~200ms each).

**HTTP/1.1 keep-alive rules:**
- Default is `Connection: keep-alive` (persistent connection)
- Server can send `Connection: close` to force close
- Idle connections timeout after 30 seconds (configurable)
- Max 6 concurrent connections per host (browser standard)

**Implementation:**
```rust
struct ConnectionPool {
    /// Map from (host, port) to a pool of idle connections.
    connections: HashMap<(String, u16), Vec<PooledConnection>>,
    /// Maximum connections per host.
    max_per_host: usize,  // 6
    /// Maximum total connections.
    max_total: usize,     // 64
}

struct PooledConnection {
    stream: TlsStream<TcpStream>,
    idle_since: Instant,
}
```

**Idle timeout:** Connections idle for >30s are closed. A background task
periodically sweeps stale connections (or check lazily on next use).

**Privacy note:** Connection pooling is per-`NetworkContext` (per browsing session).
When the user closes a tab or clears data, the pool is dropped — no residual
connections that could be used for timing attacks.

**Deliverable:** `ConnectionPool` integrated into `NetworkContext`. Multiple
sequential fetches to the same host reuse the same TCP+TLS connection.

---

### Phase P5: Cookie Jar (First-Party Only)

**Why it matters:** Most websites require cookies for basic functionality —
login sessions, CSRF tokens, preferences. Without cookies, no authenticated
page works.

**Privacy-first cookie policy:**
| Rule | Rationale |
|---|---|
| First-party cookies only | Third-party cookies are the primary cross-site tracking mechanism |
| `SameSite=Lax` enforced as default | Prevents CSRF on cross-site POST requests |
| `Secure` flag required for HTTPS origins | Cookies must not leak over HTTP |
| `HttpOnly` respected | JS cannot read sensitive cookies |
| Session cookies only by default | No persistent cookies unless user opts in |
| Cookie cap: 50 per domain, 4KB each | Prevent storage abuse |
| In-memory only by default | Cookies gone on close — no disk persistence |

**Spec:** [RFC 6265bis](https://www.ietf.org/archive/id/draft-ietf-httpbis-rfc6265bis-14.html)

**Implementation:**
```rust
struct CookieJar {
    /// Cookies keyed by (domain, path, name).
    cookies: HashMap<CookieKey, Cookie>,
}

struct Cookie {
    name: String,
    value: String,
    domain: String,
    path: String,
    expires: Option<Instant>,
    secure: bool,
    http_only: bool,
    same_site: SameSite,
}
```

**Set-Cookie parsing:** The `Set-Cookie` response header is one of the most
complex headers to parse correctly (multiple attributes, domain matching rules,
path matching, expiry formats). Use a dedicated parser, not ad-hoc string
splitting.

**Cookie attachment:** Before sending a request, match cookies by domain, path,
secure flag, and expiry. Attach as `Cookie: name=value; name2=value2` header.

**Third-party blocking:** A cookie is "third-party" if the domain of the cookie
does not match the top-level document's domain. These are dropped silently.
This is the single most impactful anti-tracking measure a browser can implement.

**Deliverable:** `CookieJar` in `NetworkContext`. `Set-Cookie` parsing from
responses. `Cookie` header attachment on requests. Third-party cookies silently
dropped. Full test suite.

---

### Phase P6: Proper Error Pages

When a fetch fails (DNS error, TLS error, timeout, HTTP 4xx/5xx), the browser
must display a human-readable error page instead of crashing or showing nothing.

| Error | User-visible message |
|---|---|
| DNS failure | "Could not find [domain]. Check the address or your connection." |
| Connection timeout | "[domain] took too long to respond." |
| TLS certificate invalid | "The certificate for [domain] is not trusted. Connection refused." |
| TLS certificate expired | "The certificate for [domain] expired on [date]. Connection refused." |
| HTTP 404 | "Page not found. The server at [domain] could not find [path]." |
| HTTP 500 | "[domain] is having problems. Try again later." |
| Response too large | "The page at [domain] is too large to load safely." |

Error pages are rendered locally — no network calls. Technical details available
in a collapsed section.

---

### Prototype Stop Point

After completing P1–P6, the browser can:
- Fetch a page with all its CSS and images
- Reuse connections for same-host resources
- Decompress gzip/brotli responses
- Maintain login sessions via first-party cookies
- Show meaningful error messages

**What will NOT work:**
- Sites requiring HTTP/2 (rare but growing — most servers fall back to 1.1)
- WebSocket connections (chat, real-time features)
- Sites requiring persistent cookies across sessions
- `fetch()` API from JavaScript
- File downloads
- Video/audio streaming
- Service Workers / Push notifications

**What WILL work:**
- Static sites, blogs, news sites
- Basic authenticated sites (login, browse, logout)
- Sites with CSS, images, and fonts
- Sites behind CDNs (connection pooling handles this)
- Sites using HSTS, secure cookies

---

## Production Milestone — "Handles the Modern Web"

### Phase A1: HTTP/2

**Spec:** [RFC 9113](https://www.rfc-editor.org/rfc/rfc9113)

**Why it matters:** HTTP/2 is used by ~60% of the top 1M sites. While most
fall back to HTTP/1.1 via ALPN negotiation, some CDNs and APIs are moving to
HTTP/2-only. Performance for sub-resource loading is significantly better with
multiplexing.

| Feature | Benefit |
|---|---|
| Multiplexing | Multiple requests on one connection — no head-of-line blocking |
| Header compression (HPACK) | Reduces redundant header bytes on sub-resources |
| Stream prioritization | CSS before images, JS before fonts |
| Server push | Server can send resources before browser asks (rarely used) |

**Recommended approach:** Use the `h2` crate (pure Rust, mature, used by
`hyper` and `reqwest`). Integrate at the connection level — after TLS handshake,
check ALPN to determine if the server supports HTTP/2, then use the `h2` client.

**Privacy note:** HTTP/2's HPACK compression maintains state across requests.
This state must be scoped to the browsing session (per `NetworkContext`) and
cleared when the user clears data. HPACK table contents could theoretically
leak information about previous requests to the same server.

---

### Phase A2: WebSocket

**Spec:** [RFC 6455](https://www.rfc-editor.org/rfc/rfc6455)

**Why it matters:** Chat applications, real-time dashboards, collaborative
editors, notifications, multiplayer games — all use WebSocket. Without it,
many modern web apps are non-functional.

**Implementation:**
1. HTTP/1.1 upgrade handshake (`Connection: Upgrade`, `Upgrade: websocket`)
2. Frame parser (text frames, binary frames, ping/pong, close)
3. Masking (client → server frames are always masked per spec)
4. `wss://` via existing TLS connection

**Privacy considerations:**
- WebSocket connections go through `NetworkContext` — same origin checks apply
- WebSocket to third-party origins requires same permission as third-party fetch
- `ws://` (unencrypted) blocked by default — same HTTPS enforcement as HTTP

**Crate recommendation:** `tungstenite` (pure Rust, well-maintained) or
hand-roll on top of existing TCP+TLS streams for minimal dependency.

---

### Phase A3: Fetch API (for JavaScript)

**Spec:** [WHATWG Fetch](https://fetch.spec.whatwg.org/)

When JavaScript calls `fetch()`, it must route through `NetworkContext`. This
is the bridge between `crates/js` and `crates/net`.

```javascript
// JavaScript in the page:
fetch('/api/data', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ key: 'value' })
})
```

**Features needed:**
| Feature | Notes |
|---|---|
| GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS | All standard methods |
| Request headers | Custom headers (with forbidden header filtering) |
| Request body | Text, JSON, FormData, Blob |
| Response object | `.json()`, `.text()`, `.blob()`, `.arrayBuffer()` |
| CORS | Cross-origin resource sharing enforcement |
| `mode: 'cors'`, `'no-cors'`, `'same-origin'` | Request mode |
| `credentials: 'same-origin'`, `'include'`, `'omit'` | Cookie inclusion |
| `AbortController` / `AbortSignal` | Request cancellation |
| Streaming response body | `ReadableStream` on response |

**CORS enforcement:** This is a major piece of work. Cross-origin requests
trigger a preflight `OPTIONS` request. The server must respond with the correct
`Access-Control-Allow-*` headers. Incorrect CORS implementation is a security
vulnerability — it must match the spec exactly.

**Privacy note:** `credentials: 'include'` on cross-origin requests is the
JavaScript equivalent of third-party cookies. Ferrum should block this by
default or require explicit user permission.

---

### Phase A4: Content Security Policy (CSP)

**Spec:** [CSP Level 3](https://www.w3.org/TR/CSP3/)

CSP is a response header that tells the browser which resources the page is
allowed to load. It is a critical defense against XSS.

```
Content-Security-Policy: default-src 'self'; script-src 'self' cdn.example.com; img-src *
```

| Directive | Controls |
|---|---|
| `default-src` | Fallback for all resource types |
| `script-src` | JavaScript sources |
| `style-src` | CSS sources |
| `img-src` | Image sources |
| `connect-src` | fetch/XHR/WebSocket destinations |
| `font-src` | Font sources |
| `frame-src` | iframe sources |
| `media-src` | Audio/video sources |
| `object-src` | Plugin/embed sources |
| `report-uri` / `report-to` | Violation reporting |

**Implementation:** Parse the CSP header after response. Before loading any
sub-resource, check it against the CSP policy. Block violations silently
(or report, per `report-to`).

**Privacy note:** `report-uri` sends violation reports to a server-specified
URL. This is a potential tracking vector. Ferrum should either suppress reports
or require user opt-in.

---

### Phase A5: HSTS Preload List

**Why it matters:** Without a preload list, HSTS only protects on repeat visits.
The first visit to `bank.com` over HTTP is still vulnerable to SSL stripping.
The preload list closes this gap.

**Implementation:**
1. Download the Chromium HSTS preload list at build time
2. Compile it into a static binary search table (or perfect hash map)
3. Check before any DNS resolution — if the domain is on the list, force HTTPS
   before the first byte leaves the machine

The list is ~100K entries. Compiled into a binary blob it's ~2–4MB.

---

### Phase A6: Certificate Transparency (CT)

**Why it matters:** A misissued TLS certificate (e.g., a rogue CA issuing a
cert for `google.com`) passes standard certificate validation. CT logs provide
public accountability — every certificate must be logged, and browsers can
verify that a certificate has been logged before trusting it.

**Implementation:**
1. Parse SCT (Signed Certificate Timestamp) from TLS extension or certificate
2. Verify SCT signature against known CT log public keys
3. Require at least 2 valid SCTs for certificates issued after a cutoff date
4. Hard-fail if no valid SCTs are present

**Current gap:** No production-ready Rust crate for CT verification exists.
May need to implement from the spec or port from Go's CT library.

---

### Phase A7: OCSP Stapling Verification

**Why it matters:** Certificate revocation. If a server's private key is
compromised and the certificate is revoked, the browser must refuse to connect.
Without OCSP checking, a revoked certificate still works.

`rustls` exposes raw OCSP staple bytes. Build a custom `ServerCertVerifier`
that validates the staple.

**Privacy note:** OCSP checking (non-stapled) sends the certificate serial
number to the CA's OCSP responder — this tells the CA which sites you visit.
Only stapled OCSP is acceptable for a privacy-first browser. Never make live
OCSP requests.

---

### Phase A8: Persistent Cookie Store (Opt-in)

Default behavior is memory-only cookies (gone on close). For users who opt in:

| Feature | Notes |
|---|---|
| Disk storage | `cookies.sqlite` or `cookies.toml` in profile directory |
| Encryption at rest | Encrypt with a key derived from OS keychain |
| Expiry enforcement | Remove expired cookies on load |
| Export/import | Plain-text export for user inspection |
| Per-domain management | UI to view/delete cookies per domain |

---

### Phase A9: Download Manager

Handle non-HTML responses (PDFs, images, archives, executables):

1. Detect non-renderable `Content-Type`
2. Prompt user for save location (no silent downloads)
3. Stream to disk with progress indication
4. Verify `Content-Length` against actual bytes received
5. Optional: hash verification if provided by server

**Privacy note:** Downloads go through `NetworkContext`. No `Range` requests
for resume (these leak download state to the server) unless user explicitly
requests resume.

---

### Phase A10: HTTP/3 (QUIC)

**Spec:** [RFC 9114](https://www.rfc-editor.org/rfc/rfc9114)

HTTP/3 uses QUIC (UDP-based transport) instead of TCP+TLS. Benefits:
- 0-RTT connection establishment
- No head-of-line blocking (even across streams)
- Connection migration (WiFi → cellular without reconnect)

**Crate recommendation:** `quinn` (pure Rust QUIC implementation).

**Priority:** Low for prototype, medium for production. ~25% of the top 1M
sites support HTTP/3 as of 2026, but all fall back to HTTP/2 or 1.1.

---

### Phase A11: Service Workers (Limited)

Service Workers intercept and cache network requests in JavaScript. They are
critical for Progressive Web Apps (PWAs) and offline functionality.

**Privacy risk:** Service Workers persist across sessions and can intercept all
requests to an origin. They are a powerful tracking mechanism.

**Ferrum approach:**
- Service Workers disabled by default
- Enabled per-site via permission system
- Cleared when user clears data
- No background sync (no waking the browser when the user isn't using it)
- No push notifications (no persistent connection to a push server)

---

## Implementation Order Summary

```
PROTOTYPE (loads real pages with sub-resources)
═══════════════════════════════════════════════════════════════════
 P1  Response Decompression              ██░░░░░░░░  ~1 week
 P2  Content-Length Buffer Sizing        █░░░░░░░░░  ~1 day
 P3  Sub-resource Loading                ████░░░░░░  ~3 weeks
 P4  Connection Pooling                  ███░░░░░░░  ~2 weeks
 P5  Cookie Jar (first-party only)       ████░░░░░░  ~3 weeks
 P6  Error Pages                         █░░░░░░░░░  ~1 week
                                                     ─────────
                                              Total: ~10 weeks
═══════════════════════════════════════════════════════════════════

PRODUCTION (handles modern web)
═══════════════════════════════════════════════════════════════════
 A1  HTTP/2                              █████░░░░░  High priority
 A2  WebSocket                           ███░░░░░░░  High priority
 A3  Fetch API (JS bridge)               ████░░░░░░  High priority
 A4  Content Security Policy             ███░░░░░░░  High priority
 A5  HSTS Preload List                   ██░░░░░░░░  High priority
 A6  Certificate Transparency            ████░░░░░░  Medium priority
 A7  OCSP Stapling                       ██░░░░░░░░  Medium priority
 A8  Persistent Cookie Store             ██░░░░░░░░  Medium priority
 A9  Download Manager                    ██░░░░░░░░  Medium priority
 A10 HTTP/3 (QUIC)                       █████░░░░░  Low priority
 A11 Service Workers (limited)           █████░░░░░  Low priority
═══════════════════════════════════════════════════════════════════
```

---

## Dependencies to Add (By Phase)

| Phase | Crate | Purpose | Approved? |
|---|---|---|---|
| P1 | `flate2` | gzip/deflate decompression | Needs review |
| P1 | `brotli` | Brotli decompression | Needs review |
| A1 | `h2` | HTTP/2 client | Needs review |
| A2 | `tungstenite` | WebSocket frames | Needs review |
| A5 | (build script) | HSTS preload list compilation | N/A |
| A10 | `quinn` | QUIC transport | Needs review |

All dependencies must go through the approval process per RULES-05 before
being added to `Cargo.toml`.

---

## Privacy Threat Model — Network Layer

| Threat | Current mitigation | Gap |
|---|---|---|
| ISP sees DNS queries | DoH via Cloudflare | None (done) |
| ISP sees visited IPs | N/A | VPN/Tor integration is out of scope |
| Server fingerprints browser via headers | `Accept-Encoding` missing (fingerprint) | Fix in P1 |
| Cross-site tracking via cookies | Not implemented yet | Fix in P5 |
| Cross-site tracking via connection timing | Connection pool is per-session | None |
| HSTS as supercookie | HSTS state cleared on session close | None |
| Certificate revocation bypass | OCSP not checked | Fix in A7 |
| First-visit SSL stripping | No preload list | Fix in A5 |
| CSP bypass via missing enforcement | CSP not implemented | Fix in A4 |
| Service Worker persistence tracking | Not implemented | Design as restricted in A11 |
| HTTP/2 HPACK state leakage | Not implemented yet | Scope to session in A1 |
| WebSocket to third-party tracking | Not implemented yet | Apply same-origin in A2 |
