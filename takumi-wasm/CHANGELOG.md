## @takumi-rs/wasm@2.21.0

### Reuse downloaded bytes across renders, and load Google Fonts for known text

Renderers now keep the images, `@font-face` files, and font URLs they download in a 32 MiB store keyed by URL, so a later render skips the download. A render that passes its own `fetch` skips the store, since that `fetch` may send credentials.

```ts
import { googleFonts } from "takumi-js/helpers";

const fonts = googleFonts({ families: ["Noto Sans JP"], text: "東京の夜景" });
```

- `googleFonts` takes `text` to load one file holding just those characters.
- A font in a render's `fonts` list takes `cache: "none"`, which drops the font once the render settles. `googleFonts` sets it on every subset through `cacheMode: "none"`.
- Under the default `"auto"`, a `googleFonts` subset loaded for `text` drops once its render settles too, since another render rarely asks for the same characters. Range subsets and other fonts stay as before.
- `CacheMode` names the `"auto" | "none"` policy that images and fonts share. `ImageCacheMode` stays as a deprecated alias.

### Migration

`images.fetchCache` is deprecated, since renders keep their downloads without it.

```diff
- await renderer.render(node, { images: { fetchCache } });
+ await renderer.render(node);
```

### `unregisterFont` leaves the built-in Geist in place

`unregisterFont` now removes only the fonts you registered. Before, unregistering `Geist` also removed the built-in Geist, so text that no registered font covered drew nothing.

```ts
// @takumi-rs/core and @takumi-rs/wasm. Use `new Painter()` in takumi-paint and `new PdfRenderer()` in takumi-pdf.
const renderer = new Renderer();

// Before: true, and the built-in Geist is gone
// After: false, since no registered font is named Geist
renderer.unregisterFont({ name: "Geist" });
```

### See what a renderer's cache holds

`Renderer`, `PdfRenderer`, and `Painter` have `cacheStats()`, which reports the bytes the pool holds and how lookups of each kind went.

```ts
const { memory, images } = renderer.cacheStats();
console.log(memory.current, memory.high);
console.log(images.hits, images.misses, images.evictions);
```

`takumi-js` exports `cacheStats` for the renderer that `render` and the other functions share. It returns `undefined` until the first render creates that renderer. In Rust, `Cache::stats` does the same for a `Cache`.

### An image moved by a fraction of a pixel blends its edges

An image that a `transform` such as `translate(0.5px)` moved by part of a pixel, drawn at its own size, snapped to the nearest whole pixel and lost the shift. It now blends across the pixel it falls between, as a browser draws it.

### The paint tree lists the CSS a render dropped

A declaration or at-rule that takumi drops no longer vanishes without a trace. The paint tree lists each one once, with its property and value as written, why takumi dropped it, and where it was written. The image stays the same.

```ts
const tree = await paint(html, { width: 600, height: 200 });

console.log(tree.ignoredDeclarations);
// [{ property: "writing-mode", value: "vertical-rl", reason: "unknown-property", origin: "style-attribute" }]
```

- `reason` is `unknown-property`, `invalid-value`, or `unknown-at-rule`.
- `origin` is `style-attribute`, `style-sheet`, or `style-object`.
- `takumi paint` prints the list under `ignored`, and its JSON output holds it as `ignoredDeclarations`.
- `needs()` in `@takumi-rs/core`, `@takumi-rs/wasm`, and `takumi-pdf` returns the same list as `ignoredDeclarations`.
- In Rust, `RenderNeeds::ignored_declarations` and `PaintDocument::ignored_declarations` hold the same list, and `StyleSheet::ignored_declarations` holds what one style sheet dropped.

### An image whose header claims too many pixels fails to load instead of aborting a PDF

A PNG or WebP whose header claims an edge longer than 8192 pixels, the limit every decode already enforced, now fails to load like any other undecodable image. A PDF used to embed such a PNG as is and abort the whole process when it sized a buffer from that header.

### An `offset-path` with huge coordinates renders instead of panicking

A `path()` in `offset-path` whose coordinates run into the quintillions, as generated or user-supplied paths can, panicked while measuring its length. It now measures, and a percentage `offset-distance` lands at that share of the whole path instead of near its start.

### Download fonts listed as URLs with the render's fetch options

A URL string in a render's `fonts` list now downloads with the `fetch`, `allowUrl`, `timeout`, `maxBytes`, and `signal` the render's images use, so `allowUrl` refuses it too.

