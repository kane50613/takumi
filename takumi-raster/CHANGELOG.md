## takumi-raster@0.4.18

### Stop rebuilding the sampler for every background-image pixel

Sampling a bitmap background rebuilt the source dimensions, the sampling footprint and a length-checked `PixmapRef` for every pixel. That state is resolved once per tile now. A background drawn at the bitmap's own size is composited as the source pixmap instead of being resampled. Output is unchanged.

## takumi-raster@0.4.13

### Place inline content inside its container's transform

A rotated or scaled container drew its inline boxes, its inline images and its outline in the wrong place. Those offsets were applied in device space, after the transform, instead of in the container's own coordinates. A container with a plain translation was never affected.

### Stroke the span that asked for it

`-webkit-text-stroke` was read off the element holding the text, so a `span` setting it for itself came out unstroked, and a nested one turning it off still got the parent's outline. The stroke now travels with the text run, in every backend.

## takumi-raster@0.4.12

### Paint a `border-area` background under the border

A `background-clip: border-area` fill painted over the border, or replaced its colour, depending on the backend. It now paints under it.

### Paint an outline above the text it wraps

A box with a negative `outline-offset` drew its own text over the outline ring. The outline now paints after everything inside the box, matching the SVG and PDF backends.

### Place replaced content from one place

`object-fit` and `object-position` place replaced content from one place. An `object-position` past 100% now clips to the content box.

### Skip the ink an underline runs through, in every backend

`text-decoration-skip-ink` breaks an underline where the glyph outlines cross it, in every backend. A gap inside a letter stays a gap.

### Rotate hue from the same matrix everywhere

`takumi_core::filter::ColorMatrix` turns a colour-transforming `filter` function into the matrix Filter Effects defines for it. The raster backend had written the `hue-rotate` coefficients out a second time and rounded the angle to whole degrees first, so `hue-rotate(45.5deg)` rotated by 45.

### Stack text shadows the way a browser does

The raster backend painted `text-shadow` in list order, putting the last one on top. CSS puts the first one there, so a stack of glows came out inverted. `SizedFontStyle::painted_text_shadows` walks them back to front for every backend, and drops the ones nobody sees.

### Resolve a `clip-path` shape from one place

The raster backend resolved `inset()`, `ellipse()`, `polygon()` and `path()` itself. An `ellipse()` took its keyword radii across both axes, so a non-square box got the wrong ones, and a percentage corner radius in `inset()` measured against the width on both axes.

### Paint an outline above the box's children

A box with a negative `outline-offset` drew its outline under its own children, so a ring dragged inside the box disappeared behind them. The outline now paints after everything the box contains.

## takumi-raster@0.4.11

### Bound allocations and loops driven by untrusted input

Three denial-of-service paths are closed. SVG rasterization and canvas allocation now cap at 16M pixels and return an error past it, instead of aborting on a huge allocation. A `background-size` past `i32::MAX` no longer wraps a repeat step negative and loops forever.

## takumi-raster@0.4.5

### Fix debug-build panic when drop-shadow hits a fully transparent element

`drop-shadow()` on an element with no visible pixels panicked with an integer underflow in debug builds. The bounds check now short-circuits before computing the empty region's size.

## takumi-raster@0.4.3

### Stop parking retired subcanvas pixmaps for the rest of the render

A canvas held on to up to eight full-size pixmaps that isolated groups had finished with, so a page with several stacking contexts kept multiples of its own viewport alive until the render returned. They are plain allocations now, freed as each group composites, which is where the rest of the scratch buffers already went.

## takumi-raster@0.4.2

### Replace `ResolvedGlyphPlacement` with `geometry::Placement`

`Placement` moves into `takumi_core::geometry` and takes over from `ResolvedGlyphPlacement`, which described the same four fields. `BuiltInlineLayout::resolved_glyphs` is now keyed to `Arc<ResolvedGlyph>`, so a glyph cache hit stops copying the outline commands.

### Cache text-stroke and faux-bold glyph masks

