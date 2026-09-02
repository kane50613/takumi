## @takumi-rs/image-response@2.13.5

### Resolve a browser-only entry in client builds

Bundlers that resolve the `browser` condition (Vite client, webpack web) now
get `bundlers/browser.mjs`, which only fetches the `.wasm` asset by `import.meta.url`. Client builds
with `noExternal` stop failing with `Cannot bundle Node.js built-in "node:fs/promises"`.
The Vite server entry reads the asset through `process.getBuiltinModule`, so
no bundler sees a Node import. These packages, plus `takumi-js` and `@takumi-rs/image-response` on top of them, now require Node 20.19 or newer.

## @takumi-rs/image-response@2.0.0

### Drop the `./wasm` export

`@takumi-rs/image-response/wasm` aliased the same file as the root entry. Import from
`@takumi-rs/image-response`.

## @takumi-rs/image-response@2.0.0-rc.6 (rc)

### Drop the `./wasm` export

`@takumi-rs/image-response/wasm` aliased the same file as the root entry. Import from
`@takumi-rs/image-response`.

## @takumi-rs/image-response@2.0.0-beta.5 (beta)

### Fix `workspace:*` leaking into the published `package.json`

Published packages shipped their inter-package dependencies as the literal
`workspace:*` range, so installing them failed with `Workspace dependency
"@takumi-rs/core" not found`. The publish step now resolves `workspace:` ranges
to concrete versions, matching what `bun` and `pnpm publish` already do.

## @takumi-rs/image-response@2.0.0-beta.4 (beta)

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

## @takumi-rs/image-response@2.0.0-beta.2 (beta)

### Expand subset font-family without borrowing the font context

A render holds the font context borrowed while it builds text layout, so per-element
`font-family` skipped subset-group expansion and routing relied on the fallback bucket, which
followed `fontique`'s hash iteration order. Content registered with `subset_of` (Rust) or
`subsetOf` (JS) could route to the wrong subset, and the result changed between renders.
Expansion now reads the subset groups without that borrow, and the fallback bucket follows
registration order.

## @takumi-rs/image-response@2.0.0-beta.1 (beta)

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

## @takumi-rs/image-response@2.0.0-beta.0 (beta)

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

# @takumi-rs/image-response

## 1.8.7

### Patch Changes

- 9604fd7: Split package export types per import/require condition so CJS consumers resolve `.d.cts`
- Updated dependencies [9604fd7]
  - takumi-js@1.8.7

## 1.8.6

### Patch Changes

- takumi-js@1.8.6

## 1.8.5

### Patch Changes

- takumi-js@1.8.5

## 1.8.4

### Patch Changes

- takumi-js@1.8.4

## 1.8.3

### Patch Changes

- takumi-js@1.8.3

## 1.8.2

### Patch Changes

- takumi-js@1.8.2

## 1.8.1

### Patch Changes

- Updated dependencies [55b058d]
  - takumi-js@1.8.1

## 1.8.0

### Patch Changes

- takumi-js@1.8.0

## 1.7.0

### Patch Changes

- 45a7f4a: Correct `onError` example in README
- Updated dependencies [89a3088]
- Updated dependencies [56579a2]
- Updated dependencies [89a3088]
  - takumi-js@1.7.0

## 1.6.0

### Patch Changes

- takumi-js@1.6.0

## 1.5.1

### Patch Changes

- takumi-js@1.5.1

## 1.5.0

### Patch Changes

- takumi-js@1.5.0

## 1.4.1

### Patch Changes

- Updated dependencies [d6936e5]
  - takumi-js@1.4.1

## 1.4.0

### Patch Changes

- takumi-js@1.4.0

## 1.3.0

### Patch Changes

- takumi-js@1.3.0

## 1.2.1

### Patch Changes

- takumi-js@1.2.1

## 1.2.0

### Patch Changes

- takumi-js@1.2.0

## 1.1.2

### Patch Changes

- takumi-js@1.1.2

## 1.1.1

### Patch Changes

- takumi-js@1.1.1

## 1.1.0

### Patch Changes

- takumi-js@1.1.0

## 1.0.16

### Patch Changes

- takumi-js@1.0.16

## 1.0.15

### Patch Changes

- Updated dependencies [3f96f48]
  - takumi-js@1.0.15

## 1.0.14

### Patch Changes

- Updated dependencies [6323299]
  - takumi-js@1.0.14

## 1.0.13

### Patch Changes

- Updated dependencies [ccfaff3]
  - takumi-js@1.0.13

## 1.0.12

### Patch Changes

- takumi-js@1.0.12

## 1.0.11

### Patch Changes

- takumi-js@1.0.11

## 1.0.10

### Patch Changes

- takumi-js@1.0.10

## 1.0.9

### Patch Changes

- takumi-js@1.0.9

## 1.0.8

### Patch Changes

- takumi-js@1.0.8

## 1.0.7

### Patch Changes

- Updated dependencies [6e9b163]
  - takumi-js@1.0.7

## 1.0.6

### Patch Changes

- takumi-js@1.0.6

## 1.0.5

### Patch Changes

- Updated dependencies [d113fb5]
  - takumi-js@1.0.5

## 1.0.4

### Patch Changes

- takumi-js@1.0.4

## 1.0.3

### Patch Changes

- takumi-js@1.0.3

## 1.0.2

### Patch Changes

- takumi-js@1.0.2

## 1.0.1

### Patch Changes

- Updated dependencies [0401db2]
  - takumi-js@1.0.1

## 1.0.0

### Major Changes

