# Project Rules — Ferrum Browser

> These rules are the single source of truth for how this codebase is written, structured, and maintained.
> Every contributor — human or AI assistant — must follow them without exception.
> If a rule seems wrong for a specific situation, open a discussion before deviating. Do not silently ignore rules.

---

## Agent Bootstrap — Read This First, Every Time

> **This section runs before anything else.** When an AI agent is given access to this repository, the very first thing it must do — before writing any code, before answering any question — is execute the checklist below. No exceptions. This ensures the agent always has accurate context about the current state of the project before making any decisions.

### On First Load of This Rules File

1. **Check if `docs/FEATURE_LIST.md` exists.**
   - If it does not exist: create it immediately using the Feature List template defined at the bottom of this section. Populate it fully based on the project context in this rules file. Do not wait to be asked. Do not ask for permission. Create it now.
   - If it exists: read it fully before proceeding. Do not assume its contents.

2. **Check if `docs/decisions/boa-audit.md` exists.**
   - If it does not exist: note that the Boa audit has not been done yet. Flag this to the developer as a pending task before any JavaScript-related work begins.
   - If it exists: read it and note whether Boa passed, failed, or required a fork.

3. **Check if `docs/sessions/` contains any session logs.**
   - If it is empty or does not exist: the project is in early stages. Proceed accordingly.
   - If logs exist: read the most recent one to understand the current state of the codebase before starting work.

4. **Report what you found** in a brief summary before doing anything else: what exists, what is missing, what the current state of the project appears to be.

### Feature List Template (for `docs/FEATURE_LIST.md`)

When creating this file, populate every section based on the full context of this rules file. Do not leave sections blank. Mark each feature with one of: `[ ] Not started`, `[~] In progress`, `[x] Complete`, `[!] Blocked`.

```markdown
# Ferrum Browser — Feature List

> Generated from RULES.md. Keep this file updated as features are completed.
> Last updated: YYYY-MM-DD

---

## Core Rendering Pipeline
- [ ] HTTP/HTTPS networking (crates/net)
- [ ] URL parsing and validation
- [ ] DNS resolution via hickory-dns
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
```

---

---

## Language & Stack

> **Why Rust?** Rust gives us memory safety without a garbage collector, which matters enormously for a browser engine. Browsers parse untrusted input constantly — malformed HTML, adversarial CSS, hostile JavaScript. A use-after-free or buffer overflow in a browser is not a minor bug; it is a security vulnerability. Rust eliminates this class of bug at compile time. The C++ prototype proved the architecture. Rust is how we make it production-safe.

- This project is written in **Rust**. All new code must be Rust.
- Legacy C++ code is being actively migrated. When touching a C++ file, rewrite it in Rust — do not patch it.
- No C, no C++, no Python, no scripting glue unless it's a one-time build tool with no runtime presence.
- Rust edition: **2024** (released with Rust 1.85, February 2025). Use it. If a dependency does not yet support edition 2024, pin it and file an issue upstream.
- Minimum supported Rust version: **1.85.0**. This is the hard floor — edition 2024 requires it. Developers should run **current stable** (1.94.0 as of March 2026). Do not develop on the MSRV floor; use the latest stable so you catch deprecations and lints early.
- Use `cargo` for everything — builds, tests, formatting, linting. No CMake, no Makefiles.

---

## Code Style & Modern Standards

> **Why strict style rules?** Inconsistent code style is not aesthetic preference — it creates real cognitive overhead when reading, reviewing, and debugging. Clippy warnings are not suggestions; they exist because the pattern being warned about has a known failure mode. `unwrap()` in library code means a consumer of your function can trigger a panic they have no way to handle. These rules exist to prevent bugs, not to enforce taste.

- Run `cargo fmt` before every commit. No unformatted code.
- Run `cargo clippy -- -D warnings` before every commit. Zero clippy warnings permitted.
- No `unwrap()` or `expect()` in library code. Use `?` propagation and proper error types.
  - `unwrap()` panics on `None` or `Err`. In library code, this crashes the caller's program with no recovery path. Use `?` to propagate errors to the caller and let them decide how to handle it.
- `unwrap()` is permitted only in tests and `main()` for fatal startup failures, with a comment explaining why.
- Use `thiserror` for defining error types. Use `anyhow` only in binaries, never in library crates.
  - `thiserror` generates structured error types that callers can match on. `anyhow` erases type information, making it impossible for library consumers to handle specific errors programmatically.
- No `unsafe` blocks without a `// SAFETY:` comment explaining the invariant being upheld.
  - The comment must explain *why* the unsafe code is actually safe — what precondition guarantees it won't cause undefined behavior.
- Prefer `&str` over `&String`, `impl Trait` over concrete types in function signatures where it makes sense.
- No `clone()` calls without a comment if the clone is non-trivial (i.e., not a cheap `Copy` type).
  - Unexpected clones are a common source of performance issues. Make them visible.
