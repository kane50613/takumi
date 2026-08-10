## takumi@2.7.0

### Restore JPEG, WebP and GIF decoding

`takumi` disables `takumi-core`'s default features and never re-enables image decoding, so every build decoded PNG and ICO only. A WebP source failed with `The image format could not be determined`. `image-decoding` is now a `takumi` feature as well, on by default, and it splits into `jpeg`, `webp` and `gif` for a build that wants one format and not the others. The napi and wasm bindings turn it on too.

## takumi@2.5.2

### Export `FontSource` from the prelude

`FontResource::new` takes anything that converts into a `FontSource`, and naming that type is the only way to reach `FontSource::from_static` or `from_shared`. It was missing from the prelude, so registering an `include_bytes!` face meant going through the semver-exempt `unstable` module.

## takumi@2.3.0

### Accept raw RGBA pixels as an image source

An image node's `src` takes `{ width, height, data, premultiplied? }` and renders the pixels without decoding; the `Bitmap` JSX helper wraps it as an `<img>`.

## takumi@2.0.0

### Refactor `font_families` and `lang` option type

Now both option takes resolved value instead of raw strings.

### Make the embedded font a true last resort

Both bindings now embed one font: a Latin Geist subset with a 400 to 700
weight axis (Geist Mono and Manrope are gone). It no longer claims the
`sans-serif` generic family and always sorts after registered fonts in
fallback selection, so generic families and unstyled text resolve to the fonts
you load. The new `FontResource::last_resort` marks a font's families to sort
after every normal registration.

### Cap animation frame rate per format

Browsers clamp any animation frame of 10ms or less to 100ms, so a high frame
rate stalls instead of playing fast. `write_animation` now rejects a frame rate
above `AnimationFormat::max_fps` with the new `AnimationFrameRateTooHigh` error:
90 fps for WebP and APNG, 50 fps for GIF (centisecond delays). The napi and WASM
`renderAnimation` bindings surface the error.

### Fix `fontFamilies` order being ignored

`fontFamilies` only fed the fallback bucket, never the root style, so text
picked whichever registered font resolved first instead of the requested
order. `FontFamily`'s default is now empty instead of a generic `sans-serif`
token, so an empty root style falls through to the fallback bucket directly.

### Fix buffer pool bucket capacity invariant

Release now buckets a buffer by the floor power of two its capacity guarantees,
and `acquire_dirty` reserves before `set_len`. A pooled buffer can no longer be
lengthened past its allocation.

### Skip mask copies and per-pixel mask lookups in canvas fast paths

Borrow the combined constraint mask directly when it already matches the
canvas viewport instead of copying it per draw, and hoist mask lookups out of
the per-pixel blit loops. Output is unchanged.

### Fix gradient stop snapping panic for oversized stop lists

Gradients with more color stops than the LUT can hold no longer panic. The
sample-index clamp and normalization passes stay within LUT bounds.

### Reuse per-scene state across animation frames

Compute each scene's font snapshot once and share its image and stylesheet
handles across frames instead of re-snapshotting and deep-cloning the whole
option tree per frame. Frame output is unchanged.

### Seal `parley::Layout` out of the inline-layout boundary

`BuiltInlineLayout::{layout, custom_inline_boxes}` are now private; the
measure-only walk moves into `BuiltInlineLayout::measure_runs`, returning
core-owned `MeasuredInlineRun`/`MeasuredInlineBox` (run text borrows the
layout). `get_parent_font_metrics`, `resolve_inline_line_metrics`,
`resolve_inline_line_states`, and `scale_text_fit_x` are no longer public.

### Add `takumi-html` for parsing HTML into a node tree

New `takumi-html` crate parses HTML + Tailwind markup into a node tree with
`from_html(source, FromHtmlOptions)`, mirroring the JS `fromHtml`. The `tw`,
`style`, `class`, `id`, `dir`, and `lang` attributes map to node styling and
metadata; `FromHtmlOptions` sets the `StylePresets` table and a `max_depth`
nesting cap. The `takumi` umbrella re-exports it under the `from-html` feature
as `takumi::from_html`, plus `Node::from_html` via the `FromHtml` prelude
trait.

### Stream animation frames straight into the encoder

Add `write_animation`, which renders a timeline and feeds each frame to the
encoder as it arrives, holding one raw frame at a time instead of the whole
sequence. Both the napi and WASM `renderAnimation` bindings use it, so a high
frame rate or a long duration no longer exhausts memory. On native the WebP
encoder still runs frames in parallel, now over bounded chunks. The WASM WebP
encoder now merges runs of identical frames like the native one, so a static or
slow animation encodes and stores far less. The eager `render_animation` +
`write_animated_*` path stays for callers that want every frame at once.

### Guard shadow and blur buffer sizing against overflow

Extreme blur radii could overflow the `u32` shadow-buffer area and panic or
over-allocate. Sizing now uses saturating math and skips shadows above a 256
Mi-pixel budget.

### Drop `background-blend-mode` from the `background` shorthand

The `background` shorthand parsed a blend-mode token and reset
`background-blend-mode`, unlike browsers, where the shorthand touches neither. It
now leaves `background-blend-mode` alone; set it through the longhand. The
`blend_mode` field is gone from the `Background` shorthand value.

### Rename the `svg` feature to `svg-source`

`svg` and `svg-backend` read as the same thing at a glance despite gating
opposite directions (image-source input vs. render output). The umbrella's
input-side feature is now `svg-source`; `svg-backend` is unchanged.

### Make `:lang()` actually match

`:lang()` parsed but never matched, like every other pseudo-class the engine treats as
stateful. It needs no live state, only the `lang` attribute inherited up the tree, which a
static render already has. It now matches BCP-47 basic filtering (`:lang(zh-Hant)`, comma-separated
ranges, `*`) against the nearest ancestor-or-self with a `lang` set — the standards-based way to
route different fonts to different languages on the same page, e.g. `:lang(ja) { font-family:
"Noto Sans JP" }` alongside `:lang(zh-Hant) { font-family: "Noto Sans TC" }`.

### Represent the `none`/`normal` initial values of `max-*` and gaps

