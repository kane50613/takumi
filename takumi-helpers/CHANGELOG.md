## @takumi-rs/helpers@2.13.0

### Support the `disclosure-open` and `disclosure-closed` counter styles

`list-style-type` now accepts `disclosure-open` and `disclosure-closed`, drawing the triangles CSS Counter Styles defines. `disclosure-closed` points the way the text runs, so it flips under `direction: rtl`. Font subsetting covers all three characters.

### Enforce fetch policies on shared image cache entries

A shared `fetchCache` hit returned cached bytes without running the calling render's `allowUrl` or `maxBytes`. Cache hits now recheck both. `allowUrl` runs against the entry URL only, not the redirect hops the original fetch followed.

### Write a group of `css` entries as an object

A `css` entry can be `{ media, rules }`, `{ supports, rules }`, or `{ layer, rules }`. A layer without `rules` declares its order alone. Takumi reads each prelude with the grammar its rule takes, so it cannot close the rule and open another.

### Return `css` from `fromJsx` and `fromHtml`

Reading the old `stylesheets` field returns the same array and warns once. It no longer appears in `Object.keys` or a spread of the result.

## @takumi-rs/helpers@2.11.0

### Keep a `;` inside an inline style value

`fromHtml` and `fromStaticMarkup` split the `style` attribute on every `;`, so a value carrying one of its own lost everything after it. `style="background-image:url(data:image/png;base64,...)"` resolved to `url(data:image/png` and rendered nothing. A quoted `font-family: "Foo; Bar"` was cut the same way. Only a `;` outside `url()` and a quoted string now ends a declaration.

### Decode the numeric character references the HTML spec defines

`&#X41;` stayed literal text because the decoder matched only a lower-case `x`, while the spec accepts either case. A reference in the C1 range now resolves through the windows-1252 table the spec names, so `&#153;` renders as `™` rather than an invisible control character, and `&#0;` becomes the replacement character rather than a raw NUL.

## @takumi-rs/helpers@2.10.0

### Lay out tables on the grid algorithm

A `<table>` used to fall back to block layout, so cells stacked instead of forming columns. Table boxes now lower onto a grid whose column tracks are shared by every row. Header groups render first, footer groups last. `colspan` and `rowspan` span tracks, captions render on the side `caption-side` picks, and a row's background lands on its cells. HTML and JSX gain element presets for `table`, `thead`, `tbody`, `tfoot`, `tr`, `td`, `th`, and `caption`.

### Render HTML and CSS list markers in PDF

Paint generated list markers in PDF output, including nested, paginated, and tagged (`Lbl`) lists. Font subsetting counts the characters the predefined marker styles generate in every backend.

## @takumi-rs/helpers@2.9.2

### Treat the synthetic HTML root as a block container

Markup written in a template literal carries whitespace either side, which parsed into text roots. The synthetic root that holds them was inline, so the leading one kept a line box and pushed the content down the page. It is a block container now, the way `<body>` is, and `fromHtml` drops the whitespace roots the way the Rust crate already did.

## @takumi-rs/helpers@2.8.0

### Render fragment, memo and forwardRef children inside `<svg>`

Inline SVG silently dropped any child it could not map to a tag name. Fragments, `memo` and `forwardRef` components now serialize the same way React does.

### Draw list markers for `<ol>` and `<ul>`

List items rendered with no bullet or number. A `display: list-item` box now generates a marker: `list-style-type`, `list-style-position` and `list-style-image` pick what it draws and where it sits, and `<ol start>` and `<li value>` set the count.

## @takumi-rs/helpers@2.7.2

### Follow Unicode emoji presentation in `extractEmojis`

`extractEmojis` replaced every pictograph with a CDN image, so `‼` and `▶` came back as color emoji. Pictographs that default to text presentation now stay text, `U+FE0F` forces the emoji image, and `U+FE0E` forces the text glyph.

