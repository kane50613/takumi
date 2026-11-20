## takumi-paint@0.7.0

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

### `unregisterFont` leaves the built-in Geist in place

`unregisterFont` now removes only the fonts you registered. Before, unregistering `Geist` also removed the built-in Geist, so text that no registered font covered drew nothing.

```ts
// @takumi-rs/core and @takumi-rs/wasm. Use `new Painter()` in takumi-paint and `new PdfRenderer()` in takumi-pdf.
const renderer = new Renderer();

// Before: true, and the built-in Geist is gone
// After: false, since no registered font is named Geist
renderer.unregisterFont({ name: "Geist" });
```

### Evict fonts one file at a time from the renderer's cache

Font files that renders load now share the renderer's `maxBytes` pool and leave it one file at a time, least recently used first. A family that keeps gaining files, such as one Google Fonts `text=` subset per title, now stays within `maxBytes`. A render keeps the fonts it uses until it finishes, and fonts from `registerFont` still stay. With `cache.fonts` set to `"none"`, each file goes as soon as the last render using it finishes.

```ts
const renderer = new Renderer({ cache: { fonts: "none" } });
```

`fontCacheMaxBytes` no longer bounds the fonts apart. A value of `0` still keeps none, as `"none"` does.

Among equally matching faces of one family, the one listed last now wins every time.

### Renders warn about what they could not draw, and load fonts for missing characters

A render now warns when it goes without a character's glyph, a font family, an optional font, or an image under `images.throwOnError: false`. Each warning prints through `console.warn` as `takumi: <message>`, the same form as the warnings for unsupported CSS, every time a render raises it. To collect the warnings somewhere else, wrap `console.warn`.

```txt
takumi: No font draws "世界". Add a font that covers them to `fonts`, or return one from `loadFallbackFonts`.
```

- `prepare()` prints the warnings it finds, and a later render of the prepared source warns only about a font that fails when it loads again.
- A `render()` without `prepare()` checks glyphs and families only when it passes `loadFallbackFonts`.
- `loadFallbackFonts(characters)` runs once when characters have no font, and the fonts it returns load with the render.
- In Rust, `RenderNeeds` has `missing_characters` and `unresolved_families`, and is now `#[non_exhaustive]`.

### A style value Takumi cannot read is ignored instead of failing the render

A node's `style` object now drops a declaration whose value is invalid, such as a negative `height`, and renders the rest, as a browser does with a `style` attribute. Where a render used to reject, `console.warn` now prints the same message after `takumi: ignoring`, and in Rust the message is a `log` warning under the `takumi::style` target.

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

### See what a renderer's cache holds

`Renderer`, `PdfRenderer`, and `Painter` have `cacheStats()`, which reports the bytes the pool holds and how lookups of each kind went.

```ts
const { memory, images } = renderer.cacheStats();
console.log(memory.current, memory.high);
console.log(images.hits, images.misses, images.evictions);
```

`takumi-js` exports `cacheStats` for the renderer that `render` and the other functions share. It returns `undefined` until the first render creates that renderer. In Rust, `Cache::stats` does the same for a `Cache`.

### takumi-pdf and takumi-paint read their WebAssembly in Vite dev SSR from a package inside the root

With `ssr.noExternal`, the Vite dev server serves the WebAssembly binary from the package's own `pkg/` folder, and the first server render failed with `Unable to locate Takumi WASM asset for SSR`. The loaders now look there too, as `@takumi-rs/wasm` already does.

## takumi-paint@0.6.0

### `PaintTree.free()` releases a tree's font files and glyph outlines

A paint tree keeps its font files and glyph outlines in WebAssembly memory until garbage collection releases them, which can take a while after many paints. `tree.free()`, or a `using` declaration, releases them right away. Calling it again does nothing, and the painter and other trees stay usable. Reading the files of a freed tree throws `The paint tree was freed`.

### A `unicodeRange` subset without a glyph falls back to the next family again

A font in `fonts` whose `unicodeRange` claims a character it has no glyph for now hands that character on to the next family in the stack, as a browser does and as 0.18 did. Before, the render left the next family's subset unloaded. The PDF render then failed with "No registered font covers". That error now reads "No font the text can use draws", and suggests checking each font's `unicodeRange` and the `font-family` stack, because a registered font can have the glyph and still be out of reach.

### A `using` block frees a renderer

`Renderer`, `PdfRenderer` and `Painter` implement `Symbol.dispose`, so `using renderer = new Renderer()` frees the renderer's WebAssembly memory when the block ends, as calling `free()` does.

### `prepare` downloads the fonts `@font-face` rules load

An `@font-face` rule in `css` or a `<style>` element now loads its font in `prepare`, `render`, and `ImageResponse`. A `src` that fails to download falls back to the next one.

## takumi-paint@0.5.0

### `PdfRenderer` and `Painter` prepare a document too

`PdfRenderer.prepare` and `Painter.prepare`, and the shared `prepare` each package exports, fetch the images a document draws and load the font subsets its text uses, a PDF's header, footer, and page bands included. `render`, `measure`, and `paint` take the prepared document, a PDF with only file options such as `metadata` and `pdfa`.