- 8566f15: **Unify both Node.js & WASM runtime**

  No longer need to choose which runtime will be used, and the WASM module will be resolved automatically.

  The existing `@takumi-rs/image-response/wasm` export will continue to work as an alias.

### Minor Changes

- 7c16cb5: Add `createImageResponse` factory API
- 9a451dd: Support passing function for `fonts` and `persistentImages` to avoid singleton pattern
- 3142b36: Add `emoji` option

### Patch Changes

- b9841a7: Fix missing dist folder
- 4cb7e23: Add `onError` option & `ready` promise
- 8770210: Skip resolving core package if Workers/edge runtime detected
- Updated internal dependencies
  - takumi-js@1.0.0

## 1.0.0-rc.17

### Patch Changes

- takumi-js@1.0.0-rc.17

## 1.0.0-rc.16

### Patch Changes

- Updated dependencies [d8e5e75]
  - takumi-js@1.0.0-rc.16

## 1.0.0-rc.15

### Patch Changes

- Updated dependencies [1a4c366]
  - takumi-js@1.0.0-rc.15

## 1.0.0-rc.14

### Patch Changes

- takumi-js@1.0.0-rc.14

## 1.0.0-rc.13

### Patch Changes

- takumi-js@1.0.0-rc.13

## 1.0.0-rc.12

### Patch Changes

- takumi-js@1.0.0-rc.12

## 1.0.0-rc.11

### Patch Changes

- takumi-js@1.0.0-rc.11

## 1.0.0-rc.10

### Patch Changes

- takumi-js@1.0.0-rc.10

## 1.0.0-rc.9

### Patch Changes

- takumi-js@1.0.0-rc.9

## 1.0.0-rc.8

### Patch Changes

- Updated dependencies [ddb8245]
  - takumi-js@1.0.0-rc.8

## 1.0.0-rc.7

### Patch Changes

- Updated dependencies [f94f541]
  - takumi-js@1.0.0-rc.7

## 1.0.0-rc.6

### Patch Changes

- Updated dependencies [f637f3b]
  - takumi-js@1.0.0-rc.6

## 1.0.0-rc.5

### Patch Changes

- takumi-js@1.0.0-rc.5

## 1.0.0-rc.4

### Patch Changes

- b9841a7: Fix missing dist folder
  - takumi-js@1.0.0-rc.4

## 1.0.0-rc.3

### Patch Changes

- takumi-js@1.0.0-rc.3

## 1.0.0-rc.2

### Patch Changes

- takumi-js@1.0.0-rc.2

## 1.0.0-rc.1

### Patch Changes

- Updated dependencies [32b38c3]
  - takumi-js@1.0.0-rc.1

## 1.0.0-rc.0

### Patch Changes

- Updated dependencies [30e06f9]
  - takumi-js@1.0.0-rc.0

## 1.0.0-beta.20

### Patch Changes

- Updated dependencies [01c4fa3]
  - @takumi-rs/helpers@1.0.0-beta.20
  - @takumi-rs/core@1.0.0-beta.20
  - @takumi-rs/wasm@1.0.0-beta.20

## 1.0.0-beta.19

### Patch Changes

- Updated dependencies [79c0c5a]
  - @takumi-rs/wasm@1.0.0-beta.19
  - @takumi-rs/core@1.0.0-beta.19
  - @takumi-rs/helpers@1.0.0-beta.19

## 1.0.0-beta.18

### Patch Changes

- Updated dependencies [0e14dd5]
  - @takumi-rs/core@1.0.0-beta.18
  - @takumi-rs/wasm@1.0.0-beta.18
  - @takumi-rs/helpers@1.0.0-beta.18

## 1.0.0-beta.17

### Patch Changes

- Updated dependencies [da2d85f]
  - @takumi-rs/wasm@1.0.0-beta.17
  - @takumi-rs/core@1.0.0-beta.17
  - @takumi-rs/helpers@1.0.0-beta.17

## 1.0.0-beta.16

### Patch Changes

- 8770210: Skip resolving core package if Workers/edge runtime detected
  - @takumi-rs/core@1.0.0-beta.16
  - @takumi-rs/wasm@1.0.0-beta.16
  - @takumi-rs/helpers@1.0.0-beta.16

## 1.0.0-beta.15

### Patch Changes

- 4cb7e23: Add `onError` option & `ready` promise
  - @takumi-rs/core@1.0.0-beta.15
  - @takumi-rs/wasm@1.0.0-beta.15
  - @takumi-rs/helpers@1.0.0-beta.15

## 1.0.0-beta.14

### Minor Changes

- 7c16cb5: Add `createImageResponse` factory API
- 9a451dd: Support passing function for `fonts` and `persistentImages` to avoid singleton pattern

### Patch Changes

- @takumi-rs/core@1.0.0-beta.14
- @takumi-rs/wasm@1.0.0-beta.14
- @takumi-rs/helpers@1.0.0-beta.14

## 1.0.0-beta.13

### Patch Changes

- Updated dependencies [2f6c8b5]
  - @takumi-rs/core@1.0.0-beta.13
  - @takumi-rs/wasm@1.0.0-beta.13
  - @takumi-rs/helpers@1.0.0-beta.13

## 1.0.0-beta.12

### Patch Changes

- Updated dependencies [6079e79]
  - @takumi-rs/core@1.0.0-beta.12
  - @takumi-rs/wasm@1.0.0-beta.12
  - @takumi-rs/helpers@1.0.0-beta.12

## 1.0.0-beta.11

### Patch Changes

- Updated dependencies [111dd88]
  - @takumi-rs/core@1.0.0-beta.11
  - @takumi-rs/wasm@1.0.0-beta.11
  - @takumi-rs/helpers@1.0.0-beta.11