## @takumi-rs/helpers@2.7.1

### Collect `<head>` styles and decode character references in `fromHtml`

`fromHtml` skipped the whole `<head>` subtree, so a full document lost every `<style>` rule while the same markup as a fragment kept them. `<style>` tags inside `<head>` now land in `stylesheets` for both `fromHtml` and `fromJsx`. Text nodes also decode HTML character references, so `&nbsp;`, `&deg;` and `&#176;` render as the characters instead of the raw source.

## @takumi-rs/helpers@2.6.0

### Route shared codepoints to the subset that declares them

A Google Fonts subset encodes more than the `unicode-range` it was cut for, and the Cyrillic and Greek ones also carry the ASCII space and the Latin capitals. Selection took the first subset whose glyphs covered a character, in family-name order, so those codepoints left the Latin subset and every word split into separate runs. Subsets now rank by the range they declare, lowest first.

## @takumi-rs/helpers@2.5.8

### Raise the default fetch timeout to 30 seconds

`AbortSignal.timeout` counts wall-clock time, so a 5-second budget aborted otherwise-healthy font fetches whenever heavy synchronous work (SSG, wasm rendering) blocked the event loop past it.

## @takumi-rs/helpers@2.4.2

### Hand font and image bytes to the bindings as Uint8Array views

Fetched fonts and images flowed to the bindings as bare ArrayBuffers, which the native binding copies. Wrapping them in a Uint8Array view costs nothing and takes the zero-copy path.

## @takumi-rs/helpers@2.4.0

### Enforce allowUrl on every redirect hop

`fetchOk` with an `allowUrl` policy now follows redirects manually (capped at 5 hops) and re-checks the resolved target of each hop, so an allowed URL can no longer redirect to a blocked address. Callers without a policy keep default redirect handling.

## @takumi-rs/helpers@2.3.3

### Honour per-element white-space when collapsing inline text

Inline whitespace collapsing read the block's white-space value for every span, so a `white-space: pre` child inside a normal-collapsing parent lost its spaces and line breaks. Each span now collapses against its own value. `<br>` also carries a `white-space: pre` preset, so its line break survives.

## @takumi-rs/helpers@2.3.0

### Accept raw RGBA pixels as an image source

An image node's `src` takes `{ width, height, data, premultiplied? }` and renders the pixels without decoding; the `Bitmap` JSX helper wraps it as an `<img>`.

## @takumi-rs/helpers@2.2.0

### Support generic on googleFonts families

A family's `generic` (e.g. `"monospace"`) propagates to every loaded coverage subset, so generic stacks like `font-mono` resolve to it.

### Claim generic font families from the JS font API

Font descriptors accept `generic` (e.g. `"monospace"`), so stacks like Tailwind's `font-mono` resolve to registered fonts without naming the family.

## @takumi-rs/helpers@2.1.1

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

## @takumi-rs/helpers@2.1.0

### Resolve hooks without react-dom and render Preact trees

`fromJsx` installs a server-semantics hook dispatcher instead of falling back
to `react-dom/server`, handles context providers and consumers natively, and
renders Preact subtrees through `preact-render-to-string`. The `react-dom`
peer dependency is gone; `preact` and `preact-render-to-string` are new
optional peers.

## @takumi-rs/helpers@2.0.2

### Accept a families array in `googleFonts`

Pass the families directly instead of wrapping them in an options object:
`googleFonts(["Inter", "Noto Sans JP"])`. The object form stays for `text`,
`display`, and the other options.

## @takumi-rs/helpers@2.0.1

### Default the Google Fonts CSS cache

`googleFonts` now caches the CSS metadata process-wide when no `cache` is
passed, so callers who omit it still fetch each URL once. Pass your own `Map`
to scope the cache, or a fresh one per call to opt out.

## @takumi-rs/helpers@2.0.0

### Cache the Google Fonts CSS promise

