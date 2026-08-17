## @takumi-rs/wasm@2.10.0

### Update the font stack

parley 0.11.1, skrifa 0.44, and write-fonts 0.50, with the hinting-gate fork rebased so a single fontations version serves layout and subsetting.

### Match Blink's spacing between a bullet marker and its item

An outside bullet keeps Blink's fixed 7px gap on top of its suffix space, an inside bullet separates with `1em`, and the `square` style draws `▪`, approximating Blink's painted size.

### Drop `reversed`, gradient marker images, and `menu`/`dir` list counting

An `<ol reversed>` now counts up, a gradient `list-style-image` falls back to the counter style, and only `ul`/`ol` scope a list's count.

## @takumi-rs/wasm@2.9.0

### Load the wasm binary in a browser bundle

Vite, webpack and Turbopack set the same export conditions for a browser build. All three resolved the Vite entry, whose `?url` import only works in Vite. Each package now exports `wasm-url`, which resolves the binary through `new URL(specifier, import.meta.url)`, the call Vite, webpack and Turbopack rewrite to the asset they emit. Pair it with `takumi-pdf/no-init`, or with the new `takumi-js/wasm/no-init`, which keeps the auto-init entry out of the bundle.

### Pick the Node entry when webpack targets Node

A webpack build for Node resolved the Vite entry, because both environments set the `module` condition and it is listed first. The build then failed on that entry's `?url` import, which only Vite reads. A `webpack` condition now routes webpack's Node target to the Node entry, and every other bundler keeps the entry it already resolved.

## @takumi-rs/wasm@2.8.1

### Ship without skrifa's hinting interpreter

Every draw is unhinted, but skrifa's TrueType hinting interpreter and autohinter survived dead-code elimination through runtime branches. A patched skrifa gates them behind a `hinting` feature, cutting ~240KB from the wasm binaries with identical rendering.

## @takumi-rs/wasm@2.8.0

### Find the WASM asset when Vite emits it beside the server bundle

A plain `vite build --ssr` with `ssrEmitAssets` writes the asset to `assets/` under the same `outDir` as the server chunk. The SSR read looked next to the chunk and in a framework's `client/` directory, missed both, and threw `Unable to locate Takumi WASM asset for SSR`.

## @takumi-rs/wasm@2.7.0

### Restore JPEG, WebP and GIF decoding

`takumi` disables `takumi-core`'s default features and never re-enables image decoding, so every build decoded PNG and ICO only. A WebP source failed with `The image format could not be determined`. `image-decoding` is now a `takumi` feature as well, on by default, and it splits into `jpeg`, `webp` and `gif` for a build that wants one format and not the others. The napi and wasm bindings turn it on too.

## @takumi-rs/wasm@2.6.0

### Route shared codepoints to the subset that declares them

A Google Fonts subset encodes more than the `unicode-range` it was cut for, and the Cyrillic and Greek ones also carry the ASCII space and the Latin capitals. Selection took the first subset whose glyphs covered a character, in family-name order, so those codepoints left the Latin subset and every word split into separate runs. Subsets now rank by the range they declare, lowest first.

## @takumi-rs/wasm@2.5.5

### Type render output as backed by `ArrayBuffer`

`render` and `renderAnimation` declared their output as `Buffer` / `Uint8Array` over `ArrayBufferLike`, so passing the bytes straight to `new Response(...)` failed to typecheck. They now declare `Buffer<ArrayBuffer>` / `Uint8Array<ArrayBuffer>`, which `BodyInit` accepts.

## @takumi-rs/wasm@2.5.0

### Add `setGlyphCacheMaxBytes`

The resolved-glyph and glyph-mask caches share an 8 MiB budget that no binding exposed. `cacheMaxBytes` looks like the knob for it but covers a different set of caches: decoded images, SVG rasters, and parsed stylesheets.

`setGlyphCacheMaxBytes` sets the glyph budget. It is a module-level function rather than a `Renderer` option because those caches live in the module and are shared by every renderer, and the budget is read the first time a cache is used, so the call has to come before the first render.

The default suits Latin text. A CJK outline runs a few kilobytes, so 8 MiB holds on the order of a thousand of them and a page of Chinese re-rasterizes glyphs it evicted a moment earlier.

`takumi-js` forwards it too. That one records the budget and hands it to the backend as it loads, so it stays synchronous and cannot race the resolution.

## @takumi-rs/wasm@2.4.0

### Unify decoded resources behind one budgeted cache

