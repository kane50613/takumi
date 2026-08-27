## takumi-js@2.13.0

### Rename the `stylesheets` render option to `css`

`css` takes inline CSS as one string or a list. The old `stylesheets` name still works everywhere and warns once on `takumi-js` and `takumi-pdf`.

## takumi-js@2.9.0

### Load the wasm binary in a browser bundle

Vite, webpack and Turbopack set the same export conditions for a browser build. All three resolved the Vite entry, whose `?url` import only works in Vite. Each package now exports `wasm-url`, which resolves the binary through `new URL(specifier, import.meta.url)`, the call Vite, webpack and Turbopack rewrite to the asset they emit. Pair it with `takumi-pdf/no-init`, or with the new `takumi-js/wasm/no-init`, which keeps the auto-init entry out of the bundle.

## takumi-js@2.5.0

### Add `setGlyphCacheMaxBytes`

The resolved-glyph and glyph-mask caches share an 8 MiB budget that no binding exposed. `cacheMaxBytes` looks like the knob for it but covers a different set of caches: decoded images, SVG rasters, and parsed stylesheets.

`setGlyphCacheMaxBytes` sets the glyph budget. It is a module-level function rather than a `Renderer` option because those caches live in the module and are shared by every renderer, and the budget is read the first time a cache is used, so the call has to come before the first render.

The default suits Latin text. A CJK outline runs a few kilobytes, so 8 MiB holds on the order of a thousand of them and a page of Chinese re-rasterizes glyphs it evicted a moment earlier.

`takumi-js` forwards it too. That one records the budget and hands it to the backend as it loads, so it stays synchronous and cannot race the resolution.

## takumi-js@2.3.0

### Accept raw RGBA pixels as an image source

An image node's `src` takes `{ width, height, data, premultiplied? }` and renders the pixels without decoding; the `Bitmap` JSX helper wraps it as an `<img>`.

## takumi-js@2.1.1

### Bundle the WebContainer fallback instead of `@takumi-rs/wasm/auto`

The node backend's fallback pulled in `auto`, whose conditions resolve against
the host bundler: Turbopack sets `module` and got Vite's `?url` import of the
binary, failing the build. It now loads `@takumi-rs/wasm/node`, which every
bundler resolves, so only `@takumi-rs/core` needs externalizing.

### Drop `preact-render-to-string`

An optional import no bundler can skip: webpack and Vite have no optional
import, so resolving it statically failed the build of every app that renders
React and never installed it. Preact trees now traverse natively. Components
calling a Preact hook no longer render — those hooks live on Preact's mangled
internals, which no dispatcher can stand in for.

### Accept `preact/compat` elements in `ReactElementLike`

`preact/compat` types `$$typeof` as `symbol | string`, so its elements — and a
propless component's `FunctionComponentElement<never>` — no longer need a cast
to reach `render`.

## takumi-js@2.1.0

### Fall back to the WASM backend in a WebContainer

Unbundled runs (e.g. `nitro dev` externalizing the package) resolve `#backend`
with Node's default conditions, so the native addon wins even when the host set
`unwasm`. The node backend now detects `process.versions.webcontainer` and loads
the WASM backend instead.

## takumi-js@2.0.3

### Select the WASM backend under the `unwasm` condition

Nitro sets `unwasm` on every preset, including Node, so `takumi-js` resolves
`#backend` to WASM there instead of the native addon a WebContainer host
(StackBlitz, CodeSandbox) can't load. Set `exportConditions: ["!unwasm"]` to keep
the native bindings on the Node preset.

## takumi-js@2.0.0

### Mark `ImageResponse.ready` rejection as handled

A failed render no longer crashes the process with an `unhandledRejection` when
the caller never awaits `ready`. The failure still reaches the stream and a
caller that does await `ready` still observes it.

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

### Pin `@takumi-rs/*` dependencies to the matching release

`takumi-js` resolved its `@takumi-rs/core`, `@takumi-rs/helpers`, and `@takumi-rs/wasm`
dependencies to an older release than itself, so `takumi-js/response` imported a helper
the pinned `@takumi-rs/helpers` did not yet export and failed to load. The internal
dependencies now track the same release as `takumi-js`.

### Keep the format tagged union on `render` options

`render`, `renderSvg`, and `renderAnimation` used a non-distributive `Omit` that
collapsed the `format`/`quality`/`lossless` union, so `{ format: "webp", quality: 80 }`
stopped type-checking. A distributive `Omit` restores it.

## takumi-js@2.0.0-rc.13 (rc)

### Mark `ImageResponse.ready` rejection as handled

