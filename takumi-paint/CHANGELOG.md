## takumi-paint@0.6.0

### A `unicodeRange` subset without a glyph falls back to the next family again

A font in `fonts` whose `unicodeRange` claims a character it has no glyph for now hands that character on to the next family in the stack, as a browser does and as 0.18 did. Before, the render left the next family's subset unloaded. The PDF render then failed with "No registered font covers". That error now reads "No font the text can use draws", and suggests checking each font's `unicodeRange` and the `font-family` stack, because a registered font can have the glyph and still be out of reach.

### `prepare` downloads the fonts `@font-face` rules load

An `@font-face` rule in `css` or a `<style>` element now loads its font in `prepare`, `render`, and `ImageResponse`. A `src` that fails to download falls back to the next one.

### A `using` block frees a renderer

`Renderer`, `PdfRenderer` and `Painter` implement `Symbol.dispose`, so `using renderer = new Renderer()` frees the renderer's WebAssembly memory when the block ends, as calling `free()` does.

### `PaintTree.free()` releases a tree's font files and glyph outlines

A paint tree keeps its font files and glyph outlines in WebAssembly memory until garbage collection releases them, which can take a while after many paints. `tree.free()`, or a `using` declaration, releases them right away. Calling it again does nothing, and the painter and other trees stay usable. Reading the files of a freed tree throws `The paint tree was freed`.

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
