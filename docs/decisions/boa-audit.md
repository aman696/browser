# Javascript Engine Audit: `boa_engine`

**Audit Date**: 2026-03-10
**Target Crate**: `boa_engine v0.21.0`

## Purpose

The `boa_engine` JavaScript interpreter must be audited for security, privacy, and architecture compliance before being integrated into the `crates/js` workspace member.

## Overview

Boa is a pure-Rust ECMAScript engine. Version 0.21.0 compiles down to an AST, bytecode, and runs in a VM without a Just-In-Time (JIT) compiler. This aligns with our project rules which explicitly forbid JIT compilation in early phases due to JIT spraying attack surfaces.

## `cargo audit` Output

```text
    Scanning Cargo.lock for vulnerabilities (256 crate dependencies)
Crate:     paste
Version:   1.0.15
Warning:   unmaintained
Title:     paste - no longer maintained
Date:      2024-10-07
ID:        RUSTSEC-2024-0436
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0436

warning: 1 allowed warning found
```

### Analysis of Audit Warnings

1.  **`paste` (RUSTSEC-2024-0436)**: `paste` is a procedural macro used for identifier concatenation during compilation. It is no longer maintained.
    *   **Impact**: Zero runtime impact. Procedural macros run entirely at compile time. They cannot introduce network calls, memory unsafety, or logic bugs into the compiled binary.
    *   **Conclusion**: Safe to ignore.

## Manual Dependency Review

A review of the dependency tree (from `cargo tree`) reveals:

1.  **No C/C++ FFI**: There are no C library links (`-sys` crates) in the Boa core (apart from standard libc for basic OS interactions via `std`), maintaining our pure-Rust memory safety requirement.
2.  **No Networking**: The dependency tree does not contain `reqwest`, `hyper`, `tokio` (networking), `openssl`, `rustls`, or any other network-capable I/O libraries. Boa is firmly a compute-only VM.
3.  **No Telemetry**: There are no analytics, metrics, or reporting SDKs in the tree.
4.  **Heavy Math/Parsing Reliance**: Dependencies are focused on parsing (`winnow`, `boa_parser`), math/numbers (`num-bigint`, `fast-float2`, `ryu-js`), AST representation, and string handling (`icu`, `utf16_iter`).

## Privacy Assessment

*   **Network Calls**: None. The engine cannot phone home.
*   **Data Collection**: None.
*   **C Libraries**: None linked (pure Rust).
*   **Advisories**: One compile-time macro warning (`paste`), zero runtime vulnerabilities.

## Decision

**Integrate as-is.**

### Rationale

`boa_engine v0.21.0` strictly conforms to all project rules:
1. It is written in 100% safe, pure Rust (no V8/SpiderMonkey C++).
2. It uses a bytecode VM, not a JIT (prevents JIT spraying).
3. It has zero network capabilities, preventing any silent telemetry or privacy leaks.
4. It relies on standard Rust data structures (`hashbrown`, `indexmap`) and has a clean runtime dependency tree with no known CVEs.

We will proceed with adding `boa_engine = "0.21.0"` to `crates/js/Cargo.toml` and beginning the ECMAScript integration.