Decoded images had a byte budget, but each SVG kept up to 32 rasterized pixmaps outside it, and every render re-parsed its stylesheets from scratch. `ImageCache` is now `ResourceCache`: SVG sources, their rasterized pixmaps, and parsed stylesheets all weigh against the same budget as decoded images. The default budget drops from 64 MiB to 16 MiB and becomes configurable — `new Renderer({ cacheMaxBytes })` in the bindings, `ResourceCache::new(max_bytes)` in Rust, with `0` disabling caching. SVG rasters and parsed stylesheets now also survive across renders, so a server re-rendering the same template stops re-rasterizing and re-parsing per request. Rust callers: `RenderOptions.stylesheet` is now `Arc<StyleSheet>`; pass `sheet.into()`.

### Return an error for viewports too large to allocate

A viewport whose pixel buffer overflowed the backing allocation used to fall back to a 1x1 canvas, so the render produced a valid-looking but wrong tiny image with no error. The root canvas is now built through a fallible path that surfaces the allocation failure as an `InvalidViewport` error instead. Internal offscreen canvases keep their bounded sizes and are unaffected.

## @takumi-rs/wasm@2.3.0

### Accept raw RGBA pixels as an image source

An image node's `src` takes `{ width, height, data, premultiplied? }` and renders the pixels without decoding; the `Bitmap` JSX helper wraps it as an `<img>`.

## @takumi-rs/wasm@2.2.2

### Fix WebAssembly parsing on Bun/Nitro

Remove `--all-features` from the `wasm-opt` configuration to prevent generating experimental WebAssembly features (such as typed function references) that cause parsing errors (e.g. `unexpected value type 100`) in bundlers like Nitro/unwasm. Explicitly enable stable features (`bulk-memory`, `mutable-globals`, and `sign-ext`) instead.

## @takumi-rs/wasm@2.2.0

### Extend the built-in Geist to weight 300

The embedded last-resort font covers `wght` 300..800 and trims unused stylistic sets, ending up slightly smaller than before.

### Claim generic font families from the JS font API

Font descriptors accept `generic` (e.g. `"monospace"`), so stacks like Tailwind's `font-mono` resolve to registered fonts without naming the family.

## @takumi-rs/wasm@2.0.2

### Accept a promise in the `fonts` option

`fonts` now takes `Promise<FontLoader[]>` as well as the plain list, so
`googleFonts` results pass straight through without `await`.

### Extend the embedded font weight axis to 800

The embedded last-resort Geist subset now covers weights 400 to 800.

## @takumi-rs/wasm@2.0.0

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

### Apply structured keyframes in `renderAnimation`

`renderAnimation` accepted a `keyframes` option but never registered it, so
structured keyframes animated with `render` yet stayed static in animations. It
now extends the stylesheet with them like the other entry points.

### Fix buffer pool bucket capacity invariant

Release now buckets a buffer by the floor power of two its capacity guarantees,
and `acquire_dirty` reserves before `set_len`. A pooled buffer can no longer be
lengthened past its allocation.

### Skip mask copies and per-pixel mask lookups in canvas fast paths

Borrow the combined constraint mask directly when it already matches the
canvas viewport instead of copying it per draw, and hoist mask lookups out of
the per-pixel blit loops. Output is unchanged.

### Make subset-group font selection deterministic

Subsets registered under one logical family (via `FontResource::subset_of`) were kept
in registration order. Callers commonly register fonts concurrently, so that order — and
therefore which subset won for a codepoint covered by more than one (e.g. overlapping
weight subsets, where the loser is faux-bolded) — varied per process. Identical input
could render to different bytes run to run.

Subsets are now held in a `BTreeSet`, ordered by their family name, so expansion and
selection no longer depend on registration timing. Same input renders identically.

### Shrink the published binaries

Size-optimize the layout and shaping crates that never run per pixel, cutting
the published WASM and native binary size.

### Share the renderer facade between the napi and wasm bindings

The napi and wasm `Renderer` wrappers now build on a shared `prepareRenderInput`
helper in `@takumi-rs/helpers/renderer`, so their option types and render bodies
live in one place. Both backends check `signal` before and after resolving
fonts and images: on native, an already-aborted signal now throws before any
resource fetch.

### Fix gradient stop snapping panic for oversized stop lists

Gradients with more color stops than the LUT can hold no longer panic. The
sample-index clamp and normalization passes stay within LUT bounds.

### Reuse per-scene state across animation frames