`max-width` and `max-height` are now a `MaxSize` value whose initial is `None`
(unbounded), instead of borrowing `Length`'s `auto`. `column-gap`, `row-gap`, and
the `gap` shorthand are now a `Gap` value whose initial is `Normal`. Rendering is
unchanged — `none` resolves like the old unbounded default and `normal` computes
to `0` — but the values now round-trip through `to_css` as `none`/`normal`.

### Cap image decode dimensions and GIF frame volume

Decoders reject images beyond 8192x8192 (via `image::Limits` for PNG and JPEG,
dimension checks for WebP) and GIFs beyond a total-frame pixel budget, stopping
decode-bomb OOM.

## takumi@2.0.0-rc.16

### Make the embedded font a true last resort

Both bindings now embed one font: a Latin Geist subset with a 400 to 700
weight axis (Geist Mono and Manrope are gone). It no longer claims the
`sans-serif` generic family and always sorts after registered fonts in
fallback selection, so generic families and unstyled text resolve to the fonts
you load. The new `FontResource::last_resort` marks a font's families to sort
after every normal registration.

## takumi@2.0.0-rc.15

### Make `:lang()` actually match

`:lang()` parsed but never matched, like every other pseudo-class the engine treats as
stateful. It needs no live state, only the `lang` attribute inherited up the tree, which a
static render already has. It now matches BCP-47 basic filtering (`:lang(zh-Hant)`, comma-separated
ranges, `*`) against the nearest ancestor-or-self with a `lang` set — the standards-based way to
route different fonts to different languages on the same page, e.g. `:lang(ja) { font-family:
"Noto Sans JP" }` alongside `:lang(zh-Hant) { font-family: "Noto Sans TC" }`.

### Fix `fontFamilies` order being ignored

`fontFamilies` only fed the fallback bucket, never the root style, so text
picked whichever registered font resolved first instead of the requested
order. `FontFamily`'s default is now empty instead of a generic `sans-serif`
token, so an empty root style falls through to the fallback bucket directly.

## takumi@2.0.0-rc.13

### Skip mask copies and per-pixel mask lookups in canvas fast paths

Borrow the combined constraint mask directly when it already matches the
canvas viewport instead of copying it per draw, and hoist mask lookups out of
the per-pixel blit loops. Output is unchanged.

### Fix gradient stop snapping panic for oversized stop lists

Gradients with more color stops than the LUT can hold no longer panic. The
sample-index clamp and normalization passes stay within LUT bounds.

### Cap image decode dimensions and GIF frame volume

Decoders reject images beyond 8192x8192 (via `image::Limits` for PNG and JPEG,
dimension checks for WebP) and GIFs beyond a total-frame pixel budget, stopping
decode-bomb OOM.

### Reuse per-scene state across animation frames

Compute each scene's font snapshot once and share its image and stylesheet
handles across frames instead of re-snapshotting and deep-cloning the whole
option tree per frame. Frame output is unchanged.

### Guard shadow and blur buffer sizing against overflow

Extreme blur radii could overflow the `u32` shadow-buffer area and panic or
over-allocate. Sizing now uses saturating math and skips shadows above a 256
Mi-pixel budget.

### Fix buffer pool bucket capacity invariant

Release now buckets a buffer by the floor power of two its capacity guarantees,
and `acquire_dirty` reserves before `set_len`. A pooled buffer can no longer be
lengthened past its allocation.

## takumi@2.0.0-rc.11

### Refactor `font_families` and `lang` option type

Now both option takes resolved value instead of raw strings.

## takumi@2.0.0-rc.10

### Drop `background-blend-mode` from the `background` shorthand

The `background` shorthand parsed a blend-mode token and reset
`background-blend-mode`, unlike browsers, where the shorthand touches neither. It
now leaves `background-blend-mode` alone; set it through the longhand. The
`blend_mode` field is gone from the `Background` shorthand value.

### Represent the `none`/`normal` initial values of `max-*` and gaps

`max-width` and `max-height` are now a `MaxSize` value whose initial is `None`
(unbounded), instead of borrowing `Length`'s `auto`. `column-gap`, `row-gap`, and
the `gap` shorthand are now a `Gap` value whose initial is `Normal`. Rendering is
unchanged — `none` resolves like the old unbounded default and `normal` computes
to `0` — but the values now round-trip through `to_css` as `none`/`normal`.

### Seal `parley::Layout` out of the inline-layout boundary

`BuiltInlineLayout::{layout, custom_inline_boxes}` are now private; the
measure-only walk moves into `BuiltInlineLayout::measure_runs`, returning
core-owned `MeasuredInlineRun`/`MeasuredInlineBox` (run text borrows the
layout). `get_parent_font_metrics`, `resolve_inline_line_metrics`,
`resolve_inline_line_states`, and `scale_text_fit_x` are no longer public.

## takumi@2.0.0-rc.9

### Bump internal crates

Releasing v0 internal crates

## takumi@2.0.0-rc.8

### Cap animation frame rate per format

Browsers clamp any animation frame of 10ms or less to 100ms, so a high frame
rate stalls instead of playing fast. `write_animation` now rejects a frame rate
above `AnimationFormat::max_fps` with the new `AnimationFrameRateTooHigh` error:
90 fps for WebP and APNG, 50 fps for GIF (centisecond delays). The napi and WASM
`renderAnimation` bindings surface the error.

### Stream animation frames straight into the encoder

Add `write_animation`, which renders a timeline and feeds each frame to the
encoder as it arrives, holding one raw frame at a time instead of the whole
sequence. Both the napi and WASM `renderAnimation` bindings use it, so a high
frame rate or a long duration no longer exhausts memory. On native the WebP
encoder still runs frames in parallel, now over bounded chunks. The WASM WebP
encoder now merges runs of identical frames like the native one, so a static or
slow animation encodes and stores far less. The eager `render_animation` +
`write_animated_*` path stays for callers that want every frame at once.

## takumi@2.0.0-rc.6

### Rename the `svg` feature to `svg-source`

`svg` and `svg-backend` read as the same thing at a glance despite gating
opposite directions (image-source input vs. render output). The umbrella's
input-side feature is now `svg-source`; `svg-backend` is unchanged.

## takumi@2.0.0-rc.2

### Add `takumi-html` for parsing HTML into a node tree