A failed render no longer crashes the process with an `unhandledRejection` when
the caller never awaits `ready`. The failure still reaches the stream and a
caller that does await `ready` still observes it.

## takumi-js@2.0.0-rc.6 (rc)

### Keep the format tagged union on `render` options

`render`, `renderSvg`, and `renderAnimation` used a non-distributive `Omit` that
collapsed the `format`/`quality`/`lossless` union, so `{ format: "webp", quality: 80 }`
stopped type-checking. A distributive `Omit` restores it.

## takumi-js@2.0.0-rc.5 (rc)

### Pin `@takumi-rs/*` dependencies to the matching release

`takumi-js` resolved its `@takumi-rs/core`, `@takumi-rs/helpers`, and `@takumi-rs/wasm`
dependencies to an older release than itself, so `takumi-js/response` imported a helper
the pinned `@takumi-rs/helpers` did not yet export and failed to load. The internal
dependencies now track the same release as `takumi-js`.

## takumi-js@2.0.0-rc.4 (rc)

### Replace `fetchResources`/`extractResourceUrls` with `prepareImages`

`@takumi-rs/helpers` exports `prepareImages({ node, sources?, fetchCache?, fetch?, timeout? })`,
which collects a node tree's remote images and fetches the ones not already supplied. Pass a
`fetchCache` (a `Map<string, Promise<ArrayBuffer>>`, or any `Map`-like store) to coalesce
concurrent fetches of the same URL and reuse the bytes across renders; a failed fetch is
evicted so a later call retries.

The `extractResourceUrls` and `fetchResources` helpers are removed. The `images` render option
takes the same group form: `{ sources, fetchCache, fetch, timeout }`.

## takumi-js@2.0.0-rc.0 (rc)

### Remove `encodeFrames`

`Renderer.encodeFrames` and its `EncodeFramesOptions` / `AnimationFrameSource`
types are gone. `renderAnimation` covers scene-based animation; pre-rendered
frame encoding had no callers.

## takumi-js@2.0.0-beta.14 (beta)

### Explain a failed native backend load

When `@takumi-rs/core` can't load on Node, the render now throws an error
pointing at the `module` option for the WASM backend, instead of surfacing the
raw native loader failure.

## takumi-js@2.0.0-beta.13 (beta)

### Keep the Vite WASM loader out of Node bundles

The WASM escape hatch always carries an explicit `module`, so it now loads
`wasm-init` directly instead of `@takumi-rs/wasm/auto`. A Next/webpack node
build that only uses napi no longer drags the Vite `?url` binary loader into
its graph, where the unresolvable query broke the build.

## takumi-js@2.0.0-beta.10 (beta)

### Resolve the render backend through import conditions

A `#backend` import map now selects napi on Node/Bun and WASM on workers, edge,
and browsers at resolve time, replacing the runtime global sniffing and
`@vite-ignore`d dynamic imports. Bundlers no longer drag the native
`@takumi-rs/core` binary into worker/edge output, and `@takumi-rs/wasm` resolves
under pnpm's strict layout.

## takumi-js@2.0.0-beta.6 (beta)

### Keep the native core out of edge bundles

The `@takumi-rs/core` import was reachable from edge builds, pulling its native
`.node` binding into the bundle and pushing it past the runtime size limit. The
import is now gated behind an inline `NEXT_RUNTIME !== "edge"` check so the edge
bundler drops it.

## takumi-js@2.0.0-beta.5 (beta)

### Fix `workspace:*` leaking into the published `package.json`

Published packages shipped their inter-package dependencies as the literal
`workspace:*` range, so installing them failed with `Workspace dependency
"@takumi-rs/core" not found`. The publish step now resolves `workspace:` ranges
to concrete versions, matching what `bun` and `pnpm publish` already do.

## takumi-js@2.0.0-beta.4 (beta)

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

## takumi-js@2.0.0-beta.2 (beta)

### Expand subset font-family without borrowing the font context

A render holds the font context borrowed while it builds text layout, so per-element
`font-family` skipped subset-group expansion and routing relied on the fallback bucket, which
followed `fontique`'s hash iteration order. Content registered with `subset_of` (Rust) or
`subsetOf` (JS) could route to the wrong subset, and the result changed between renders.
Expansion now reads the subset groups without that borrow, and the fallback bucket follows
registration order.

## takumi-js@2.0.0-beta.1 (beta)

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

## takumi-js@2.0.0-beta.0 (beta)

### Add top-level `renderSvg` and `renderAnimation`

Both mirror `render`: same JSX/HTML/node input and resource pipeline, returning a vector SVG string and an encoded animation.

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