Compute each scene's font snapshot once and share its image and stylesheet
handles across frames instead of re-snapshotting and deep-cloning the whole
option tree per frame. Frame output is unchanged.

### Remove `encodeFrames`

`Renderer.encodeFrames` and its `EncodeFramesOptions` / `AnimationFrameSource`
types are gone. `renderAnimation` covers scene-based animation; pre-rendered
frame encoding had no callers.

### Replace `fetchResources`/`extractResourceUrls` with `prepareImages`

`@takumi-rs/helpers` exports `prepareImages({ node, sources?, fetchCache?, fetch?, timeout? })`,
which collects a node tree's remote images and fetches the ones not already supplied. Pass a
`fetchCache` (a `Map<string, Promise<ArrayBuffer>>`, or any `Map`-like store) to coalesce
concurrent fetches of the same URL and reuse the bytes across renders; a failed fetch is
evicted so a later call retries.

The `extractResourceUrls` and `fetchResources` helpers are removed. The `images` render option
takes the same group form: `{ sources, fetchCache, fetch, timeout }`.

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

### Type keyframe declarations with `csstype` instead of DOM's `CSSStyleDeclaration`

`KeyframesMap` and `KeyframeRule` typed each keyframe's declarations as
`Record<string, CSSStyleDeclaration>`, requiring every CSS property on a single
offset and needing the `DOM` lib. Declarations are now typed with `csstype`'s
`Properties`, an optional peer dependency, so a single offset only needs the
properties it sets and consumers without the `DOM` lib still typecheck.

### Accept a bare URL string in `fonts`

`fonts` entries can now be a URL string, e.g. `fonts: ["https://example.com/Inter.woff2"]`.
The bytes are fetched on demand and keyed by the URL; family name, weight, and style come
from the font file. The object form stays for overriding those. Adds `fontFromUrl` to
`@takumi-rs/helpers`.

### Cap image decode dimensions and GIF frame volume

Decoders reject images beyond 8192x8192 (via `image::Limits` for PNG and JPEG,
dimension checks for WebP) and GIFs beyond a total-frame pixel budget, stopping
decode-bomb OOM.

## @takumi-rs/wasm@2.0.0-rc.16 (rc)

### Make the embedded font a true last resort

Both bindings now embed one font: a Latin Geist subset with a 400 to 700
weight axis (Geist Mono and Manrope are gone). It no longer claims the
`sans-serif` generic family and always sorts after registered fonts in
fallback selection, so generic families and unstyled text resolve to the fonts
you load. The new `FontResource::last_resort` marks a font's families to sort
after every normal registration.

## @takumi-rs/wasm@2.0.0-rc.14 (rc)

### Type keyframe declarations with `csstype` instead of DOM's `CSSStyleDeclaration`

`KeyframesMap` and `KeyframeRule` typed each keyframe's declarations as
`Record<string, CSSStyleDeclaration>`, requiring every CSS property on a single
offset and needing the `DOM` lib. Declarations are now typed with `csstype`'s
`Properties`, an optional peer dependency, so a single offset only needs the
properties it sets and consumers without the `DOM` lib still typecheck.

## @takumi-rs/wasm@2.0.0-rc.13 (rc)

### Skip mask copies and per-pixel mask lookups in canvas fast paths

Borrow the combined constraint mask directly when it already matches the
canvas viewport instead of copying it per draw, and hoist mask lookups out of
the per-pixel blit loops. Output is unchanged.

### Fix gradient stop snapping panic for oversized stop lists

Gradients with more color stops than the LUT can hold no longer panic. The
sample-index clamp and normalization passes stay within LUT bounds.

### Share the renderer facade between the napi and wasm bindings

The napi and wasm `Renderer` wrappers now build on a shared `prepareRenderInput`
helper in `@takumi-rs/helpers/renderer`, so their option types and render bodies
live in one place. Both backends check `signal` before and after resolving
fonts and images: on native, an already-aborted signal now throws before any
resource fetch.

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

## @takumi-rs/wasm@2.0.0-rc.8 (rc)

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

### Apply structured keyframes in `renderAnimation`

`renderAnimation` accepted a `keyframes` option but never registered it, so
structured keyframes animated with `render` yet stayed static in animations. It
now extends the stylesheet with them like the other entry points.

## @takumi-rs/wasm@2.0.0-rc.4 (rc)

### Replace `fetchResources`/`extractResourceUrls` with `prepareImages`