`googleFonts`'s `cache` now stores the in-flight `Promise<string>` instead of
the resolved CSS, so concurrent calls for the same URL share one request
instead of each missing and fetching. A failed fetch evicts itself, so the
next call retries. The cache type is now
`Pick<Map<string, Promise<string>>, "get" | "set" | "delete">`.

### Share the renderer facade between the napi and wasm bindings

The napi and wasm `Renderer` wrappers now build on a shared `prepareRenderInput`
helper in `@takumi-rs/helpers/renderer`, so their option types and render bodies
live in one place. Both backends check `signal` before and after resolving
fonts and images: on native, an already-aborted signal now throws before any
resource fetch.

### Match the Chromium UA stylesheet for default element styles

Parse the relative font keywords `bolder`/`lighter` (`font-weight`) and
`larger`/`smaller` (`font-size`), resolving to the values Chromium uses. Expand
the default element presets to cover lists, `sub`/`sup`, `ins`/`del`, forms,
`details`/`summary`, and `search`.

### Bound remote fetches with byte caps and default timeouts

Remote image, font, and Google Fonts CSS fetches now reject bodies past a byte
cap (`maxBytes`, default 32 MiB; 2 MiB for CSS) and apply the 5 s timeout to
every fetch, not just images. Set `timeout: 0` to disable it. A new `allowUrl`
hook on `FetchOptions` skips URLs it rejects.

### Add the `@takumi-rs/helpers/renderer` entrypoint

The shared renderer wrapper backing the napi and wasm bindings is now exported
as `@takumi-rs/helpers/renderer`.

### Fix `googleFonts` losing your declared family order

`googleFonts` returned subsets in whatever order Google's `css2` response happened to list
`@font-face` blocks in — not the order families were passed in `families`. A render with no
explicit `fontFamilies` falls back to registration order, so a Han-unified codepoint shared by
two requested families (e.g. `"Noto Sans TC"` and `"Noto Sans JP"`) could pick the wrong one
regardless of how `families` was ordered. `googleFonts` now sorts its result to match the
caller's declared order.

### Replace `fetchResources`/`extractResourceUrls` with `prepareImages`

`@takumi-rs/helpers` exports `prepareImages({ node, sources?, fetchCache?, fetch?, timeout? })`,
which collects a node tree's remote images and fetches the ones not already supplied. Pass a
`fetchCache` (a `Map<string, Promise<ArrayBuffer>>`, or any `Map`-like store) to coalesce
concurrent fetches of the same URL and reuse the bytes across renders; a failed fetch is
evicted so a later call retries.

The `extractResourceUrls` and `fetchResources` helpers are removed. The `images` render option
takes the same group form: `{ sources, fetchCache, fetch, timeout }`.

### Keep elements as containers when children carry no text

An element whose children resolved to a textless iterable (e.g. `{[]}`) became an
empty text node instead of a container, so its `background` and other box styles
never painted. Such elements now stay containers.

### Shrink and sharpen the Google Fonts family type

Group the ~1940 families by their distinct weight/style/axis shape (152 of them) so
`GoogleFontFamily` builds a discriminated union over shapes, not families. The shipped
`.d.ts` drops from ~192 KB to ~58 KB and the checker does ~75% fewer instantiations. The
object form now autocompletes each known family's weight, style, and axes, and still accepts
a name built at runtime. The generator refuses to write a catalog with under 1000 families.

### Accept a bare URL string in `fonts`

`fonts` entries can now be a URL string, e.g. `fonts: ["https://example.com/Inter.woff2"]`.
The bytes are fetched on demand and keyed by the URL; family name, weight, and style come
from the font file. The object form stays for overriding those. Adds `fontFromUrl` to
`@takumi-rs/helpers`.

### Add `baseUrl` to `googleFonts`

`googleFonts` takes an optional `baseUrl`, defaulting to Google Fonts, so an API-compatible
css2 mirror can be used instead, e.g. `baseUrl: "https://fonts.bunny.net/css2"` for Bunny Fonts.