```tsx
await render(<div>Hello</div>, {
  width: 1200,
  height: 630,
  fonts: ["https://fonts.example.com/Brand.woff2"],
  images: { allowUrl: (url) => url.startsWith("https://cdn.example.com/") },
});
// Before: downloads the font
// After: rejects with "URL blocked by allowUrl policy: https://fonts.example.com/Brand.woff2"
```

### A JSX `<svg>` or `<img>` sized with `"100%"` or `"24px"` draws at that size

A JSX `<svg>` or `<img>` whose `width` or `height` was a string with a unit, such as `"100%"`, `"2em"`, or `"24px"`, drew nothing. It now draws at that size, as the same markup passed as HTML does.

```tsx
<div style={{ display: "flex", width: 200, height: 100 }}>
  <svg width="100%" height="50%" viewBox="0 0 2 1">
    …
  </svg>
</div>
// Before: the svg draws nothing
// After: the svg's box is 200 × 50, since a percentage resolves against the containing block
```

- An `<svg>` reads `width` and `height` as CSS lengths, such as `2em`, and ignores a negative one.
- An `<img>` reads them as HTML does, as pixels or a percentage, so `width="2em"` is 2 pixels.

### A style value Takumi cannot read is ignored instead of failing the render

A node's `style` object now drops a declaration whose value is invalid, such as a negative `height`, and renders the rest, as a browser does with a `style` attribute. Where a render used to reject, `console.warn` now prints the same message after `takumi: ignoring`, and in Rust the message is a `log` warning under the `takumi::style` target.

### Every renderer takes its cache size in one `cache` option

`Renderer`, `PdfRenderer`, and `Painter` take a `cache` option. Its `maxBytes`, 128 MiB by default, is one pool for everything a renderer reuses. Setting `images`, `glyphs`, or `fonts` to `"none"` keeps none of that kind. The options apply when the renderer is created.

```ts
const renderer = new Renderer({
  cache: {
    maxBytes: 256 * 1024 * 1024,
    glyphs: "none",
  },
});
```

### Migration

`cacheMaxBytes`, `fontCacheMaxBytes`, and `FontCacheOptions` are deprecated. In `@takumi-rs/core` and `@takumi-rs/wasm`, `setGlyphCacheMaxBytes` is deprecated, and only `0` still changes anything.

```diff
- setGlyphCacheMaxBytes(0);
- const renderer = new Renderer({ cacheMaxBytes: 64 * 1024 * 1024 });
+ const renderer = new Renderer({ cache: { maxBytes: 64 * 1024 * 1024, glyphs: "none" } });

  // types
- let options: FontCacheOptions;
+ let options: RendererOptions;
```

### Animated PNGs and GIFs are smaller and encode faster when frames repeat

Each frame of an animated PNG now stores only the pixels that changed since the frame before it, and a frame that changes nothing stores one pixel. An animated GIF stores an image held across several frames once, shown for their total delay, so it has fewer frames. Both decode to the same images at the same times as before. GIF frames of 10ms or less stay separate, since browsers stretch each one to 100ms.

### Renders warn about what they could not draw, and load fonts for missing characters

A render now warns when it goes without a character's glyph, a font family, an optional font, or an image under `images.throwOnError: false`. Each warning prints through `console.warn` as `takumi: <message>`, the same form as the warnings for unsupported CSS, every time a render raises it. To collect the warnings somewhere else, wrap `console.warn`.

```txt
takumi: No font draws "世界". Add a font that covers them to `fonts`, or return one from `loadFallbackFonts`.
```

- `prepare()` prints the warnings it finds, and a later render of the prepared source warns only about a font that fails when it loads again.
- A `render()` without `prepare()` checks glyphs and families only when it passes `loadFallbackFonts`.
- `loadFallbackFonts(characters)` runs once when characters have no font, and the fonts it returns load with the render.
- In Rust, `RenderNeeds` has `missing_characters` and `unresolved_families`, and is now `#[non_exhaustive]`.

### GIFs, APNGs, and animated WebPs draw at the device pixel ratio, and as backgrounds

With a `devicePixelRatio` above 1, an animated image drew at its CSS size in device pixels. It filled the top-left of its box, and its right column and bottom row streaked across the rest. A single-frame GIF drew the same way. An animated image used as a `background-image` drew nothing at all. Both now draw as a still PNG does.

