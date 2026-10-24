## takumi-pdf@0.18.2

### Free the layout data of a page once it is drawn

`render_pages` and `render` free the layout data a page held, such as its text lines and cached layouts, once the page is drawn and no later page paints from it. The source tree and the tree of boxes stay in memory for the whole render, so peak memory still grows with the size of the document. A document that lays out to its end before the first page is drawn frees nothing early. A page that would paint from a released page fails with `Error::PageReleased` instead.

### Break positioned boxes across the pages of flex, grid and table containers

In paged output, an absolutely positioned box in a flex, grid or table container now breaks across pages with its container, and one placed in a grid area that starts on a later page lands on that page.

### Repeat a fixed box from the page it starts on

In paged output, a `position: fixed` box now repeats on the page that holds its static position and on every page after it, no longer on the pages before.

### Draw an SVG's bitmaps and filtered groups in a PDF without rasterizing

takumi-pdf embeds the PNG, JPEG, and WebP images inside an SVG as PDF images, and draws an SVG group without its filter. A CSS color `filter` on an SVG `<img>` or `background-image` recolors the SVG instead of rasterizing it, and takumi-svg writes an SVG's bitmaps as `<image>` elements. `SvgOp::Image` and `SvgOp::PushFilter` replace `SvgOp::Raster`, `SvgSource::vector_ops` no longer takes a raster scale, and resvg's renderer moves behind the `svg-raster` feature, on by default.

### Parse every `break-before`, `break-after`, and `break-inside` keyword

`break-before` and `break-after` now accept `avoid`, `avoid-page`, `avoid-column`, `column`, `recto`, and `verso`, and keep `left` and `right` as their own values. `break-inside` accepts `avoid-page` and `avoid-column`. In a PDF, `recto` and `verso` force a page break, and `break-inside: avoid-page` keeps a box on one page as `avoid` does. A PDF does not yet avoid a break between boxes for `break-before` or `break-after`.

### Lay out tables with Chromium's table algorithm

Tables now follow Chromium's table layout instead of a grid approximation, so column widths, row heights, and collapsed borders match Chromium. `<col>` and `<colgroup>` size their columns and paint their backgrounds, and `visibility: collapse`, `empty-cells: hide`, and `display: inline-table` work. A `rowspan` now stops at the end of its row group.

### Paint outside list markers as boxes of the page

An outside list marker now paints and stacks as Chrome paints it. In a tagged PDF, the text of a list item stays in one marked-content region instead of being split around its marker.

### Stream PDF documents with page counters, page-number bands or internal links

`render` now draws a page at a time for documents whose header, footer or content shows `pageNumber`, `totalPages` or `targetPageNumber`, or links to an element with `href="#id"`. Passes that only lay the pages out count them and find the targets first, so the output is unchanged and peak memory drops, at the cost of laying the content out once more. A counter that is still moving after three passes lays the document out to its end before the first page is drawn.

### Compose PDF pages while later pages still lay out

`render` draws a page as soon as no page still to lay out can paint onto it, so a long document holds one page's scene at a time. Layout errors, including `TooManyPages`, can now arrive after earlier pages were composed, so an error from drawing a page can come before one from laying out a later page.

### Break boxes inside a positioned inline box across pages

In paged output, an absolutely positioned box whose containing block is a `position: relative` inline box now moves to the next page when it starts past the page end, and breaks across pages when it is taller than the space left.

### Lean a synthesized italic right and keep it where its advance puts it

A run slanted for a face with no italic of its own leaned left, and was drawn about a quarter of an em to the right, into the next word in LTR text and into the previous word in RTL text. It now leans right about its own baseline, as in PNG output.

## takumi-pdf@0.18.0

### Shape with HarfBuzz 14.5.1

Shaping moves to harfrust 0.14, which matches HarfBuzz 14.5.1: Bengali ra-phala such as গ্র joins again, marks attach correctly after a ligated multiple substitution, and Hangul runs apply `calt` to the characters around them. A PDF now draws a glyph from a font's SVG table only when that glyph has SVG data.