## @takumi-rs/helpers@2.0.0-rc.15 (rc)

### Fix `googleFonts` losing your declared family order

`googleFonts` returned subsets in whatever order Google's `css2` response happened to list
`@font-face` blocks in — not the order families were passed in `families`. A render with no
explicit `fontFamilies` falls back to registration order, so a Han-unified codepoint shared by
two requested families (e.g. `"Noto Sans TC"` and `"Noto Sans JP"`) could pick the wrong one
regardless of how `families` was ordered. `googleFonts` now sorts its result to match the
caller's declared order.

## @takumi-rs/helpers@2.0.0-rc.13 (rc)

### Bound remote fetches with byte caps and default timeouts

Remote image, font, and Google Fonts CSS fetches now reject bodies past a byte
cap (`maxBytes`, default 32 MiB; 2 MiB for CSS) and apply the 5 s timeout to
every fetch, not just images. Set `timeout: 0` to disable it. A new `allowUrl`
hook on `FetchOptions` skips URLs it rejects.

### Share the renderer facade between the napi and wasm bindings

The napi and wasm `Renderer` wrappers now build on a shared `prepareRenderInput`
helper in `@takumi-rs/helpers/renderer`, so their option types and render bodies
live in one place. Both backends check `signal` before and after resolving
fonts and images: on native, an already-aborted signal now throws before any
resource fetch.

## @takumi-rs/helpers@2.0.0-rc.7 (rc)

### Shrink and sharpen the Google Fonts family type

Group the ~1940 families by their distinct weight/style/axis shape (152 of them) so
`GoogleFontFamily` builds a discriminated union over shapes, not families. The shipped
`.d.ts` drops from ~192 KB to ~58 KB and the checker does ~75% fewer instantiations. The
object form now autocompletes each known family's weight, style, and axes, and still accepts
a name built at runtime. The generator refuses to write a catalog with under 1000 families.

## @takumi-rs/helpers@2.0.0-rc.6 (rc)

### Add the `@takumi-rs/helpers/renderer` entrypoint

The shared renderer wrapper backing the napi and wasm bindings is now exported
as `@takumi-rs/helpers/renderer`.

## @takumi-rs/helpers@2.0.0-rc.4 (rc)

### Replace `fetchResources`/`extractResourceUrls` with `prepareImages`

`@takumi-rs/helpers` exports `prepareImages({ node, sources?, fetchCache?, fetch?, timeout? })`,
which collects a node tree's remote images and fetches the ones not already supplied. Pass a
`fetchCache` (a `Map<string, Promise<ArrayBuffer>>`, or any `Map`-like store) to coalesce
concurrent fetches of the same URL and reuse the bytes across renders; a failed fetch is
evicted so a later call retries.

The `extractResourceUrls` and `fetchResources` helpers are removed. The `images` render option
takes the same group form: `{ sources, fetchCache, fetch, timeout }`.

### Keep elements as containers when children carry no text

An element whose children resolved to a textless iterable (e.g. `{[]}`) became an
empty text node instead of a container, so its `background` and other box styles
never painted. Such elements now stay containers.

## @takumi-rs/helpers@2.0.0-rc.3 (rc)

### Add `baseUrl` to `googleFonts`

`googleFonts` takes an optional `baseUrl`, defaulting to Google Fonts, so an API-compatible
css2 mirror can be used instead, e.g. `baseUrl: "https://fonts.bunny.net/css2"` for Bunny Fonts.

## @takumi-rs/helpers@2.0.0-rc.0 (rc)

### Match the Chromium UA stylesheet for default element styles

Parse the relative font keywords `bolder`/`lighter` (`font-weight`) and
`larger`/`smaller` (`font-size`), resolving to the values Chromium uses. Expand
the default element presets to cover lists, `sub`/`sup`, `ins`/`del`, forms,
`details`/`summary`, and `search`.