`ImageSource::dimensions` gives an image's natural size in its own pixels.

### A render aborted while its `fonts` list resolves starts no image loader

When the `signal` aborts while a promised `fonts` list is still resolving, the render now rejects as soon as the list resolves, without calling any image's `data()` loader. Before, it called each loader and then rejected.

### Glyphs, Tailwind classes, and `@font-face` faces stay in the render's `Cache`

Two renderers in one process no longer share glyphs, compiled Tailwind classes, `@font-face` faces, or decoration intercepts. Each `Cache` keeps its own glyphs in the pool it shares with every other kind of data. `glyphs(ImageCacheMode::None)` keeps none.

```rust
let cache = CacheOptions::builder().glyphs(ImageCacheMode::None).build();
```

In `@takumi-rs/core` and `@takumi-rs/wasm`, `setGlyphCacheMaxBytes(0)` now keeps no glyphs only in renderers created after the call, and any other size keeps them in the shared pool. In `takumi-js`, it reaches only the renderer behind `render`, not a `Renderer` from `takumi-js/node`.

### Migration

`set_glyph_cache_max_bytes` in `takumi-core` is deprecated. A size of `0` still keeps no glyphs in the caches built after the call, and a larger size no longer bounds them apart.

```diff
- set_glyph_cache_max_bytes(0);
- let cache = CacheOptions::builder().build();
+ let cache = CacheOptions::builder().glyphs(ImageCacheMode::None).build();
```

### A cropped image decoded at its drawn size draws sharp

An image that `object-fit: cover` or `object-position` crops, decoded at the size it draws, could pass through the bilinear filter and come out slightly soft, as when a photo 4032 pixels wide draws 1200 wide. It now copies its pixels, and draws faster.

### A font one render lists no longer changes another render

Each render now draws with the fonts its own `fonts` list names and the fonts `registerFont` registered. Before, a renderer kept using the fonts earlier renders had listed, so the same request could come out differently.

```ts
await renderer.render(html, { fonts: [brandRegular] });
await renderer.render(html, { fonts: [brandBold] });
await renderer.render(html, { fonts: [brandRegular] });
// Before: the third render draws Brand with the bold face the second one listed
// After: the third render matches the first
```

A family that only another render listed now falls back to the next family. List the font in this render, or register it with `registerFont`.

The options types of `@takumi-rs/core` and `@takumi-rs/wasm` no longer list `failedFontFiles`, a field only the renderer itself sets.

### Evict fonts one file at a time from the renderer's cache

Font files that renders load now share the renderer's `maxBytes` pool and leave it one file at a time, least recently used first. A family that keeps gaining files, such as one Google Fonts `text=` subset per title, now stays within `maxBytes`. A render keeps the fonts it uses until it finishes, and fonts from `registerFont` still stay. With `cache.fonts` set to `"none"`, each file goes as soon as the last render using it finishes.

```ts
const renderer = new Renderer({ cache: { fonts: "none" } });
```

`fontCacheMaxBytes` no longer bounds the fonts apart. A value of `0` still keeps none, as `"none"` does.

Among equally matching faces of one family, the one listed last now wins every time.

### Render without a font that fails to load when the font is optional

A font in a render's `fonts` list takes `optional: true`. When an optional font fails to download, `allowUrl` refuses it, or its bytes do not decode, the render goes on without it instead of rejecting.

```tsx
await render(<div style={{ fontFamily: "Brand, Inter" }}>Hello</div>, {
  width: 1200,
  height: 630,
  fonts: [
    fontFromUrl("https://example.com/Inter.woff2"),
    fontFromUrl("https://cdn.example.com/Brand.woff2", { optional: true }),
  ],
});
```

### An `svg` element drawn in two text colors shows each color

An `svg` element that inherits `color`, through `color="currentColor"` on its root or a `<style>` rule, now draws in the `color` of each box it sits in. Before, every copy took the first color it was drawn in. An SVG image that uses its host's fonts follows each host too.

### Migration

`SvgSource::vector_ops` is removed. `SvgSource::picture` returns the same ops, shared with the cache.

```diff
- let ops = source.vector_ops(&host, viewport);
+ let picture = source.picture(&host, viewport);
+ let ops = picture.ops();
```

### A WOFF font in `@takumi-rs/wasm` fails with an error that says to convert it

