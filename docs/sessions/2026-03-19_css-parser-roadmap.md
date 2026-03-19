# CSS Parser Roadmap — Ferrum Browser

> A complete implementation plan for `crates/css`. Covers what a privacy-first
> Rust browser shipping in 2026 must support, split into a prototype milestone
> (renders real pages, even if ugly) and a production milestone (competitive
> with mainstream engines on the top 1000 sites).

---

## Current State

`crates/css/src/lib.rs` is a stub. A `parse_css()` function exists that returns
`CssError::NotImplemented`. The only dependency is `thiserror`. Everything below
is greenfield.

---

## How a CSS Engine Works — Pipeline Overview

```
  Raw CSS text
       │
  ┌────▼─────┐
  │ Tokenizer │  CSS Syntax Level 3 — produces tokens (idents, numbers, strings, etc.)
  └────┬──────┘
       │
  ┌────▼──────┐
  │  Parser   │  Consumes tokens → produces a stylesheet AST (rules, selectors, declarations)
  └────┬──────┘
       │
  ┌────▼──────────┐
  │ Selector      │  Match selectors against DOM nodes — specificity + cascade order
  │ Matching      │
  └────┬──────────┘
       │
  ┌────▼──────────┐
  │ Cascade &     │  For each DOM node, resolve which declaration wins per property
  │ Specificity   │  (origin, specificity, order, !important, inheritance)
  └────┬──────────┘
       │
  ┌────▼──────────┐
  │ Computed      │  Resolve relative values (em, %, inherit, initial, currentColor)
  │ Values        │  into absolute values (px, rgba)
  └────┬──────────┘
       │
  ┌────▼──────────┐
  │ Used Values   │  Final values after layout constraints are applied
  │ (in layout)   │  (e.g. percentage widths resolved against parent)
  └──────────────┘
```

Each stage is a distinct module in `crates/css`. The layout crate consumes the
computed values and produces the box tree.

---

## Prototype Milestone — "Renders Real Pages"

The goal is to render the top 100 sites with correct structure, readable text,
and roughly correct layout. Colors, spacing, and text should look right. Complex
layouts (grid, flexbox) can fall back to block flow. Animations are ignored.

### Phase P1: Tokenizer (CSS Syntax Level 3)