## 1.0.0-beta.10

### Major Changes

- 8566f15: **Unify both Node.js & WASM runtime**

  No longer to choose what runtime to be used, and wasm module will be resolved automatically.

  The existing `@takumi-rs/image-response/wasm` export will continue to work as an alias.

### Patch Changes

- Updated dependencies [8566f15]
  - @takumi-rs/core@1.0.0-beta.10
  - @takumi-rs/wasm@1.0.0-beta.10
  - @takumi-rs/helpers@1.0.0-beta.10

## 1.0.0-beta.9

### Patch Changes

- @takumi-rs/core@1.0.0-beta.9
- @takumi-rs/wasm@1.0.0-beta.9
- @takumi-rs/helpers@1.0.0-beta.9

## 1.0.0-beta.8

### Patch Changes

- Updated dependencies [9b411ce]
  - @takumi-rs/wasm@1.0.0-beta.8
  - @takumi-rs/core@1.0.0-beta.8
  - @takumi-rs/helpers@1.0.0-beta.8

## 1.0.0-beta.7

### Patch Changes

- @takumi-rs/core@1.0.0-beta.7
- @takumi-rs/wasm@1.0.0-beta.7
- @takumi-rs/helpers@1.0.0-beta.7

## 1.0.0-beta.6

### Patch Changes

- @takumi-rs/core@1.0.0-beta.6
- @takumi-rs/wasm@1.0.0-beta.6
- @takumi-rs/helpers@1.0.0-beta.6

## 1.0.0-beta.5

### Patch Changes

- @takumi-rs/core@1.0.0-beta.5
- @takumi-rs/wasm@1.0.0-beta.5
- @takumi-rs/helpers@1.0.0-beta.5

## 1.0.0-beta.4

### Patch Changes

- Updated dependencies [f1b6104]
  - @takumi-rs/helpers@1.0.0-beta.4
  - @takumi-rs/core@1.0.0-beta.4
  - @takumi-rs/wasm@1.0.0-beta.4

## 1.0.0-beta.3

### Patch Changes

- Updated dependencies [b5b8531]
  - @takumi-rs/helpers@1.0.0-beta.3
  - @takumi-rs/core@1.0.0-beta.3
  - @takumi-rs/wasm@1.0.0-beta.3

## 1.0.0-beta.2

### Minor Changes

- 3142b36: Add `emoji` option

### Patch Changes

- Updated dependencies [3142b36]
  - @takumi-rs/helpers@1.0.0-beta.2
  - @takumi-rs/core@1.0.0-beta.2
  - @takumi-rs/wasm@1.0.0-beta.2

## 1.0.0-beta.1

### Patch Changes

- Updated dependencies [256ef21]
  - @takumi-rs/core@1.0.0-beta.1
  - @takumi-rs/wasm@1.0.0-beta.1
  - @takumi-rs/helpers@1.0.0-beta.1

## 1.0.0-beta.0

### Patch Changes

- Updated dependencies [188079f]
- Updated dependencies [188079f]
- Updated dependencies [188079f]
  - @takumi-rs/core@1.0.0-beta.0
  - @takumi-rs/wasm@1.0.0-beta.0
  - @takumi-rs/helpers@1.0.0-beta.0

## 0.73.1

### Patch Changes

- @takumi-rs/core@0.73.1
- @takumi-rs/wasm@0.73.1
- @takumi-rs/helpers@0.73.1

## 0.73.0

### Patch Changes

- Updated dependencies [349636a]
  - @takumi-rs/helpers@0.73.0
  - @takumi-rs/core@0.73.0
  - @takumi-rs/wasm@0.73.0

## 0.72.0

### Patch Changes

- @takumi-rs/core@0.72.0
- @takumi-rs/wasm@0.72.0
- @takumi-rs/helpers@0.72.0

## 0.71.7

### Patch Changes

- @takumi-rs/core@0.71.7
- @takumi-rs/wasm@0.71.7
- @takumi-rs/helpers@0.71.7

## 0.71.6

### Patch Changes

- Updated dependencies [1a111a6]
  - @takumi-rs/core@0.71.6
  - @takumi-rs/wasm@0.71.6
  - @takumi-rs/helpers@0.71.6

## 0.71.5

### Patch Changes

- @takumi-rs/core@0.71.5
- @takumi-rs/wasm@0.71.5
- @takumi-rs/helpers@0.71.5

## 0.71.4

### Patch Changes

- @takumi-rs/core@0.71.4
- @takumi-rs/wasm@0.71.4
- @takumi-rs/helpers@0.71.4

## 0.71.3

### Patch Changes

- a279b4c: Add `dithering` option for smoother gradients
- Updated dependencies [a279b4c]
  - @takumi-rs/core@0.71.3
  - @takumi-rs/wasm@0.71.3
  - @takumi-rs/helpers@0.71.3

## 0.71.2

### Patch Changes

- @takumi-rs/core@0.71.2
- @takumi-rs/wasm@0.71.2
- @takumi-rs/helpers@0.71.2

## 0.71.1

### Patch Changes

- Updated dependencies [a22b234]
  - @takumi-rs/core@0.71.1
  - @takumi-rs/wasm@0.71.1
  - @takumi-rs/helpers@0.71.1

## 0.71.0

### Patch Changes

- Updated dependencies [0930cdb]
- Updated dependencies [812029d]
  - @takumi-rs/core@0.71.0
  - @takumi-rs/wasm@0.71.0
  - @takumi-rs/helpers@0.71.0

## 0.70.4

### Patch Changes