- All public API items must have doc comments (`///`). No exceptions.
  - Document what the function does, what each parameter means, and what errors it can return. Future you will thank present you.
- Use `#[must_use]` on functions that return `Result` or values the caller should not silently discard.

### Code Readability — Written for Humans First

> **The standard:** A person who has never written Rust but understands general programming concepts should be able to read any function in this codebase and understand what it is doing and why. Rust's syntax is already unfamiliar to many developers. Do not make it harder by writing clever, dense, or over-abstracted code.

- **Do not comment every line.** A comment on every line creates noise that makes code *harder* to read, not easier. If every line needs explaining individually, the code is too complex — simplify it instead.
- **Comment at the block level.** If a group of lines work together to accomplish one thing, put a single comment above or below the block explaining what that thing is and why it's done that way. One comment per logical unit of work.
  ```rust
  // Parse the host and port out of the URL separately so we can
  // pass them to getaddrinfo independently. Combining them causes
  // DNS resolution failures on some platforms.
  let colon = host_port.find(':').unwrap_or(host_port.len());
  let host = &host_port[..colon];
  let port = if colon < host_port.len() { &host_port[colon+1..] } else { default_port };
  ```
- **Name things so they explain themselves.** A well-named variable or function eliminates the need for a comment entirely. `is_third_party_cookie` needs no comment. `flag` needs a comment and a rename.
- **Complexity must earn its place.** If a Rust-specific construct — an iterator chain, a trait bound, a combinator — solves the problem meaningfully better than the simple version (faster, safer, less error-prone, handles edge cases the simple version misses), use it. Rust's power exists for a reason. Do not neuter the codebase in the name of readability. But if the complex version and the simple version produce the same result with no meaningful difference in performance or correctness, choose the simple version every time. The test is: does the complexity pay for itself? If yes, keep it and explain it with a block comment. If no, delete it.
  ```rust
  // Use the iterator version here — it handles the empty slice case
  // automatically and short-circuits on the first invalid entry,
  // which a for loop would require extra state to replicate cleanly.
  let all_valid = entries.iter().all(|e| e.is_valid());

  // But for a straightforward transformation with no edge case logic,
  // the loop is clearer and does the same job:
  let mut results = Vec::with_capacity(items.len());
  for item in &items {
      results.push(item.value * 2);
  }
  ```
- **Break complex expressions into named intermediate variables.** Do not write a single expression that does five things. Write five lines, each named, each doing one thing. The compiler will optimise them away; the reader will thank you.
  ```rust
  // Hard to read:
  let result = data.iter().filter(|x| x.is_valid()).map(|x| x.value * 2).sum::<u32>();

  // Readable:
  let valid_entries = data.iter().filter(|x| x.is_valid());
  let doubled_values = valid_entries.map(|x| x.value * 2);
  let total: u32 = doubled_values.sum();
  ```
- **Explain Rust-specific constructs when they are non-obvious.** If you use a lifetime annotation, a `PhantomData`, an `Arc<Mutex<T>>`, or a non-trivial trait bound, add a block comment explaining what it is doing in plain English — not Rust jargon. Assume the reader knows what a function and a loop are. Do not assume they know what `'a: 'b` means.
- **Function length.** If a function is longer than ~40 lines, it is probably doing more than one thing. Break it up. Each function should do exactly one thing and be nameable in a single sentence.
- **No abbreviations in names** unless they are universally understood (`url`, `html`, `css`, `id`). Write `connection` not `conn`, `response` not `resp`, `certificate` not `cert`. Abbreviations save typing once and cost comprehension every time the code is read.

---

## Architecture

> **Why separate crates?** A browser engine is not one program — it is a pipeline of loosely coupled subsystems. The networking layer should not know about DOM structure. The layout engine should not know about TLS. Separate crates enforce this separation at the compiler level: if `crates/layout` tries to call into `crates/net` directly, the build fails. This makes the architecture honest. It also means each crate can be tested, fuzzed, and reasoned about in isolation.

- Modular crate structure. Each major browser component is its own crate under `crates/`:
  - `crates/net` — HTTP/HTTPS networking. Owns all socket and TLS code. Exposes a `fetch(request) -> Response` interface. Nothing else touches sockets.
  - `crates/html` — Tokenizer, parser, DOM tree construction. Input: raw HTML bytes. Output: a DOM tree. Follows WHATWG spec.
  - `crates/css` — CSS parsing and cascade resolution. Input: raw CSS text + DOM tree. Output: computed styles per node.
  - `crates/layout` — Box model and layout engine. Input: styled DOM. Output: a layout tree with positions and dimensions.
  - `crates/render` — Painting and compositing. Input: layout tree. Output: pixels on screen.
  - `crates/js` — JavaScript engine. See the JavaScript Engine section below for the full policy on this.
  - `crates/security` — TLS policy enforcement, cookie jar, content blocking rules, HSTS store. All security decisions live here.
  - `crates/browser` — Top-level binary. Wires all crates together. Contains no business logic of its own.