# takumi-js

## 1.8.7

### Patch Changes

- 9604fd7: Split package export types per import/require condition so CJS consumers resolve `.d.cts`
- Updated dependencies [81c3678]
- Updated dependencies [9604fd7]
  - @takumi-rs/core@1.8.7
  - @takumi-rs/helpers@1.8.7
  - @takumi-rs/wasm@1.8.7

## 1.8.6

### Patch Changes

- @takumi-rs/core@1.8.6
- @takumi-rs/wasm@1.8.6
- @takumi-rs/helpers@1.8.6

## 1.8.5

### Patch Changes

- @takumi-rs/core@1.8.5
- @takumi-rs/wasm@1.8.5
- @takumi-rs/helpers@1.8.5

## 1.8.4

### Patch Changes

- @takumi-rs/core@1.8.4
- @takumi-rs/wasm@1.8.4
- @takumi-rs/helpers@1.8.4

## 1.8.3

### Patch Changes

- Updated dependencies [bfc6e55]
  - @takumi-rs/core@1.8.3
  - @takumi-rs/wasm@1.8.3
  - @takumi-rs/helpers@1.8.3

## 1.8.2

### Patch Changes

- Updated dependencies [041e5fd]
  - @takumi-rs/wasm@1.8.2
  - @takumi-rs/core@1.8.2
  - @takumi-rs/helpers@1.8.2

## 1.8.1

### Patch Changes

- 55b058d: Disable default fonts in the managed renderer when custom fonts are provided, matching the renderer constructor behavior.
- Updated dependencies [55b058d]
  - @takumi-rs/wasm@1.8.1
  - @takumi-rs/core@1.8.1
  - @takumi-rs/helpers@1.8.1

## 1.8.0

### Patch Changes

- Updated dependencies [ae2c9aa]
  - @takumi-rs/core@1.8.0
  - @takumi-rs/wasm@1.8.0
  - @takumi-rs/helpers@1.8.0

## 1.7.0

### Patch Changes

- 89a3088: Route Deno to the WASM bindings instead of the native addon
- 56579a2: Re-export `FontLoader`, `FontLoaderSync`, `ImageSourceLoader`, and `ImageSourceLoaderSync` from the package root
- 89a3088: Honor `AbortSignal` on the WASM render path
- Updated dependencies [b908a4d]
- Updated dependencies [4748c22]
- Updated dependencies [42d0d03]
- Updated dependencies [80e29da]
  - @takumi-rs/helpers@1.7.0
  - @takumi-rs/core@1.7.0
  - @takumi-rs/wasm@1.7.0

## 1.6.0

### Patch Changes

- @takumi-rs/core@1.6.0
- @takumi-rs/wasm@1.6.0
- @takumi-rs/helpers@1.6.0

## 1.5.1

### Patch Changes

- @takumi-rs/core@1.5.1
- @takumi-rs/wasm@1.5.1
- @takumi-rs/helpers@1.5.1

## 1.5.0

### Patch Changes

- @takumi-rs/core@1.5.0
- @takumi-rs/wasm@1.5.0
- @takumi-rs/helpers@1.5.0

## 1.4.1

### Patch Changes

- d6936e5: Expose `takumi-js/helpers/html` subpath export
  - @takumi-rs/core@1.4.1
  - @takumi-rs/wasm@1.4.1
  - @takumi-rs/helpers@1.4.1

## 1.4.0

### Patch Changes

- Updated dependencies [e83ab19]
- Updated dependencies [1d5daed]
  - @takumi-rs/helpers@1.4.0
  - @takumi-rs/core@1.4.0
  - @takumi-rs/wasm@1.4.0

## 1.3.0

### Patch Changes

- @takumi-rs/core@1.3.0
- @takumi-rs/wasm@1.3.0
- @takumi-rs/helpers@1.3.0

## 1.2.1

### Patch Changes

- @takumi-rs/core@1.2.1
- @takumi-rs/wasm@1.2.1
- @takumi-rs/helpers@1.2.1

## 1.2.0

### Patch Changes

- @takumi-rs/core@1.2.0
- @takumi-rs/wasm@1.2.0
- @takumi-rs/helpers@1.2.0

## 1.1.2

### Patch Changes

- @takumi-rs/core@1.1.2
- @takumi-rs/wasm@1.1.2
- @takumi-rs/helpers@1.1.2

## 1.1.1

### Patch Changes

- @takumi-rs/core@1.1.1
- @takumi-rs/wasm@1.1.1
- @takumi-rs/helpers@1.1.1

## 1.1.0

### Patch Changes