- @takumi-rs/core@0.70.4
- @takumi-rs/wasm@0.70.4
- @takumi-rs/helpers@0.70.4

## 0.70.3

### Patch Changes

- Updated dependencies [a6fdb08]
  - @takumi-rs/core@0.70.3
  - @takumi-rs/wasm@0.70.3
  - @takumi-rs/helpers@0.70.3

## 0.70.2

### Patch Changes

- Updated dependencies [7270512]
  - @takumi-rs/core@0.70.2
  - @takumi-rs/wasm@0.70.2
  - @takumi-rs/helpers@0.70.2

## 0.70.1

### Patch Changes

- @takumi-rs/core@0.70.1
- @takumi-rs/wasm@0.70.1
- @takumi-rs/helpers@0.70.1

## 0.70.0

### Patch Changes

- Updated dependencies [fb8ccf8]
- Updated dependencies [a69dd7d]
  - @takumi-rs/core@0.70.0
  - @takumi-rs/wasm@0.70.0
  - @takumi-rs/helpers@0.70.0

## 0.69.5

### Patch Changes

- @takumi-rs/core@0.69.5
- @takumi-rs/wasm@0.69.5
- @takumi-rs/helpers@0.69.5

## 0.69.4

### Patch Changes

- @takumi-rs/core@0.69.4
- @takumi-rs/wasm@0.69.4
- @takumi-rs/helpers@0.69.4

## 0.69.3

### Patch Changes

- @takumi-rs/core@0.69.3
- @takumi-rs/wasm@0.69.3
- @takumi-rs/helpers@0.69.3

## 0.69.2

### Patch Changes

- @takumi-rs/core@0.69.2
- @takumi-rs/wasm@0.69.2
- @takumi-rs/helpers@0.69.2

## 0.69.1

### Patch Changes

- @takumi-rs/core@0.69.1
- @takumi-rs/wasm@0.69.1
- @takumi-rs/helpers@0.69.1

## 0.69.0

### Patch Changes

- Updated dependencies [62525f9]
- Updated dependencies [12034df]
  - @takumi-rs/helpers@0.69.0
  - @takumi-rs/core@0.69.0
  - @takumi-rs/wasm@0.69.0

## 0.68.17

### Patch Changes

- @takumi-rs/core@0.68.17
- @takumi-rs/wasm@0.68.17
- @takumi-rs/helpers@0.68.17

## 0.68.16

### Patch Changes

- 5dcd679: specify `type: "bytes"` in `ReadableStream` construction
  - @takumi-rs/core@0.68.16
  - @takumi-rs/wasm@0.68.16
  - @takumi-rs/helpers@0.68.16

## 0.68.15

### Patch Changes

- @takumi-rs/core@0.68.15
- @takumi-rs/wasm@0.68.15
- @takumi-rs/helpers@0.68.15

## 0.68.14

### Patch Changes

- c5b8029: avoid duplicated `init` calls
  - @takumi-rs/core@0.68.14
  - @takumi-rs/wasm@0.68.14
  - @takumi-rs/helpers@0.68.14

## 0.68.13

### Patch Changes

- @takumi-rs/core@0.68.13
- @takumi-rs/wasm@0.68.13
- @takumi-rs/helpers@0.68.13

## 0.68.12

### Patch Changes

- @takumi-rs/core@0.68.12
- @takumi-rs/wasm@0.68.12
- @takumi-rs/helpers@0.68.12

## 0.68.11

### Patch Changes

- @takumi-rs/core@0.68.11
- @takumi-rs/wasm@0.68.11
- @takumi-rs/helpers@0.68.11

## 0.68.10

### Patch Changes

- @takumi-rs/core@0.68.10
- @takumi-rs/wasm@0.68.10
- @takumi-rs/helpers@0.68.10

## 0.68.9

### Patch Changes

- @takumi-rs/core@0.68.9
- @takumi-rs/wasm@0.68.9
- @takumi-rs/helpers@0.68.9

## 0.68.8

### Patch Changes

- @takumi-rs/core@0.68.8
- @takumi-rs/wasm@0.68.8
- @takumi-rs/helpers@0.68.8

## 0.68.7

### Patch Changes

- @takumi-rs/core@0.68.7
- @takumi-rs/wasm@0.68.7
- @takumi-rs/helpers@0.68.7

## 0.68.6

### Patch Changes

- @takumi-rs/core@0.68.6
- @takumi-rs/wasm@0.68.6
- @takumi-rs/helpers@0.68.6

## 0.68.5

### Patch Changes

- @takumi-rs/core@0.68.5
- @takumi-rs/wasm@0.68.5
- @takumi-rs/helpers@0.68.5

## 0.68.4

### Patch Changes

- @takumi-rs/core@0.68.4
- @takumi-rs/wasm@0.68.4
- @takumi-rs/helpers@0.68.4

## 0.68.3

### Patch Changes

- @takumi-rs/core@0.68.3
- @takumi-rs/wasm@0.68.3
- @takumi-rs/helpers@0.68.3

## 0.68.2

### Patch Changes

- Updated dependencies [f58f974]
- Updated dependencies [f58f974]
  - @takumi-rs/helpers@0.68.2
  - @takumi-rs/core@0.68.2
  - @takumi-rs/wasm@0.68.2

## 0.68.1

### Patch Changes

- @takumi-rs/core@0.68.1
- @takumi-rs/wasm@0.68.1
- @takumi-rs/helpers@0.68.1

## 0.68.0

### Patch Changes

- Updated dependencies [7684faa]
- Updated dependencies [7684faa]
  - @takumi-rs/wasm@0.68.0
  - @takumi-rs/core@0.68.0
  - @takumi-rs/helpers@0.68.0

