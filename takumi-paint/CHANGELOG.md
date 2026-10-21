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