- @takumi-rs/core@1.1.0
- @takumi-rs/wasm@1.1.0
- @takumi-rs/helpers@1.1.0

## 1.0.16

### Patch Changes

- @takumi-rs/core@1.0.16
- @takumi-rs/wasm@1.0.16
- @takumi-rs/helpers@1.0.16

## 1.0.15

### Patch Changes

- 3f96f48: Remove `ReadableStream.type` param
  - @takumi-rs/core@1.0.15
  - @takumi-rs/wasm@1.0.15
  - @takumi-rs/helpers@1.0.15

## 1.0.14

### Patch Changes

- 6323299: Add `RenderInput` type, support passing Node
  - @takumi-rs/core@1.0.14
  - @takumi-rs/wasm@1.0.14
  - @takumi-rs/helpers@1.0.14

## 1.0.13

### Patch Changes

- ccfaff3: Removed `Promise.withResolvers` usage to be compatibable with older Node.js
  - @takumi-rs/core@1.0.13
  - @takumi-rs/wasm@1.0.13
  - @takumi-rs/helpers@1.0.13

## 1.0.12

### Patch Changes

- @takumi-rs/core@1.0.12
- @takumi-rs/wasm@1.0.12
- @takumi-rs/helpers@1.0.12

## 1.0.11

### Patch Changes

- @takumi-rs/core@1.0.11
- @takumi-rs/wasm@1.0.11
- @takumi-rs/helpers@1.0.11

## 1.0.10

### Patch Changes

- @takumi-rs/core@1.0.10
- @takumi-rs/wasm@1.0.10
- @takumi-rs/helpers@1.0.10

## 1.0.9

### Patch Changes

- @takumi-rs/core@1.0.9
- @takumi-rs/wasm@1.0.9
- @takumi-rs/helpers@1.0.9

## 1.0.8

### Patch Changes

- Updated dependencies [8886c01]
- Updated dependencies [b287c43]
  - @takumi-rs/helpers@1.0.8
  - @takumi-rs/core@1.0.8
  - @takumi-rs/wasm@1.0.8

## 1.0.7

### Patch Changes

- 6e9b163: Fix stack overflow when inline-block presented
  - @takumi-rs/core@1.0.7
  - @takumi-rs/wasm@1.0.7
  - @takumi-rs/helpers@1.0.7

## 1.0.6

### Patch Changes

- @takumi-rs/core@1.0.6
- @takumi-rs/wasm@1.0.6
- @takumi-rs/helpers@1.0.6

## 1.0.5

### Patch Changes

- d113fb5: Fix HTML not decoded
- Updated dependencies [d113fb5]
  - @takumi-rs/helpers@1.0.5
  - @takumi-rs/core@1.0.5
  - @takumi-rs/wasm@1.0.5

## 1.0.4

### Patch Changes

- @takumi-rs/core@1.0.4
- @takumi-rs/wasm@1.0.4
- @takumi-rs/helpers@1.0.4

## 1.0.3

### Patch Changes

- @takumi-rs/core@1.0.3
- @takumi-rs/wasm@1.0.3
- @takumi-rs/helpers@1.0.3

## 1.0.2

### Patch Changes

- @takumi-rs/core@1.0.2
- @takumi-rs/wasm@1.0.2
- @takumi-rs/helpers@1.0.2

## 1.0.1

### Patch Changes

- 0401db2: Fix `fonts` not accepting Node.js `Buffer`
  - @takumi-rs/core@1.0.1
  - @takumi-rs/wasm@1.0.1
  - @takumi-rs/helpers@1.0.1

## 1.0.0

### Major Changes

- 30e06f9: Release all-in-one package

### Minor Changes

- f637f3b: Set default emoji source to "twemoji"
- 1373f0a: Support `ico` format

### Patch Changes

- d8e5e75: Set default `ImageResponse` format to png
- 1a4c366: Avoid importing wasm binary in Node environment
- f94f541: Fix wasm initialization
- ddb8245: Fix edge runtime check
- 32b38c3: Fix readme logo
- Updated internal dependencies
  - @takumi-rs/wasm@1.0.0
  - @takumi-rs/helpers@1.0.0
  - @takumi-rs/core@1.0.0

## 1.0.0-rc.17

### Patch Changes

- Updated dependencies [6767ad9]
  - @takumi-rs/helpers@1.0.0-rc.17
  - @takumi-rs/core@1.0.0-rc.17
  - @takumi-rs/wasm@1.0.0-rc.17

## 1.0.0-rc.16

### Patch Changes