`@takumi-rs/helpers` exports `prepareImages({ node, sources?, fetchCache?, fetch?, timeout? })`,
which collects a node tree's remote images and fetches the ones not already supplied. Pass a
`fetchCache` (a `Map<string, Promise<ArrayBuffer>>`, or any `Map`-like store) to coalesce
concurrent fetches of the same URL and reuse the bytes across renders; a failed fetch is
evicted so a later call retries.

The `extractResourceUrls` and `fetchResources` helpers are removed. The `images` render option
takes the same group form: `{ sources, fetchCache, fetch, timeout }`.

## @takumi-rs/wasm@2.0.0-rc.1 (rc)

### Make subset-group font selection deterministic

Subsets registered under one logical family (via `FontResource::subset_of`) were kept
in registration order. Callers commonly register fonts concurrently, so that order — and
therefore which subset won for a codepoint covered by more than one (e.g. overlapping
weight subsets, where the loser is faux-bolded) — varied per process. Identical input
could render to different bytes run to run.

Subsets are now held in a `BTreeSet`, ordered by their family name, so expansion and
selection no longer depend on registration timing. Same input renders identically.

## @takumi-rs/wasm@2.0.0-rc.0 (rc)

### Remove `encodeFrames`

`Renderer.encodeFrames` and its `EncodeFramesOptions` / `AnimationFrameSource`
types are gone. `renderAnimation` covers scene-based animation; pre-rendered
frame encoding had no callers.

### Accept a bare URL string in `fonts`

`fonts` entries can now be a URL string, e.g. `fonts: ["https://example.com/Inter.woff2"]`.
The bytes are fetched on demand and keyed by the URL; family name, weight, and style come
from the font file. The object form stays for overriding those. Adds `fontFromUrl` to
`@takumi-rs/helpers`.

## @takumi-rs/wasm@2.0.0-beta.14 (beta)

### Declare csstype as a runtime dependency

The published type definitions import `csstype`, but it was only a
devDependency, so consumers hit `Cannot find module 'csstype'`. Move it to
`dependencies`.

## @takumi-rs/wasm@2.0.0-beta.10 (beta)

### Resolve `edge-light` in the `/auto` export

`@takumi-rs/wasm/auto` now maps the `edge-light` condition (Next.js / Vercel
Edge) to the `?module` loader, so edge bundlers get the binary form they need
instead of falling through to the Vite `?url` loader.

## @takumi-rs/wasm@2.0.0-beta.9 (beta)

### Fix the Node bundler entries for ESM and CJS

The `node` and `bun` entries imported `../dist/export` without a file extension,
which Node's ESM resolver rejects with `ERR_MODULE_NOT_FOUND`; they now import
`../dist/export.mjs`. The `./auto` export also gains a `require` branch that
resolves to `node.cjs`, so `require("@takumi-rs/wasm/auto")` works on every
supported Node version instead of only those that can `require` an ES module.

## @takumi-rs/wasm@2.0.0-beta.6 (beta)

### Resolve the SSR WASM asset without guessing the output dir