`@takumi-rs/wasm` cannot read WOFF fonts. Registering one now fails with an error that names the format, instead of `Unsupported font format`.

```ts
await renderer.registerFont(woffBytes);
// Before: Unsupported font format
// After: This build cannot read WOFF fonts. Convert the font to a format it reads, such as TTF
```

The fonts docs now list the formats each package reads. In Rust, a `takumi-core` build without the `woff` or `woff2` feature returns `FontError::FormatLeftOut` for that format.

### `require` loads the WebAssembly backend again

A CommonJS build that renders on the WebAssembly backend, such as one in a WebContainer, works again. It used to fail with `TypeError: (0 , _takumi_rs_wasm.default) is not a function`.

```js
const { init } = require("takumi-js/wasm");
// Before: init is the module object, not a function
// After: init loads the WebAssembly binary
```

`require("takumi-js/wasm/no-init")` returns the same `init`, and `@takumi-rs/wasm` now exports `init` by name as well as by default.

### Vite's dev server finds the WebAssembly binary for a bundled SSR dependency

When `ssr.noExternal` bundles `@takumi-rs/wasm` into the dev server's SSR modules, the first render now finds the WebAssembly binary in the package. It used to fail with this error:

```text
Unable to locate Takumi WASM asset for SSR: /node_modules/@takumi-rs/wasm/pkg/takumi_wasm_bg.wasm
```

### Unsupported CSS warns through `console.warn`

`@takumi-rs/core`, `@takumi-rs/wasm`, and `takumi-js` on either of them now report a CSS property takumi does not support through `console.warn` each time they parse a style sheet or `style` attribute that uses it. A render that reuses a `css` list the renderer still holds parsed does not warn again. Before, the declaration dropped without a word, so a typo such as `widht` changed the render silently. Other warnings takumi logs, such as an animated image frame that fails to decode, print the same way.

## @takumi-rs/wasm@2.20.0

### A `unicodeRange` subset without a glyph falls back to the next family again

A font in `fonts` whose `unicodeRange` claims a character it has no glyph for now hands that character on to the next family in the stack, as a browser does and as 0.18 did. Before, the render left the next family's subset unloaded. The PDF render then failed with "No registered font covers". That error now reads "No font the text can use draws", and suggests checking each font's `unicodeRange` and the `font-family` stack, because a registered font can have the glyph and still be out of reach.

### A `using` block frees a renderer

`Renderer`, `PdfRenderer` and `Painter` implement `Symbol.dispose`, so `using renderer = new Renderer()` frees the renderer's WebAssembly memory when the block ends, as calling `free()` does.

### `prepare` downloads the fonts `@font-face` rules load

An `@font-face` rule in `css` or a `<style>` element now loads its font in `prepare`, `render`, and `ImageResponse`. A `src` that fails to download falls back to the next one.

### `registerFont` registers one buffer again under another name, weight, style or `unicodeRange`

Passing the same `data` buffer to `registerFont` a second time with other descriptors used to return the first registration, so the new family or weight never existed. Each set of descriptors now registers its own face, as `@font-face` rules sharing one `src` do. A lazy font's key includes its `unicodeRange` too.

## @takumi-rs/wasm@2.19.0

### Render on Nitro's `bun` preset instead of failing

Nitro's `bun` preset used to answer every request with a 500 and `ENOENT ... _libs/@takumi-rs/[object WebAssembly.Module]`. The WASM packages now load their binary through unwasm there, as they do on other Nitro presets.

### Renderers take HTML directly

`render`, `renderSvg`, `measure`, and the WASM `renderAsDataUrl` accept an HTML string in place of a node tree. The markup is parsed in Rust, and the CSS of its `<style>` elements follows the `css` option. A `<style>` with a `media` attribute applies only where its query matches. `takumi-html` adds `parse_html`, which returns that CSS with the tree. `fromHtml` from `@takumi-rs/helpers/html` is deprecated in favor of passing the string and will be removed in v3.

### `Renderer.prepare` fetches what a render needs ahead of it

`renderer.prepare(source, options)` styles an HTML string or node tree, fetches the images it draws, and loads the font subsets its text uses. `render`, `renderSvg`, and `measure` take the prepared source with only output options such as `format`. `renderer.prepareAnimation(options)` does the same for every scene of an animation, and `renderAnimation` takes its result. `prepareImages` from `@takumi-rs/helpers` is deprecated in favor of `prepare`.