- d8e5e75: Set default `ImageResponse` format to png
  - @takumi-rs/core@1.0.0-rc.16
  - @takumi-rs/wasm@1.0.0-rc.16
  - @takumi-rs/helpers@1.0.0-rc.16

## 1.0.0-rc.15

### Patch Changes

- 1a4c366: Avoid importing wasm binary in Node environment
  - @takumi-rs/core@1.0.0-rc.15
  - @takumi-rs/wasm@1.0.0-rc.15
  - @takumi-rs/helpers@1.0.0-rc.15

## 1.0.0-rc.14

### Patch Changes

- Updated dependencies [eb34add]
  - @takumi-rs/helpers@1.0.0-rc.14
  - @takumi-rs/core@1.0.0-rc.14
  - @takumi-rs/wasm@1.0.0-rc.14

## 1.0.0-rc.13

### Patch Changes

- @takumi-rs/core@1.0.0-rc.13
- @takumi-rs/wasm@1.0.0-rc.13
- @takumi-rs/helpers@1.0.0-rc.13

## 1.0.0-rc.12

### Patch Changes

- @takumi-rs/core@1.0.0-rc.12
- @takumi-rs/wasm@1.0.0-rc.12
- @takumi-rs/helpers@1.0.0-rc.12

## 1.0.0-rc.11

### Patch Changes

- Updated dependencies [b2e304a]
- Updated dependencies [b2e304a]
  - @takumi-rs/helpers@1.0.0-rc.11
  - @takumi-rs/core@1.0.0-rc.11
  - @takumi-rs/wasm@1.0.0-rc.11

## 1.0.0-rc.10

### Patch Changes

- Updated dependencies [cc9e63c]
  - @takumi-rs/helpers@1.0.0-rc.10
  - @takumi-rs/core@1.0.0-rc.10
  - @takumi-rs/wasm@1.0.0-rc.10

## 1.0.0-rc.9

### Patch Changes

- @takumi-rs/core@1.0.0-rc.9
- @takumi-rs/wasm@1.0.0-rc.9
- @takumi-rs/helpers@1.0.0-rc.9

## 1.0.0-rc.8

### Patch Changes

- ddb8245: Fix edge runtime check
  - @takumi-rs/core@1.0.0-rc.8
  - @takumi-rs/wasm@1.0.0-rc.8
  - @takumi-rs/helpers@1.0.0-rc.8

## 1.0.0-rc.7

### Patch Changes

- f94f541: Fix wasm initialization
  - @takumi-rs/core@1.0.0-rc.7
  - @takumi-rs/wasm@1.0.0-rc.7
  - @takumi-rs/helpers@1.0.0-rc.7

## 1.0.0-rc.6

### Minor Changes

- f637f3b: Set default emoji source to "twemoji"

### Patch Changes

- @takumi-rs/core@1.0.0-rc.6
- @takumi-rs/wasm@1.0.0-rc.6
- @takumi-rs/helpers@1.0.0-rc.6

## 1.0.0-rc.5

### Patch Changes

- @takumi-rs/core@1.0.0-rc.5
- @takumi-rs/wasm@1.0.0-rc.5
- @takumi-rs/helpers@1.0.0-rc.5

## 1.0.0-rc.4

### Patch Changes

- Updated dependencies [7ff886b]
- Updated dependencies [7ff886b]
  - @takumi-rs/helpers@1.0.0-rc.4
  - @takumi-rs/core@1.0.0-rc.4
  - @takumi-rs/wasm@1.0.0-rc.4

## 1.0.0-rc.3

### Patch Changes

- Updated dependencies [bc6243a]
- Updated dependencies [532bc96]
  - @takumi-rs/wasm@1.0.0-rc.3
  - @takumi-rs/core@1.0.0-rc.3
  - @takumi-rs/helpers@1.0.0-rc.3

## 1.0.0-rc.2

### Patch Changes

- Updated dependencies [26b5557]
  - @takumi-rs/core@1.0.0-rc.2
  - @takumi-rs/wasm@1.0.0-rc.2
  - @takumi-rs/helpers@1.0.0-rc.2

## 1.0.0-rc.1

### Patch Changes

- 32b38c3: Fix readme logo
  - @takumi-rs/core@1.0.0-rc.1
  - @takumi-rs/wasm@1.0.0-rc.1
  - @takumi-rs/helpers@1.0.0-rc.1

## 1.0.0-rc.0

### Major Changes

- 30e06f9: Release all-in-one package

### Patch Changes

- @takumi-rs/core@1.0.0-rc.0
- @takumi-rs/wasm@1.0.0-rc.0
- @takumi-rs/helpers@1.0.0-rc.0