### Accept a bare URL string in `fonts`

`fonts` entries can now be a URL string, e.g. `fonts: ["https://example.com/Inter.woff2"]`.
The bytes are fetched on demand and keyed by the URL; family name, weight, and style come
from the font file. The object form stays for overriding those. Adds `fontFromUrl` to
`@takumi-rs/helpers`.

## @takumi-rs/helpers@2.0.0-beta.10 (beta)

### Rename the `googleFonts` family field from `family` to `name`

A `GoogleFontFamily` object now spells its family as `name`, not `family` —
`googleFonts({ families: [{ name: "Inter", weight: [400, 700] }] })`. Reads
cleaner next to `families` and matches the `name` field on rendered fonts. Bare
string families are unchanged.

## @takumi-rs/helpers@2.0.0-beta.9 (beta)

### Widen the React peer range and declare `engines`

Relax the `react` peer from `^19.2.5` back to `^18.0.0 || ^19.0.0`, matching
`react-dom` and dropping the peer warning on React 19.2.x patch releases. All
published packages now declare `engines.node: ">=18"`.

## @takumi-rs/helpers@2.0.0-beta.8 (beta)

### Render every weight of a variable Google Font

A variable font is served as one woff2 reused across weights. `googleFonts` now
collapses those faces into a single weightless face so the renderer drives the
`wght` axis, instead of pinning every weight to the file's default and leaving
`font-weight: 700` looking regular.

### Drop the `text` option from `googleFonts`

A `text=` request strips each face's `unicode-range`, so every subset claims full
coverage and overlaps the others, making glyph routing ambiguous and defeating the
render-time codepoint subsetting and the CSS/woff2 caches. Render already downloads
only the glyphs the content uses, so the option was redundant.

## @takumi-rs/helpers@2.0.0-beta.7 (beta)

### Type `googleFonts` families against the Google Fonts catalog

`GoogleFontFamily` now knows every Google Font, so known families autocomplete
their available `weight`, `style`, and variable `axes`. Any other string still
passes, keeping private or brand-new families working.

Set a variable axis per family, e.g. `{ family: "Inter", axes: { opsz: "14..32" } }`.
The catalog is generated at build time and is types-only, so the runtime bundle is
unchanged.

### Subset Google Fonts inside `render`

`googleFonts({ families })` returns every coverage subset of each family, with its
`unicode-range`, a distinct name under one `subsetOf`, and a stable key. `render`
registers only the subsets the content draws, so a multilingual image pulls a
handful of blocks instead of whole fonts. Set `subset: false` to register
everything; call `subsetFonts({ fonts, source })` to trim a set yourself.

This replaces `googleFont` and `googleFontSubsets` with one object-shaped
`googleFonts`. Distinct subset names mean a glyph routes to the file that covers it
rather than a same-named sibling that lacks it.

## @takumi-rs/helpers@2.0.0-beta.4 (beta)

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

## @takumi-rs/helpers@2.0.0-beta.1 (beta)

### Load only the Google Font subsets the content needs

`googleFontSubsets(content, families)` scans the codepoints a render uses, fetches every family's metadata in one css2 request, and keeps just the matching `unicode-range` subsets, so a multilingual image pulls a handful of CJK blocks instead of a whole font. Pass a `cache` Map to reuse the CSS across renders.

### Group coverage subsets under one logical family

`FontResource::subset_of` (Rust) and the `subsetOf` font field (JS) register a font as a subset of a logical family. A render expands `font-family: {logical}` into every subset registered under it, in order, so each script routes to the subset that covers it — distinct families no longer share a single fallback chain.

# @takumi-rs/helpers

## 1.8.7

### Patch Changes

- 9604fd7: Split package export types per import/require condition so CJS consumers resolve `.d.cts`

## 1.8.6

## 1.8.5

## 1.8.4

## 1.8.3