`takumi-js` renders through `prepare`, so it fetches the images the styled tree draws. An image that `css`, a `<style>` element, or a `::before` rule sets now renders, an image under `display: none` is no longer fetched, and a custom `fetch` now receives relative URLs such as `/logo.png`.

### Parse HTML strings in Rust in every renderer

`takumi-paint` and `takumi-pdf` take HTML strings as `@takumi-rs/core` and `@takumi-rs/wasm` do, and `takumi-pdf` takes them for header and footer bands too. A `renderAnimation` scene can be an HTML string, its `<style>` rules applying to the animation. With `emoji: "from-font"`, `takumi-js` hands HTML to the renderer instead of parsing it in JavaScript.

### Font subsets load as a browser loads them

When `fonts` holds `unicodeRange` subsets, as `googleFonts` returns, a render now loads only the subsets that font matching reaches for each character, through that text's own font stack, weight, and style. With `font-family: Inter, "Noto Sans JP"`, Latin text no longer loads Noto Sans JP's Latin subset, and a bold subset loads only when bold text uses it.

The text comes from the styled tree, so a subset needed only by a stylesheet's generated content, a list marker, `text-transform`, the `text` elements of an SVG image, or a `takumi-pdf` page counter now loads. `collectCodepoints`, `LIST_MARKER_CHARACTERS`, and `subsetFonts` are deprecated.

### Editors point imports of `@takumi-rs/image-response` and `@takumi-rs/wasm/vite` to their replacements

Editors now flag imports from `@takumi-rs/image-response`, which only re-exports `takumi-js/response`, and from `@takumi-rs/wasm/vite`, which resolves to the same loader as `@takumi-rs/wasm/auto` under Vite. Both keep working and will be removed in v3.

## @takumi-rs/wasm@2.18.2

### Faster renderer initialization

Renderers now load the bundled variable Geist font without decompressing WOFF2 or copying its bytes. Custom WOFF2 fonts remain supported.

## @takumi-rs/wasm@2.18.0

### Draw an outline that sets only `outline-style`

An element with `outline-style: solid` and no `outline-width` now draws a `medium` (3px) outline, as browsers do. It used to draw no outline unless another outline longhand, such as `outline-color`, was also set.

### Apply the dimension limit to each GIF frame

A GIF frame wider or taller than 8192 pixels now fails to decode, as a GIF whose canvas is that large already does.

## @takumi-rs/wasm@2.17.1

### Skip an SVG too large to rasterize instead of failing the render

An SVG image or background that would rasterize past the pixel budget, such as `background-size: cover` on an SVG with an extreme `viewBox`, is now left undrawn. The whole render used to fail with `InvalidPixmapSize`.

### Clip a repeated background to its box

A repeated `background-image` whose tiles do not divide the box evenly, such as 15px tiles in a 96px box, now stops at the box's edge. Its last row and column of tiles used to paint past it.

### Scale images up faster

A magnified image lerped every source row across the destination width again for each destination row that read it. Each source row is now lerped once and reused, so drawing a small image across a large box takes about 40% less time. The output is unchanged.

### Type the `Renderer` methods' results again

2.17.0 declared `render`, `renderAsDataUrl`, `renderSvg`, `measure`, `renderAnimation`, and `registerFont` as returning `any`, so `takumi-js`'s `render()` and `measure()` inferred `any` too. They now declare their promised results, such as `Promise<Uint8Array<ArrayBuffer>>` for `render`.

### Blur what lies past the canvas edge

Since 2.15.0, an element with a pixel-moving filter such as `blur()` that reached past the canvas's top or left edge lost that part before filtering, so its blur faded toward the edge. The filter layer now takes in what the filter reads past the canvas, as Chrome's compositor does.

## @takumi-rs/wasm@2.17.0

### Bound render fonts with `fontCacheMaxBytes`

Past this cap in decoded font bytes, a renderer releases the least recently used fonts that renders' `fonts` lists loaded, and a later render loads them again. Fonts from `registerFont` stay, and a running render keeps the fonts it started with. The wasm `Renderer` now passes its options, such as `cacheMaxBytes`, to the native renderer.

### Release fonts a renderer no longer needs

`unregisterFont({ name })` removes a family with all its `unicodeRange` subsets, and `clearFonts()` goes back to the bundled fallback font. A later render that lists a released font registers it again. In Rust, use `Fonts::unregister_family`.