## 0.67.3

### Patch Changes

- Updated dependencies [06b118d]
  - @takumi-rs/helpers@0.67.3
  - @takumi-rs/core@0.67.3
  - @takumi-rs/wasm@0.67.3

## 0.67.2

### Patch Changes

- Updated dependencies [ca61b5e]
- Updated dependencies [e8cc16c]
  - @takumi-rs/wasm@0.67.2
  - @takumi-rs/core@0.67.2
  - @takumi-rs/helpers@0.67.2

## 0.67.1

### Patch Changes

- @takumi-rs/core@0.67.1
- @takumi-rs/wasm@0.67.1
- @takumi-rs/helpers@0.67.1

## 0.67.0

### Patch Changes

- @takumi-rs/core@0.67.0
- @takumi-rs/wasm@0.67.0
- @takumi-rs/helpers@0.67.0

## 0.66.14

### Patch Changes

- @takumi-rs/core@0.66.14
- @takumi-rs/wasm@0.66.14
- @takumi-rs/helpers@0.66.14

## 0.66.13

### Patch Changes

- Updated dependencies [c97a402]
  - @takumi-rs/wasm@0.66.13
  - @takumi-rs/core@0.66.13
  - @takumi-rs/helpers@0.66.13

## 0.66.12

### Patch Changes

- 499280d: do not require `module` if `renderer` is provided
- Updated dependencies [7389d6e]
  - @takumi-rs/core@0.66.12
  - @takumi-rs/wasm@0.66.12
  - @takumi-rs/helpers@0.66.12

## 0.66.11

### Patch Changes

- @takumi-rs/core@0.66.11
- @takumi-rs/wasm@0.66.11
- @takumi-rs/helpers@0.66.11

## 0.66.10

### Patch Changes

- @takumi-rs/core@0.66.10
- @takumi-rs/wasm@0.66.10
- @takumi-rs/helpers@0.66.10

## 0.66.9

### Patch Changes

- @takumi-rs/core@0.66.9
- @takumi-rs/wasm@0.66.9
- @takumi-rs/helpers@0.66.9

## 0.66.8

### Patch Changes

- @takumi-rs/core@0.66.8
- @takumi-rs/wasm@0.66.8
- @takumi-rs/helpers@0.66.8

## 0.66.7

### Patch Changes

- bf2db74: avoid overriding `format` field for wasm version
- Updated dependencies [a3e7f9c]
- Updated dependencies [a3e7f9c]
  - @takumi-rs/wasm@0.66.7
  - @takumi-rs/core@0.66.7
  - @takumi-rs/helpers@0.66.7

## 0.66.6

### Patch Changes

- @takumi-rs/core@0.66.6
- @takumi-rs/wasm@0.66.6
- @takumi-rs/helpers@0.66.6

## 0.66.5

### Patch Changes

- @takumi-rs/core@0.66.5
- @takumi-rs/wasm@0.66.5
- @takumi-rs/helpers@0.66.5

## 0.66.4

### Patch Changes

- @takumi-rs/core@0.66.4
- @takumi-rs/wasm@0.66.4
- @takumi-rs/helpers@0.66.4

## 0.66.3

### Patch Changes

- @takumi-rs/core@0.66.3
- @takumi-rs/wasm@0.66.3
- @takumi-rs/helpers@0.66.3

## 0.66.2

### Patch Changes

- Updated dependencies [291e5cc]
  - @takumi-rs/helpers@0.66.2
  - @takumi-rs/core@0.66.2
  - @takumi-rs/wasm@0.66.2

## 0.66.1

### Patch Changes

- @takumi-rs/core@0.66.1
- @takumi-rs/wasm@0.66.1
- @takumi-rs/helpers@0.66.1

## 0.66.0

### Patch Changes

- Updated dependencies [80da5a7]
- Updated dependencies [5b7ea89]
- Updated dependencies [f811582]
- Updated dependencies [94d7959]
  - @takumi-rs/core@0.66.0
  - @takumi-rs/wasm@0.66.0
  - @takumi-rs/helpers@0.66.0

## 0.65.0

### Patch Changes

- Updated dependencies [1319540]
  - @takumi-rs/core@0.65.0
  - @takumi-rs/wasm@0.65.0
  - @takumi-rs/helpers@0.65.0

## 0.64.1

### Patch Changes

- Updated dependencies [0dc36ce]
  - @takumi-rs/core@0.64.1
  - @takumi-rs/wasm@0.64.1
  - @takumi-rs/helpers@0.64.1

## 0.64.0

### Patch Changes

- Updated dependencies [1600ff0]
- Updated dependencies [1600ff0]
- Updated dependencies [1600ff0]
- Updated dependencies [1600ff0]
  - @takumi-rs/wasm@0.64.0
  - @takumi-rs/core@0.64.0
  - @takumi-rs/helpers@0.64.0

## 0.63.2

### Patch Changes

- @takumi-rs/core@0.63.2
- @takumi-rs/wasm@0.63.2
- @takumi-rs/helpers@0.63.2

## 0.63.1

### Patch Changes

- Updated dependencies [9fb085f]
  - @takumi-rs/core@0.63.1
  - @takumi-rs/wasm@0.63.1
  - @takumi-rs/helpers@0.63.1

## 0.63.0

### Patch Changes

- Updated dependencies [87a12b4]
- Updated dependencies [75b0f10]
- Updated dependencies [ec9708d]
  - @takumi-rs/wasm@0.63.0
  - @takumi-rs/core@0.63.0
  - @takumi-rs/helpers@0.63.0