## 1.8.2

## 1.8.1

## 1.8.0

## 1.7.0

### Minor Changes

- b908a4d: Add `googleFont()` to load Google Fonts (static or variable) as font descriptors
- 80e29da: Type the `textFit` style property on React's `CSSProperties` so it no longer needs a cast

### Patch Changes

- 4748c22: Make `extractResourceUrls` collects `url(...)` from `tw` arbitrary values #742
- 42d0d03: Use a per-request timeout in `fetchResources` so one slow URL no longer consumes the whole batch's timeout budget

## 1.6.0

## 1.5.1

## 1.5.0

## 1.4.1

## 1.4.0

### Patch Changes

- e83ab19: Fix `<pre>` default preset omitting `white-space: pre`

## 1.3.0

## 1.2.1

## 1.2.0

## 1.1.2

## 1.1.1

## 1.1.0

## 1.0.16

## 1.0.15

## 1.0.14

## 1.0.13

## 1.0.12

## 1.0.11

## 1.0.10

## 1.0.9

## 1.0.8

### Patch Changes

- 8886c01: Replace noto emoji source with `googlefonts/noto-emoji` with Unicode 17 support
- b287c43: Update Twemoji to v17

## 1.0.7

## 1.0.6

## 1.0.5

### Patch Changes

- d113fb5: Fix HTML not decoded

## 1.0.4

## 1.0.3

## 1.0.2

## 1.0.1

## 1.0.0

### Minor Changes

- b2e304a: Add `extractResourceUrls` helper in JS
- 3142b36: Add `extractEmojis` helper
- 01c4fa3: Resolves `useContext` hook (support `lucide-react` v1)
- 7ff886b: Add `fromHtml` API

### Patch Changes

- 7ff886b: Fallback to react-dom/server if needed to render with hooks
- b5b8531: Support sourcing class name from `class` field
- cc9e63c: Bundle `ultrahtml` instead of externalized #621
- f1b6104: Loosen `cache` map type
- 6767ad9: Support fluent emoji API

## 1.0.0-rc.17

### Patch Changes

- 6767ad9: Support fluent emoji API

## 1.0.0-rc.16

## 1.0.0-rc.15

## 1.0.0-rc.14

### Minor Changes

- eb34add: Replaced `fromStaticMarkup` with `fromHtml`

## 1.0.0-rc.13

## 1.0.0-rc.12

## 1.0.0-rc.11

### Minor Changes

- b2e304a: Add `extractResourceUrls` in pure JS to avoid extra roundtrip to native bindings

## 1.0.0-rc.10

### Patch Changes

- cc9e63c: Bundle `ultrahtml` instead of externalized #621

## 1.0.0-rc.9

## 1.0.0-rc.8

## 1.0.0-rc.7

## 1.0.0-rc.6

## 1.0.0-rc.5

## 1.0.0-rc.4

### Minor Changes

- 7ff886b: Add `fromStaticMarkup` API

### Patch Changes

- 7ff886b: Fallback to react-dom/server if needed to render with hooks

## 1.0.0-rc.3

## 1.0.0-rc.2

## 1.0.0-rc.1

## 1.0.0-rc.0

## 1.0.0-beta.20

### Minor Changes

- 01c4fa3: Resolves `useContext` hook (support `lucide-react` v1)

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

## 1.0.0-beta.8

## 1.0.0-beta.7

## 1.0.0-beta.6

## 1.0.0-beta.5

## 1.0.0-beta.4

### Patch Changes

- f1b6104: Loosen `cache` map type

## 1.0.0-beta.3

### Patch Changes

- b5b8531: Support sourcing class name from `class` field

## 1.0.0-beta.2

### Minor Changes

- 3142b36: Add `extractEmojis` helper

## 1.0.0-beta.1

## 1.0.0-beta.0

## 0.73.1

## 0.73.0

### Minor Changes