### Bound render fonts with `fontCacheMaxBytes`

Past this cap in decoded font bytes, a renderer releases the least recently used fonts that renders' `fonts` lists loaded, and a later render loads them again. Fonts from `registerFont` stay, and a running render keeps the fonts it started with. The wasm `Renderer` now passes its options, such as `cacheMaxBytes`, to the native renderer.

### Release fonts a renderer no longer needs

`unregisterFont({ name })` removes a family with all its `unicodeRange` subsets, and `clearFonts()` goes back to the bundled fallback font. A later render that lists a released font registers it again. In Rust, use `Fonts::unregister_family`.

## takumi-pdf@0.17.3

### Copy text correctly from PDF in viewers that ignore `/ActualText`

When several glyphs draw one cluster, such as `S` and an acute for `Ś`, or a Devanagari consonant and its matra, each glyph now maps to its own characters in the font's ToUnicode table. Before, each mapped to the whole cluster, so pdf.js (Firefox), pypdf, and pdfminer read `Świnoujście` as `ŚŚwinoujśŚcie`, `मोटा` as `मोमोटाटा`, and a plain `S` elsewhere in the document as `Ś`.

### Keep words whole when a letter is drawn as a base and a mark

PDF text extraction no longer splits a word after a letter the font draws as a base glyph and a combining mark, such as `Ś` from a font that has only `S` and U+0301. Before, `pdftotext` read `Świnoujście` as `Ś winoujś cie`.

## takumi-pdf@0.17.0

### Register font subsets with `unicodeRange`

A font now takes `unicodeRange`, the codepoint ranges its `@font-face` declares. It serves only those codepoints, as in Chrome, so each subset of a family registers under the same `name`:

```ts
const fonts = [
  { name: "Inter", data: latin, unicodeRange: [[0x0000, 0x00ff]] },
  { name: "Inter", data: cyrillic, unicodeRange: [[0x0400, 0x045f]] },
];
```

`googleFonts` returns its subsets this way. `subsetOf` and `subsetRank` are deprecated: `subsetOf` now only sets the family name, and `subsetRank` does nothing. In Rust, use `FontResource::unicode_range`.

### Stop leaking a direction mark into PDF text

Paragraphs set their direction directly instead of starting with an invisible U+200E or U+200F mark, so text extracted from a PDF no longer contains it. A right-to-left paragraph that starts with an inline box before any text still gets the mark.

### Support `font-optical-sizing`

Variable fonts with an `opsz` axis now set it to the font size, as Chrome does, so fonts like Inter draw their display cut at large sizes. `font-optical-sizing: none` or an `opsz` value in `font-variation-settings` turns this off.

### Embed fonts at their `font-variation-settings` in PDF

PDF output and the paint tree now instance a variable font at the axes `font-variation-settings` sets. Before, they used the default instance, so glyphs drew at the wrong weight or width while the layout used the right one.

## takumi-pdf@0.16.1

### Lay out paragraphs with inline elements faster

A paragraph holding spans no longer lays its text out a second time to find its baseline. Large documents lay out about 8% faster.

### Lay out inline spans faster

An inline span works out its strut from its line height alone, without building its whole text style. Large documents with many spans lay out about 5% faster.

### Lay out Latin text faster

Text below U+02C7, such as Latin, no longer scans the CJK tables to decide whether underlines skip its ink. Large documents lay out about 3% faster.

### Look up glyphs faster

Each glyph's cache key now hashes 12 bytes instead of copying a 600-byte hasher. Large documents lay out about 2% faster.

### Expand each font stack once per render

A `font-family` stack is matched against registered font subsets once per render, not on every text style. Large documents lay out about 4% faster.

### Read each glyph's ink box once

A glyph's ink box is worked out when the glyph is resolved, not on every use. Large documents lay out about 7% faster.

