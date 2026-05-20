# `::before` / `::after` Pseudo-Element Support — Design

**Status:** Draft
**Date:** 2026-05-20
**Branch:** `crate/pseudo-before-after`
**Issue:** [#703](https://github.com/kane50613/takumi/issues/703)
**Depends on:** [#709](https://github.com/kane50613/takumi/pull/709) (forgiving-pseudo-selectors)

## 1. Goal

Make `::before` and `::after` rules functional in takumi: parse the `content` property, resolve it against the originating element, synthesize matching pseudo boxes in the render tree, and feed them through the existing Taffy layout / Vello render pipeline. Out of scope: every other pseudo-element, every other `content` value family. Those continue to fall to the `IgnoredPseudoElement` path from #709.

## 2. Scope

**In scope:**

- Pseudo-elements: `::before`, `::after`
- `content` values: `none`, `normal`, `<string>`, `attr(<name>[, <fallback>])`, `<image>` (`url()`, `linear-gradient()`, `radial-gradient()`, `conic-gradient()`)
- `content` as a _list_ (`content: "Prefix: " attr(label) url(icon.png)`)
- `display` overrides: `inline` (default), `block`, `inline-block`
- Cascade: pseudo inherits from originating element
- Replaced-element exclusion: `NodeKind::Image` (and SVG) cannot generate pseudos

**Out of scope (this PR):**

- `::marker` and `display: list-item`
- Counters (`counter-reset`/`-increment`/`-set`, `counter()`/`counters()`)
- Quotes (`quotes`, `open-quote`/`close-quote`/`no-*-quote`)
- `display: flex` / `grid` on pseudo (pseudo has no children to lay out)
- `::first-line`, `::first-letter`, `::prefix`, `::suffix`
- `attr()` typed form (`attr(name <type>, <fallback>)`)
- `target-*()`, `leader()`, `string()`, `content()`, `contents`

Out-of-scope `content` values that the parser still encounters (e.g. `counter()`) downgrade to `none` — the rule is preserved (so authors can keep them for future browser parity) but the pseudo does not generate a box.

## 3. Architecture: Strategy A — Render-tree synthesis

Pseudo boxes are synthesized inside `RenderNode::from_node_iterative`, **not** inserted into the input `Node` arena. This keeps `StyleArena` indexing, selector matching, and `Node`-level public API untouched. The render tree already supports synthetic nodes (`node: None`) via `anonymous_text_item` / `anonymous_block_container`, so we reuse the abstraction.

Modeled on Blink's `PseudoElement::AttachLayoutTree` (`third_party/blink/renderer/core/dom/pseudo_element.cc:521`) which:

1. Reads `style.GetContentData()` (a linked list of `ContentData` items)
2. For each non-alt item, creates a `LayoutObject` child of the pseudo's box
3. Filters via `IsChildAllowed`, skips when `ContentBehavesAsNormal()`

We mirror this in idiomatic Rust. Differences from Blink: no GC, no separate `Element` subclass, no ViewTransition / a11y / counter machinery.

### 3.1 Render tree shape

For an element with both pseudos and original children:

```
RenderNode (originating element)
├── RenderNode (::before pseudo box, node: None, ComputedStyle from ::before cascade)
│   ├── anonymous_text_item("Prefix: ")        ← from `<string>`
│   ├── anonymous_image_item(url(icon.png))    ← from `<image>`, NEW helper
│   └── anonymous_text_item(attr(label) value) ← from attr(), resolved
├── ... original children ...
└── RenderNode (::after pseudo box)
    └── ... same shape ...
```

Pseudo box's `display`:

- `inline` (default) → reuse `anonymous_box_context`-style inline container
- `block` → reuse `anonymous_block_container`
- `inline-block` → `Display::InlineBlock` with `node: None`, same context construction

For a single-item content where the pseudo is `display: inline`, we MAY fold the pseudo box into the lone child to save a layer (i.e. text item directly with pseudo's style). The plan starts simple — always emit a pseudo box, even for one item — and only adds the fold-optimization if profiling shows it matters.

## 4. Data model

### 4.1 New `Content` property value

```rust
// in layout/style/properties/content.rs (NEW)

pub(crate) enum ContentValue {
    Normal,                  // ::before/::after: behaves as None
    None,
    Items(Box<[ContentItem]>),
}

pub(crate) enum ContentItem {
    Text(Box<str>),
    Image(ImageSourceInput),
    AttrRef { name: TakumiIdent, fallback: Box<str> },
    // Unsupported (counter, quote, ...) are dropped at parse time:
    // the whole `content` declaration becomes ContentValue::None.
}
```

Default is `ContentValue::Normal` (per spec). `Image` reuses the existing `ImageSourceInput` so gradients and `url()` flow through the same `resolve()` path as `<img>` `src` and `background-image`.

`AttrRef` is resolved at render-tree-build time (after cascade, before pseudo box creation) — not at parse time — because the originating element's `metadata.attributes` is only known then. Resolved `AttrRef` becomes a `Text` item.

### 4.2 New `IgnoredPseudoElement` discriminant

```rust
// in layout/style/selector.rs (MODIFIED from #709)

pub(crate) enum PseudoElementKind {
    Before,
    After,
    Other(TakumiIdent),  // everything else still falls through to "ignored"
}

pub(crate) struct ParsedPseudoElement(PseudoElementKind);
```

`ParsedPseudoElement` replaces #709's `IgnoredPseudoElement`. `parse_pseudo_element` maps `"before"` / `"after"` (case-insensitive) to the `Before` / `After` variants; everything else stays as `Other` and is never matched, identical to today's behavior.

### 4.3 Matching output

```rust
// in layout/style/matching.rs (MODIFIED)

pub(crate) struct NodeMatchedDeclarations<'a> {
    pub element: MatchedDeclarationsView<'a>,
    pub before: Option<MatchedDeclarationsView<'a>>,
    pub after: Option<MatchedDeclarationsView<'a>>,
}

pub(crate) fn match_stylesheets_view<'a>(
    root: &Node,
    stylesheet: &'a StyleSheet,
    viewport: Viewport,
) -> Vec<NodeMatchedDeclarations<'a>> { ... }
```

The matching loop bucket-sorts each matched rule by the selector's terminal pseudo-element (read via `selectors::parser::Selector::pseudo_element()`). Rules with `Other` pseudo are discarded for that element (no bucket exists). Replaced elements (`NodeKind::Image`) get `before: None, after: None` regardless of what matched.

## 5. Cascade & ComputedStyle for pseudos

The pseudo's `MatchedDeclarationsView` is fed through the existing `build_style_layers` → `inherit` pipeline, with the **originating element's `ComputedStyle` as the parent**. This means:

- Inherited properties (color, font-\*, line-height, etc.) flow from element → pseudo, matching spec semantics
- The pseudo's own declarations (background, padding, content, etc.) override
- `display` defaults to `Display::Inline` if unset on the pseudo (CSS UA default; we set this as the pseudo box's initial value, not via UA stylesheet)

No new cascade infrastructure required; we reuse `resolve_computed_style` with the originating element as parent.

## 6. Pipeline integration

### 6.1 `from_node_iterative` (`layout/tree.rs`)

The current iterative builder visits each input `Node`, resolves its style, then recurses children. We modify the **post-style step** (after `resolve_computed_style` for the current node):

```text
1. resolve element's ComputedStyle (existing)
2. if matched_decls[index].before.is_some() and !is_replaced(node):
       build pseudo box → prepend to pending_children
3. recurse children (existing)
4. if matched_decls[index].after.is_some() and !is_replaced(node):
       build pseudo box → append to rendered_children
```

`is_replaced` returns `true` for `NodeKind::Image`. Containers and text are not replaced.

### 6.2 `build_pseudo_box` (NEW helper)

```text
input:  pseudo's MatchedDeclarationsView, originating ComputedStyle, viewport
output: Option<RenderNode<'g>>   (None if content resolves to no items)

steps:
  1. compute pseudo ComputedStyle via build_style_layers + inherit
  2. read content: ContentValue
     - Normal | None       → return None
     - Items(items)        → continue
  3. resolve each ContentItem:
     - Text(s)             → keep
     - AttrRef { name, fallback }
                           → look up originating Node metadata.attributes
                             - Some(value) → Text(value.into())
                             - None        → Text(fallback)
                             - empty string after resolution → drop the item
     - Image(src)          → keep (resolve at render time, same as <img>)
  4. if zero items remain (e.g. empty string content) → return None
  5. build children:
     - for each Text → anonymous_text_item with pseudo style
     - for each Image → anonymous_image_item with pseudo style + image data
  6. wrap children in pseudo container:
     - display: inline       → inline-friendly synthetic RenderNode
     - display: block        → anonymous_block_container
     - display: inline-block → similar to inline, layout_style_override sets InlineBlock
  7. return Some(pseudo_render_node)
```

### 6.3 `anonymous_image_item` (NEW)

Mirror of `anonymous_text_item` but carrying an `ImageData` (or `ImageSourceInput`) instead of a `String`. Layout/measure path uses existing `measure_image_node`; render path uses existing `draw_image`. Mostly a constructor + wiring — no new layout logic.

## 7. Content property parsing

`StyleDeclarationBlock::parse` handles `content:` by dispatching to a new parser in `layout/style/properties/content.rs`. The grammar we accept (a subset of [css-content-3 §2.1](https://www.w3.org/TR/css-content-3/#content-property)):

```
content        = normal | none | <content-list>
content-list   = [ <string> | <image> | <attr-fn> ]+
attr-fn        = attr( <ident> [, <string>]? )
```

Anything else in the value (counter, quote, target-\*, leader, ...) makes the **entire `content` declaration** fall back to `ContentValue::None`. We don't drop the whole rule — `color`/`background`/etc. on the same pseudo still apply, the pseudo just generates no content box.

`<image>` is parsed via the existing image-value parser (`FromCss for ImageSourceInput`-equivalent path used by `background-image`).

## 8. Edge cases

| Case                                      | Behavior                                                                                                           |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `content: ""`                             | After empty-item drop, items list is empty → no pseudo box                                                         |
| `content: normal` on `::before`/`::after` | Treated as `none`                                                                                                  |
| `content: none`                           | No pseudo box, but the rule's other declarations still apply to the (nonexistent) pseudo (i.e. silently no-op)     |
| `::before` on `<img>` / `NodeKind::Image` | `is_replaced(node)` skips both pseudos                                                                             |
| `::before` on root                        | Allowed, just generates inline content at start of root                                                            |
| Multiple matched `::before` rules         | Standard cascade picks one resolved `content`; `MatchedDeclarationsView` already orders these                      |
| `attr()` missing attribute, no fallback   | Resolved to empty string → item dropped                                                                            |
| `attr()` missing attribute, with fallback | Resolved to fallback string                                                                                        |
| Image fails to resolve                    | The item still occupies a zero-sized box (same as `<img>` with broken `src`); other items in the list still render |
| `display: none` on pseudo                 | Computed `display` is consulted before box construction; `None` ⇒ skip                                             |
| `display: flex` / `grid` / anything else  | Downgraded to `block`. Pseudo has at most a flat list of text/image items, so flex/grid semantics add nothing here |

## 9. Files affected

**Modified:**

- `takumi/src/layout/style/selector.rs` — split `IgnoredPseudoElement` into `ParsedPseudoElement { Before, After, Other }`
- `takumi/src/layout/style/matching.rs` — return `NodeMatchedDeclarations`, bucket by terminal pseudo
- `takumi/src/layout/style/properties/mod.rs` — register `content` property
- `takumi/src/layout/style/mod.rs` — `ComputedStyle::content`
- `takumi/src/layout/tree.rs` — `from_node_iterative` pseudo synthesis, `build_pseudo_box`, `anonymous_image_item`
- `takumi/src/layout/node/mod.rs` — `is_replaced(&Node)` helper (or `Node::is_replaced()`)

**New:**

- `takumi/src/layout/style/properties/content.rs` — `ContentValue`, `ContentItem`, parser

**Tests:**

- `takumi/tests/measure_tests.rs` — pseudo affects layout sizes (inline, block, inline-block, with images)
- `takumi/src/layout/style/selector.rs` — `::before` / `::after` parse round-trip
- `takumi/src/layout/style/properties/content.rs` — content value parsing matrix
- `takumi/tests/pseudo_element_tests.rs` (new file) — end-to-end measure/render with pseudos

**Changeset:** `.changeset/pseudo-before-after.md`

## 10. Test matrix

Minimal coverage to merge:

1. **Parse / cascade**
   - `::before { content: "x"; color: red }` parses, applies to originating element's first child
   - `::after { content: none }` → no pseudo box
   - `::before { content: normal }` → no pseudo box
   - `::before { content: counter(foo) }` → no pseudo box (unsupported), other props apply
2. **Content list**
   - `content: "a" "b"` → two text items concatenated visually
   - `content: "Hello " attr(name)` → text + resolved attr
   - `content: attr(missing, "fallback")` → fallback used
   - `content: url(data:image/png;base64,...) "label"` → image + text
3. **Display**
   - `display: inline` (default) → flows with parent's inline context
   - `display: block` → forces new block in parent
   - `display: inline-block` → atomic inline
4. **Replaced element exclusion**
   - `img::before { content: "x" }` → no pseudo created
5. **Edge**
   - `content: ""` → no pseudo box
   - Pseudo with `display: none` → no pseudo box

## 11. Non-goals reminder

This design intentionally stops short of:

- `::marker` (needs `display: list-item`, list-style machinery, optional counter integration)
- Counter machinery (`counter-reset`/`-increment`, `counter()` in content)
- Quote machinery (`quotes`, `open-quote` / `close-quote`)
- `attr()` typed form
- `::first-line` / `::first-letter` (require post-layout re-styling)
- Pseudo-of-pseudo (`::before::first-letter`, etc.)

Follow-up issues should be opened for each.

## 12. Migration impact

- Public API: no breaking changes. `Node` shape, `Style`, `RenderOptions` all unchanged.
- Existing CSS that uses `::before`/`::after` will start producing visible content where it previously produced nothing. This is a behavior change but matches user intent (and previously the entire rule was kept via #709's ignore path, just with no effect).
- Performance: one extra `MatchedDeclarationsView` per element per pseudo bucket (3× memory for matched declarations, but each bucket is small). Render tree gains 0–2 synthetic nodes per element. Negligible.