## 0.62.8

### Patch Changes

- @takumi-rs/core@0.62.8
- @takumi-rs/wasm@0.62.8
- @takumi-rs/helpers@0.62.8

## 0.62.7

### Patch Changes

- Updated dependencies [43036a0]
- Updated dependencies [54e8bec]
  - @takumi-rs/core@0.62.7
  - @takumi-rs/wasm@0.62.7
  - @takumi-rs/helpers@0.62.7

## 0.62.6

### Patch Changes

- @takumi-rs/core@0.62.6
- @takumi-rs/wasm@0.62.6
- @takumi-rs/helpers@0.62.6

## 0.62.5

### Patch Changes

- @takumi-rs/core@0.62.5
- @takumi-rs/wasm@0.62.5
- @takumi-rs/helpers@0.62.5

## 0.62.4

### Patch Changes

- Updated dependencies [56ec805]
  - @takumi-rs/core@0.62.4
  - @takumi-rs/wasm@0.62.4
  - @takumi-rs/helpers@0.62.4

## 0.62.3

### Patch Changes

- Updated dependencies [e5d41bc]
- Updated dependencies [33c9ba0]
  - @takumi-rs/core@0.62.3
  - @takumi-rs/wasm@0.62.3
  - @takumi-rs/helpers@0.62.3

## 0.62.2

### Patch Changes

- @takumi-rs/core@0.62.2
- @takumi-rs/wasm@0.62.2
- @takumi-rs/helpers@0.62.2

## 0.62.1

### Patch Changes

- @takumi-rs/core@0.62.1
- @takumi-rs/wasm@0.62.1
- @takumi-rs/helpers@0.62.1

## 0.62.0

### Patch Changes

- @takumi-rs/core@0.62.0
- @takumi-rs/wasm@0.62.0
- @takumi-rs/helpers@0.62.0

## 0.61.1

### Patch Changes

- @takumi-rs/core@0.61.1
- @takumi-rs/wasm@0.61.1
- @takumi-rs/helpers@0.61.1

## 0.61.0

### Patch Changes

- @takumi-rs/core@0.61.0
- @takumi-rs/wasm@0.61.0
- @takumi-rs/helpers@0.61.0

## 0.60.8

### Patch Changes

- @takumi-rs/core@0.60.8
- @takumi-rs/wasm@0.60.8
- @takumi-rs/helpers@0.60.8

## 0.60.7

### Patch Changes

- @takumi-rs/core@0.60.7
- @takumi-rs/wasm@0.60.7
- @takumi-rs/helpers@0.60.7

## 0.60.6

### Patch Changes

- @takumi-rs/core@0.60.6
- @takumi-rs/wasm@0.60.6
- @takumi-rs/helpers@0.60.6

## 0.60.5

### Patch Changes

- 66e28c7: fix should default to `webp` if no `format` option provided
  - @takumi-rs/core@0.60.5
  - @takumi-rs/wasm@0.60.5
  - @takumi-rs/helpers@0.60.5

## 0.60.4

### Patch Changes

- @takumi-rs/core@0.60.4
- @takumi-rs/wasm@0.60.4
- @takumi-rs/helpers@0.60.4

## 0.60.3

### Patch Changes

- @takumi-rs/core@0.60.3
- @takumi-rs/wasm@0.60.3
- @takumi-rs/helpers@0.60.3

## 0.60.2

### Patch Changes

- @takumi-rs/core@0.60.2
- @takumi-rs/wasm@0.60.2
- @takumi-rs/helpers@0.60.2

## 0.60.1

### Patch Changes

- @takumi-rs/core@0.60.1
- @takumi-rs/wasm@0.60.1
- @takumi-rs/helpers@0.60.1

## 0.60.0

### Patch Changes

- @takumi-rs/core@0.60.0
- @takumi-rs/wasm@0.60.0
- @takumi-rs/helpers@0.60.0

## 0.59.1

### Patch Changes

- @takumi-rs/core@0.59.1
- @takumi-rs/wasm@0.59.1
- @takumi-rs/helpers@0.59.1

## 0.59.0

### Patch Changes

- @takumi-rs/core@0.59.0
- @takumi-rs/wasm@0.59.0
- @takumi-rs/helpers@0.59.0

## 0.58.0

### Minor Changes

- 3bd63df: add `defaultStyles` option to opt-out from default styles #343

### Patch Changes

- Updated dependencies [0deafbd]
- Updated dependencies [3bd63df]
  - @takumi-rs/helpers@0.58.0
  - @takumi-rs/core@0.58.0
  - @takumi-rs/wasm@0.58.0

## 0.57.6

### Patch Changes

- @takumi-rs/core@0.57.6
- @takumi-rs/wasm@0.57.6
- @takumi-rs/helpers@0.57.6

## 0.57.5

### Patch Changes

- @takumi-rs/core@0.57.5
- @takumi-rs/wasm@0.57.5
- @takumi-rs/helpers@0.57.5

## 0.57.4

### Patch Changes

- @takumi-rs/core@0.57.4
- @takumi-rs/wasm@0.57.4
- @takumi-rs/helpers@0.57.4

## 0.57.3

### Patch Changes

- @takumi-rs/core@0.57.3
- @takumi-rs/wasm@0.57.3
- @takumi-rs/helpers@0.57.3

## 0.57.2

### Patch Changes

- @takumi-rs/core@0.57.2
- @takumi-rs/wasm@0.57.2
- @takumi-rs/helpers@0.57.2

## 0.57.1

### Patch Changes

- @takumi-rs/core@0.57.1
- @takumi-rs/wasm@0.57.1
- @takumi-rs/helpers@0.57.1