- No circular dependencies between crates. If you need to introduce one, the correct fix is to extract shared types into a `crates/types` or `crates/shared` crate that both depend on.
- Separate data from behavior. DOM nodes are data structures; parsing logic lives in a separate parser module that produces them.
- No global mutable state. Use dependency injection or pass state explicitly through function parameters or context structs.

---

## Minimal Bloat — Privacy Bloat Only

> **What bloat means here:** Bloat is not about features. Users can sign in, view PDFs, watch videos, use extensions — all of that is fine and should work. Bloat means **privacy-violating overhead**: code, dependencies, or features that collect, transmit, or expose user data without explicit informed consent. A sign-in feature that stores credentials locally is not bloat. A sign-in feature that sends browsing history to a sync server in the background is bloat. The distinction is always: does this serve the user, or does it serve someone else at the user's expense?

- **No background network calls the user did not initiate.** The browser must not make any network request that the user did not directly trigger by navigating, clicking, or explicitly enabling a feature. No silent pings, no update checks, no telemetry, no "phone home" of any kind.
- **No features that require an account on a third-party server by default.** Features that optionally integrate with external services are fine — as long as they are opt-in, clearly labelled, and the browser functions fully without them.
- **No default search engine or service that monetises user queries.** The default must be a privacy-respecting option. User can change it to anything they want, including Google — that is their choice to make.
- **No bundled tracking SDKs or analytics libraries** regardless of how they are labelled. "Privacy-preserving analytics" is still analytics. It does not belong here.
- **No DRM infrastructure** that requires contacting a licence server. DRM systems are identification systems by design.
- **Features that need external services must be clearly opt-in** with a plain-language explanation of what data leaves the device and where it goes, before the user enables them. Not buried in settings. Not pre-checked.

---

## JavaScript Engine

> **Why not build from scratch?** A JavaScript engine that correctly implements the ECMAScript specification — including all the edge cases real websites depend on — is one of the most complex pieces of software ever written. V8 alone has tens of millions of lines of code and hundreds of full-time engineers. Building one from scratch that is both correct and stable would take years and would block every other part of the browser indefinitely. Boa is a pure-Rust ECMAScript engine that is actively maintained, auditable, and has no C FFI in its core. It is the right starting point. The goal is not to build a JS engine — the goal is to build a browser. Use the best available Rust tool for JS and focus engineering effort on the browser-specific problems that no existing library solves.

### Policy

1. **Use Boa as the JavaScript engine.** Boa (`boa_engine` on crates.io) is a pure-Rust ECMAScript bytecode compiler and VM. Current version: **v0.21** (October 2025). It passes **94.12%** of the ECMAScript test262 conformance suite, up from 89.92% in v0.20. It has no C++ dependencies, no network calls, and its full source is auditable. It self-describes as experimental but ships regular releases and is actively maintained. Key capabilities in v0.21: refactored async API, error backtraces, near-complete Temporal API support, and procedural macros for Rust↔JS interop. The crate name to add to `Cargo.toml` is `boa_engine`, not `boa`.

2. **Audit Boa before integrating.** Before adding `boa-engine` to `Cargo.toml`, run `cargo audit` against its dependency tree and manually review its `Cargo.toml` for any dependencies that make network calls, collect data, or link C libraries. When the audit is complete — pass or fail — create `docs/decisions/boa-audit.md` with the following sections:
   - **Audit date**
   - **Boa version audited**
   - **Full dependency tree** (output of `cargo tree` for boa-engine)
   - **cargo audit output**
   - **Manual review findings** — for each dependency, note whether it is safe, has concerns, or is a blocker
   - **Privacy assessment** — does any dependency make network calls? Link C libraries? Have unresolved RUSTSEC advisories?
   - **Decision** — Integrate as-is / Integrate with modifications / Fork required / Reject
   - **If fork required** — list exactly what needs to change and why
   This file must exist before any JavaScript code is written. It is the paper trail for the decision.

3. **If Boa needs changes to meet this project's requirements, fork it — do not patch around it.** If the audit reveals issues, or if Boa is missing functionality this browser needs, the correct approach is:
   - Fork the Boa repository under this project's GitHub organisation
   - Give the AI assistant access to the forked repository
   - Make only the changes needed — do not rewrite Boa, do not diverge from upstream unnecessarily
   - Document every change from upstream in `docs/decisions/boa-fork-changes.md` with the reason for each change
   - Periodically sync upstream Boa changes into the fork to stay current with bug fixes and spec compliance improvements

4. **Do not embed V8, SpiderMonkey, or JavaScriptCore.** These are C++ libraries. They reintroduce C++ memory safety bugs, dramatically increase binary size, and cannot be audited in any reasonable time frame. The fact that major browsers use them is not a justification.