- 349636a: **Deprecate `AnyNode`, replaced with strict `Node` type**

## 0.72.0

## 0.71.7

## 0.71.6

## 0.71.5

## 0.71.4

## 0.71.3

## 0.71.2

## 0.71.1

## 0.71.0

## 0.70.4

## 0.70.3

## 0.70.2

## 0.70.1

## 0.70.0

## 0.69.5

## 0.69.4

## 0.69.3

## 0.69.2

## 0.69.1

## 0.69.0

### Minor Changes

- 62525f9: **BREAKING CHANGE: `fromJsx()` now returns `{ node, stylesheets }`**

  Before:

  ```tsx
  const node = fromJsx(<div />);

  renderer.render(node);
  ```

  After:

  ```tsx
  const { node, stylesheets } = fromJsx(<div />);

  renderer.render(node, {
    stylesheets,
  });
  ```

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

### Patch Changes

- f58f974: add `tailwindClassesProperty` option
- f58f974: optimize JSX iterable/text collection, style hot path, and SVG lookup

## 0.68.1

## 0.68.0

## 0.67.3

### Patch Changes

- 06b118d: fix width/height not being passed to created node #448

## 0.67.2

## 0.67.1

## 0.67.0

## 0.66.14

## 0.66.13

## 0.66.12

## 0.66.11

## 0.66.10

## 0.66.9

## 0.66.8

## 0.66.7

## 0.66.6

## 0.66.5

## 0.66.4

## 0.66.3

## 0.66.2

### Patch Changes

- 291e5cc: add `cache` option for `fetchResources` util

## 0.66.1

## 0.66.0

## 0.65.0

## 0.64.1

## 0.64.0

### Minor Changes

- 1600ff0: make `fetchResources` return array

## 0.63.2

## 0.63.1

## 0.63.0

### Minor Changes

- ec9708d: add `fetchResources` function

## 0.62.8

## 0.62.7

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

### Minor Changes

- 0deafbd: decouple base Chromium styles (or customized from `defaultStylePresets`) from `style` field to independent `preset` field.
- 3bd63df: add `defaultStyles` option to opt-out from default styles #343

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

## 0.54.2

## 0.54.1

## 0.54.0

## 0.53.1

## 0.53.0

### Minor Changes

- 364fa11: **deprecate `PartialStyle`, use `CSSProperties` instead.**

## 0.52.2

## 0.52.1

## 0.52.0

### Patch Changes

- 776a18e: pass `img`, `svg` width/height/tw property when parsing

## 0.51.1

## 0.51.0

## 0.50.0

## 0.49.1

## 0.49.0

## 0.48.0

## 0.47.0

## 0.46.6

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

### Minor Changes

- 702c419: drop `createTailwindFn` as its built-in now
- 702c419: drop `twrnc` dependency (now zero 🎉)

## 0.44.0

## 0.43.1

### Patch Changes

- 4bd994f: ignore void elements (head/meta/link/style/script)
- 4bd994f: handles `br` hard breaking
- 4bd994f: fix pre/body/hr/center style preset

## 0.43.0

### Minor Changes

- 9247090: Add `createTailwindFn` function

## 0.42.0

## 0.41.0

## 0.40.2

## 0.40.1

## 0.40.0

## 0.39.0

## 0.38.1

## 0.38.0

## 0.37.0

## 0.36.2

## 0.36.1

### Patch Changes

- f12f11a: fix default text chunk should be `display: inline`

## 0.36.0

## 0.35.2

## 0.35.1

## 0.35.0

### Minor Changes

- 264fa71: add `inline` and `block` display value to text related tags preset

### Patch Changes

- 3d6745f: chore: fix typo on @takumi-rs/helpers readme

## 0.34.0

### Patch Changes

- 7c402d8: setup npm trusted publisher

## 0.33.1

## 0.33.0

### Patch Changes

- 4635fcb: ignore `false` in jsx parsing #206