## 0.57.0

### Patch Changes

- Updated dependencies [42572bb]
  - @takumi-rs/core@0.57.0
  - @takumi-rs/wasm@0.57.0
  - @takumi-rs/helpers@0.57.0

## 0.56.1

### Patch Changes

- @takumi-rs/core@0.56.1
- @takumi-rs/wasm@0.56.1
- @takumi-rs/helpers@0.56.1

## 0.56.0

### Patch Changes

- @takumi-rs/core@0.56.0
- @takumi-rs/wasm@0.56.0
- @takumi-rs/helpers@0.56.0

## 0.55.4

### Patch Changes

- @takumi-rs/core@0.55.4
- @takumi-rs/wasm@0.55.4
- @takumi-rs/helpers@0.55.4

## 0.55.3

### Patch Changes

- @takumi-rs/core@0.55.3
- @takumi-rs/wasm@0.55.3
- @takumi-rs/helpers@0.55.3

## 0.55.2

### Patch Changes

- @takumi-rs/core@0.55.2
- @takumi-rs/wasm@0.55.2
- @takumi-rs/helpers@0.55.2

## 0.55.1

### Patch Changes

- @takumi-rs/core@0.55.1
- @takumi-rs/wasm@0.55.1
- @takumi-rs/helpers@0.55.1

## 0.55.0

### Patch Changes

- @takumi-rs/core@0.55.0
- @takumi-rs/wasm@0.55.0
- @takumi-rs/helpers@0.55.0

## 0.54.3

### Patch Changes

- Updated dependencies [8c6e17e]
  - @takumi-rs/core@0.54.3
  - @takumi-rs/wasm@0.54.3
  - @takumi-rs/helpers@0.54.3

## 0.54.2

### Patch Changes

- @takumi-rs/core@0.54.2
- @takumi-rs/wasm@0.54.2
- @takumi-rs/helpers@0.54.2

## 0.54.1

### Patch Changes

- @takumi-rs/core@0.54.1
- @takumi-rs/wasm@0.54.1
- @takumi-rs/helpers@0.54.1

## 0.54.0

### Patch Changes

- @takumi-rs/core@0.54.0
- @takumi-rs/wasm@0.54.0
- @takumi-rs/helpers@0.54.0

## 0.53.1

### Patch Changes

- @takumi-rs/core@0.53.1
- @takumi-rs/wasm@0.53.1
- @takumi-rs/helpers@0.53.1

## 0.53.0

### Patch Changes

- Updated dependencies [364fa11]
  - @takumi-rs/helpers@0.53.0
  - @takumi-rs/core@0.53.0
  - @takumi-rs/wasm@0.53.0

## 0.52.2

### Patch Changes

- @takumi-rs/core@0.52.2
- @takumi-rs/wasm@0.52.2
- @takumi-rs/helpers@0.52.2

## 0.52.1

### Patch Changes

- @takumi-rs/core@0.52.1
- @takumi-rs/wasm@0.52.1
- @takumi-rs/helpers@0.52.1

## 0.52.0

### Patch Changes

- Updated dependencies [776a18e]
  - @takumi-rs/helpers@0.52.0
  - @takumi-rs/core@0.52.0
  - @takumi-rs/wasm@0.52.0

## 0.51.1

### Patch Changes

- @takumi-rs/core@0.51.1
- @takumi-rs/wasm@0.51.1
- @takumi-rs/helpers@0.51.1

## 0.51.0

### Patch Changes

- Updated dependencies [27ac6c5]
  - @takumi-rs/core@0.51.0
  - @takumi-rs/wasm@0.51.0
  - @takumi-rs/helpers@0.51.0

## 0.50.0

### Minor Changes

- d02bb3e: **drop `initWasm`**, add `module` option

### Patch Changes

- @takumi-rs/core@0.50.0
- @takumi-rs/wasm@0.50.0
- @takumi-rs/helpers@0.50.0

## 0.49.1

### Patch Changes

- Updated dependencies [569e8f8]
  - @takumi-rs/wasm@0.49.1
  - @takumi-rs/core@0.49.1
  - @takumi-rs/helpers@0.49.1

## 0.49.0

### Minor Changes

- 4fddbd2: export `initWasm`, `initWasmSync` method

### Patch Changes

- Updated dependencies [4fddbd2]
  - @takumi-rs/wasm@0.49.0
  - @takumi-rs/core@0.49.0
  - @takumi-rs/helpers@0.49.0

## 0.48.0

### Patch Changes

- Updated dependencies [c3f1b7d]
  - @takumi-rs/core@0.48.0
  - @takumi-rs/wasm@0.48.0
  - @takumi-rs/helpers@0.48.0

## 0.47.0

### Patch Changes

- @takumi-rs/core@0.47.0
- @takumi-rs/wasm@0.47.0
- @takumi-rs/helpers@0.47.0

## 0.46.6

### Patch Changes

- Updated dependencies [73c07ff]
- Updated dependencies [73c07ff]
  - @takumi-rs/core@0.46.6
  - @takumi-rs/wasm@0.46.6
  - @takumi-rs/helpers@0.46.6

## 0.46.5

### Patch Changes

- Updated dependencies [b97af12]
  - @takumi-rs/core@0.46.5
  - @takumi-rs/wasm@0.46.5
  - @takumi-rs/helpers@0.46.5

## 0.46.4

### Patch Changes

- @takumi-rs/core@0.46.4
- @takumi-rs/wasm@0.46.4
- @takumi-rs/helpers@0.46.4

## 0.46.3

### Patch Changes