## @takumi-rs/wasm@2.16.0

### Register font subsets with `unicodeRange`

A font now takes `unicodeRange`, the codepoint ranges its `@font-face` declares. It serves only those codepoints, as in Chrome, so each subset of a family registers under the same `name`:

```ts
const fonts = [
  { name: "Inter", data: latin, unicodeRange: [[0x0000, 0x00ff]] },
  { name: "Inter", data: cyrillic, unicodeRange: [[0x0400, 0x045f]] },
];
```

`googleFonts` returns its subsets this way. `subsetOf` and `subsetRank` are deprecated: `subsetOf` now only sets the family name, and `subsetRank` does nothing. In Rust, use `FontResource::unicode_range`.

## @takumi-rs/wasm@2.13.5

### Resolve a browser-only entry in client builds

Bundlers that resolve the `browser` condition (Vite client, webpack web) now
get `bundlers/browser.mjs`, which only fetches the `.wasm` asset by `import.meta.url`. Client builds
with `noExternal` stop failing with `Cannot bundle Node.js built-in "node:fs/promises"`.
The Vite server entry reads the asset through `process.getBuiltinModule`, so
no bundler sees a Node import. These packages, plus `takumi-js` and `@takumi-rs/image-response` on top of them, now require Node 20.19 or newer.

## @takumi-rs/wasm@2.13.0

### Resolve `tw` utilities through CSS variables

Utilities now read the CSS variables Tailwind compiles them to, falling back to the built-in value. Define tokens in `:root`. `--color-brand-500` makes `bg-brand-500` work, and spacing, fonts, shadows, animations and breakpoints follow the same rule.

Gradients now match Tailwind on two counts. Stops alone no longer paint without `bg-linear-*`, `bg-radial` or `bg-conic`, and a missing `to` stop fades to `transparent`.

### Let stylesheet rules win over `tw` utilities

Utilities now sit in the last cascade layer, below unlayered CSS and above rules in a named `@layer`. Important reverses that order. An important utility beats unlayered important CSS but loses to one in a named layer. Inline important declarations stay on top. A template that relied on `tw` beating a matching rule needs a fix. Move that rule into a layer, or mark the utility `!`.

### Parse `@theme` blocks as `:root` rules

A Tailwind v4 source stylesheet now works in `css` without compiling it first. `@theme` declarations land on `:root`, and `@keyframes` inside the block register. Modifiers like `reference` read the same way. The `prefix()` modifier is not supported.

### Compose filters and transforms through custom properties

Filter, translate, scale and grid-line utilities now compose through `--tw-*` variables like Tailwind's compiled CSS. Stacked filters follow Tailwind's fixed chain order instead of class order.

### Write a `css` entry as an object

A `css` entry can be a rule, `{ selector, style, rules }`, or an animation, `{ keyframes, steps }`. Takumi checks the selector and every value before the entry reaches the parser, so a token that comes from application data cannot escape the rule it was written for. The `keyframes` option is deprecated and goes away in v3.

### Turn on Preflight through `@import "tailwindcss"`

The import line at the top of a Tailwind v4 stylesheet now works. Preflight replaces the UA preset. Margins and padding go, lists lose their markers, and `h1` through `h6` drop their font sizing. It also brings the universal border reset, link and table resets, block-level images, and `hidden` on any element. Author rules outrank Preflight, apart from `hidden`, which it marks important. Other `@import` targets stay unsupported.

### Write a group of `css` entries as an object

A `css` entry can be `{ media, rules }`, `{ supports, rules }`, or `{ layer, rules }`. A layer without `rules` declares its order alone. Takumi reads each prelude with the grammar its rule takes, so it cannot close the rule and open another.

### Expand `@apply` inside stylesheet rules

`.card { @apply mt-4 bg-brand-500; }` now expands through the `tw` parser where it is written, `!` suffix included. Variants like `md:` are rejected. A static render has nothing for them to gate on.

### Rename the `stylesheets` render option to `css`

`css` takes inline CSS as one string or a list. The old `stylesheets` name still works everywhere and warns once on `takumi-js` and `takumi-pdf`.

### Render `<text>` elements in SVG image sources

SVG images with `<text>`, `<tspan>` and `textPath` now draw their text using
the registered fonts instead of dropping it. Glyphs render from font outlines;
color emoji glyphs inside SVG text are not supported.

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
