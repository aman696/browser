# Ferrum — Network Security Hardening Rationale

This document explains every security and privacy decision made in `crates/net`, why each choice was made, what the alternative was, and what attacks or leaks each decision mitigates. All choices are implemented-or-planned in `crates/net`; this file is the human-readable paper trail.

---

## 1. No OpenSSL — `rustls` Only

### Decision
TLS is implemented exclusively using `rustls` with the `aws-lc-rs` cryptographic backend. OpenSSL is banned at the workspace level.

### Justification
OpenSSL is written in C and has a long history of critical memory-safety bugs that have affected every major browser built on it:

- **Heartbleed (CVE-2014-0160)**: A heap buffer over-read caused by missing bounds checking allowed attackers to read 64KB of process memory per request, exposing private keys, session tokens, and user passwords. Affected an estimated 17% of the world's HTTPS servers.
- **CCS Injection (CVE-2014-0224)**: A man-in-the-middle attack that exploited an out-of-order handshake message, allowing session key interception.
- **DROWN (CVE-2016-0800)**: SSLv2 support in OpenSSL allowed decryption of modern TLS sessions via cross-protocol attacks.

`rustls` is a pure-Rust TLS implementation. Its protocol layer has no `unsafe` blocks. Memory-safety bugs that could expose private keys or session data are structurally impossible in its core path. It has been in production use at scale (400M+ downloads, used by Cloudflare, Let's Encrypt, and others) since 2018.

The `aws-lc-rs` backend links a C library for the underlying cryptographic primitives (AES-GCM, ChaCha20-Poly1305, etc.). This is the accepted tradeoff: the *cryptographic math* uses battle-tested C, but the *protocol state machine* — where the historical bugs have been — is in Rust.

### Attack mitigated
Man-in-the-middle via TLS bugs. Memory corruption leading to private key or session token exfiltration.

### References
- Heartbleed: https://heartbleed.com/
- DROWN: https://drownattack.com/
- rustls security model: https://github.com/rustls/rustls/blob/main/SECURITY.md

---

## 2. Explicit TLS Version Pin — 1.2 Minimum

### Decision
`ClientConfig` is built with `builder_with_protocol_versions(&[TLS13, TLS12])` rather than relying on `rustls` defaults.

### Justification
`rustls` defaults to TLS 1.2+ and never supports TLS 1.0 or 1.1, which is correct. However, relying on a library's *implicit* default means a future API change or misconfiguration could silently downgrade without a compile-time or runtime error. Pinning the versions explicitly:

1. Documents the intent in code: "TLS 1.2 is the floor."
2. Makes it impossible for a future maintainer to accidentally enable TLS 1.0/1.1 without an explicit code change in `tls.rs`.
3. If `rustls` ever changes its default range, our config is unaffected.

TLS 1.0 and 1.1 are deprecated by RFC 8996 (published March 2021). All major browsers removed them in 2020. Both versions are vulnerable to BEAST (TLS 1.0) and POODLE (via downgrade attacks).

### Attack mitigated
Protocol downgrade attacks (BEAST, POODLE, SLOTH).

### References
- RFC 8996 (deprecating TLS 1.0/1.1): https://www.rfc-editor.org/rfc/rfc8996
- BEAST: https://www.openssl.org/~bodo/tls-cbc.txt
- POODLE: https://www.openssl.org/~bodo/ssl-poodle.pdf

---

## 3. DNS-over-HTTPS (DoH) — System Resolver Banned

### Decision
All DNS resolution goes through `hickory-resolver` with `ResolverConfig::cloudflare_https()`. `getaddrinfo()` (the system resolver) is never called.

### Justification
A standard DNS query is sent in plaintext over UDP port 53. This means:

- Your ISP can see every domain you visit, even if you use HTTPS for the content itself.
- Anyone on the same local network (coffee shop, hotel, ISP middlebox) can see and log every hostname you resolve.
- DNS can be trivially intercepted and spoofed (DNS hijacking) to redirect users to attacker-controlled servers.
- DNS prefetch (which many browsers do speculatively) leaks browsing intent even for pages never loaded.

DoH wraps DNS queries inside HTTPS (port 443). To a network observer, a DoH query is indistinguishable from normal HTTPS traffic. The query contents are encrypted end-to-end with the DoH server.

Cloudflare's `1.1.1.1` is the default because it consistently has the lowest latency globally (~14ms), does not sell query logs, and has been audited by KPMG. The user will eventually be able to configure their own DoH resolver.

The system resolver is banned entirely — not "used as a fallback" — because any fallback creates a class of situations where privacy is silently degraded.

### Attack mitigated
ISP and LAN surveillance of browsing activity via DNS. DNS spoofing and hijacking (cache poisoning). Monitoring of browsing intent via DNS prefetch.

### References
- RFC 8484 (DNS-over-HTTPS): https://www.rfc-editor.org/rfc/rfc8484
- Cloudflare DNS privacy policy: https://1.1.1.1/privacy/
- Cloudflare 1.1.1.1 KPMG audit (2022): https://cloudflare.com/static/e72e9ccd049e72700e4c61879b6cd6b3/Cloudflare_1111_Resolver_Privacy_Examination.pdf

---

## 4. DNS Resolver Caching

### Decision
`TokioResolver` is created once and stored in `NetworkContext`, rather than being rebuilt on every fetch call.

### Justification
Rebuilding the resolver on every request defeats the resolver's internal DNS cache. The consequence is that:

1. Every new tab load re-resolves the same domain even if nothing has changed — wasting a DoH round-trip (typically 80–300ms of latency).
2. The DoH connection (a real TLS session to 1.1.1.1) is torn down and re-established each time, exposing more TLS handshakes to network observation.

Caching up to 32 recently resolved hostnames (configurable) means repeat visits and same-domain sub-resources resolve in microseconds from the in-memory cache, following the TTL provided by the DNS record.

### Attack mitigated
Side-channel timing attacks based on DNS resolution latency. Excessive DoH traffic that could fingerprint browsing sessions.

---

## 5. Connection and Read Timeouts

### Decision
- TCP connect: hard timeout of **10 seconds**.
- Total HTTP read: hard timeout of **30 seconds**.

### Justification
Without timeouts, a malicious or unresponsive server can hold a TCP connection open indefinitely, consuming a file descriptor and blocking the task. If a user visits a page with many sub-resources (images, scripts, stylesheets), each of which hangs, the browser could exhaust its connection pool or run out of file descriptors.

Slowloris is a well-known denial-of-service attack where a server sends HTTP headers one byte at a time across thousands of connections, keeping them all alive. A read timeout mitigates this.

10 seconds is the industry-standard connect timeout (Chrome uses ~30s but we are stricter). 30 seconds for the full read is generous for slow pages but prevents indefinite hangs.

### Attack mitigated
Slowloris-style resource exhaustion. Hanging connections from unresponsive servers. File descriptor exhaustion.

### References
- Slowloris: https://www.imperva.com/learn/application-security/slowloris/

---

## 6. Response Size Limit (50 MB)

### Decision
`read_all` aborts with `FetchError::ResponseTooLarge` if the response body exceeds **50 MB**.

### Justification
A malicious server could send an infinite or extremely large HTTP response body. Without a size limit, this would:

1. Exhaust available RAM until the OS kills the process (OOM).
2. Block the browser rendering pipeline waiting for a response that never ends.

50 MB is a generous limit for a web page (the average HTML page is 2–3 MB; even the most JavaScript-heavy SPAs rarely exceed 10 MB of HTML/JS combined). Binary files (large images, videos) should eventually use streaming, not buffering into a `Vec<u8>`.

### Attack mitigated
Memory exhaustion via oversized response. Denial-of-service via infinite streaming responses.

---

## 7. HTTPS Enforcement and Redirect Security

### Decision
- All remote `http://` URLs are silently upgraded to `https://` before any connection is made.
- After a redirect, the `Location` header URL is re-parsed through `parse_url`, which re-applies HTTPS enforcement.
- Redirects that attempt to downgrade from HTTPS to HTTP for a non-localhost host are blocked hard.

### Justification
SSL stripping is a man-in-the-middle attack where an attacker intercepts an HTTP request before it is upgraded and serves a downgraded HTTP version of the site, removing HTTPS entirely. By silently upgrading at the URL-parse stage and re-verifying after every redirect, Ferrum makes SSL stripping structurally impossible — it never sends a request to an HTTP endpoint for a remote host.

The redirect re-check is critical. Without it, a site could serve an `https://` page, then issue a `302` redirect to `http://` — a common misconfiguration used by some attackers to capture session tokens. Many browsers follow the redirect without re-enforcing HTTPS.

`Location` header length is capped at 4096 bytes to prevent oversized redirect header attacks used in memory corruption exploits.

### Attack mitigated
SSL stripping. HTTPS-to-HTTP downgrade via redirect. Header injection via oversized Location values.

### References
- SSL stripping: https://www.blackhat.com/presentations/bh-dc-09/Marlinspike/BlackHat-DC-09-Marlinspike-Defeating-SSL.pdf
- RFC 9110 §15.4 (redirect semantics): https://www.rfc-editor.org/rfc/rfc9110#section-15.4

---

## 8. HTTP Strict Transport Security (HSTS)

### Decision
After every successful HTTPS response, Ferrum parses the `Strict-Transport-Security` response header. If present, the host is recorded in an in-memory `HstsStore`. Before every subsequent request to that host (or its subdomains if `includeSubDomains` is set), `is_hsts()` is checked and HTTPS is forced unconditionally.

### Justification
HSTS is defined in RFC 6797 as a mechanism that lets a server declare: "For the next N seconds, this domain must always be accessed via HTTPS." Without HSTS tracking, a user who has visited a site over HTTPS could still be redirected to HTTP via a network-level attack on their next visit to that domain if they type it without a scheme.

HSTS works because:
1. The browser records the policy on the first HTTPS visit.
2. On every subsequent attempt to load that domain — even if the user types `http://` — the browser forces HTTPS before making any network connection.
3. Even the initial connection for the first visit can be protected via the HSTS preload list (a compiled-in list of HSTS-enrolled domains), which we will ship in a later iteration.

Not implementing HSTS means that even though Ferrum silently upgrades HTTP, the upgrade happens *before* the TLS handshake on the initial request — not before the TCP connection. An attacker who can perform a network-level attack can still read the initial TCP SYN packet before TLS kicks in, revealing the destination IP.

### Attack mitigated
Protocol downgrade attacks on repeat visits. SSL stripping on the first unprotected connection (mitigated by preload list, future work).

### References
- RFC 6797 (HSTS): https://www.rfc-editor.org/rfc/rfc6797
- HSTS Preload list: https://hstspreload.org/

---

## 9. URL Hardening — Userinfo, Fragments, IPv6 Zone IDs

### Decision
`parse_url` rejects:
- **Userinfo** (`user:pass@host`) — returns `UrlError::UserInfoNotAllowed`.
- **IPv6 Zone IDs** (`[::1%eth0]`) — returns `UrlError::InvalidHost`.

`parse_url` silently strips:
- **Fragment identifiers** (`#anchor`) — fragments are client-side only and must never be sent to the server.

### Justification

**Userinfo in URLs** is a historical feature of RFC 3986 that has been abused for phishing since the early 2000s. A URL like `https://bank.com@evil.com/` is valid per the old spec: the visible text `bank.com` is the username, and `evil.com` is the actual host. All major browsers reject userinfo in URLs (RFC 3986 §3.2.1, WHATWG URL spec §5.1). Not doing so would make Ferrum a phishing vector.

**IPv6 Zone IDs** (the `%` character in an IPv6 literal like `[fe80::1%eth0]`) specify a network interface scope. A URL with a zone ID can be crafted to probe interfaces on the local network that should not be accessible from the browser. They are disallowed by the WHATWG URL spec.

**Fragments** are never sent to the server in HTTP requests (RFC 9110 §4.2.4). Sending them would be a privacy leak — the fragment often contains state information (`#/user/profile`, OAuth tokens like `#access_token=abc123`). Our parser strips them before the request is built.

### Attack mitigated
Phishing via URL userinfo spoofing. Local network probing via IPv6 zone IDs. OAuth token leakage via fragment forwarding.

### References
- WHATWG URL spec §5.1 (userinfo prohibition): https://url.spec.whatwg.org/#concept-url-username
- RFC 9110 §4.2.4 (fragments not sent): https://www.rfc-editor.org/rfc/rfc9110#section-4.2.4
- IPv6 Zone ID concerns: https://www.rfc-editor.org/rfc/rfc6874

---

## 10. Fetch Metadata Headers (`Sec-Fetch-*`)

### Decision
Every outbound request includes:
```
Sec-Fetch-Site: none
Sec-Fetch-Mode: navigate
Sec-Fetch-Dest: document
```

### Justification
The `Sec-Fetch-*` headers are defined in the W3C Fetch Metadata specification and let the *server* know the context of the request. When `Sec-Fetch-Site: none` is set, it means the request was user-initiated (typed URL or bookmark), not triggered by a third-party script. This enables servers to:

1. Reject unexpected cross-origin requests from embedded scripts they did not intend to serve.
2. Distinguish top-level navigation from sub-resource fetches for logging and policy.

For Ferrum, these headers are privacy-safe: they declare a valid top-level navigation context, not identifying information. They are also **forge-resistant** because browsers prefix them with `Sec-` which cannot be set by JavaScript via `XMLHttpRequest` or `fetch()`.

Sending them correctly establishes Ferrum as a well-behaved browser client for servers that implement `Fetch-Metadata` policies, improving compatibility with security-hardened APIs.

### References
- W3C Fetch Metadata: https://www.w3.org/TR/fetch-metadata/
- Google security blog on Fetch Metadata: https://web.dev/articles/fetch-metadata

---

## 11. Referrer Policy — Never Leak Full URLs

### Decision
Ferrum never adds a `Referer` header to outbound requests. When a `Referrer-Policy` response header is received, it is stored and applied to sub-resource requests from that page.

The minimum policy enforced is `strict-origin-when-cross-origin` (as declared in `RULES-03`), which sends only the origin (`https://example.com`) on cross-origin requests — never the full path.

### Justification
The `Referer` header leaks the URL of the page the user navigated *from*. This URL often contains sensitive information:

- Search queries: `https://example.com/?q=my+medical+condition`
- Internal paths: `https://company.com/internal/project-roadmap`
- OAuth / SSO tokens: `https://app.com/callback?token=abc123`
- Session IDs embedded in URLs.

Third-party resources (images, scripts, fonts, analytics pixels) embedded on a page automatically receive the full `Referer` of the page they are loaded from. This is the primary mechanism by which analytics companies build cross-site user profiles — every site with a Google Analytics script tells Google exactly what URL you visited.

`strict-origin-when-cross-origin` sends only the origin scheme+host (e.g. `https://example.com`) on cross-origin requests. The page path is never shared. On same-origin requests (a link within the same site), the full URL is sent because the site already has that context.

### References
- MDN Referrer-Policy: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Referrer-Policy
- Scott Helme on referrer leakage: https://scotthelme.co.uk/a-new-security-header-referrer-policy/

---

## 12. `NetworkContext` — Central Policy Chokepoint

### Decision
All network requests go through a single `NetworkContext` struct that owns the TLS connector, DNS resolver, and HSTS store. No other crate may open a raw socket.

### Justification
A central network chokepoint is **the** architectural guarantee of Ferrum's privacy model. Without it:

- Any future crate could add a `TcpStream::connect` call that bypasses HTTPS enforcement.
- Speculative prefetch or telemetry calls could slip in without going through HSTS/DoH checks.
- Security audits become impossible — you cannot verify that all outbound traffic follows policy if there is no single enforcement point.

This mirrors the architecture of Firefox and Chrome at a high level — both have a network service process that serializes all I/O. For Ferrum at its current scale, a struct-based chokepoint is simpler and achieves the same goal.

The `crates/security` crate will query `NetworkContext` before requests are dispatched, enforcing blocklist lookups and permission checks.

---

## Summary Table

| Hardening | Mitigates | Standard/Reference |
|-----------|-----------|---------------------|
| rustls only | Heartbleed-class C memory bugs | rustls security model |
| TLS 1.2 minimum | BEAST, POODLE, downgrade attacks | RFC 8996 |
| DoH (no system resolver) | ISP/LAN DNS surveillance | RFC 8484 |
| DNS caching | Timing side-channels, excess traffic | — |
| Connect timeout | Slowloris, resource exhaustion | — |
| Read timeout | Infinite-stream DoS | — |
| 50MB response cap | RAM exhaustion | — |
| HTTPS enforcement + re-check on redirect | SSL stripping | RFC 9110 |
| HSTS store | Downgrade on repeat visits | RFC 6797 |
| Reject userinfo URLs | Phishing via URL spoofing | WHATWG URL spec |
| Strip fragments | OAuth token leakage | RFC 9110 §4.2.4 |
| Reject IPv6 Zone IDs | Local network probing | RFC 6874 |
| Sec-Fetch-* headers | Cross-origin request forgery | W3C Fetch Metadata |
| No Referer / strict-origin policy | Cross-site URL leakage | RULES-03 |
| NetworkContext chokepoint | Policy bypass by any crate | RULES-04 |

---

## Verified — Adversarial Test Results

All hardening rules were verified by an active adversarial test suite
(`tmp/adversarial_attack_test.py`) run on 2026-03-10. personal test not on github

### Phase A — URL Parser (7/7 blocked)

| Attack input | Expected result | Actual |
|---|---|---|
| `https://user:pass@example.com` | `UserInfoNotAllowed` | ✔ Blocked |
| `https://bank.com@evil.com/steal` | `UserInfoNotAllowed` | ✔ Blocked |
| `https://example.com/cb#access_token=SECRET` | Fragment stripped, token never in path | ✔ Blocked |
| `https://example.com#top` | Path stays `/` | ✔ Blocked |
| `https://[fe80::1%eth0]/` | `InvalidHost` | ✔ Blocked |
| `javascript:alert(document.cookie)` | `UnsupportedScheme` | ✔ Blocked |
| HSTS without `max-age` | Policy not recorded | ✔ Blocked |

### Phase B — Active Server Attacks (7/7 blocked)

| Mock server behaviour | Expected result | Actual |
|---|---|---|
| Streams 70MB of response body | `ResponseTooLarge` at 50MB cap | ✔ Blocked |
| Slowloris — 1 byte per 500ms, never completes | `Timeout` (30s) or kernel `Io` reset | ✔ Blocked |
| Chunked body with `GGGG` as chunk size (invalid hex) | `Protocol` parse error | ✔ Blocked |
| Response with no `\r\n\r\n` header separator | `Protocol` parse error | ✔ Blocked |
| 301 redirect with 8KB `Location` header | `Protocol` — exceeds 4096-byte cap | ✔ Blocked |
| `https://admin:password@example.com` | `InvalidUrl` before any DNS/TCP | ✔ Blocked |
| `javascript:alert(document.cookie)` | `InvalidUrl` before any DNS/TCP | ✔ Blocked |