- b153ef3: fix dependency version on `prepublishOnly` script
  - @takumi-rs/core@0.46.3
  - @takumi-rs/wasm@0.46.3
  - @takumi-rs/helpers@0.46.3

## 0.46.2

### Patch Changes

- fc6d0cc: replace `peerDependencies` with `publishConfig`
  - @takumi-rs/core@0.46.2
  - @takumi-rs/wasm@0.46.2
  - @takumi-rs/helpers@0.46.2

## 0.46.1

### Patch Changes

- @takumi-rs/core@0.46.1
- @takumi-rs/wasm@0.46.1
- @takumi-rs/helpers@0.46.1

## 0.46.0

### Patch Changes

- @takumi-rs/core@0.46.0
- @takumi-rs/wasm@0.46.0
- @takumi-rs/helpers@0.46.0

## 0.45.3

### Patch Changes

- @takumi-rs/core@0.45.3
- @takumi-rs/wasm@0.45.3
- @takumi-rs/helpers@0.45.3

## 0.45.2

### Patch Changes

- @takumi-rs/core@0.45.2
- @takumi-rs/wasm@0.45.2
- @takumi-rs/helpers@0.45.2

## 0.45.1

### Patch Changes

- @takumi-rs/core@0.45.1
- @takumi-rs/wasm@0.45.1
- @takumi-rs/helpers@0.45.1

## 0.45.0

### Minor Changes

- 702c419: remove `jsx` options

### Patch Changes

- Updated dependencies [702c419]
- Updated dependencies [702c419]
  - @takumi-rs/helpers@0.45.0
  - @takumi-rs/core@0.45.0
  - @takumi-rs/wasm@0.45.0

## 0.44.0

### Patch Changes

- @takumi-rs/core@0.44.0
- @takumi-rs/wasm@0.44.0
- @takumi-rs/helpers@0.44.0

## 0.43.1

### Patch Changes

- Updated dependencies [4bd994f]
- Updated dependencies [4bd994f]
- Updated dependencies [4bd994f]
  - @takumi-rs/helpers@0.43.1
  - @takumi-rs/core@0.43.1
  - @takumi-rs/wasm@0.43.1

## 0.43.0

### Minor Changes

- 3be9d4d: Add `jsx` options support to configure `fromJsx`

### Patch Changes

- Updated dependencies [9247090]
- Updated dependencies [796c578]
  - @takumi-rs/helpers@0.43.0
  - @takumi-rs/core@0.43.0
  - @takumi-rs/wasm@0.43.0

## 0.42.0

### Patch Changes

- 5f32366: Handles assets fetching WASM version
  - @takumi-rs/core@0.42.0
  - @takumi-rs/wasm@0.42.0
  - @takumi-rs/helpers@0.42.0

## 0.41.0

### Minor Changes

- 6653f75: add wasm backend support

### Patch Changes

- @takumi-rs/core@0.41.0
- @takumi-rs/wasm@0.41.0
- @takumi-rs/helpers@0.41.0

## 0.40.2

### Patch Changes

- @takumi-rs/core@0.40.2
- @takumi-rs/helpers@0.40.2

## 0.40.1

### Patch Changes

- @takumi-rs/core@0.40.1
- @takumi-rs/helpers@0.40.1

## 0.40.0

### Patch Changes

- @takumi-rs/core@0.40.0
- @takumi-rs/helpers@0.40.0

## 0.39.0

### Patch Changes

- Updated dependencies [71ae4a5]
  - @takumi-rs/core@0.39.0
  - @takumi-rs/helpers@0.39.0

## 0.38.1

### Patch Changes

- @takumi-rs/core@0.38.1
- @takumi-rs/helpers@0.38.1

## 0.38.0

### Patch Changes

- Updated dependencies [7245e49]
- Updated dependencies [7245e49]
  - @takumi-rs/core@0.38.0
  - @takumi-rs/helpers@0.38.0

## 0.37.0

### Patch Changes

- @takumi-rs/core@0.37.0
- @takumi-rs/helpers@0.37.0

## 0.36.2

### Patch Changes

- @takumi-rs/core@0.36.2
- @takumi-rs/helpers@0.36.2

## 0.36.1

### Patch Changes

- Updated dependencies [f12f11a]
  - @takumi-rs/helpers@0.36.1
  - @takumi-rs/core@0.36.1

## 0.36.0

### Patch Changes

- @takumi-rs/core@0.36.0
- @takumi-rs/helpers@0.36.0

## 0.35.2

### Patch Changes

- @takumi-rs/core@0.35.2
- @takumi-rs/helpers@0.35.2

## 0.35.1

### Patch Changes

- Updated dependencies [f18e9c5]
  - @takumi-rs/core@0.35.1
  - @takumi-rs/helpers@0.35.1

## 0.35.0

### Patch Changes

- Updated dependencies [264fa71]
- Updated dependencies [12a2d3f]
- Updated dependencies [3d6745f]
  - @takumi-rs/helpers@0.35.0
  - @takumi-rs/core@0.35.0

## 0.34.0

### Patch Changes

- 7c402d8: setup npm trusted publisher
- Updated dependencies [7c402d8]
  - @takumi-rs/helpers@0.34.0
  - @takumi-rs/core@0.34.0

## 0.33.1

### Patch Changes

- Updated dependencies [df18b3d]
  - @takumi-rs/core@0.33.1
  - @takumi-rs/helpers@0.33.1

## 0.33.0

### Patch Changes

- Updated dependencies [98755a7]
- Updated dependencies [4635fcb]
  - @takumi-rs/core@0.33.0
  - @takumi-rs/helpers@0.33.0