`ResolvedOutlineGlyph::Plain` and `ResolvedOutlineGlyph::Color` gain an `ink` field, which `outline_ink` computes from their paths.

## takumi-pdf@0.16.0

### Paint backgrounds as Chrome does

- `background-repeat: space` puts the first and last tiles on the edges. `round` rounds to the nearest tile count.
- Shorter `background-size`, `-position`, `-repeat` and `-blend-mode` lists cycle over the layers.
- `background-blend-mode` in image output blends only with the box's own layers, not with what is behind it.
- Gradient hard stops land on Chrome's pixels.
- A repeating gradient with all stops at one position paints its last color.
- A tile smaller than 1/64px paints nothing.
- SVG places tiles at exact positions. PDF no longer repeats a layer along an axis that does not repeat.
- A stray token, as in `background-repeat: repeat bogus`, makes the declaration invalid.

### Draw text decorations as Chrome does

- Underlines, overlines and line-throughs sit and size as Chrome draws them, on whole pixels.
- `text-underline-position`, `text-underline-offset` and `text-decoration-thickness: from-font` follow Chrome.
- `text-decoration: overline 12px red` keeps its thickness.
- A decoration reaches the text of every in-flow box inside its element, and nested decorations all draw.
- An underline uses the baseline and font of the element that sets it.
- `text-decoration-skip-ink` also cuts overlines and wavy lines. It no longer cuts around CJK, Hangul, emoji, `/`, `\` or `_`.
- Text under `opacity`, `filter` or a blend mode keeps decorations that reach past its box.
- PDF applies a span's `opacity` to its decorations.

### Measure content widths as Chrome does

- A box sized to its content rounds its text width up to 1/64px, as Chrome's `LayoutUnit` does, not to a whole pixel.
- A shrink-to-fit box whose text wraps, such as a flex item, takes the width it wraps at.
- `width`, `height` and `flex-basis` ignore `fit-content(<length-percentage>)`, which Chrome allows only in grid tracks.

### Draw outlines as Chrome does

- An inline element's outline draws `double`, `groove`, `ridge`, `inset` and `outset`.
- An inline outline wraps the element's border box on each line, nested elements included. `plain <b>bold</b> text` now gets one.
- A wrapped inline outline follows Chrome's shape, rounds by `border-radius`, and shades 3D styles per edge.
- A one-line inline outline paints like a box outline.
- Inline outlines are no longer 1px narrower than Chrome's.
- Dashed and dotted outlines start each edge on a dash, and translucent ones no longer darken at the corners.
- A box without `border-radius` keeps square outline corners.
- PDF draws an inline outline as one contour across lines, at the element's opacity.

### Paint SVG and PDF output like the image output

- A block's background no longer paints twice, so a negative `z-index` child behind it shows.
- Spans with their own `font-size` inside a `font-size: 0` block paint.
- PDF paints text that overflows a zero-sized box, and content that overflows a `height: 0` inline-block.
- SVG path data no longer turns a second moveto into a stray line.

### Paint in CSS paint order, as Chrome does

- Positioned boxes and `z-index: auto` stacking contexts paint above later in-flow siblings. A `position: relative` box nudged over the next block stays visible.
- Floats paint above in-flow block backgrounds, and before the text they sit in.
- Text paints above every block background in its stacking context, so overflowing text stays visible.
- A box with `overflow: hidden` paints its text above later siblings' backgrounds.
- A positioned box whose containing block is outside a clip escapes the clip.
- A filtered box inside an `overflow: hidden` parent stays inside the parent.
- PDF applies a scaled or rotated clipping box's transform once, not twice.

### Show `background-clip: text` through the text Chrome shows it through

- The background shows through the text of child blocks, inline-blocks, floats and positioned children, not just the box's own text.
- Nested `background-clip: text` spans each show their background, outer span first.
- The background shows through underlines, overlines and line-throughs too.
- It shows through text at `opacity: 0` or `scale(0)`, and skips the floats and positioned boxes Chrome skips.
- `text-shadow` paints on top of the clipped background, and a child's own background covers it.
- A clipped background renders about 30% faster and peaks 35% lower in memory.

### Register WOFF2 fonts faster

Registering a WOFF2 font takes about 25% less time, so a cold start with a large CJK font is ready sooner.

### Fit text with `text-fit` as Chrome does

- Fixed `letter-spacing`, `word-spacing` and `line-height` keep their size when a line scales.
- Text strokes, shadows and decorations scale with the line.
- A line within 2px of its box stays unscaled, and so does one that spacing already fills.
- `grow` limits under 100% and `shrink` limits over 100% stop scaling.
- Indented, centered, right-aligned and right-to-left lines land where Chrome puts them, on the same baseline.
- Span offsets and backgrounds no longer scale twice.
- With `text-overflow: ellipsis`, a line that fits once scaled gets no ellipsis.

### Repeat table headers and footers on every page, as Chrome prints them

- A `tfoot` no taller than a quarter of the page repeats below the last row on each page. The structure tree keeps one `TFoot`.
- A header or footer group repeats only with `break-inside: avoid`. The `thead` and `tfoot` presets set it, as Chrome's print stylesheet does.

### Share computed styles between boxes

Boxes share the parts of their computed style they leave unchanged, as Chrome's do. An 11,000-row table takes about a quarter less memory.

`ComputedStyle` fields now sit in groups named after Blink's, each behind an `Rc`. `RenderContext::style` is an `Rc<ComputedStyle>`, and `RenderContextBuilder::style` still takes a `Box<ComputedStyle>`.

```rust
let width = style.box_data.width;
let color = style.inherited_data.color;