5. **JavaScript is disabled by default, enabled per-site.** Users enable JavaScript per-site through the permission system. This is not negotiable. JavaScript is the primary vector for tracking scripts and browser exploits. The user must consciously choose to run it on any given site.

6. **No JIT compiler in early phases.** Boa currently uses a bytecode VM, not a JIT. Do not add JIT compilation. JIT spraying is a known exploitation technique and JIT compilers are a major security attack surface. If performance becomes a real problem, address it through VM optimisation before considering JIT.

---

## Privacy — Non-Negotiable

> **The core principle:** The browser is a tool that serves the user, not the network. Every privacy rule below exists because modern browsers have systematically eroded user privacy in favour of advertisers, analytics companies, and their own telemetry pipelines. Ferrum does not do this. These are not features to be toggled — they are the default state of the browser and most cannot be turned off even by the user, because the threat model assumes the user may be coerced or tricked into disabling them.

- **No telemetry. No analytics. No crash reporting. No usage metrics.** Not even opt-in. If you want to know how the browser is used, read user feedback. Do not instrument users.
- **No auto-updates that phone home** without explicit user action. Update checks must be triggered by the user, not the browser.
- Third-party cookies: **blocked by default**. A cookie set by `tracker.com` while the user visits `news.com` is a cross-site tracking mechanism, not a feature. User must explicitly enable per-site exceptions.
- No prefetching or preconnecting to URLs the user has not navigated to. Prefetch leaks browsing intent to servers the user never chose to contact.
- No DNS prefetch. DNS queries reveal browsing history to the resolver even if the user never loads the page.
- Referrer policy: `strict-origin-when-cross-origin` as default. This sends the origin (e.g. `https://example.com`) but not the full path (e.g. `https://example.com/private/page`) on cross-origin requests. Pages cannot override this to `unsafe-url` which would expose full URLs to third parties.
- No fingerprinting surface exposure:
  - Canvas fingerprinting: `canvas.toDataURL()` and `getImageData()` must return noise-injected or blank results for cross-origin contexts.
  - Font enumeration: do not expose the user's installed font list to pages.
  - Battery API: return `null` or a fixed value. Battery level is a fingerprinting vector.
- HTTPS enforced for all non-localhost traffic. Downgrade attempts (e.g. SSL stripping) must hard-fail with an error page, not a warn-and-proceed dialog.
- HSTS (HTTP Strict Transport Security) must be respected. Ship a preload list of known HSTS domains with the binary so protection is immediate before the first visit.
- No storing browsing history, cookies, or cache to disk without explicit user opt-in. Default: everything lives in memory and is lost on close.
- Certificate pinning errors must hard-fail. A certificate that doesn't match the pin is either an attack or a misconfiguration — neither should be silently bypassed.

---

## Privacy Warning System

> **Why warn instead of hard-block?** Hard-blocking every privacy-hostile site would make the browser unusable — a significant portion of the web runs trackers, third-party scripts, and fingerprinting. The goal is not to decide for the user but to make the cost of visiting a site transparent before they pay it. The user is an adult. Give them the information, respect their decision, then enforce exactly what they consented to — nothing more.

### When to Trigger a Warning

A privacy warning interstitial must be shown before loading any site that meets one or more of these criteria:

- **Known tracker network**: domain is on the EasyPrivacy or Disconnect.me tracker blocklist
- **Known data broker**: domain is classified as a data broker or people-search site
- **Forced third-party cookies**: the site's known behavior requires third-party cookies to function (e.g. cross-site SSO without a first-party fallback)
- **Known fingerprinting scripts**: domain is flagged in the fingerprinting section of the Disconnect.me list
- **No HTTPS**: the site does not support HTTPS at all
- **Expired or invalid certificate**: TLS certificate is expired, self-signed (non-localhost), or fails chain validation

### What the Warning Page Must Show

The interstitial is a full-page block rendered locally — no external resources, no network calls to generate it. It must display:

1. **The domain being visited** — clearly, at the top, in large text. No ambiguity about which site triggered the warning.
2. **Why it was flagged** — a plain-language explanation of the specific reason(s). Not a generic "this site may be unsafe." Specific: "This site runs Google Analytics, which tracks your activity across other websites." or "This site has no HTTPS support. Any data you send can be read by anyone on your network."
3. **What will be enabled if the user proceeds** — an explicit list of what the browser will allow for this domain if the user clicks through. Example:
   - "Third-party cookies: will be allowed for this site only"
   - "Canvas API: will return real values for this site"
   - "Referrer header: will include full URL for this site"
4. **Two actions only**:
   - `Go back` — returns to the previous page. Default focused button.
   - `I understand the risks — continue to [domain]` — proceeds with exactly the permissions listed above granted for this session only.

### Permission Granting Rules