New `takumi-html` crate parses HTML + Tailwind markup into a node tree with
`from_html(source, FromHtmlOptions)`, mirroring the JS `fromHtml`. The `tw`,
`style`, `class`, `id`, `dir`, and `lang` attributes map to node styling and
metadata; `FromHtmlOptions` sets the `StylePresets` table and a `max_depth`
nesting cap. The `takumi` umbrella re-exports it under the `from-html` feature
as `takumi::from_html`, plus `Node::from_html` via the `FromHtml` prelude
trait.

## takumi@2.0.0-beta.11

### Add CSS `offset-path` support

The path accepts `ray()`, the basic shapes `path()`/`circle()`/`ellipse()`/`polygon()`/`inset()`,
and a bare `<coord-box>`

`offset-distance`, `offset-rotate`, `offset-anchor`, `offset-position`, and the `offset` shorthand control placement.

`url()` references are not supported.

## takumi@2.0.0-beta.4

### Render text language-aware with the `lang` attribute

The `lang` attribute sets a node's language and inherits to its descendants; a
nested `lang` overrides, and an empty or invalid value clears it. The language
reaches the shaper as the locale, so the same code point draws its
language-specific glyph (Han unification across `ja` / `zh` / `ko`) and lines
break per language. Pass `lang` in the render options to set a default for the
whole tree.

## takumi@2.0.0-beta.2

### Expand subset font-family without borrowing the font context

A render holds the font context borrowed while it builds text layout, so per-element
`font-family` skipped subset-group expansion and routing relied on the fallback bucket, which
followed `fontique`'s hash iteration order. Content registered with `subset_of` (Rust) or
`subsetOf` (JS) could route to the wrong subset, and the result changed between renders.
Expansion now reads the subset groups without that borrow, and the fallback bucket follows
registration order.

## takumi@2.0.0-beta.1

### Match browser fake-bold for synthesized weights

Synthesize bold only at the CSS bold threshold (weight >= 600), so a medium weight keeps its regular face instead of being faux-bolded. Scale the synthetic stroke with text size — `1/24` easing to `1/32` per Skia — instead of a constant fraction, so large text is no longer over-emboldened.

### Keep font metadata when registering loaded fonts

`registerFont` passed only the resolved bytes to the engine, dropping each descriptor's
`name`/`subsetOf`/`weight`/`style`. Subsets that should register under unique names collapsed
onto their intrinsic family, so coverage variants were lost — text rendered as tofu, and which
variant survived depended on fetch-completion order, so the same content rendered differently
each run. Forward the descriptor so the override reaches the engine.

### Key the glyph cache by blob id instead of pointer

A freed font blob's address gets reused by a later font, aliasing its cached glyphs. Use the
blob's stable, never-reused id.

### Load only the Google Font subsets the content needs

`googleFontSubsets(content, families)` scans the codepoints a render uses, fetches every family's metadata in one css2 request, and keeps just the matching `unicode-range` subsets, so a multilingual image pulls a handful of CJK blocks instead of a whole font. Pass a `cache` Map to reuse the CSS across renders.

### Group coverage subsets under one logical family

`FontResource::subset_of` (Rust) and the `subsetOf` font field (JS) register a font as a subset of a logical family. A render expands `font-family: {logical}` into every subset registered under it, in order, so each script routes to the subset that covers it — distinct families no longer share a single fallback chain.

## takumi@2.0.0-beta.0

### Rename render entry points and return a `Bitmap`

`measure_layout` becomes `measure`, `render_sequence_animation` becomes `render_animation`, and `ImageOutputFormat` becomes `OutputFormat`. `render` returns a `Bitmap` newtype instead of `image::RgbaImage`, and `write_image` takes `&Bitmap`.

### Make fonts and images explicit per-render resources

Drop the persistent image store and `GlobalContext`, and pass fonts and images per render. `registerFont` replaces `loadFont`/`loadFontSync`/`loadFonts`, each render takes a `fontFamilies` fallback chain, and `images` replaces `fetchedResources`.

### Align CSS handling with the spec

Fix the `border-*-width`/`outline-width` defaults, negative `scale`, the `position: static` default, and `line-clamp` longhand splitting.

### Split `takumi` into `takumi-core`, `takumi-raster`, and `takumi-svg` behind a re-export facade

### Minimize the public API

`takumi::prelude` exposes the stable data structures, entry-point functions sit at the crate root, the full backend crates move behind an `unstable` feature, and backend internals drop to `pub(crate)`.

### Rename the `raster` feature to `raster-backend`

This mirrors `svg-backend`, and `rayon` no longer enables it implicitly.

### Stop resolving `currentColor` in SVG images against the host color

### Model image output quality per format

`ImageOutputFormat::Jpeg`/`WebP` carry a `Quality`, a new `WebPLossless` variant replaces lossless WebP (a `lossless` flag in the napi/wasm bindings), and `write_image` drops its quality argument.

# takumi

## 1.8.7

## 1.8.6

### Patch Changes

- cfac305: Align empty `inline-block`, `inline-flex`, and `inline-grid` boxes to the baseline by their bottom margin edge
- 653f23c: Stop painting `outline` around the text of non-inline elements
- c1dd195: Scale `vertical-align: sub`/`super` by font size instead of line height
- 6f55980: Fix line-box height for `vertical-align: top`/`bottom` boxes taller than the line height

## 1.8.5

### Patch Changes

- b998cfd: Fix Tailwind per-side border colors and border-width rendering
- 1389d75: Support `safe`/`unsafe` overflow keywords on `align-items` and `justify-content` (taffy 0.11)
- Updated dependencies [b998cfd]
- Updated dependencies [1389d75]
  - takumi-css@0.1.2

## 1.8.4

### Patch Changes

- 72ee5dd: Fix vendor-prefixed property names not being resolved

## 1.8.3

## 1.8.2

### Patch Changes

- 041e5fd: Encode PNG/APNG with the `zlib-rs` deflate backend at level 7 to speed up and smaller file size

## 1.8.1

### Patch Changes

- d1ff41f: Skip painting zero-sized nodes instead of compositing them through a full-viewport offscreen canvas, fixing a severe slowdown for zero-sized nodes with `opacity`.

## 1.8.0

### Patch Changes