Both rasterized through `render_mask` on every draw, so CJK bold, which triggers synthesis at weight 600 and up, paid a full stroke rasterization per glyph per render. They go through the shared glyph cache now, keyed on the stroke as well as the outline. Stroked masks land on the same quarter-pixel grid as the fill, which shifts antialiasing on stroked and synthesized text by a fraction of a pixel and stops the stroke drifting from the fill it outlines.

### Write per-frame delays into animated PNG output

Every APNG frame was written with the shortest frame's delay. The delay was set once on the encoder, before the header, so the header's `fcTL` covered the whole animation, on the premise that APNG has no per-frame duration. It does: each frame carries its own `fcTL`, and the `png` crate exposes it as `Writer::set_frame_delay`.

The header now takes the first frame's duration and every later frame gets its own. A 150 ms timeline that renders as frames of 33, 33, 34, 33 and 17 ms used to play back as five 17 ms frames, roughly 1.8× too fast. A short scene followed by a long hold was much worse. WebP and GIF already wrote per-frame delays and are unchanged.

### Couple the overflow axes and stop mixed overflow from blanking a node

`overflow-x` and `overflow-y` were read straight off the computed style, so `overflow-x: hidden` next to `overflow-y: visible` stayed mixed. CSS Overflow 3 says a `visible` axis paired with one that is neither `visible` nor `clip` computes to a scrolling value instead, which is why Chrome clips both axes there. `resolve_overflows` now applies that coupling, so the pair reaches layout, painting, and the SVG backend already resolved. `clip` next to `visible` is a legal combination and still passes through untouched.

That legal pair then hit a second bug. The mask builder marks an unclipped axis with `u32::MAX`, and the identity-transform fast path narrowed it with `as i32`, which truncates to `-1` rather than saturating. The clip rectangle came out empty, so the node rendered nothing at all. The comparison now clamps before narrowing, matching the rotated path, which had always compared in `u32`.

## takumi-raster@0.4.1

### Share the glyph caches across worker threads