- Permissions granted on the warning page apply **to the current session only** by default. They are not persisted to disk unless the user explicitly checks "Remember this for [domain]".
- Granting permission to visit a site enables **only what was listed on the warning page** for that specific domain. It does not disable the privacy model globally or for other domains.
- Permissions are **per-origin**, not per-site. Granting `example.com` does not grant `tracker.example.com` or `cdn.example.com`.
- If the user grants a permission and the site later requests additional permissions not listed on the warning (e.g. the page tries to enable additional APIs), those requests are silently denied. No second warning — the user already made an informed choice about this site.
- Permissions granted with "Remember this" are stored in a local `permissions.toml` file that the user can inspect and edit. No opaque binary formats.

### Implementation Notes

- The warning interstitial must be implemented in `crates/security` as a locally-rendered HTML page. It must not make any network requests.
- The blocklists (EasyPrivacy, Disconnect.me) must be shipped with the binary and updated only when the user explicitly triggers an update. No silent background list updates.
- The `SecurityManager` in `crates/security` owns the permission store. All other crates query it before making requests or enabling APIs — they do not make their own blocking decisions.
- The warning page must be keyboard-navigable. `Go back` must be the default focused element so that pressing Enter dismisses it safely.
- Do not show the warning for localhost or `127.0.0.1` regardless of any other criteria.

---

## Networking

> **Why these specific choices?** The networking layer is where most privacy leaks happen and where most security vulnerabilities live. Each choice below is deliberate: rustls over OpenSSL because C memory bugs have caused critical browser CVEs for decades; DoH because plaintext DNS exposes every site you visit to your ISP and network; a central NetworkContext because ad-hoc socket calls scattered across crates are impossible to audit.

- Use `rustls` for TLS. No OpenSSL.
  - OpenSSL is written in C. Its CVE history includes Heartbleed, numerous heap overflows, and use-after-free bugs. Rustls is a pure-Rust TLS implementation. Current version as of early 2026 is **0.23.36**, actively maintained and used in production at scale (400M+ downloads). It requires Rust 1.71+.
  - Rustls ships two cryptography backends: `aws-lc-rs` (default, better performance, post-quantum support) and `ring` (easier cross-platform build). Use `aws-lc-rs` as the default. If a target platform cannot build `aws-lc-rs`, fall back to `ring` with a documented reason.
  - Note: `aws-lc-rs` links a C library internally. This is acceptable because it is contained entirely within the crypto provider and rustls itself has no unsafe code in its protocol implementation. The tradeoff is documented and accepted.