- bcac11f: Fix discrete animations to switch at 50% progress and correct `alternate` iteration parity
- bcac11f: Fix `filter: none`, `@media not (...)` / `only`, `blur()`, `repeat()`, and `@supports` parsing
- bcac11f: Ignore non-string custom property values instead of storing an empty string
- bcac11f: Resolve `direction` (keyword or `var()`) before logical properties in the same block
- bcac11f: Allow constructing `Flex` directly
- bcac11f: Guard `i16` grid-line negation and non-positive device-pixel-ratio
- bcac11f: Reduce shipped WebAssembly binary size (`opt-level` and `wasm-opt -Oz`)
- bcac11f: Keep equal-priority Tailwind utilities in source order

## 1.7.0

### Minor Changes

- ece3e03: Add `position: static` and `position: fixed`
- 4748c22: Support Tailwind `mask-` utility

### Patch Changes

- ef7e816: Fix absolutely positioned children being mispositioned inside a `display: block` `position: relative` container that has in-flow siblings
- 9952b65: Accept inline image bytes (`Uint8Array`/`ArrayBuffer`) for image node `src`.
- b41405d: Fix `background-image`/`mask-image` SVGs with only a `viewBox` to scale to the box
- b9b4359: Fix `tw` arbitrary URLs (`mask-[url(...)]`, `bg-[url(...)]`) broken by `:` and `_` parsing

## 1.6.0

### Minor Changes

- 9d9b65e: Render `::before` / `::after` pseudo-elements #703

## 1.5.1

### Patch Changes

- 2321dbf: Fix `rem` units double-applying device-pixel ratio on descendant elements

## 1.5.0

### Minor Changes

- 9dc58e3: Add `margin-inline-start`, `margin-inline-end`, `padding-inline-start`, `padding-inline-end` CSS longhands, resolved to physical sides via `direction`
- 9dc58e3: Audit Tailwind utilities against v4 spec and fix the drift

## 1.4.1

### Patch Changes

- 72c19fd: Fix `text-fit: grow` with `background-clip: text` and `-webkit-text-stroke`

## 1.4.0

### Minor Changes

- a607651: Support `:is()` / `:where()` and stop dropping rules that contain unsupported pseudo-classes/elements
- 15fdabb: Support `lh` and `rlh` CSS length units

### Patch Changes

- dc04e09: Drop whitespace-only text nodes in block containers #711
- 26f1276: Fixes `var()` function detection #712
- dc31096: **Fix `object-position` is not inherited**

## 1.3.0

### Minor Changes

- 832ecd2: Add `ToCss` trait
- c4f705e: Support relative color syntax #693

### Patch Changes

- e073060: Resolve `rem` against the root element's computed font-size
- 7dbff69: Drop whitespace-only text nodes in block-like HTML containers #695

## 1.2.1

### Patch Changes

- 1fb35bb: Fix text-fit with text-align
- fa9abe6: Fix opacity not applied in inline layout
- fa7c55c: Fix line height resolves percentage to unitless

## 1.2.0

### Minor Changes

- 0f84a52: Support `text-fit` property

## 1.1.2

### Patch Changes

- 903f0ea: Drop libm dependency

## 1.1.1

### Patch Changes

- adc48da: Treat absolute/floated children as out-of-flow for inline layout detection

## 1.1.0

### Minor Changes

- 785d760: Support more border styles

## 1.0.16

### Patch Changes

- 092b4fd: Support inline float flow

## 1.0.15

### Patch Changes

- 3be6898: Reduce path rendering allocation

## 1.0.14

## 1.0.13

## 1.0.12

### Patch Changes

- 151a56e: Fix inline replaced sizing with border-box and line-height baseline alignment

## 1.0.11

### Patch Changes

- b755451: Fix Tailwind font size 5xl above line height

## 1.0.10

### Patch Changes

- b5e38f8: Fix Tailwind leading parsing #665
- 7d91b9c: Fix image drawing offset #664

## 1.0.9

### Patch Changes

- 32fa381: Preserve first line when max height is smaller than line height

## 1.0.8

## 1.0.7

### Patch Changes

- 6e9b163: Fix stack overflow when inline-block presented

## 1.0.6

### Patch Changes

- 2c90eaa: Improve vertical alignment for inline boxes

## 1.0.5

## 1.0.4

### Patch Changes

- 818e2f9: Fix image downscale quality regression

## 1.0.3

### Patch Changes

- be5b03f: Skip drawing if paint bounds out of canvas viewport

## 1.0.2

### Patch Changes

- 17304ac: Fix paint bound calculation on inline content #644
- 49ce893: Fix `plus-darker` blending #643

## 1.0.1

### Patch Changes

- 25dd037: Fix opacity compositing on sub canvas

## 1.0.0

### Major Changes

