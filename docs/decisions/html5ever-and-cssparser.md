# Architectural Decision: Parsing Pipeline Libraries

**Date**: 2026-04-15
**Target Crates**: `html5ever` (HTML) and `cssparser` (CSS)

## Purpose

To finalize the strategy for building Ferrum's rendering pipeline — specifically the HTML and CSS parsing layers — to focus engineering effort on Ferrum's novel privacy architecture.

## Overview

The HTML tokenizer, currently partially implemented, would require an estimated 3-4 months to reach full WHATWG `html5lib-tests` compliance (error recovery, ~80 states).
A CSS tokenizer and parser would require similar effort to safely implement the CSS Syntax Level 3 specification.

Servo (the Mozilla-originated Rust browser engine, also funded by NLnet) has already solved both of these problems via `html5ever` and `cssparser` respectively. These are production-grade libraries, with `cssparser` currently powering Firefox's Stylo engine.

## Assessment

*   **html5ever**: A pure-Rust, WHATWG-compliant HTML tokenizer and tree builder.
    *   *Challenge*: It uses a callback-based `TreeSink` API. Bridging this with Ferrum's bumpalo-based, lifetime-tied `'arena` DOM will require careful interface boundary design, potentially using isolated `unsafe` raw pointers to satisfy the borrow checker.
*   **cssparser**: A pure-Rust, CSS Syntax Level 3 implementation.
    *   *Challenge*: It is intentionally low-level. It handles tokens and rules, but not cascade, inheritance, or selector matching. We will build those high-level layers within Ferrum.

## Decision

**Use `html5ever` and `cssparser`**. 

I will avoid duplicating years of foundational parsing work already done by the Rust browser ecosystem. By integrating these battle-tested libraries, we accelerate reaching a usable static-page rendering state. 

This decision allows the project to focus entirely on its unique value propositions:
1. The cascade and layout engines.
2. Forcing all resource loads (images, stylesheets) retrieved by the parsed HTML through our custom, non-bypassable `NetworkContext` privacy chokepoint. 

## Consequence

*   `crates/html`: Discard custom WHATWG state machine progress. Integrate `html5ever` and build the `TreeSink` implementation for our arena DOM.
*   `crates/css`: Integrate `cssparser`. Develop the custom cascade and property resolution engine on top of it.
*   `docs/FEATURE_LIST.md`: Mark custom parsing as superseded, replace with integration tasks.