- Use `hickory-dns` (formerly trust-dns) for DNS resolution. No system resolver.
  - The system resolver sends plaintext DNS queries exposing every domain the user visits to the local network and ISP. `hickory-dns` gives full control over the resolution pipeline including DoH support. Current version is **0.25.2**. It is pre-1.0 and APIs may change, but it is actively funded — ISRG (Let's Encrypt) and ICANN are funding development toward production deployment at Let's Encrypt in 2026. The resolver crate (`hickory-resolver`) is the relevant one for this project, not the server.
  - Use the `hickory-resolver` crate specifically. It integrates with Tokio and supports DoH via feature flags.
- DNS-over-HTTPS (DoH) must be supported and must be the default once implemented.
  - DoH encrypts DNS queries inside HTTPS, preventing network-level surveillance of browsing activity. Cloudflare (`1.1.1.1`) and NextDNS are acceptable defaults; user must be able to configure their own resolver.
- HTTP/1.1 first, HTTP/2 as a planned extension. Do not implement HTTP/2 until HTTP/1.1 is solid and tested.
  - HTTP/2 multiplexing and HPACK header compression are non-trivial. Building them on an unstable HTTP/1.1 foundation creates compounding bugs. Get the foundation right first.
- All network requests must go through a central `NetworkContext` that enforces policy (HTTPS-only, referrer stripping, cookie rules). No component makes raw socket calls directly.
  - A `NetworkContext` is a single chokepoint where security and privacy policy is enforced. If any crate can make arbitrary socket calls, the security model is unauditable.

---

## HTML & Parsing

> **Why the WHATWG spec?** The web is built on malformed HTML. Every browser that has tried to implement "clean" HTML parsing and skip the spec's error-handling rules has ended up rendering major websites wrong. The WHATWG spec exists specifically because browser vendors spent years documenting exactly how browsers handle malformed input in the wild. Following it is not perfectionism — it is the only way to render the actual web correctly.

- Parser must follow the **WHATWG HTML Living Standard** for tokenization and tree construction. Reference: https://html.spec.whatwg.org/multipage/parsing.html
- Do not write a bespoke parser that "mostly works." If deviating from spec, document exactly why with a `// SPEC DEVIATION:` comment that includes the spec section being deviated from and the reason.
- The DOM must be owned through an arena allocator (use `typed-arena` or `bumpalo`). No `Box<DOMNode>` chains.
  - DOM trees are deeply interconnected graphs with parent/child/sibling pointers. Rust's ownership model does not naturally express this. An arena allocator sidesteps the problem: all nodes are allocated in one contiguous block, live as long as the arena, and are freed all at once when parsing is done. This is also significantly faster than individual heap allocations per node.
- Text nodes must store `&str` slices into the original source where possible to avoid unnecessary allocation.
  - Most text content in HTML does not need to be copied. Slice references into the original source byte buffer are zero-cost.

---

## No AI Features

> **Why is this explicit?** Because every major browser vendor is currently integrating LLMs, "AI assistants", and cloud-based features into their browsers. These features invariably send user data — browsing context, selected text, page content — to remote servers. This is incompatible with the privacy model of this browser. Ferrum renders pages. It does not think about them, summarize them, or phone home about them.

- No LLM integration. No "AI-powered" anything. This is a hard line, not a guideline.
- No smart address bar suggestions that call external APIs. Autocomplete from local history and bookmarks only.
- No content summarization, translation assistance, or reading mode that makes external network requests.
- No built-in ad "optimization", content ranking, or "personalization" of any kind.
- The browser renders what the page says. It does not editorialize, rewrite, or annotate content.

---

## Documentation

> **Why document everything?** A browser engine is complex enough that even the original author will forget why a decision was made six months later. Documentation is not for other people — it is for future you. An AI assistant reading undocumented code will hallucinate intent. A documented codebase produces better AI suggestions because the context is explicit.

- Every public function, struct, enum, and trait must have a `///` doc comment.
- Doc comments must explain **what** the item does, **why** it exists, and **what invariants** it upholds or assumes. "What the code does" is readable from the code. The doc comment should explain what is not obvious from reading the signature.
- Every crate must have a `//! Crate-level doc comment` at the top of `lib.rs` or `main.rs` explaining the crate's purpose, its place in the architecture, and its primary public API surface.
- Non-obvious code decisions must have an inline `//` comment explaining the reasoning. If you had to think about it for more than 30 seconds, future readers will too.
- Deviations from the WHATWG spec: use `// SPEC DEVIATION: [spec section] — [reason]`.
- Unsafe blocks: use `// SAFETY: [explanation of why this is safe]`.
- Performance-sensitive sections: use `// PERF: [what optimization is being done and why]`.
- Security-relevant sections: use `// SECURITY: [what threat is being mitigated]`.
- Keep a `CHANGELOG.md` at the repo root. Every meaningful change goes in it before merging.
- Architecture decisions that affect multiple crates go in `docs/decisions/` as short ADR (Architecture Decision Record) files. Format: problem statement, options considered, decision made, rationale.

---

## Testing

> **Why fuzz testing specifically for the parser?** HTML parsers are the single most common source of browser security vulnerabilities historically. Fuzzing finds the inputs that crash or mis-parse in ways that unit tests never will because no human thinks to write those test cases. It is not optional for a parser that will process untrusted input from the internet.

- Every public function in every crate must have at least one unit test.
- Parser and networking code must have integration tests against real or captured HTTP responses. Use `cargo test` with fixture files in `tests/fixtures/`.
- Use `cargo test` — no external test runners.
- Fuzz testing for the HTML tokenizer and CSS parser is required before those components are considered stable. Use `cargo-fuzz`. Fuzz targets live in `fuzz/`.
- Test files live in `tests/` at the crate root for integration tests, or in `#[cfg(test)]` modules in the source file for unit tests.
- Test names must describe the scenario being tested, not the function being called. `test_parse_unclosed_tag_recovers_correctly` is good. `test_parse` is not.

---

## Platform Support

> **Why Linux and Windows both tier-1?** The C++ prototype was Windows-only because of `winsock2`. That was a mistake. Rust's `std::net` and `tokio` abstract over platform sockets, so there is no reason to ever write platform-specific networking code. Linux is the natural development environment for a systems project; Windows is where most end users are. Both must work.

- Linux (x86_64 and aarch64) and Windows (x86_64) are tier-1 targets. Both must compile and pass all tests on every PR.
- macOS is a tier-2 target — best effort, not blocking. Do not break it intentionally.
- No platform-specific code outside of clearly marked `#[cfg(target_os = "windows")]` or `#[cfg(unix)]` blocks.
- No `winsock2`, no `windows.h`, no Windows-specific socket code anywhere. Use `tokio::net` or `std::net` which compile correctly on all platforms.

---

## Dependencies

> **Why audit dependencies?** Your codebase is only as trustworthy as its dependency tree. A compromised or vulnerable crate upstream can introduce security vulnerabilities you didn't write and may not notice. The rules below are not bureaucracy — they are a lightweight supply chain security practice.

- Every new dependency requires justification in the PR description: what it does, why it's needed, why an existing dependency doesn't already cover it.
- Prefer crates that are `no_std` compatible where possible — this signals that the crate has minimal implicit dependencies and assumptions.
- Audit dependencies with `cargo audit` before any release. A RUSTSEC advisory is a blocker unless there is a documented reason it does not apply.
- Banned dependencies:
  - Anything with telemetry, analytics, or network calls outside of explicitly network-purpose crates
  - `openssl-sys` or any crate that links OpenSSL (use `rustls`)
  - Any crate with a known unresolved RUSTSEC advisory at time of adding

---

## Session Commit Logs

> **Why a session log per commit?** Git commit messages tell you *what* changed. They do not tell you *why* a decision was made, *what alternatives were considered*, *what was tried and failed*, or *what the mental model was at the time*. Six months later, when a bug surfaces in code you don't remember writing, a session log is the difference between a 10-minute fix and a 2-hour archaeology session. This is especially important for a browser engine where a single function may implement a subtle part of a spec with non-obvious edge cases.

### Rule

Before every `git commit`, a session log file must be created. No exceptions. The commit must not be made if the session log does not exist.

### File Location and Naming

All session logs live in `docs/sessions/`. The filename format is:

```
docs/sessions/YYYY-MM-DD_short-description.md
```

Examples:
- `docs/sessions/2026-03-10_html-tokenizer-raw-text-mode.md`
- `docs/sessions/2026-03-11_tls-rustls-migration.md`
- `docs/sessions/2026-03-12_privacy-warning-interstitial.md`

Use today's date and a short kebab-case description of what the session accomplished. If multiple commits happen in one day on the same feature, append `-2`, `-3` etc.

### Required Sections

Every session log must contain all of the following sections. Do not skip any. If a section has nothing to say, write "N/A" and one sentence explaining why.

---

```markdown
# Session: [Short description matching filename]

**Date:** YYYY-MM-DD
**Crates modified:** [list of crates touched, e.g. `crates/net`, `crates/html`]
**Files changed:** [list of files added, modified, or deleted]

---

## What Was Built

[Plain-language description of what was implemented or changed in this session.
Write this as if explaining to a teammate who wasn't there. No jargon without
explanation. 2-5 paragraphs.]

---

## Why These Decisions Were Made

[For every non-obvious implementation decision, explain the reasoning. Why this
approach and not an alternative? What tradeoffs were made? If you chose arena
allocation over Box<T>, say why. If you chose a specific error type structure,
explain what it enables. This is the most important section.]

---

## What Was Tried and Didn't Work

[Honest account of dead ends, failed approaches, and wrong assumptions. This
section saves future contributors from repeating the same mistakes. If you tried
three approaches before landing on the one that works, write them all down with
why they failed.]

---

## Spec References

[If this session involved implementing any part of the WHATWG HTML spec, CSS
spec, HTTP spec, TLS spec, or any other standard, list the specific sections
referenced. Format: "WHATWG HTML § 13.2.5 — Tokenization" with a URL if
applicable. Write N/A if no spec was involved.]

---

## Known Limitations and TODOs

[What does this implementation NOT handle yet? What edge cases are known but
deferred? What would need to change if requirements evolved? Be specific.
"Doesn't handle chunked transfer encoding" is good. "Needs more work" is not.]

---

## Test Coverage

[What tests were written for this session's code? List the test names or
describe what scenarios are covered. If no tests were written, explain why and
when they will be added. "Will add fuzz tests in next session" is acceptable.
"Didn't write tests" with no plan is not.]
```

---

### Enforcement

- The `docs/sessions/` directory must be committed in the same commit as the code it documents. Do not commit code and add the session log later.
- The session log is part of the commit, not a separate follow-up commit.
- If using the AI assistant to write code: the AI assistant must generate the session log draft as part of the same response that produces the code. The developer reviews and edits it before committing — do not commit an AI-generated session log without reading it.
- Session logs are permanent. Do not delete or rewrite them after the fact. If a decision turned out to be wrong, document that in the next session's "What Was Tried and Didn't Work" section.

---

## Web Compatibility

> **The goal:** A user should be able to visit any mainstream website — news, social media, e-commerce, video streaming, web apps — and have it work correctly. A browser that protects privacy but breaks half the web will not be used. Privacy and compatibility are not opposites. They are a tradeoff that must be managed carefully: block what can be blocked without breaking functionality, warn about the rest, and give the user the final say.

- **Target compatibility: modern web standards as of 2024.** HTML5, CSS3, ES2022 JavaScript, WebSockets, Fetch API, Service Workers (with privacy constraints), Web Workers, Canvas (with fingerprinting protection), CSS Grid, CSS Flexbox, media queries, responsive images. If a mainstream site requires it, we must support it.
- **Graceful degradation, not hard failure.** If a feature is not yet implemented, the browser must not crash or show a blank page. It must render what it can and skip what it cannot. A page missing its animations or a non-critical third-party widget is acceptable. A page that is completely blank because one CSS feature is unimplemented is not.
- **Do not break sites by blocking too aggressively.** The privacy warning system exists precisely so that blocking is transparent and reversible. If blocking a tracker breaks a site's core functionality, the user must be informed and given the option to allow it for that site. Silent breakage is worse than no protection at all.
- **Test against real-world sites during development.** Do not only test against hand-crafted HTML fixtures. Periodically test against high-traffic sites (Wikipedia, GitHub, news sites, YouTube) to catch compatibility regressions early. Document failures in `docs/compatibility-notes.md`.
- **CSS compatibility is as important as HTML compatibility.** Most modern sites are layout-dependent. A site that renders with broken layout is functionally unusable even if the HTML parsed correctly.
- **Chunked transfer encoding, gzip/deflate/brotli compression, and HTTP redirects must work correctly.** These are not edge cases — virtually every production web server uses them.
- **Mixed content policy:** Block active mixed content (scripts, iframes loaded over HTTP on an HTTPS page) hard. Warn on passive mixed content (images over HTTP). Do not silently allow either.

---

## Browser UI

> **The design principle:** The browser should feel like it gets out of the way. The user came to visit a website, not to interact with the browser. The UI should be invisible when everything is working, and clear and honest when it needs to communicate something (a privacy warning, a certificate error, a blocked resource). First-time users should be able to navigate immediately without reading documentation. There is no onboarding flow, no tour, no setup wizard.

### Start Page

- The start page is rendered **entirely locally**. No remote content, no news feed, no search suggestions loaded from a server, no "top sites" that phone home. It is a static local HTML page.
- The start page contains: a search bar (submits to the configured search engine), and nothing else by default. Clean, minimal, fast.
- No sponsored content, no "recommended articles", no partner logos. Ever.
- The user can customise the start page background colour or set a local image. No cloud wallpaper service.

### Address Bar

- Shows the current URL in full. Does not hide the protocol or path.
- HTTPS sites: show a simple lock icon, no colour theatrics.
- HTTP sites: show a clear "Not secure" label in the address bar. Not a popup, not a modal — always visible while on that page.
- Privacy-flagged sites (from the warning system): show a persistent indicator that this site has active privacy exceptions granted.
- Autocomplete from local history and bookmarks only. No search engine suggestions that send keystrokes to a remote server before the user presses Enter.

### General UI Rules

- **Minimalist by default.** Toolbar contains: back, forward, reload, address bar, privacy indicator, menu. Nothing else unless the user adds it.
- **No modal dialogs for routine actions.** Closing a tab does not ask "are you sure?". Opening a new tab does not ask what to do. Modals are reserved for irreversible destructive actions only.
- **Privacy indicator** in the toolbar shows at a glance: whether JavaScript is enabled for the current site, whether any tracker was blocked on this page, and whether any privacy exceptions are active. One icon, expandable on click. Not intrusive.
- **Settings must be discoverable without documentation.** A user who has never seen the browser before must be able to find the JavaScript toggle, the cookie settings, and the search engine setting within 60 seconds without help.
- **Error pages are honest and human-readable.** "This site's certificate has expired" not "ERR_CERT_DATE_INVALID". "The connection was refused" not "ERR_CONNECTION_REFUSED". Show the technical detail in a collapsed section for users who want it.
- **No animations on functional UI elements.** Tabs open instantly. Pages load without animated spinners that delay perceived performance. Animations are permitted only for transitions that aid spatial understanding (e.g. tab switching).
- **UI must work at any window size.** The browser must be usable at 800×600 and at 4K. No fixed-width layouts in the browser chrome.
- **Keyboard navigable.** Every action in the browser must be reachable without a mouse. Address bar: `Ctrl+L`. New tab: `Ctrl+T`. Settings: accessible via keyboard. Privacy indicator: expandable via keyboard.

---

## What the AI Assistant Must Not Do

> These rules exist because AI assistants will, without explicit instruction, do all of these things. They are trained on codebases that do them. The rules below override that default behavior for this project.

- Do not suggest adding telemetry, analytics, or any form of user tracking — not even "optional" or "privacy-respecting" variants.
- Do not suggest OpenSSL or any C-based TLS library. Always suggest `rustls`.
- Do not write `unwrap()` in library code. If you are about to write `unwrap()`, write `?` instead and propagate the error.
- Do not refactor working, tested code into a different architecture without being explicitly asked. Unsolicited refactors introduce bugs and waste review time.
- Do not add dependencies without flagging it explicitly in your response: "This requires adding crate X (version Y) to Cargo.toml."
- Do not suggest AI/ML features. This browser has no AI features by design. If asked to add them, decline and explain why.
- Do not port C++ code to Rust by translating it line-for-line. The C++ used raw pointers, manual memory management, and Windows-specific APIs. The Rust rewrite should use idiomatic Rust: ownership, `Result`, arena allocation, `tokio` for async I/O. Rewrite the logic, do not transliterate the syntax.
- Do not generate code without doc comments on public items. Every public function, struct, and enum you generate must have a `///` doc comment.
- When generating code intended for a commit, always produce a session log draft in the same response. Format it exactly as specified in the Session Commit Logs section. Label it clearly as a draft for the developer to review. Do not skip this even if the change seems small.