The glyph mask and resolved-glyph caches were thread-local maps under one process-wide byte counter, but eviction only pruned the inserting thread. An idle worker kept its share forever, so real retention multiplied with the thread pool (the emoji-bitmap growth noted in #1023). Both caches are now process-global `quick_cache` instances splitting the same 8 MiB budget: a glyph resolved on one thread is a hit on every thread, and eviction is global. `GlyphCache` methods take `&self` and `get` returns a clone; `set_glyph_cache_max_bytes` now applies to caches not yet used, so call it before the first render.

### Stop the glyph mask cache from retaining more memory than its budget

Cached glyph masks were charged `mask.len()` bytes but stored buffers recycled from the canvas buffer pool, which hands out any larger bucket — so a KB-sized mask could pin a much larger allocation and the 8 MiB budget under-enforced by a pool-state-dependent factor (#1023). A/B benchmarks showed the pool itself has no measurable win over the allocator on the render suites, so scratch buffers are now plain allocations: cached masks own exactly-sized buffers charged by capacity, and the buffer pool is gone. A retention test renders unique text over gradient cards and asserts live heap bytes stay near the budget.

## takumi-raster@0.4.0

### Bound the thread-local glyph caches by bytes

The resolved-glyph and glyph-mask caches were thread-local maps capped at 4096 entries each: per-entry size was unbounded, the cap multiplied per worker thread, and overflowing flushed the whole map so hot glyphs paid the rebuild cost. Both caches now weigh entries in bytes against one process-wide 8 MiB budget, tunable through `takumi_core::resources::glyph_cache::set_glyph_cache_max_bytes`. Going over budget first drops entries no recent render touched, then only as many fresh ones as the overage requires; the map is never flushed whole. A retention test renders the same content 200 times and asserts live heap bytes stay flat, so budget regressions fail in CI.

### Unify decoded resources behind one budgeted cache

Decoded images had a byte budget, but each SVG kept up to 32 rasterized pixmaps outside it, and every render re-parsed its stylesheets from scratch. `ImageCache` is now `ResourceCache`: SVG sources, their rasterized pixmaps, and parsed stylesheets all weigh against the same budget as decoded images. The default budget drops from 64 MiB to 16 MiB and becomes configurable — `new Renderer({ cacheMaxBytes })` in the bindings, `ResourceCache::new(max_bytes)` in Rust, with `0` disabling caching. SVG rasters and parsed stylesheets now also survive across renders, so a server re-rendering the same template stops re-rasterizing and re-parsing per request. Rust callers: `RenderOptions.stylesheet` is now `Arc<StyleSheet>`; pass `sheet.into()`.

### Return an error for viewports too large to allocate

A viewport whose pixel buffer overflowed the backing allocation used to fall back to a 1x1 canvas, so the render produced a valid-looking but wrong tiny image with no error. The root canvas is now built through a fallible path that surfaces the allocation failure as an `InvalidViewport` error instead. Internal offscreen canvases keep their bounded sizes and are unaffected.

### Blend animated WebP frames by default on the native binding

`AnimatedWebpOptions::builder()` left `blend` and `dispose` unset, so they fell back to `false` while the type's `Default` and the wasm backend use `blend: true`. Native animated WebP now alpha-blends frames over prior content like wasm does, so animations with partially transparent later frames render the same on both backends. The builder defaults are pinned to the `Default` values.

## takumi-raster@0.3.1

### Raise the lossless WebP compression effort

Encode `WebPLossless` at effort 50 instead of 20, shrinking output by about 10% at 1.5x the encode time. Drop the `alpha_compression` assignment, which libwebp never read on either path.

### Encode lossless WebP without a YUV420 round trip

Set `use_argb` on the imported picture so `WebPLossless` and lossless animated WebP keep their source pixels instead of passing through chroma subsampling.

## takumi-raster@0.3.0

### Apply filter references without the base64 roundtrip

`apply_svg_filter` hands the layer to resvg through a custom href resolver as fast-compressed PNG bytes, dropping the base64 encode, data-URI decode, and multi-megabyte XML parse.

### Support SVG filter references in `filter`

`filter` and `backdrop-filter` accept `url(data:image/svg+xml,...)` with inline `<filter>` markup, mixing freely with filter functions. The raster backend executes the graph through resvg; the SVG backend emits the markup verbatim.

## takumi-raster@0.2.5

### Decode GIF frames lazily

Decode only the first GIF frame up front and the rest on first sample past it; static renders no longer decode the whole animation. `GifSource::frame_at_time` returns `Arc<ImageBuffer>` and `RenderedImage::Borrowed` becomes `Sampled`.

### Decode bitmaps at draw size

Bitmaps loaded through `ImageCache` stay encoded until draw time, decode scaled to the box they are drawn into, and cache per content and target size. Adds `ImageSource::Encoded`; `get_or_decode` returns it instead of `Bitmap` for PNG/JPEG/WebP. `cache: "none"` keeps returning an eagerly decoded `Bitmap`.

### Memoize GIF frames at draw size

Frames past the first decode scaled to the box the GIF is drawn into, so an animation's memoized timeline holds draw-sized frames instead of canvas-sized ones. Adds `GifSource::frame_at_time_covering`.

### Render animation frames in parallel chunks

`write_animation` renders one chunk of rayon threads' worth of frames in parallel between encoder drains, keeping at most one chunk of raw frames in memory.

## takumi-raster@0.2.3

### Fix `fontFamilies` order being ignored

`fontFamilies` only fed the fallback bucket, never the root style, so text
picked whichever registered font resolved first instead of the requested
order. `FontFamily`'s default is now empty instead of a generic `sans-serif`
token, so an empty root style falls through to the fallback bucket directly.

## takumi-raster@0.2.2

### Skip mask copies and per-pixel mask lookups in canvas fast paths

Borrow the combined constraint mask directly when it already matches the
canvas viewport instead of copying it per draw, and hoist mask lookups out of
the per-pixel blit loops. Output is unchanged.

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