Rc::make_mut(&mut style.box_data).width = Size::default();
```

### Snap boxes to pixels as Chrome does

- Layout keeps exact positions and sizes. Backgrounds, borders, shadows, outlines, images and clips snap to whole pixels as they paint. `measure` reports the exact sizes.
- A line keeps its exact height: `line-height: 1.2` at 32px is 38.39px, not 39px.
- Background and mask tiles land on Chrome's pixels, including `round`, `space`, `cover` and `contain`.
- A tile outside the box, or under an opaque border, no longer paints.
- An image drawn through a rounded or clipped shape no longer shifts half a pixel and blurs.
- A solid edge between pixels, such as a decoration line, covers its edge pixels in proportion.
- A root with `display: flex`, `grid` or `flow-root` and no width fills the viewport.
- An ellipsis follows the last word, not the trailing space.
- In PDF, a repeating layer behind `background-clip: text` repeats across the whole text.
- A `background-clip: text` background no longer shows past its box through a text stroke, and no longer scales with `text-fit`.

### Mix absolute units in `calc()`

`calc()` now adds `cm`, `mm`, `in`, `pt`, `pc` and `q` together, so mixing them no longer fails to parse.

### Read node trees faster

Each node's keys are read once. A key that belongs to another node type and comes before `type` is now parsed, so a malformed one is an error.

### Paint visible children of `visibility: hidden` elements

A `visibility: hidden` element hides only itself, so children with `visibility: visible` paint. `opacity: 0` and `display: none` still hide the whole subtree.

### Place absolutely positioned boxes as Chrome does

- An `auto`-width absolute box wraps to the space its insets and margins leave, so `left: 50%` text stays inside the edge.
- An absolute box inside a paragraph starts where it sits in the line, and the text around it stays on one line.
- An absolute box inside a `position: relative` span takes its offsets from the span and no longer breaks the word around it.

### Collapse white space as Chrome does

- `&nbsp;`, U+3000 and other non-CSS spaces keep their width.
- Spaces at the start and end of a paragraph drop out, as in `<span>Label </span>` inside a flex row.
- A float at the start of a line no longer keeps the space after it.
- `<br>` always starts a new line.
- Under `pre-wrap`, `"A \nB"` renders as two lines.
- A mixed-direction line hangs its trailing space past the edge.

### Refuse node trees nested deeper than 512 levels

A deeper tree now fails with `nodes nest deeper than 512 levels`, Blink's parser limit, instead of crashing the process. `takumi_html::DEFAULT_MAX_DEPTH` is now `takumi_core::layout::node::MAXIMUM_DOM_TREE_DEPTH`. `takumi::DEFAULT_MAX_DEPTH` keeps its name.

### Lay out inline boxes as Chrome does

- A span's background, border and outline cover its font's ascent and descent, not the whole `line-height`.
- A span's background and outline stop at the last glyph before a line break.
- A span's side margins push the text beside it apart. Negative margins overlap it.
- Each line is at least the block's own `line-height`. Under `line-height: normal`, a fallback font such as emoji grows the line.
- A span with a larger font makes its line as tall as Chrome does.
- `vertical-align` on a span moves its text, background and children, and grows the line to fit.
- Text in a `sub` or `super` span no longer kerns against the text around it.
- A span's `background-image` paints as one strip across its lines, so gradient text inside a heading shows.
- `box-decoration-break: clone` repeats a span's border, padding and corners on every line.
- A `direction: rtl` span puts its start padding, border and margin on the right.

### Drop WOFF1 from the wasm packages

`@takumi-rs/wasm` and `takumi-pdf` load TTF, OTF, and WOFF2, but not WOFF1. `@takumi-rs/core` keeps WOFF1.

### Clip as Chrome does

- An element with more than one of `clip-path`, `mask-image` and a clipping `overflow` applies all of them.
- A `path()` clip that fails to parse no longer hides the element.
- A rounded image clips to the curve of its content box.
- In `circle()`, a percentage radius and `closest-side` or `farthest-side` resolve as CSS defines them.
- SVG resolves a percentage vertical radius in `inset(... round ...)` against the box height.

### Break pages as Chrome prints them

- A page break falls between line boxes. A paragraph taller than a page no longer leaves the first page blank, and short paragraphs can split too.
- A line or image taller than a page moves to the next page, then continues over the pages after it.
- A box's top and bottom border and padding stay with its first and last line.
- A table row whose cell padding does not fit moves to the next page whole.

### Paint `box-shadow` as Chrome does

- The first shadow in the list sits on top in every output.
- A spread shadow follows Chrome's corner radius, so square corners stay square.
- Shadows of scaled or rotated boxes blur by the transformed radius.
- Image output keeps fractional offsets.
- SVG no longer paints an outer shadow under a translucent box.
- PDF shadows no longer leave a hairline, fill a box they moved off, or stack their alpha when blurred.

### Lay text out once

Painting, page breaking and PDF output reuse the lines layout made instead of shaping text again. A 1,500-page document renders in about a sixth less memory and a tenth less time.

`FragmentItems`, `OwnContent::fragment_items` and `RenderContext::release_shaped_text` are new.

### Paint text shadows as Chrome does

- A `text-shadow` on a `<span>` paints.
- Shadows cover text decorations too, and the first listed shadow sits on top.
- Color emoji cast a shadow in their own shape.
- Each run paints underline, text, then line-through, as CSS orders them.
- Wavy, dotted and dashed underlines keep their shadows.
- PDF blurs text shadows, colors emoji shadows, and no longer repeats shadowed words when text is copied.

### Write smaller PDFs

- Each page writes only the clips and groups it shows. Documents with `overflow: hidden` or `opacity` come out smaller, and long ones render faster.
- A tagged PDF with more than 65,535 structure elements compresses its structure tree. An 11,000-row table is 3.5 times smaller.

### Lay out tables as Chrome does

- An `auto`-width table shrinks to fit its content.
- Columns share the width through the CSS table algorithm. Percentage, `min-width` and `max-width` cell widths size their columns.
- `width: min-content`, `max-content`, `fit-content` and `stretch` size a table from its columns.
- `thead`, `tbody` and `tfoot` default to `vertical-align: middle`, as in Chrome.
- `vertical-align: baseline` cells line up on the row's deepest baseline.
- A non-cell inside a row gets wrapped in an anonymous cell.
- Tables with more than 10,000 rows lay out every row. Rows past the 10,000th used to pile up.

### Draw double, dotted, dashed, and wavy text decorations

`text-decoration-style` accepts `double`, `dotted`, `dashed`, and `wavy`, drawn as Blink draws them. Tailwind gains `decoration-solid`, `decoration-double`, `decoration-dotted`, `decoration-dashed`, and `decoration-wavy`.

### Fix PDF rendering bugs

- Gradients in repeated backgrounds no longer draw 4/3 too large or off-center.
- Text under `opacity`, a blend mode or a mask keeps the right edge of its last glyph.
- A tagged PDF no longer panics on an inline `<img>` with a background, border or shadow.
- A text shadow past a page break stays with its line instead of vanishing.

### Paint borders as Chrome does

- Sides of different colors or styles meet on the corner diagonal, with Chrome's miters and no seams.
- Rounded borders with mixed sides clip each side and cut corners as Chrome does.
- Dashes start at the corners.
- `dotted` up to 3px wide draws square dots that end on a whole dot. Wider ones draw round dots.
- `inset`, `outset`, `groove` and `ridge` shade like Chrome and stay visible on very dark colors.
- A `double` side under 3px and a 1px `groove` or `ridge` paint solid.
- `corner-shape` values such as `bevel`, `scoop` and `notch` keep the border's thickness around the corner.
- A collapsed table border stays square, even with `border-radius`.
- `background-clip: border-area` follows dashed, dotted and double borders.
- PDF draws dashed and dotted sides next to sides of other styles.

### Shrink the wasm packages

`@takumi-rs/wasm` is about 18 KB smaller gzipped, and `takumi-pdf` about 7 KB.

## takumi-pdf@0.15.0

### Trim an image to its content edge curve

A `border-radius` on an image was ignored unless the box also clipped its overflow, so a rounded picture came out square.

### Fill a page exactly with a box as tall as the content window

A box sized to the page snapped to a whole pixel past the fractional window and opened an empty page after it.

### Write the CIDFont default width as an integer

The PDF spec types `/DW` as an integer. Poppler ignores a real one and falls back to the spec default of 1000, so every glyph the entry covered advanced far too far in poppler-based viewers.

### Text wraps at the width layout measured

A box with a fractional width wrapped its text against the pixel-snapped content box at paint, so a nearly full line pushed its last word onto a line the layout never reserved and it overlapped the block below.

### Override the header and footer on some pages

`pages` takes `{ first, last, odd, even }`, each overriding `header` or `footer` for the pages it covers. A band is a node tree, or `false` to draw none.

### Accept object stylesheet rules in PDF bindings

Match the PDF binding's `css` type to the object rules already accepted at runtime.

### Render uncovered characters with `uncoveredText`

A single character no registered font covers used to fail the whole render, which leaves a server rendering text someone else wrote with no way through. `uncoveredText: "placeholder"` draws the font's glyph 0 instead, and `"blank"` draws nothing. Both keep the character's width, so the line does not reflow. The default stays `"error"`.

Every PDF/A level and both PDF/UA levels forbid glyph 0, so `"placeholder"` is now rejected there by name instead of failing as a generic write error.

### Forced page breaks no longer open empty pages

A `break-before: page` or `break-after: page` with only spacing beside it on its page is dropped, and trailing spacing past the last content box no longer opens a page.

### Resolve viewport units against the page area in paged output

`100vh` in paged content was `0` because the content column lays out at unbounded height; it now equals the page area height, as in print media.

## takumi-pdf@0.14.1

### Resolve a browser-only entry in client builds

Bundlers that resolve the `browser` condition (Vite client, webpack web) now
get `bundlers/browser.mjs`, which only fetches the `.wasm` asset by `import.meta.url`. Client builds
with `noExternal` stop failing with `Cannot bundle Node.js built-in "node:fs/promises"`.
The Vite server entry reads the asset through `process.getBuiltinModule`, so
no bundler sees a Node import. These packages, plus `takumi-js` and `@takumi-rs/image-response` on top of them, now require Node 20.19 or newer.

## takumi-pdf@0.14.0

### Apply `@media print` rules to PDF output

PDF renders now match the `print` media type, and image renders match
`screen`. `Viewport::media_target` picks which one a render resolves against.

### Paint inline span backgrounds

A `display: inline` span fills its `background-color` under the text, one
rounded fragment per line. Horizontal padding reserves space on the line and
the fragment grows by it, so badge and pill markup renders instead of
silently dropping the background.

### Select output pages with pageRanges

`pageRanges` keeps only the listed pages, like a print dialog. Each entry is
a 1-based page number or an inclusive `{ from, to }` span. Layout and page
counters still run over the whole document, so a kept page shows the numbers
it would in full output.

## takumi-pdf@0.13.0

### Tag tables with PDF structure elements

Tagged output maps `<table>` markup to `Table`, `THead`, `TBody`, `TFoot`,
`TR`, `TH` and `TD` structure elements, with `Caption` for `<caption>`,
`Scope` on header cells, and `RowSpan`/`ColSpan` on spanning cells. A table
that spans pages stays one `Table` element. Screen readers navigate the
table by row and column.

## takumi-pdf@0.12.0

### Resolve `tw` utilities through CSS variables

Utilities now read the CSS variables Tailwind compiles them to, falling back to the built-in value. Define tokens in `:root`. `--color-brand-500` makes `bg-brand-500` work, and spacing, fonts, shadows, animations and breakpoints follow the same rule.

Gradients now match Tailwind on two counts. Stops alone no longer paint without `bg-linear-*`, `bg-radial` or `bg-conic`, and a missing `to` stop fades to `transparent`.

### Let stylesheet rules win over `tw` utilities

Utilities now sit in the last cascade layer, below unlayered CSS and above rules in a named `@layer`. Important reverses that order. An important utility beats unlayered important CSS but loses to one in a named layer. Inline important declarations stay on top. A template that relied on `tw` beating a matching rule needs a fix. Move that rule into a layer, or mark the utility `!`.

### Parse `@theme` blocks as `:root` rules

A Tailwind v4 source stylesheet now works in `css` without compiling it first. `@theme` declarations land on `:root`, and `@keyframes` inside the block register. Modifiers like `reference` read the same way. The `prefix()` modifier is not supported.

### Name takumi in the PDF's `/Producer`

Every rendered PDF now carries `takumi-pdf` and its version in the info
dictionary's `/Producer` and in XMP's `pdf:Producer`, which identifies the
renderer that wrote the file. Documents that set no metadata get it too.

### Compose filters and transforms through custom properties

Filter, translate, scale and grid-line utilities now compose through `--tw-*` variables like Tailwind's compiled CSS. Stacked filters follow Tailwind's fixed chain order instead of class order.

### Embed opaque PNG images without decoding them

A PNG with no alpha channel now goes into the PDF as its own compressed stream
instead of being decoded and recompressed. Paletted sources keep their palette
as an `/Indexed` colour space rather than widening every pixel to RGB.

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

## takumi-pdf@0.2.0

### Publish takumi-pdf, the wasm PDF package

`render(jsx)` turns a node tree or JSX into a paged PDF with selectable text and embedded subset fonts, on Node, Bun, and Cloudflare Workers. Options mirror Puppeteer's `page.pdf()`: `size` (`"a4"`, `"letter"`, `{ width, height }`), `landscape`, per-side margins, and repeating header/footer bands with Chromium-style `pageNumber`/`totalPages` class hooks and CSS counter styles, while `viewport` renders a fixed single page instead. Fonts, images, and stylesheets round out the options.