The Vite bundler entry mapped the `?url` asset to disk by guessing a `client/`
directory, which broke dev (`/@fs/` URLs) and frameworks with a different layout
(e.g. Waku's `public/`). It now reads the asset colocated with the server chunk
via `import.meta.url`, with the `client/` paths kept as fallbacks.

### Let cached font buffers be garbage-collected

The renderer cached each registered font by its buffer in a `Map`, pinning the
data for the renderer's lifetime even after the caller dropped its reference.
Buffers now live in a `WeakMap`, so they are freed once nothing else holds them.

## @takumi-rs/wasm@2.0.0-beta.5 (beta)

### Fix `workspace:*` leaking into the published `package.json`

Published packages shipped their inter-package dependencies as the literal
`workspace:*` range, so installing them failed with `Workspace dependency
"@takumi-rs/core" not found`. The publish step now resolves `workspace:` ranges
to concrete versions, matching what `bun` and `pnpm publish` already do.

## @takumi-rs/wasm@2.0.0-beta.4 (beta)

### Re-release all packages in sync

Earlier beta releases drifted out of lockstep, so some published packages
depended on versions that were never published. Bump and publish the set
together so the beta tag is consistent and every inter-package dependency
resolves.

### Render text language-aware with the `lang` attribute

The `lang` attribute sets a node's language and inherits to its descendants; a
nested `lang` overrides, and an empty or invalid value clears it. The language
reaches the shaper as the locale, so the same code point draws its
language-specific glyph (Han unification across `ja` / `zh` / `ko`) and lines
break per language. Pass `lang` in the render options to set a default for the
whole tree.

## @takumi-rs/wasm@2.0.0-beta.2 (beta)

### Expand subset font-family without borrowing the font context

A render holds the font context borrowed while it builds text layout, so per-element
`font-family` skipped subset-group expansion and routing relied on the fallback bucket, which
followed `fontique`'s hash iteration order. Content registered with `subset_of` (Rust) or
`subsetOf` (JS) could route to the wrong subset, and the result changed between renders.
Expansion now reads the subset groups without that borrow, and the fallback bucket follows
registration order.

## @takumi-rs/wasm@2.0.0-beta.1 (beta)

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

## @takumi-rs/wasm@2.0.0-beta.0 (beta)

### Add a `renderSvg` method

Render a node tree to an SVG document string, alongside the raster `render`.

### Model the output format as a discriminated union

`quality` and `lossless` appear only on formats that accept them, and out-of-range WebP quality clamps instead of throwing.

### Make fonts and images explicit per-render resources

Drop the persistent image store and `GlobalContext`, and pass fonts and images per render. `registerFont` replaces `loadFont`/`loadFontSync`/`loadFonts`, each render takes a `fontFamilies` fallback chain, and `images` replaces `fetchedResources`.

### Make the `Renderer` constructor parameterless and register fonts with `registerFont`

The embedded default fonts decode once and stay shared across renderers.

### Remove the `createImageResponse` factory

Pass options straight to `ImageResponse`.

### Resolve lazy image loaders inside the managed `Renderer`

The render signal now comes from options.

### Accept `fonts` and `fontFamilies` in `renderAnimation` and `encodeFrames`

These match the `render` signature.

### Add a per-image `cache` mode (`auto` | `none`)

Controls decode caching one image at a time.

# @takumi-rs/wasm

## 1.8.7

### Patch Changes

- 9604fd7: Split package export types per import/require condition so CJS consumers resolve `.d.cts`
- Updated dependencies [9604fd7]
  - @takumi-rs/helpers@1.8.7

## 1.8.6

### Patch Changes

- @takumi-rs/helpers@1.8.6

## 1.8.5

### Patch Changes

- @takumi-rs/helpers@1.8.5

## 1.8.4

### Patch Changes

- @takumi-rs/helpers@1.8.4

## 1.8.3

### Patch Changes

- @takumi-rs/helpers@1.8.3

## 1.8.2

### Patch Changes

- 041e5fd: Fix `+simd128` flag being override by CI configuration
  - @takumi-rs/helpers@1.8.2

## 1.8.1

### Patch Changes

- 55b058d: Hold renderer state behind a lock so all methods take `&self`, preventing a panic from permanently breaking the wasm-bindgen borrow flag.
  - @takumi-rs/helpers@1.8.1

## 1.8.0

### Minor Changes

- ae2c9aa: Built with nightly Rust toolchain with `panic=immediate-abort` to reduce binary size

### Patch Changes

- @takumi-rs/helpers@1.8.0

## 1.7.0

### Patch Changes

- Updated dependencies [b908a4d]
- Updated dependencies [4748c22]
- Updated dependencies [42d0d03]
- Updated dependencies [80e29da]
  - @takumi-rs/helpers@1.7.0

## 1.6.0

### Patch Changes

- @takumi-rs/helpers@1.6.0

## 1.5.1

### Patch Changes

- @takumi-rs/helpers@1.5.1

## 1.5.0

### Patch Changes

- @takumi-rs/helpers@1.5.0

## 1.4.1

### Patch Changes

- @takumi-rs/helpers@1.4.1

## 1.4.0

### Patch Changes

- Updated dependencies [e83ab19]
  - @takumi-rs/helpers@1.4.0

## 1.3.0

### Patch Changes

- @takumi-rs/helpers@1.3.0

## 1.2.1

### Patch Changes

- @takumi-rs/helpers@1.2.1

## 1.2.0

### Patch Changes

- @takumi-rs/helpers@1.2.0

## 1.1.2

### Patch Changes

- @takumi-rs/helpers@1.1.2

## 1.1.1

### Patch Changes

- @takumi-rs/helpers@1.1.1

## 1.1.0

### Patch Changes

- @takumi-rs/helpers@1.1.0

## 1.0.16

### Patch Changes

- @takumi-rs/helpers@1.0.16

## 1.0.15

### Patch Changes

- @takumi-rs/helpers@1.0.15

## 1.0.14

### Patch Changes

- @takumi-rs/helpers@1.0.14

## 1.0.13

### Patch Changes

- @takumi-rs/helpers@1.0.13

## 1.0.12

### Patch Changes

- @takumi-rs/helpers@1.0.12

## 1.0.11

### Patch Changes

- @takumi-rs/helpers@1.0.11

## 1.0.10

### Patch Changes

- @takumi-rs/helpers@1.0.10

## 1.0.9

### Patch Changes

- @takumi-rs/helpers@1.0.9

## 1.0.8

### Patch Changes

- Updated dependencies [8886c01]
- Updated dependencies [b287c43]
  - @takumi-rs/helpers@1.0.8

## 1.0.7

### Patch Changes

- @takumi-rs/helpers@1.0.7

## 1.0.6

### Patch Changes

- @takumi-rs/helpers@1.0.6

## 1.0.5

### Patch Changes

- Updated dependencies [d113fb5]
  - @takumi-rs/helpers@1.0.5

## 1.0.4

### Patch Changes

- @takumi-rs/helpers@1.0.4

## 1.0.3

### Patch Changes

- @takumi-rs/helpers@1.0.3

## 1.0.2

### Patch Changes

- @takumi-rs/helpers@1.0.2

## 1.0.1

### Patch Changes

- @takumi-rs/helpers@1.0.1

## 1.0.0

### Major Changes

- 188079f: **Changed initial `display` value from `flex` to `inline`**

  This is to comply with [the CSSWG spec](https://drafts.csswg.org/css-display/#the-display-properties).

  You should update your code to use `display: flex` if you want to use flexbox.

### Minor Changes

- 79c0c5a: Embed full-axis Manrope as the default font
- 1373f0a: Support `ico` format

### Patch Changes

- 9b411ce: Reduce wasm size with more stripping
- da2d85f: Support unwasm for `/auto` export
- b2e304a: Replaced native `extractResourceUrls` with JS version to avoid roundtrip
- bc6243a: Add `bun` conditional exports
- Updated internal dependencies
  - @takumi-rs/helpers@1.0.0

## 1.0.0-rc.17

### Patch Changes

- Updated dependencies [6767ad9]
  - @takumi-rs/helpers@1.0.0-rc.17

## 1.0.0-rc.16

### Patch Changes

- @takumi-rs/helpers@1.0.0-rc.16

## 1.0.0-rc.15

### Patch Changes

- @takumi-rs/helpers@1.0.0-rc.15

## 1.0.0-rc.14

### Patch Changes

- Updated dependencies [eb34add]
  - @takumi-rs/helpers@1.0.0-rc.14

## 1.0.0-rc.13

### Patch Changes

- @takumi-rs/helpers@1.0.0-rc.13

## 1.0.0-rc.12

### Patch Changes

- @takumi-rs/helpers@1.0.0-rc.12

## 1.0.0-rc.11

### Patch Changes

- b2e304a: Replaced native `extractResourceUrls` with JS version to avoid roundtrip
- Updated dependencies [b2e304a]
  - @takumi-rs/helpers@1.0.0-rc.11

## 1.0.0-rc.10

### Patch Changes

- Updated dependencies [cc9e63c]
  - @takumi-rs/helpers@1.0.0-rc.10

## 1.0.0-rc.9

### Patch Changes

- @takumi-rs/helpers@1.0.0-rc.9

## 1.0.0-rc.8

### Patch Changes

- @takumi-rs/helpers@1.0.0-rc.8

## 1.0.0-rc.7

### Patch Changes

- @takumi-rs/helpers@1.0.0-rc.7

## 1.0.0-rc.6

### Patch Changes

- @takumi-rs/helpers@1.0.0-rc.6

## 1.0.0-rc.5

### Patch Changes

- @takumi-rs/helpers@1.0.0-rc.5

## 1.0.0-rc.4

### Patch Changes

- Updated dependencies [7ff886b]
- Updated dependencies [7ff886b]
  - @takumi-rs/helpers@1.0.0-rc.4

## 1.0.0-rc.3

### Patch Changes

- bc6243a: Add `bun` conditional exports
  - @takumi-rs/helpers@1.0.0-rc.3

## 1.0.0-rc.2

### Patch Changes

- @takumi-rs/helpers@1.0.0-rc.2

## 1.0.0-rc.1

### Patch Changes

- @takumi-rs/helpers@1.0.0-rc.1

## 1.0.0-rc.0

### Patch Changes

- @takumi-rs/helpers@1.0.0-rc.0

## 1.0.0-beta.20

### Patch Changes

- Updated dependencies [01c4fa3]
  - @takumi-rs/helpers@1.0.0-beta.20

## 1.0.0-beta.19

### Minor Changes

- 79c0c5a: Embed full-axis Manrope as the default font

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.19

## 1.0.0-beta.18

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.18

## 1.0.0-beta.17

### Patch Changes

- da2d85f: Support unwasm for `/auto` export
  - @takumi-rs/helpers@1.0.0-beta.17

## 1.0.0-beta.16

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.16

## 1.0.0-beta.15

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.15

## 1.0.0-beta.14

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.14

## 1.0.0-beta.13

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.13

## 1.0.0-beta.12

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.12

## 1.0.0-beta.11

### Patch Changes

- 111dd88: Add `/auto` export
  - @takumi-rs/helpers@1.0.0-beta.11

## 1.0.0-beta.10

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.10

## 1.0.0-beta.9

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.9

## 1.0.0-beta.8

### Patch Changes

- 9b411ce: Reduce wasm size with more stripping
  - @takumi-rs/helpers@1.0.0-beta.8

## 1.0.0-beta.7

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.7

## 1.0.0-beta.6

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.6

## 1.0.0-beta.5

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.5

## 1.0.0-beta.4

### Patch Changes

- Updated dependencies [f1b6104]
  - @takumi-rs/helpers@1.0.0-beta.4

## 1.0.0-beta.3

### Patch Changes

- Updated dependencies [b5b8531]
  - @takumi-rs/helpers@1.0.0-beta.3

## 1.0.0-beta.2

### Patch Changes

- Updated dependencies [3142b36]
  - @takumi-rs/helpers@1.0.0-beta.2

## 1.0.0-beta.1

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.1

## 1.0.0-beta.0

### Major Changes

- 188079f: **Changed initial `display` value from `flex` to `inline`**

  This is to comply with [the CSSWG spec](https://drafts.csswg.org/css-display/#the-display-properties).

  You should update your code to use `display: flex` if you want to use flexbox.

### Patch Changes

- @takumi-rs/helpers@1.0.0-beta.0

## 0.73.1

### Patch Changes

- @takumi-rs/helpers@0.73.1

## 0.73.0

### Minor Changes

- 349636a: **Deprecate `AnyNode`, replaced with strict `Node` type**

### Patch Changes

- Updated dependencies [349636a]
  - @takumi-rs/helpers@0.73.0

## 0.72.0

## 0.71.7

## 0.71.6

### Patch Changes

- 1a111a6: Add `keyframes` render option

## 0.71.5

## 0.71.4

## 0.71.3

### Patch Changes

- a279b4c: Add `dithering` option for smoother gradients

## 0.71.2

## 0.71.1

### Patch Changes

- a22b234: Optimize for speed instead of binary size

## 0.71.0

### Minor Changes

- 0930cdb: **BREAKING CHANGE: `renderAnimation` now take "scenes" with keyframe animations instead of frames**

  Original frame-by-frame encoding has been renamed to `encodeFrames`

## 0.70.4

## 0.70.3

## 0.70.2

## 0.70.1

## 0.70.0

### Minor Changes

- a69dd7d: **Reverting `.asUint8Array()` changes**

  As it's very dangerous to use `asUint8Array()` without proper handling and recycling, we are reverting the changes.

  The `render` and `renderAnimation` methods now return `Uint8Array` instead of `WasmBuffer` class.

  ```tsx
  const image = renderer.render(node, options);

  controller.enqueue(image);
  ```

## 0.69.5

## 0.69.4

## 0.69.3

## 0.69.2

## 0.69.1

## 0.69.0

## 0.68.17

## 0.68.16

## 0.68.15

## 0.68.14

## 0.68.13

## 0.68.12

## 0.68.11

## 0.68.10

## 0.68.9

## 0.68.8

## 0.68.7

## 0.68.6

## 0.68.5

## 0.68.4

## 0.68.3

## 0.68.2

## 0.68.1

## 0.68.0

### Minor Changes

- 7684faa: **`WasmBuffer` class for zero-copy rendering**

  Before:

  ```tsx
  const buffer = renderer.render();
  ```

  After (with `using` keyword):

  ```tsx
  using buffer = renderer.render();
  const bytes = buffer.asUint8Array();
  // buffer is automatically disposed when it goes out of scope
  ```

  After (manual freeing):

  ```tsx
  const buffer = renderer.render();
  const bytes = buffer.asUint8Array();

  buffer.free();

  // If you forget to free the buffer, it will be leaked.
  // Do NOT attempt to read from bytes after freeing to avoid use-after-free exploit.
  ```

### Patch Changes

- 7684faa: refactor font loading to reduce buffer copying

## 0.67.3

## 0.67.2

### Patch Changes

- ca61b5e: add cjs build

## 0.67.1

## 0.67.0

## 0.66.14

## 0.66.13

### Patch Changes

- c97a402: add `ConstructRendererOptions` to `Renderer` constructor

## 0.66.12

### Patch Changes

- 7389d6e: add persistent image cache

## 0.66.11

## 0.66.10

## 0.66.9

## 0.66.8

## 0.66.7

### Patch Changes

- a3e7f9c: add `raw` output format
- a3e7f9c: document all the functions

## 0.66.6

## 0.66.5

## 0.66.4

## 0.66.3

## 0.66.2

## 0.66.1

## 0.66.0

### Minor Changes

- f811582: new `/node` entrypoint that handles synchronous WASM module initialization

### Patch Changes

- 5b7ea89: fix `AnimationFrameSource` deserialization
- 94d7959: fix SVG persistent image loading

## 0.65.0

### Minor Changes

- 1319540: new `measure()` API

## 0.64.1

## 0.64.0

### Minor Changes

- 1600ff0: **`putPersistentImage` now takes `ImageSource` insead of `src` & `data`**
- 1600ff0: make `fetchedResources` accept `ImageSource` array instead of map

## 0.63.2

## 0.63.1

## 0.63.0

### Minor Changes

- 87a12b4: rename `collectNodeFetchTasks` to `extractResourceUrls`

## 0.62.8

## 0.62.7

### Patch Changes

- 54e8bec: return error instead of panicing #389

## 0.62.6

## 0.62.5

## 0.62.4

## 0.62.3

## 0.62.2

## 0.62.1

## 0.62.0

## 0.61.1

## 0.61.0

## 0.60.8

## 0.60.7

## 0.60.6

## 0.60.5

## 0.60.4

## 0.60.3

## 0.60.2

## 0.60.1

## 0.60.0

## 0.59.1

## 0.59.0

## 0.58.0

## 0.57.6

## 0.57.5

## 0.57.4

## 0.57.3

## 0.57.2

## 0.57.1

## 0.57.0

## 0.56.1

## 0.56.0

## 0.55.4

## 0.55.3

## 0.55.2

## 0.55.1

## 0.55.0

## 0.54.3

### Patch Changes

- 8c6e17e: make `render(options)` parameter optional

## 0.54.2

## 0.54.1

## 0.54.0

## 0.53.1

## 0.53.0

## 0.52.2

## 0.52.1

## 0.52.0

## 0.51.1

## 0.51.0

### Minor Changes

- 27ac6c5: support `devicePixelRatio` value

## 0.50.0

## 0.49.1

### Patch Changes

- 569e8f8: fix next.js helper to override prototype

## 0.49.0

### Minor Changes

- 4fddbd2: add nextjs turbopack helper `@takumi-rs/wasm/next`

## 0.48.0

### Minor Changes

- c3f1b7d: support optional width/height

## 0.47.0

## 0.46.6

### Patch Changes

- 73c07ff: fix `takumi_wasm_bg.wasm` import points to incorrect declartion file

## 0.46.5

## 0.46.4

## 0.46.3

## 0.46.2

## 0.46.1

## 0.46.0

## 0.45.3

## 0.45.2

## 0.45.1

## 0.45.0

## 0.44.0

## 0.43.1

## 0.43.0

## 0.42.0

## 0.41.0

## 0.40.2

## 0.40.1

## 0.40.0

## 0.39.0

## 0.38.1

## 0.38.0

### Minor Changes

- 6e37d24: add panic handler that calls `console.error()`.
- d92472f: align `render*` function props with NAPI version.
- e4cf948: deprecate `loadFontWithInfo` (merged into `loadFont`)

## 0.37.0

## 0.36.2

## 0.36.1

## 0.36.0

## 0.35.2

## 0.35.1

## 0.35.0

## 0.34.0

### Patch Changes

- 7c402d8: setup npm trusted publisher

## 0.33.1

## 0.33.0

### Minor Changes

- 98755a7: **drop support for `debug` field, replace with `draw_debug_border` option in rendering functions**
