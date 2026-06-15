# Decouple `takumi` into `takumi-core` + `takumi-paint`

Date: 2026-06-15
Status: design — awaiting review

## Goal

Split the painting backend out of the `takumi` crate so the layout/style/scene
layer is backend-agnostic, enabling a future `takumi-svg` crate that emits real
vector SVG (paths, gradients, filters, glyph outlines) instead of a rasterized
image embedded as a base64 data URL.

Scope of **this** spec: the `takumi-core` / `takumi-paint` decouple only. The
`takumi-svg` backend is a separate follow-up PR; its feasibility analysis is
recorded in the appendix.

## Decisions

- **Sequencing:** decouple PR first; `takumi-svg` is a later, separate PR.
- **Topology:** keep `takumi` as a thin re-export facade; add `takumi-core` and
  `takumi-paint` underneath. Non-breaking for downstream and for the
  napi/wasm/server bindings, which keep depending on `takumi`.
- **Seam depth:** thin seam now (core emits the existing display list), structured
  for later (scene-resolution logic written as pure functions ready to promote to
  core when `takumi-svg` lands).
- **No `PaintBackend` trait yet:** the trait would be modeled on `Canvas`'s
  imperative pixel/mask API, a poor fit for declarative SVG. Defer until the SVG
  backend exists to inform its shape.

## Crate topology

```text
takumi-css                       (exists; CSS parse/cascade — unchanged)
   ▲
takumi-core                      (NEW)
   ▲          ▲
takumi-paint  │                  (NEW)
   ▲          │
takumi (facade, all `pub use`)
   ▲
takumi-napi-core / takumi-wasm / takumi-server   (unchanged)
```

The `takumi` facade re-exports core + paint at the existing module paths so
`takumi::layout::*`, `takumi::resources::*`, `takumi::rendering::render`, and
`takumi::GlobalContext` resolve exactly as today.

## What moves where

### takumi-core (~9.5K LOC)

- `layout/` — node tree, `tree.rs`, inline layout, matching.
- `resources/` — font contexts, image store (`ImageSource`, including retained SVG
  markup).
- `error.rs`.
- `GlobalContext`.
- `RenderContext` — promoted from `pub(crate)` in `rendering/mod.rs`.
- Display-list types + `build_stacking_contexts()` — moved out of
  `rendering/stacking_context.rs`; the building half is already backend-agnostic.

### takumi-paint (~10.5K LOC)

- All of `rendering/` — `Canvas`, drawing modules, blur/filters, `paint_context()`,
  `write.rs` output formats, and the `render()` entry returning `RgbaImage`.

## Breaking the Node → Canvas coupling

Six `draw_*` methods on `Node` (`layout/node/mod.rs:621-880`) call `Canvas`
directly: `draw_outset_box_shadow`, `draw_inset_box_shadow`, `draw_background`,
`draw_content`, `draw_border`, `draw_outline`. These move to `takumi-paint` as
free functions over `&RenderNode` / `RenderContext`.

Two frictions this exposes:

1. **`pub(crate)` → real `pub` API.** Paint code currently reads node/style/context
   internals via `pub(crate)`; across a crate boundary that breaks. Expose what
   paint needs through deliberate `pub` accessors in named internal modules — not
   `#[doc(hidden)]` band-aids. This is the bulk of the churn.
2. After the move, `takumi-core::layout::node` has **no** dependency on paint;
   `takumi-paint` depends on core. No dependency cycle.

## Scene-resolution: structured for promotion

The backend-agnostic "what to draw" logic stays in `takumi-paint` for now but is
refactored into bounded modules with pure `input → drawable-description`
signatures (no `Canvas` argument):

- background layer collection (`collect_background_layers`)
- border path / Bézier geometry
- shadow parameter resolution
- glyph outline + position resolution
- clip-shape resolution

Each carries a module doc-comment noting intended promotion to core. When
`takumi-svg` lands, these modules move up and both backends share them with zero
rewrite.

## Validation

- **Byte-identical raster output:** the golden fixture suite
  (`tests/fixtures-generated/`) must be unchanged after the split — the regression
  gate. (Goldens are Linux-canonical; take updated webps from CI changed-files
  artifact if any platform drift appears, do not regenerate locally.)
- `cargo build` of bindings/server unchanged.
- `cargo clippy` clean.
- napi JS bench run to confirm no perf regression — module boundaries can affect
  inlining.

## Appendix: `takumi-svg` feasibility (follow-up PR)

The painting layer is SVG-friendly because the hard parts are already vector data
at paint time. Coverage matrix:

| Feature                          | SVG mapping                                                                 | Verdict                               |
| -------------------------------- | --------------------------------------------------------------------------- | ------------------------------------- |
| Solid backgrounds                | `<rect>`                                                                    | green                                 |
| Borders + radius (dashed/dotted) | `<path>` Béziers (geometry built in `border.rs`); per-side = up to 4 paths  | green                                 |
| Linear / radial gradients        | native `<linearGradient>` / `<radialGradient>`                              | green                                 |
| Conic gradients                  | no SVG 1.1 construct                                                        | **red — only hard blocker**           |
| Box-shadow (offset+blur)         | `<filter>` `feGaussianBlur`+`feOffset`+`feDropShadow`                       | yellow — spread/inset need extra work |
| Text                             | skrifa glyph outlines available (`text_drawing.rs:284`) → glyph `<path>`s   | green                                 |
| Images (bitmap/gif)              | `<image href="data:...">`                                                   | green                                 |
| SVG-source images                | original markup retained (`image.rs:43`) → inline nested `<svg>`, zero loss | green                                 |
| clip-path / overflow             | `<clipPath>` (all shapes native)                                            | green                                 |
| opacity / 18 blend modes         | `opacity` / `mix-blend-mode`                                                | green                                 |
| affine transforms                | `transform="matrix(...)"` 1:1                                               | green                                 |

Sketched `takumi-svg` PR plan:

- thin emitter consuming the display list + promoted scene-resolution modules
- conic gradient → raster fallback (`<image>`) or multi-stop approximation
- text as glyph `<path>`s (faithful, no font-availability dependency)
- SVG-source images inlined as nested `<svg>`
- box-shadow via `<filter>`; spread via path expansion / `feMorphology`