**Spec:** [CSS Syntax 3 §4](https://www.w3.org/TR/css-syntax-3/#tokenization)

The tokenizer converts raw CSS text into a stream of typed tokens. This is the
foundation — every other phase consumes tokens.

| Token type | Example | Notes |
|---|---|---|
| `<ident-token>` | `color`, `margin-left` | Case-insensitive for properties |
| `<function-token>` | `rgb(`, `url(` | Ident followed by `(` |
| `<at-keyword-token>` | `@media`, `@import` | `@` followed by ident |
| `<hash-token>` | `#fff`, `#main` | `#` followed by name chars |
| `<string-token>` | `"hello"`, `'world'` | Supports escape sequences |
| `<number-token>` | `42`, `3.14`, `-1` | Integer or float, with sign |
| `<percentage-token>` | `50%` | Number followed by `%` |
| `<dimension-token>` | `16px`, `2em`, `90deg` | Number followed by unit |
| `<whitespace-token>` | ` `, `\n`, `\t` | Collapsed to single token |
| `<colon>` | `:` | Property/value separator |
| `<semicolon>` | `;` | Declaration terminator |
| `<comma>` | `,` | Selector list separator |
| `<{-token>`, `<}-token>` | `{`, `}` | Block delimiters |
| `<(-token>`, `<)-token>` | `(`, `)` | Function arguments |
| `<[-token>`, `<]-token>` | `[`, `]` | Attribute selectors |
| `<delim-token>` | `*`, `.`, `>`, `+`, `~` | Single code point |
| `<CDC-token>`, `<CDO-token>` | `-->`, `<!--` | Legacy HTML comment delimiters |
| `<EOF-token>` | | End of input |

**Key implementation detail:** The tokenizer must handle CSS escape sequences
(`\XX` hex escapes, `\` followed by any character), which appear in selectors
and string values. Get this wrong and every ID/class with special characters breaks.

**Deliverable:** `css::tokenizer::tokenize(&str) -> Vec<Token>` with unit tests
for every token type, escape sequences, and error recovery (bad escapes, unterminated
strings).

---

### Phase P2: Parser — Stylesheet AST

**Spec:** [CSS Syntax 3 §5](https://www.w3.org/TR/css-syntax-3/#parsing)

Consumes tokens and produces a structured AST:

```rust
struct Stylesheet {
    rules: Vec<Rule>,
}

enum Rule {
    Style(StyleRule),       // .foo { color: red }
    AtRule(AtRule),          // @media, @import, @font-face
}

struct StyleRule {
    selectors: SelectorList,
    declarations: Vec<Declaration>,
}

struct Declaration {
    property: PropertyName,
    value: CssValue,
    important: bool,
}
```

**Properties to parse in prototype** (enough to render readable pages):

| Category | Properties |
|---|---|
| **Display** | `display` (block, inline, none, inline-block) |
| **Box model** | `margin`, `padding`, `border`, `width`, `height`, `max-width`, `min-width`, `max-height`, `min-height`, `box-sizing` |
| **Text** | `font-family`, `font-size`, `font-weight`, `font-style`, `line-height`, `text-align`, `text-decoration`, `text-transform`, `letter-spacing`, `word-spacing`, `white-space` |
| **Color** | `color`, `background-color`, `opacity` |
| **Background** | `background-image` (url only), `background-repeat`, `background-position`, `background-size` |
| **Position** | `position` (static, relative, absolute, fixed), `top`, `right`, `bottom`, `left`, `z-index` |
| **Overflow** | `overflow`, `overflow-x`, `overflow-y` |
| **List** | `list-style-type`, `list-style-position` |
| **Table** | `border-collapse`, `border-spacing` |
| **Visibility** | `visibility`, `cursor` |
| **Float** | `float`, `clear` |

**Shorthand expansion:** `margin: 10px 20px` must expand to `margin-top: 10px`,
`margin-right: 20px`, `margin-bottom: 10px`, `margin-left: 20px`. Same for
`padding`, `border`, `background`, `font`. This is where most CSS parsers have
subtle bugs — get the shorthand reset rules right from the start.

**Error recovery:** CSS is designed to be forward-compatible. Unknown properties
and invalid values must be ignored, not cause a parse error that kills the
stylesheet. The spec defines precise error recovery rules for every context.

**Deliverable:** `css::parser::parse(tokens) -> Stylesheet` with tests for
shorthand expansion, `!important`, error recovery, and `@import`/`@media` at-rules.

---

### Phase P3: Selectors and Matching

**Spec:** [Selectors Level 4](https://www.w3.org/TR/selectors-4/)

The selector engine matches CSS rules against DOM nodes. This is the hottest path
in the CSS engine — it runs for every node × every rule.

**Prototype selectors** (covers ~95% of real-world CSS):

| Selector | Example | Priority |
|---|---|---|
| Universal | `*` | Must have |
| Type | `div`, `p`, `a` | Must have |
| Class | `.nav`, `.active` | Must have |
| ID | `#header` | Must have |
| Descendant | `div p` | Must have |
| Child | `div > p` | Must have |
| Attribute | `[href]`, `[type="text"]` | Must have |
| Attribute substring | `[class~="foo"]`, `[href^="https"]` | Must have |
| Pseudo-class | `:first-child`, `:last-child`, `:nth-child(n)` | Must have |
| Pseudo-class | `:hover`, `:focus`, `:active`, `:visited` | Must have |
| Pseudo-class | `:not()`, `:is()`, `:where()` | Must have |
| Adjacent sibling | `h1 + p` | Must have |
| General sibling | `h1 ~ p` | Must have |
| Pseudo-element | `::before`, `::after` | Must have |

**Specificity calculation:** `(id_count, class_count, type_count)` — this is
well-defined in the spec and must be exact. Wrong specificity = wrong styles
on every major website.

**Matching direction:** Match right-to-left. Start from the rightmost (key)
selector, check the current element, then walk up the DOM tree to verify
ancestors. This is how every production browser does it — left-to-right matching
is O(n × m) where n is DOM depth and m is rule count.

**Deliverable:** `css::selector::matches(node, selector) -> bool` and
`css::selector::specificity(selector) -> (u32, u32, u32)` with full test suite.

---

### Phase P4: Cascade and Inheritance

**Spec:** [CSS Cascade 4](https://www.w3.org/TR/css-cascade-4/)

For each DOM node, determine the final computed value of every property.

**Cascade order** (highest to lowest priority):
1. User-agent `!important` declarations
2. Author `!important` declarations
3. Author normal declarations
4. User-agent normal declarations
5. Inherited value from parent
6. Property's initial value

Within the same origin and importance, higher specificity wins. Within the
same specificity, later declaration order wins.

**Inheritance:** Some properties inherit by default (`color`, `font-family`,
`line-height`, `text-align`, etc.). Others don't (`margin`, `padding`, `border`,
`display`, etc.). The spec defines this per property. Must be a lookup table.

**`inherit`, `initial`, `unset` keywords:** Must handle all three on any property.

**Deliverable:** `css::cascade::compute_styles(dom, stylesheets) -> StyledTree`
where each node carries its computed `StyleProperties`.

---

### Phase P5: Value Resolution

Convert relative and abstract CSS values into concrete values the layout engine
can consume:

| Input | Output | Rule |
|---|---|---|
| `2em` | `32px` (if parent font-size is 16px) | Relative to inherited font-size |
| `50%` width | Resolved during layout | Percentage of containing block |
| `auto` margin | Resolved during layout | Depends on context |
| `red` | `rgba(255, 0, 0, 1.0)` | Named color → RGBA |
| `#0af` | `rgba(0, 170, 255, 1.0)` | Hex shorthand → RGBA |
| `rgb(0, 170, 255)` | `rgba(0, 170, 255, 1.0)` | Function → RGBA |
| `currentColor` | Parent's computed `color` | Resolve at use site |
| `inherit` | Parent's computed value | Walk up tree |
| `initial` | Property's initial value | Spec-defined |

**Color parsing** is its own sub-problem: named colors (147 CSS named colors),
3/4/6/8-digit hex, `rgb()`, `rgba()`, `hsl()`, `hsla()`. Prototype needs all of
these — they appear on virtually every website.

**Deliverable:** `css::values::resolve(property, raw_value, context) -> ComputedValue`

---

### Prototype Stop Point

After completing P1–P5, the browser can:
- Fetch a page (net crate — done)
- Parse HTML into a DOM (html crate — done)
- Parse `<style>` blocks and inline styles
- Match selectors to DOM nodes
- Compute styles for every node
- Hand off computed styles to the layout engine

This is the **minimum viable CSS engine**. Layout (crates/layout) takes over from
here and uses the computed styles to build the box tree.

**What will look wrong at this point:**
- No flexbox or grid — complex layouts fall back to block/inline flow
- No animations or transitions — pages are static
- No `calc()` — computed values only handle simple arithmetic
- No media queries beyond basic screen/print
- No custom properties (`--var`) or `var()` function
- No web fonts (system fonts only)

**What will look right:**
- Text is readable with correct fonts, sizes, weights, colors
- Block and inline layout is structurally correct
- Margins, padding, borders render correctly
- Links are styled, headings are sized, lists have bullets
- Colors and backgrounds appear
- `display: none` hides elements
- Basic responsive behavior via percentage widths

---

## Production Milestone — "Competitive on Top 1000 Sites"

These features are needed to render modern websites without visible breakage.
They are ordered by impact — how many sites break without them.

### Phase A1: Flexbox (CSS Flexible Box Layout)

**Spec:** [CSS Flexbox Level 1](https://www.w3.org/TR/css-flexbox-1/)

**Why it matters:** ~90% of modern websites use flexbox for navigation bars,
card layouts, form layouts, and page structure. Without flexbox, most sites
will have severely broken layouts.

| Property | Notes |
|---|---|
| `display: flex`, `display: inline-flex` | Container setup |
| `flex-direction` | row, column, row-reverse, column-reverse |
| `flex-wrap` | nowrap, wrap, wrap-reverse |
| `justify-content` | flex-start, center, space-between, space-around, space-evenly |
| `align-items`, `align-self` | stretch, center, flex-start, flex-end, baseline |
| `align-content` | Multi-line alignment |
| `flex-grow`, `flex-shrink`, `flex-basis` | Item sizing |
| `order` | Visual reordering |
| `gap`, `row-gap`, `column-gap` | Spacing between items |

**Complexity:** High. The flex layout algorithm has 9 steps with multiple
sub-steps. Intrinsic sizing, min/max constraints, and flex-basis interaction
make this the hardest single layout feature to implement correctly.

---

### Phase A2: CSS Grid

**Spec:** [CSS Grid Layout Level 1](https://www.w3.org/TR/css-grid-1/)

**Why it matters:** ~40% of modern sites use grid for page-level layout.
Dashboard UIs, image galleries, and magazine-style layouts depend on it.

| Property | Notes |
|---|---|
| `display: grid`, `display: inline-grid` | Container setup |
| `grid-template-columns`, `grid-template-rows` | Track sizing with `fr`, `auto`, lengths |
| `grid-template-areas` | Named area placement |
| `grid-column`, `grid-row` | Item placement (start/end lines) |
| `grid-gap` / `gap` | Track spacing |
| `grid-auto-flow` | Implicit track placement (row, column, dense) |
| `grid-auto-rows`, `grid-auto-columns` | Implicit track sizing |
| `justify-items`, `align-items` | Cell alignment |
| `minmax()`, `repeat()`, `auto-fill`, `auto-fit` | Track sizing functions |

---

### Phase A3: Media Queries

**Spec:** [Media Queries Level 4](https://www.w3.org/TR/mediaqueries-4/)

| Feature | Example |
|---|---|
| `@media screen` | Screen vs print |
| `@media (max-width: 768px)` | Responsive breakpoints |
| `@media (min-width: 1024px)` | Desktop targeting |
| `@media (prefers-color-scheme: dark)` | Dark mode |
| `@media (prefers-reduced-motion: reduce)` | Accessibility |
| `@media (orientation: portrait)` | Device orientation |
| Combinators: `and`, `not`, `or`, `,` | Compound queries |

**Privacy note:** `prefers-color-scheme` and `prefers-reduced-motion` are
fingerprinting vectors. Ferrum should default to `light` and `no-preference`
unless the user explicitly opts in. Document this as a privacy-first default.

---

### Phase A4: Custom Properties and `var()`

**Spec:** [CSS Custom Properties Level 1](https://www.w3.org/TR/css-variables-1/)

```css
:root {
    --primary: #0066cc;
    --spacing: 16px;
}
.button {
    color: var(--primary);
    padding: var(--spacing);
}
```

Custom properties inherit down the DOM tree and are resolved at computed-value
time. Cyclic references must be detected and treated as invalid. Fallback values
(`var(--x, red)`) must work.

**Why it matters:** Most design systems and CSS frameworks (Tailwind, Bootstrap 5,
Material UI) use custom properties extensively. Without them, theming breaks on
nearly every modern site.

---

### Phase A5: `calc()`, `min()`, `max()`, `clamp()`

```css
width: calc(100% - 2rem);
font-size: clamp(14px, 2vw, 22px);
padding: min(5%, 20px);
```

These math functions are used pervasively in responsive designs. `calc()` alone
appears on ~70% of modern sites. Must handle mixed units (%, px, em, vw)
where resolution happens at different pipeline stages.

---

### Phase A6: Transitions and Animations

**Specs:** [CSS Transitions](https://www.w3.org/TR/css-transitions-1/),
[CSS Animations](https://www.w3.org/TR/css-animations-1/)

| Feature | Properties |
|---|---|
| Transitions | `transition-property`, `transition-duration`, `transition-timing-function`, `transition-delay` |
| Animations | `@keyframes`, `animation-name`, `animation-duration`, `animation-timing-function`, `animation-iteration-count`, `animation-direction`, `animation-fill-mode` |
| Timing functions | `ease`, `linear`, `ease-in-out`, cubic-bezier, `steps()` |

**Privacy note:** CSS animation timing can be used as a side channel for
fingerprinting (measuring frame rate to detect GPU model). Ferrum should
quantize animation frame timing to standard 60fps boundaries.

---

### Phase A7: Transforms

**Spec:** [CSS Transforms Level 1](https://www.w3.org/TR/css-transforms-1/)

| Function | Notes |
|---|---|
| `translate()`, `translateX()`, `translateY()` | Movement |
| `scale()`, `scaleX()`, `scaleY()` | Sizing |
| `rotate()` | 2D rotation |
| `skew()`, `skewX()`, `skewY()` | Distortion |
| `matrix()` | Full 2D transform matrix |
| `transform-origin` | Center of transformation |

Level 2 (3D transforms: `perspective`, `rotateX/Y/Z`, `translate3d`) is deferred
to post-production.

---

### Phase A8: Web Fonts (`@font-face`)

**Spec:** [CSS Fonts Level 4](https://www.w3.org/TR/css-fonts-4/)

```css
@font-face {
    font-family: "CustomFont";
    src: url("/fonts/custom.woff2") format("woff2");
    font-weight: 400;
    font-display: swap;
}
```

| Feature | Notes |
|---|---|
| `@font-face` parsing | family, src, weight, style, display |
| WOFF2 decoding | Compressed font format (requires `woff2` crate) |
| `font-display` | `swap`, `fallback`, `optional` — controls FOIT/FOUT |
| Font matching | Map `font-family` stack to available fonts |

**Privacy note:** Font requests are a tracking vector. The `Referer` header
(already suppressed by Ferrum) and timing of font requests can fingerprint users.
Consider loading fonts through `NetworkContext` with the same privacy policy as
any other fetch. `font-display: swap` should be the enforced default to avoid
invisible text while fonts load.

---

### Phase A9: Pseudo-elements and Generated Content

```css
.tooltip::before {
    content: attr(data-tip);
}
.required::after {
    content: " *";
    color: red;
}
```

`::before` and `::after` with `content` are used on ~60% of modern sites for
icons, decorations, clearfixes, and accessible labels. The parser must create
synthetic DOM nodes for these and apply styles to them.

---

### Phase A10: Advanced Selectors

| Selector | Example | Notes |
|---|---|---|
| `:has()` | `div:has(> img)` | Parent selector — relatively new, growing adoption |
| `:nth-of-type()` | `p:nth-of-type(2n)` | Type-filtered nth |
| `:empty` | `div:empty` | No children |
| `:checked`, `:disabled` | Form pseudo-classes | Form styling |
| `::placeholder` | Input placeholder styling | |
| `::selection` | Text highlight styling | |

---

### Phase A11: Multi-column Layout

**Spec:** [CSS Multi-column Layout](https://www.w3.org/TR/css-multicol-1/)

Lower priority — used by ~5% of sites (news articles, long-form content).

---

### Phase A12: Containment and Container Queries

**Specs:** [CSS Containment Level 2](https://www.w3.org/TR/css-contain-2/),
[CSS Container Queries](https://www.w3.org/TR/css-contain-3/)

```css
.card-container {
    container-type: inline-size;
}
@container (min-width: 400px) {
    .card { flex-direction: row; }
}
```

Growing adoption in 2025–2026. Component-level responsive design. This will
become as important as media queries within 2 years.

---

## Implementation Order Summary

```
PROTOTYPE (renders pages, text is readable, structure is correct)
═══════════════════════════════════════════════════════════════════
 P1  Tokenizer                           ██░░░░░░░░  ~2 weeks
 P2  Parser + AST                        ████░░░░░░  ~3 weeks
 P3  Selectors + Matching                ███░░░░░░░  ~2 weeks
 P4  Cascade + Inheritance               ███░░░░░░░  ~2 weeks
 P5  Value Resolution + Colors           ██░░░░░░░░  ~2 weeks
                                                     ─────────
                                              Total: ~11 weeks
═══════════════════════════════════════════════════════════════════

PRODUCTION (competitive on modern web)
═══════════════════════════════════════════════════════════════════
 A1  Flexbox                             █████░░░░░  High — most impactful
 A2  Grid                                █████░░░░░  High — second most impactful
 A3  Media Queries                       ███░░░░░░░  High — responsive design
 A4  Custom Properties / var()           ██░░░░░░░░  High — theming
 A5  calc/min/max/clamp                  ██░░░░░░░░  High — responsive sizing
 A6  Transitions + Animations            ████░░░░░░  Medium — visual polish
 A7  Transforms                          ███░░░░░░░  Medium — visual polish
 A8  Web Fonts (@font-face + WOFF2)      ████░░░░░░  Medium — typography
 A9  Pseudo-elements + content           ██░░░░░░░░  Medium — icons, decorations
 A10 Advanced Selectors (:has, etc.)     ██░░░░░░░░  Medium — modern CSS
 A11 Multi-column                        █░░░░░░░░░  Low — niche usage
 A12 Container Queries                   ███░░░░░░░  Low now, high by 2027
═══════════════════════════════════════════════════════════════════
```

---

## Dependencies to Add

| Crate | Purpose | Phase |
|---|---|---|
| `cssparser` (servo) | Consider as tokenizer reference — Mozilla's CSS tokenizer, battle-tested | P1 |
| `selectors` (servo) | Consider as selector matching reference | P3 |
| `woff2-decoder` or similar | WOFF2 font decompression | A8 |

**Decision needed:** Write our own tokenizer/parser from scratch (full control,
educational, matches the approach taken for HTML) or use Servo's `cssparser` crate
(battle-tested, spec-compliant, but adds a significant dependency). The HTML
tokenizer was written from scratch successfully — same approach is recommended
for CSS to maintain consistency and understanding of the codebase.

---

## Privacy Considerations Specific to CSS

1. **`@import` fetches** go through `NetworkContext` — no bypass
2. **`url()` in backgrounds/fonts** go through `NetworkContext`
3. **`prefers-color-scheme`** defaults to `light` (no OS leak)
4. **`prefers-reduced-motion`** defaults to `no-preference` (no OS leak)
5. **Font enumeration via CSS** (`font-family` probing) — return generic fallback
   metrics, never confirm specific installed fonts
6. **CSS `:visited`** — only allow color changes, no layout changes (prevents
   history sniffing via computed style inspection)
7. **Animation timing quantization** — 60fps boundaries only