### Render on Nitro's `bun` preset instead of failing

Nitro's `bun` preset used to answer every request with a 500 and `ENOENT ... _libs/@takumi-rs/[object WebAssembly.Module]`. The WASM packages now load their binary through unwasm there, as they do on other Nitro presets.

### Parse HTML strings in Rust in every renderer

`takumi-paint` and `takumi-pdf` take HTML strings as `@takumi-rs/core` and `@takumi-rs/wasm` do, and `takumi-pdf` takes them for header and footer bands too. A `renderAnimation` scene can be an HTML string, its `<style>` rules applying to the animation. With `emoji: "from-font"`, `takumi-js` hands HTML to the renderer instead of parsing it in JavaScript.

### Font subsets load as a browser loads them

When `fonts` holds `unicodeRange` subsets, as `googleFonts` returns, a render now loads only the subsets that font matching reaches for each character, through that text's own font stack, weight, and style. With `font-family: Inter, "Noto Sans JP"`, Latin text no longer loads Noto Sans JP's Latin subset, and a bold subset loads only when bold text uses it.

The text comes from the styled tree, so a subset needed only by a stylesheet's generated content, a list marker, `text-transform`, the `text` elements of an SVG image, or a `takumi-pdf` page counter now loads. `collectCodepoints`, `LIST_MARKER_CHARACTERS`, and `subsetFonts` are deprecated.

### `takumi-pdf` and `takumi-paint` drop their deprecated options

`takumi-pdf` no longer takes `stylesheets`. Pass CSS through `css`, which takes one string or a list. The `subsetOf` and `subsetRank` font fields are gone from both packages. Register each subset under its family `name` with its `unicodeRange`.

## takumi-paint@0.4.4

### Faster renderer initialization

Renderers now load the bundled variable Geist font without decompressing WOFF2 or copying its bytes. Custom WOFF2 fonts remain supported.

## takumi-paint@0.4.0

### Bound render fonts with `fontCacheMaxBytes`

Past this cap in decoded font bytes, a renderer releases the least recently used fonts that renders' `fonts` lists loaded, and a later render loads them again. Fonts from `registerFont` stay, and a running render keeps the fonts it started with. The wasm `Renderer` now passes its options, such as `cacheMaxBytes`, to the native renderer.

### Release fonts a renderer no longer needs

`unregisterFont({ name })` removes a family with all its `unicodeRange` subsets, and `clearFonts()` goes back to the bundled fallback font. A later render that lists a released font registers it again. In Rust, use `Fonts::unregister_family`.

## takumi-paint@0.3.0

### Read a run's outlines one glyph at a time

`TextRun.outline()` is now `outlines()`, which returns each glyph's outline as SVG path data in the order of `glyphs`, with `null` for a glyph the run's color does not fill. A renderer can draw, animate, or hit-test each letter on its own. Replace `run.outline()` with `run.outlines().join("")`. In Rust, `PaintTree::run_outline` is now `run_outlines`.

### Map each glyph of a paint tree back to its text

A `Glyph` now has `cluster`: where its cluster starts in the run's `text`, in UTF-16 code units. Glyphs of one cluster share it, so a renderer can split a run into words or animate it a letter at a time without guessing from glyph positions.

### Write a paint tree to JSON

`JSON.stringify(tree)` now writes a versioned `SerializedPaintTree`: the tree's nodes, steps, and effects by index, and every glyph's outline. A tree painted on a server can be handed to a browser or a worker as plain data. Font files are left out.

## takumi-paint@0.2.0

### Register font subsets with `unicodeRange`

A font now takes `unicodeRange`, the codepoint ranges its `@font-face` declares. It serves only those codepoints, as in Chrome, so each subset of a family registers under the same `name`:

```ts
const fonts = [
  { name: "Inter", data: latin, unicodeRange: [[0x0000, 0x00ff]] },
  { name: "Inter", data: cyrillic, unicodeRange: [[0x0400, 0x045f]] },
];
```

`googleFonts` returns its subsets this way. `subsetOf` and `subsetRank` are deprecated: `subsetOf` now only sets the family name, and `subsetRank` does nothing. In Rust, use `FontResource::unicode_range`.

## takumi-paint@0.1.1

### Write glyph outlines only when a run asks for them

`paint()` no longer writes every run's outline up front. `run.outline()` builds it when called. A 2,000-section document paints 3 times faster in about a third of the memory.

In Rust, `TextRun::outline` is replaced by `PaintTree::run_outline(node, run)`.

### Keep less memory for each painted tree

A painted tree no longer keeps a second copy of itself in WebAssembly memory. Keeping several large trees at once takes about a third less memory.

## takumi-paint@0.1.0

### Get what to draw from JSX, HTML, or a node tree

`paint()` lays out the input and returns a `PaintTree` instead of pixels. `nodes` lists each box, text run and image with its CSS resolved. `steps` lists what to draw, in paint order.
