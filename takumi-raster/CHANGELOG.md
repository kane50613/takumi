## takumi-raster@0.2.0

### Seal remaining `taffy`/`parley` leaks out of the public `style` API

`Position`, `Display`, `AlignItems`, `JustifyContent`, `BoxSizing`, `Direction`,
`FlexDirection`, `FlexWrap`, `Overflow`, `TextAlign`, `GridPlacement`,
`GridAutoFlow`, `GridRepetitionCount`, and `GridTemplateAreas` no longer
implement `From<_>` for their `taffy`/`parley` counterparts; the conversions
are now `pub(crate)` inherent methods (`into_taffy`/`into_parley`).
`TextWrapMode`, `WordBreak`, and `OverflowWrap` lose their `parley` `From`
impls the same way. `ResolvedVerticalAlign::apply` is now `pub(crate)`.

`takumi-raster`: dropped the unused `From<OutputFormat> for image::ImageFormat`
impl, which pinned the `image` crate into the public API without being called
anywhere.

## takumi-raster@0.1.0

### Clip backdrop-filter output by the node's mask and clip-path

The filtered backdrop painted across the whole border box even when the node
had a `mask` or `clip-path`, unlike browsers where the mask applies to the
filtered backdrop too. The backdrop composite is now attenuated by the node
mask, and a fully masked-out node skips the backdrop filter entirely.

### Stream animation frames straight into the encoder

Add `write_animation`, which renders a timeline and feeds each frame to the
encoder as it arrives, holding one raw frame at a time instead of the whole
sequence. Both the napi and WASM `renderAnimation` bindings use it, so a high
frame rate or a long duration no longer exhausts memory. On native the WebP
encoder still runs frames in parallel, now over bounded chunks. The WASM WebP
encoder now merges runs of identical frames like the native one, so a static or
slow animation encodes and stores far less. The eager `render_animation` +
`write_animated_*` path stays for callers that want every frame at once.

### Cap animation frame rate per format

Browsers clamp any animation frame of 10ms or less to 100ms, so a high frame
rate stalls instead of playing fast. `write_animation` now rejects a frame rate
above `AnimationFormat::max_fps` with the new `AnimationFrameRateTooHigh` error:
90 fps for WebP and APNG, 50 fps for GIF (centisecond delays). The napi and WASM
`renderAnimation` bindings surface the error.

### Rename the animated encoders to `write_animated_*`

`encode_animated_gif`, `encode_animated_png`, and `encode_animated_webp` are
now `write_animated_gif`, `write_animated_png`, and `write_animated_webp`,
matching `write_image`. Signatures are unchanged.

## takumi-raster@0.1.0-rc.5

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

## takumi-raster@0.1.0-rc.4

### Rename the animated encoders to `write_animated_*`

`encode_animated_gif`, `encode_animated_png`, and `encode_animated_webp` are
now `write_animated_gif`, `write_animated_png`, and `write_animated_webp`,
matching `write_image`. Signatures are unchanged.

## takumi-raster@0.1.0-rc.3

### Clip backdrop-filter output by the node's mask and clip-path

The filtered backdrop painted across the whole border box even when the node
had a `mask` or `clip-path`, unlike browsers where the mask applies to the
filtered backdrop too. The backdrop composite is now attenuated by the node
mask, and a fully masked-out node skips the backdrop filter entirely.

## takumi-raster@0.1.0-beta.5

### Fix `path()` ignoring device-pixel-ratio

`offset-path: path()` and `clip-path: path()` used the authored SVG coordinates verbatim while every other basic shape crossed the dpr boundary through `to_px`. At `devicePixelRatio != 1` the path was mis-scaled by `1/dpr`. The coordinates are now scaled into device space like the other shapes.

## takumi-raster@0.1.0-beta.3

### Apply `fontFamilies` as the default font

Text without an explicit `font-family` ignored the render's `fontFamilies` (and
any fonts passed through it), always using the embedded fallback. The
`fontFamilies` stack is now the root default, so passed fonts render without a
per-node `font-family`.

## takumi-raster@0.1.0-beta.2

### Render text language-aware with the `lang` attribute

The `lang` attribute sets a node's language and inherits to its descendants; a
nested `lang` overrides, and an empty or invalid value clears it. The language
reaches the shaper as the locale, so the same code point draws its
language-specific glyph (Han unification across `ja` / `zh` / `ko`) and lines
break per language. Pass `lang` in the render options to set a default for the
whole tree.

## takumi-raster@0.1.0-beta.1

### Match browser fake-bold for synthesized weights

Synthesize bold only at the CSS bold threshold (weight >= 600), so a medium weight keeps its regular face instead of being faux-bolded. Scale the synthetic stroke with text size — `1/24` easing to `1/32` per Skia — instead of a constant fraction, so large text is no longer over-emboldened.

## takumi-raster@0.1.0-beta.0

### Rename render entry points and return a `Bitmap`

`measure_layout` becomes `measure`, `render_sequence_animation` becomes `render_animation`, and `ImageOutputFormat` becomes `OutputFormat`. `render` returns a `Bitmap` newtype instead of `image::RgbaImage`, and `write_image` takes `&Bitmap`.

### Split `takumi` into `takumi-core`, `takumi-raster`, and `takumi-svg` behind a re-export facade

### Minimize the public API

`takumi::prelude` exposes the stable data structures, entry-point functions sit at the crate root, the full backend crates move behind an `unstable` feature, and backend internals drop to `pub(crate)`.

### Rename the `raster` feature to `raster-backend`

This mirrors `svg-backend`, and `rayon` no longer enables it implicitly.

### Model image output quality per format

`ImageOutputFormat::Jpeg`/`WebP` carry a `Quality`, a new `WebPLossless` variant replaces lossless WebP (a `lossless` flag in the napi/wasm bindings), and `write_image` drops its quality argument.