- 3b4f03d: **Removed `FetchTaskCollection`, switch to `Node::resource_urls` & `Style::resource_urls` instead.**
- 7da94c5: Remove `SpacePair::from_reversed_pair`
- 188079f: **Replaced `RenderOptionsBuilder` with `RenderOptions::builder()`**
  Switch to [typed-builder](https://docs.rs/typed-builder) for compile time options validation, no unwrap needed.
  Before:

  ```rust
  let options = RenderOptionsBuilder::default().build().unwrap();
  ```

  After:

  ```rust
  let options = RenderOptions::builder().build();
  ```

- 7f0b66b: **Removed `parse_svg_str`, use `SvgSource::from_str` instead.**
- cac231c: **Updated parameter type in `Viewport` constructor, removed `impl From<(u32, u32)>`**
- b0e13d8: **Private `ImageSource::size()`**
- 188079f: **Changed initial `display` value from `flex` to `inline`**

  This is to comply with [the CSSWG spec](https://drafts.csswg.org/css-display/#the-display-properties).

  You should update your code to use `display: flex` if you want to use flexbox.

- 4a114d5: Removed `detailed_css_error` feature
- 1373f0a: **Replace `TakumiError` with `takumi::error::Error`**
- 80535ba: Declare `border_style`, `border_color` as shorthand

### Minor Changes

- 7da94c5: Support `order`, `z-index` longhand, `flex-flow`, `place-items`, `place-content`, `place-self` shorthand
- 1ccf8a9: Support `direction`, `float`, `clear` properties
- 256ef21: Remove public `load_font` function
- 2b68b8a: Selects GIF frame based on `time_ms`
- 00013a8: Support repeating gradients
- b09ce0b: Support buffer input for image node `src` field
- 1373f0a: Support `ico` format
- b2e304a: Rework on internal rendering pipeline to be performant
- 14ac37b: Support `text-indent` property

### Patch Changes

- 7a79268: Fix linear gradient direction keywords handles incorrectly
- a118b5d: Add blending fast path, blur downscale scaling
- cd47ace: Add bilinear interpolation fast path
- 7a79268: Set default color interpolation method to Oklab
- 27e38bd: Fix `calc()` infinity scaler calculation
- ef692db: Remove `fast_image_resize` dependency
- b0e13d8: Fix DPR not applied when resolving image intrinsic size
- e1de442: Drop `fast_image_resize` with direct sampling approach
- 02c4000: Fix `background-image` layers drawing order
- dc6126d: Support `<calc-keyword>`
- 1aa4442: Optimize gradient performance
- 3d2eab2: Blockify node when `position: absolute` #572
- 80535ba: Support `border-top/right/bottom/left` shorthand properties

## 1.0.0-rc.17

## 1.0.0-rc.16

## 1.0.0-rc.15

### Minor Changes

- 2b68b8a: Selects GIF frame based on `time_ms`
- b09ce0b: Support buffer input for image node `src` field

## 1.0.0-rc.14

### Patch Changes

- cd47ace: Add bilinear interpolation fast path

## 1.0.0-rc.13

### Patch Changes

- a118b5d: Add blending fast path, blur downscale scaling

## 1.0.0-rc.12

### Patch Changes

- 1aa4442: Optimize gradient performance

## 1.0.0-rc.11

### Minor Changes

- 1ccf8a9: Support `direction`, `float`, `clear` properties
- b2e304a: Rework on internal rendering pipeline to be performant

## 1.0.0-rc.10

## 1.0.0-rc.9

### Major Changes

- 4a114d5: Removed `detailed_css_error` feature

### Patch Changes

- 7a79268: Fix linear gradient direction keywords handles incorrectly
- 7a79268: Set default color interpolation method to Oklab

## 1.0.0-rc.8

## 1.0.0-rc.7

## 1.0.0-rc.6

### Major Changes

- 7f0b66b: **Removed `parse_svg_str`, use `SvgSource::from_str` instead.**

## 1.0.0-rc.5

### Patch Changes

- ef692db: Remove `fast_image_resize` dependency

## 1.0.0-rc.4

## 1.0.0-rc.3

## 1.0.0-rc.2

## 1.0.0-rc.1

## 1.0.0-rc.0

### Minor Changes

- 14ac37b: Support `text-indent` property

## 1.0.0-beta.20

### Major Changes

- 3b4f03d: **Removed `FetchTaskCollection`, switch to `Node::resource_urls` & `Style::resource_urls` instead.**

## 1.0.0-beta.19

## 1.0.0-beta.18

## 1.0.0-beta.17

## 1.0.0-beta.16

## 1.0.0-beta.15

## 1.0.0-beta.14

## 1.0.0-beta.13

## 1.0.0-beta.12

## 1.0.0-beta.11

## 1.0.0-beta.10

## 1.0.0-beta.9

### Major Changes

- 7da94c5: Remove `SpacePair::from_reversed_pair`

### Minor Changes

- 7da94c5: Support `order`, `z-index` longhand, `flex-flow`, `place-items`, `place-content`, `place-self` shorthand

## 1.0.0-beta.8

### Patch Changes

- e1de442: Drop `fast_image_resize` with direct sampling approach

## 1.0.0-beta.7

### Major Changes

- cac231c: **Updated parameter type in `Viewport` constructor, removed `impl From<(u32, u32)>`**

## 1.0.0-beta.6

### Major Changes

- b0e13d8: **Private `ImageSource::size()`**

### Patch Changes

- b0e13d8: Fix DPR not applied when resolving image intrinsic size
- 02c4000: Fix `background-image` layers drawing order

## 1.0.0-beta.5

### Patch Changes

- 27e38bd: Fix `calc()` infnity scaler calculation

## 1.0.0-beta.4

### Patch Changes

- dc6126d: Support `<calc-keyword>`

## 1.0.0-beta.3

### Minor Changes

- 00013a8: Support repeating gradients

### Patch Changes

- 3d2eab2: Blockify node when `position: absolute` #572

## 1.0.0-beta.2

## 1.0.0-beta.1

### Minor Changes

- 256ef21: Remove public `load_font` function

## 1.0.0-beta.0

### Major Changes

- 188079f: **Replaced `RenderOptionsBuilder` with `RenderOptions::builder()`**

  Switch to [typed-builder](https://docs.rs/typed-builder) for compile time options validation, no unwrap needed.

  Before:

  ```rust
  let options = RenderOptionsBuilder::default().build().unwrap();
  ```

  After:

  ```rust
  let options = RenderOptions::builder().build();
  ```

- 188079f: **`GlobalContext` fields are now private**
- 188079f: **Changed initial `display` value from `flex` to `inline`**

  This is to comply with [the CSSWG spec](https://drafts.csswg.org/css-display/#the-display-properties).

  You should update your code to use `display: flex` if you want to use flexbox.

## 0.73.1

### Patch Changes

- 27dc8aa: Add `ImageSource::from_bytes` method, switch to `libwebp`
- 43c4ff8: Improved CSS error details

## 0.73.0

### Minor Changes

- be1a220: **Migrate to pure Node struct without generic support**

  Before:

  ```rust
  let mut node = NodeKind::Container(ContainerNode {
    children: Some(Box::from([
      NodeKind::Text(TextNode {
        text: "Hello, world!".to_string(),
        style: None,
        tw: None,
        preset: None,
        tag_name: None,
        class_name: None,
        id: None,
      }),
    ])),
    preset: None,
    style: None,
    tw: None,
    tag_name: None,
    class_name: None,
    id: Some("root".to_string()),
  });
  ```

  After:

  ```rust
  let node = Node::container([Node::text("Hello, world!")]).with_id("root");
  ```

### Patch Changes

- e6f3cf1: Fix negative offsets for oversized `background-position` #558

## 0.72.0

### Minor Changes

- c3e584d: **Remove `css_stylesheet_parsing` feature**
- 5cfa29f: bump MSRV to 1.91
- a7956fd: Remove `Y_FIRST` variable on `SpacePair` for simplicity

### Patch Changes

- 57163ec: Support `@layer`, `@property`, `@supports`
- aa85f72: Fix `gap` resolves incorrectly
- d6fde4c: Fix `font-family`, `font-variation-settings`, `font-feature-settings` parsing in stylesheets

## 0.71.7

### Patch Changes

- 384bc9d: Support Tailwind box/text shadow color
- 85970f3: Support CSS variables and `var()` function
- 9a5a542: Support `@media` CSS at-rule

## 0.71.6

### Patch Changes

- 1a111a6: Add `keyframes` render option

## 0.71.5

### Patch Changes

- 02d7e3a: Support `.ttc` fonts loading

## 0.71.4

### Patch Changes

- 1d284ac: Improve gradient hot paths

## 0.71.3

### Patch Changes

- a279b4c: Add `dithering` option for smoother gradients

## 0.71.2

### Patch Changes

- 5c9a2f6: Rasterize repeated background tiles before compositing

## 0.71.1

## 0.71.0

### Minor Changes

- 812029d: Support lossy webp animation rendering
- 0930cdb: Support CSS keyframe animation rendering and animation properties
- 812029d: Add `encode_animated_gif` method
- 0930cdb: **BREAKING CHANGE: Refactor public style API to declaration-based styles**

  Replace the old field-based / `CssValue`-driven style construction with `StyleDeclaration` and `style.with(...)`.

  Before:

  ```rust
  let style = StyleBuilder::default()
    .font_size(Some(48.0.into()))
    .margin(Sides([Px(4.0); 4]))
    .build()
    .unwrap();
  ```

  After:

  ```rust
  let style = Style::default()
    .with(StyleDeclaration::font_size(Px(48.0).into()))
    .with_margin(Sides([Px(4.0); 4]));
  ```

### Patch Changes

- 5ad65bf: Fix panic when nested inline nodes presented
- 1c9b8ac: Support background-size: auto
- 52f1dd5: Fix `background-position`, `mask-position` now default to `top left`

## 0.70.4

### Patch Changes

- 4895e79: Fix nested node's resource urls not being collected
- e200a64: Fix comma-separated list parsing for `background`, `box-shadow`, `text-shadow`
- c5465f7: Fix flex text node self-alignment

## 0.70.3

## 0.70.2

### Patch Changes

- edd121c: Use `libwebp-sys` crate for non-wasm targets webp encoding

## 0.70.1

### Patch Changes

- 36fba31: Strip `text` & `tspan` nodes from SVG

## 0.70.0

### Patch Changes

- 217b9b0: Refactor rendering to avoid stack overflow in deep nested nodes
- bf3b981: fix regression image node size measurement in absolute positioned container

## 0.69.5

### Patch Changes

- 9d9f43b: Support viewport & container length units

## 0.69.4

### Patch Changes

- 1058a19: Pass `currentColor` to svg
- 1dc8413: Support gradient color-space interpolation

## 0.69.3

### Patch Changes

- 8b9642e: Support `<length>` value in `vertical-align`
- 2dda025: Support `color-mix()` function

## 0.69.2

### Patch Changes

- 83a8198: Support `linear/radial/conic-gradient()` double-position color stops syntax

## 0.69.1

### Patch Changes

- a81238d: Fix intrinsic SVG sizing in absolute flex shrink-to-fit measurement

## 0.69.0

### Minor Changes

- 62525f9: Parses CSS stylesheets with new `css_stylesheet_parsing` feature

### Patch Changes

- 55ea202: fixes `plus-darker` blending regression #501

## 0.68.17

### Patch Changes

- f186186: improve gradient quality with dithering

## 0.68.16

### Patch Changes

- 757339e: remove png `try_collect_palette`, add compression control via `quality` field
- bd4d602: support gif decoding

## 0.68.15

### Patch Changes

- 2c6f5c3: optimize interpolation, blur performance

## 0.68.14

## 0.68.13

### Patch Changes

- 27429b0: improve blur performance

## 0.68.12

### Patch Changes

- d8d0fa8: parse Tailwind background image gradient

## 0.68.11

### Patch Changes

- f0120ae: refactor text ellipsis
- 60942e9: support Tailwind 4.2 new colors (Taupe, Mauve, Mist and Olive)
- bf54cac: implement `BufferPool` to reuse buffers

## 0.68.10

### Patch Changes

- f1e2c62: support `vertical-align` property
- 1e30ea7: support `text-decoration-thickness` property

## 0.68.9

### Patch Changes

- 2016a3c: fix nested inline-block caused infinite recursive calls

## 0.68.8

### Patch Changes

- b7d7570: fix box-shadow rendering
- 4b13519: support `overflow: clip`

## 0.68.7

### Patch Changes

- 9248c6e: support `inline-block`, `inline-flex`, `inline-grid` layout, closes #219
- 2639396: fix decoration rounding caused gap
- 91037be: fix style shorthand overrides

## 0.68.6

### Patch Changes

- 51a5bd5: support `text-decoration-skip-ink` property
- 642cf06: refactor `TextDecorationLines` to use `bitflags`, parses Tailwind `underline`, `overline`...
- 57d6594: fix text painting order for shadow, decoration and actual content to avoid overlapping
- 9215906: fix embolden and skew should avoid emojis
- 53069e4: refactor taffy tree structure, support `calc()`

## 0.68.5

### Patch Changes

- 8fcbb90: support `font-synthesis-weight` & `font-synthesis-style` properties for faux-bold & skew
- 07649be: fix text overline decoration should be drawn before text

## 0.68.4

### Patch Changes

- d6f1cce: use `libm` for text rendering
- 37864fd: support `outline` properties

## 0.68.3

### Patch Changes

- 585e1ba: support `background: conic-gradient()`
- 755b998: support `font-stretch` property
- 12ef8ce: loosen SVG check
- db327e6: fix overflow hidden constraint should include border radius mask
- 133c6a4: fix line height "normal" behavior
- 3191865: fix inheritance should store computed instead of initial value

## 0.68.2

## 0.68.1

### Patch Changes

- fb32121: parse `bg-none` for Tailwind `background-image` and improve isolation coverage

## 0.68.0

### Minor Changes

- 7684faa: refactor font loading to reduce buffer copying

## 0.67.3

### Patch Changes

- 0632470: unpremultiply SVG pixels after rasterization #446
- 1e9b23e: fix plus-lighter compositing #447

## 0.67.2

## 0.67.1

### Patch Changes

- 4d8955d: fix text wrap ignores original height & line clamp constraint #439

## 0.67.0

### Minor Changes

- 691df9d: support `mix-blend-mode` and `isolation` property

### Patch Changes

- 7e34727: support `visibility` property
- ba7aa93: fix prevent text balancer from forcing breaks #437

## 0.66.14

### Patch Changes

- 102c24a: include DPR when calculating `text-wrap: balance` #434

## 0.66.13

## 0.66.12

### Patch Changes

- 7389d6e: add font loading cache

## 0.66.11

### Patch Changes

- 8f28434: refactor inline box drawing & measure API

## 0.66.10

### Patch Changes

- e457042: fix line height should be resolved absolute value

## 0.66.9

### Patch Changes

- c917b6d: add `parse_tw_with_arbitrary` method
- 9929474: fix missing Tailwind `leading-*` keywords

## 0.66.8

### Patch Changes

- 28a8348: merge `draw_background_color` & `draw_background_image` implementation

## 0.66.7

## 0.66.6

### Patch Changes

- 2e7dbed: support `strokeLinejoin` property for text stroke

## 0.66.5

### Patch Changes

- c8244eb: fix text clip drawing order

## 0.66.4

### Patch Changes

- 5f2b5ac: add `WebkitTextFillColor` property
- 7eed4a1: support background clipping on text stroke

## 0.66.3

### Patch Changes

- 77c7107: fix `WebkitTextStroke` deserialize naming

## 0.66.2

### Patch Changes

- 058f87a: use `zune-core` & `zune-jpeg` dev branch version (not published to crates.io)

## 0.66.1

### Patch Changes

- 7e513fc: add `text-shadow-*`, `drop-shadow-*` tailwind properties parser

## 0.66.0

## 0.65.0

### Minor Changes

- 1319540: new `measure()` API

## 0.64.1

## 0.64.0

### Patch Changes

- 6571216: fix viewport check should include defined values

## 0.63.2

### Patch Changes

- 63088f4: make `background_color` field optional, draw background color on text spans #220

## 0.63.1

## 0.63.0

## 0.62.8

### Patch Changes

- b0a21a4: refactor opacity blending should be on render level

## 0.62.7

## 0.62.6

### Patch Changes

- a10f933: fix tailwind filter classes (blur, brightness, etc.) now append instead of replace

## 0.62.5

### Patch Changes

- dd1c0e1: fix tailwind `text-pretty` & `text-balance` not being parsed

## 0.62.4

## 0.62.3

## 0.62.2

### Patch Changes

- 57cca21: Improve backdrop filter performance
- 520f15d: Improve drop shadow performance and reduce allocation

## 0.62.1

### Patch Changes

- 5214274: refactor `overlay_image` to take any `GenericImageView` (avoid unnecessary `RgbaImage` recreation)

## 0.62.0

### Minor Changes

- 4675458: use `Box` slices instead of `Vec` to optimize memory

### Patch Changes

- 7849598: SIMD enhanced `interpolate_rgba`
- a774aa6: optimize filters to render using LUTs

## 0.61.1

### Patch Changes

- 19235dd: support AVX2 & AVX-512 SIMD blurring
- 8066f93: bump MSRV to 1.89

## 0.61.0

### Minor Changes

- c4bf981: enrich CSS error

  The error message is much more helpful now.

  > InvalidArg, invalid type: integer `123`, expected a value of 'currentColor' or \<color>; also accepts 'initial' or 'inherit'.

- 98e9254: support `backdrop-filter`

## 0.60.8

### Patch Changes

- 4c6bf92: fix text drawing bypasses overflow constrain check

## 0.60.7

### Patch Changes

- f07b7f5: switch to gaussian box blur, integer based alpha blending

## 0.60.6

### Patch Changes

- 7813b86: use bit masking for faster alpha quantiazation

## 0.60.5

### Patch Changes

- 12415ba: fix alpha blending precision

## 0.60.4

### Patch Changes

- 6f74c75: fix `try_collect_palette` collecting over 256 colors

## 0.60.3

### Patch Changes

- 5e1cb26: try collect png palette if possible

## 0.60.2

### Patch Changes

- 946fc9e: update ellipsis condition explicity check `text-overflow: ellipsis`

## 0.60.1

### Patch Changes

- 71ab744: Unify text node & inline logic

  Brings more unified and consistent ellipsis, transform, collapse, measurement behavior.

## 0.60.0

### Minor Changes

- ef3ec72: support `text-wrap: balance` & `pretty` (`text-wrap-style`)!

## 0.59.1

### Patch Changes

- c6b4eab: use stack blur algorithm
- 8f02159: add `sepia()` filter, tailwind `filter` parsers

## 0.59.0

### Minor Changes

- 13eca0e: rename `LengthUnit` to `Length` #347
- 4dee0c0: support `blur()`, `drop-shadow()` filter, premultiply alpha blending for shadows

## 0.58.0

### Minor Changes

- 0deafbd: decouple base Chromium styles (or customized from `defaultStylePresets`) from `style` field to independent `preset` field.

## 0.57.6

### Patch Changes

- 68e8fc2: fix inline style order should be greater than tailwind styles

## 0.57.5

### Patch Changes

- 9bf3333: disable font hinting, apply normalized coordinates to glyph scaler

## 0.57.4

### Patch Changes

- a8ebbba: remove redundant style property wrapper
- a8ebbba: fix `matrix()` function parsing
- a8ebbba: support `col`, `row` tailwind grid properties

## 0.57.3

### Patch Changes

- fa2f034: fix COLR layers blending

## 0.57.2

### Patch Changes

- 695f34a: fix passing opacity to COLR palette

## 0.57.1

### Patch Changes

- 61191b2: handles `background-size` for rasterized images
- 260dbd0: optimize `encode_animated_webp` to reduce allocation

## 0.57.0

### Minor Changes

- 42572bb: **Drop `avif` format support**

### Patch Changes

- 26173c5: add `create_background_image` fast path for exact one image

## 0.56.1

### Patch Changes

- f4d54fa: fix `opacity` should be applied to image as well
- 1972df9: fix `background-size` css parsing
- 1972df9: support `background`, `mask` shorthand

## 0.56.0

### Minor Changes

- 1ac44c4: `mask-image` behaves correctly like a "mask" now instead of overlay image.
- 1ac44c4: support `background-clip`

### Patch Changes

- c1260a2: `line-clamp` should has ellipsis if overflow

## 0.55.4

### Patch Changes

- 34bf0af: fix mask image on text drawing overflows

## 0.55.3

### Patch Changes

- cd93ee9: handles special case of `text-overflow: ellipsis` + `text-wrap: nowrap`

## 0.55.2

### Patch Changes

- 274c716: reuse masking buffer to avoid allocation

## 0.55.1

### Patch Changes

- 3df6648: use `RefCell` internally for scratch buffer

## 0.55.0

### Minor Changes

- 5e79e33: support COLR emoji font drawing (e.g. twemoji)

### Patch Changes

- 5e79e33: reuse buffer for masking to reduce allocation

## 0.54.3

## 0.54.2

### Patch Changes

- df1aa7e: update `parley` to `0.7`

## 0.54.1

### Patch Changes

- b16fd1b: fix whitespace keywords parsing

## 0.54.0

### Minor Changes

- e8ea400: refactor `TakumiError` struct and eliminate `unwrap()` calls

### Patch Changes

- e6a0934: Crate: fix justify-content css parse

## 0.53.1

### Patch Changes

- 29a575c: optimize `CssValue` deserialize implementation to reduce generated `Visitor` variant

## 0.53.0

### Minor Changes

- 7740504: drop `ts_rs` support
- 4623702: **`textStroke` related properties will have prefix `WebkitTextStroke`**

## 0.52.2

### Patch Changes

- 563bf31: optimize transform to reduce multiplications

## 0.52.1

### Patch Changes

- 3fa5c55: optimize tailwind parser function size

## 0.52.0

### Minor Changes

- ed409d4: refactor `overflow` & `clip-path` rendering to avoid extra allocations
- b9b0a85: speed up out of viewport image rendering

### Patch Changes

- ed409d4: make transform behave correctly

## 0.51.1

### Patch Changes

- eb26a60: fix `overflow`, `clip-path`, `background-position` deserialization

## 0.51.0

### Minor Changes

- 27ac6c5: support `devicePixelRatio` value

## 0.50.0

## 0.49.1

## 0.49.0

## 0.48.0

### Minor Changes

- c3f1b7d: support optional width/height

## 0.47.0

### Minor Changes

- 7d3dbf1: replace `csscolorparser` with `color` crate to support more color functions

## 0.46.6

## 0.46.5

## 0.46.4

### Patch Changes

- 37610e0: bump `csscolorparser` to 0.8

## 0.46.3

## 0.46.2

## 0.46.1

### Patch Changes

- 9365705: fix `justify-between`, `around`, `evenly` tailwind parsing

## 0.46.0

### Minor Changes

- 18bbc7c: support tailwind breakpoint & important parsing #273

## 0.45.3

### Patch Changes

- 3cf3867: fix `bg-size-[…]`, `bg-position-[…]` arbitrary value parsing

## 0.45.2

### Patch Changes

- d28e982: add `background-image` arbitrary value parsing
- 3c0243b: fix gradient step parsing
- 1ba2585: bump minimum rust version to 1.88
- 3c0243b: prevent panicing in font weight parsing

## 0.45.1

### Patch Changes

- 97ba495: fix `rounded` parsing

## 0.45.0

### Minor Changes

- 702c419: add tailwind parser
- 702c419: support `inline`/`block` for padding/margin/inset/border-width

## 0.44.0

### Minor Changes

- 368fc1c: Support `textWrap`, `textWrapMode`, `whiteSpace`, `whiteSpaceCollapse` properties

  **BREAKING CHANGE: by default text will collapse instead of preserve**, use `whiteSpace: pre;` to get the same behavior

## 0.43.1

## 0.43.0

## 0.42.0

### Minor Changes

- 44368b8: remove all Mutex/RwLock uses
- 44368b8: replace noise-v1 to use lighter hash function, only `opacity()` & `seed()` is supported

## 0.41.0

### Patch Changes

- 8318812: fix `PositionComponent` should be untagged

## 0.40.2

### Patch Changes

- 21a9988: add `word-break: break-word` as alias for `word-break: normal` + `overflow-wrap: anywhere`
- ddae1b5: fix `letter-spacing`, `word-spacing` should not divide by font size

## 0.40.1

### Patch Changes

- 8751a1b: fix fetch tasks collecting being overwritten

## 0.40.0

### Minor Changes

- ae7062f: support `clip-path`, `clip-rule`

### Patch Changes

- ae7062f: fix inline content not being clipped by overflow constraints

## 0.39.0

### Minor Changes

- 71ae4a5: use `data-url` crate, **remove `image_data_uri` feature**

### Patch Changes

- 71ae4a5: parallelize background image layers rendering

## 0.38.1

### Patch Changes

- 88a56ed: use faster noise crate `fastnoiselite`
- 88a56ed: use `crossbeam-channel`

## 0.38.0

### Minor Changes

- 7245e49: Add `FetchTask` for resources need to be fetch externally.

## 0.37.0

### Minor Changes

- 92f4dd8: support `opacity` property
- e6a1c39: refactor internal image/text measuring to match browser overflow behavior
- 0dfb76b: support overflow `hidden`, `visible`

## 0.36.2

### Patch Changes

- 568f76f: fix box shadow not being parsed

## 0.36.1

## 0.36.0

### Minor Changes

- 95715d0: support `filter` on images (except `blur()` and `drop-shadow()`)

## 0.35.2

### Patch Changes

- cac5444: remove glyph cache

## 0.35.1

## 0.35.0

### Minor Changes

- 264fa71: implement inline layout
- 264fa71: make all nodes' `style` field optional

### Patch Changes

- 12a2d3f: fix `aspect-ratio`, `flex-grow` numberic value parsing

## 0.34.0

### Minor Changes

- c06cdce: support `currentColor` keyword

### Patch Changes

- 7c402d8: setup npm trusted publisher

## 0.33.1

## 0.33.0

### Minor Changes

- 98755a7: **drop support for `debug` field, replace with `draw_debug_border` option in rendering functions**
- 5f15925: support `flex` shorthand property
- aa965bd: support `translate`, `rotate`, `scale` property
- 656be8d: support custom ellipsis character for `line-clamp`, `text-overflow`

### Patch Changes

- a9f3999: fix border width on image node that caused offset to be applied twice
