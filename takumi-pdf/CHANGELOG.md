## takumi-pdf@0.16.0

### Shrink the wasm packages

Borders, outlines, text and decorations now compile once for every output format instead of once per backend. `@takumi-rs/wasm` is about 18 KB smaller gzipped, and `takumi-pdf` about 7 KB.

### Place absolutely positioned boxes as Chrome does

- An `auto`-width absolute box wraps its content to the containing block's width minus its insets and margins, so `left: 50%` text no longer runs past the right edge.
- An absolute box inside a paragraph keeps the text around it on one line and starts where it sits in that line.
- An absolute box inside a `position: relative` span takes its offsets from that span's box, and no longer lets a line break inside the word around it.

### Measure content widths as Chrome does

A box sized to its content now takes its text's width rounded up to a 64th of a pixel, as Chrome's `LayoutUnit` does, rather than to a whole pixel. A shrink-to-fit box whose text wraps, such as a flex item, now takes the width it wraps at rather than its widest line.

`width`, `height` and `flex-basis` no longer accept `fit-content(<length-percentage>)`. Chrome treats it as invalid there and keeps it for grid tracks, so the declaration is now ignored.

### Draw double, dotted, dashed, and wavy text decorations

`text-decoration-style` and the `text-decoration` shorthand now accept `double`, `dotted`, `dashed`, and `wavy`, drawn with Blink's spacing and wave shape, and Tailwind gains `decoration-solid`, `decoration-double`, `decoration-dotted`, `decoration-dashed`, and `decoration-wavy`.

### Drop WOFF1 decoding from the wasm packages

`@takumi-rs/wasm` and `takumi-pdf` load TTF, OTF, and WOFF2 but no longer decode WOFF1. `@takumi-rs/core` keeps WOFF1.

### Read node trees faster

Each node's keys are read once instead of being buffered first. A key another node type owns that comes before `type` is now parsed, so a malformed one is an error instead of being ignored.

### Lay out and snap boxes to pixels as Chrome does

- Background and mask tiles are sized, positioned and snapped to pixels in 1/64px layout units, as Chrome does. `round`, `space`, `cover` and `contain` tiles now land on the same pixels as Chrome's.
- A tile that sits outside the box, or under an opaque border, no longer paints there.
- A repeating layer seen through `background-clip: text` in PDF repeats across the whole text instead of showing one tile.
- An image or background drawn through a rounded or clipped shape no longer shifts half a pixel and blurs in the image output.
- Layout keeps boxes at their exact positions and sizes. Backgrounds, borders, shadows, outlines, images and overflow clips snap to whole pixels as they paint, where Chrome snaps them. `measure` reports the unrounded sizes.
- A line of text keeps its exact height, so a `line-height: 1.2` line at 32px is 38.39px tall instead of 39px.
- A `background-clip: text` background no longer shows past its box through a text stroke, and no longer scales with `text-fit`.
- A root with `display: flex`, `grid` or `flow-root` and no width fills the viewport, as a block-level box does in Chrome.
- An ellipsis follows the last word directly instead of the line's trailing space.
- A solid rectangle whose edge falls between pixels, such as a decoration line or an inline box, covers its edge pixels in proportion in the image output, as the SVG output does.

### Render decorated inline images in tagged PDFs

A tagged PDF panicked with "can't start marked content twice" when an inline `<img>` had a background, border, or box shadow. The decorations now paint as artifacts outside the image's own tag.

### Scale gradient transforms to the page unit

A gradient inside a tiling pattern was built in pixels while its matrix resolves against the page in points, so every gradient drew 4/3 too large and a radial or conic one also landed off-centre.

### Run a line taller than a page on over the pages after it

In paged PDF output, a line, image or other unsplittable box taller than a page now moves to the next page when content precedes it, then continues over the pages after it, as Chrome prints it. Before, the page cut through it where it fell, and a tall line drew only on the page holding its baseline, losing the rest.

### Show a box's `background-clip: text` background through the text of every box inside it

A box with `background-clip: text` now shows its background through the text of its child blocks, inline-blocks, floats and positioned children, not just its own inline text, as in Chrome. It paints with the rest of the background, so `text-shadow` lands on top of it and a child's own background covers it.

### Clip as Chrome does

- A percentage corner radius in `clip-path: inset(... round ...)` resolves its vertical radius against the box height in the SVG output.
- A `path()` clip that cannot be parsed no longer hides its element in the image output.
- An element with more than one of `clip-path`, `mask-image` and a clipping `overflow` applies all of them in the image output.
- A rounded image clips to the curve of its content box, the border radius less the border and padding.
- `circle()` in `clip-path` and `offset-path` resolves a percentage radius against the box's diagonal over √2, and `closest-side` or `farthest-side` against all four sides.

### Repeat a `box-decoration-break: clone` span's edges on every line

An inline span with `box-decoration-break: clone` now draws its border, padding and corner radii on every line it wraps onto, and each line makes room for the repeated start edge, as in Chrome. Before, the wrapped lines kept `slice`'s open edges.

### Lay out tables as Chrome does

- A block-level `auto`-width table shrinks to fit its content instead of filling its container.
- Columns share the table's width through the CSS table width algorithm, so a table narrower than its content no longer squeezes a column to its minimum. Percentage, `min-width` and `max-width` cell widths constrain their columns instead of resizing the cell inside them.
- The HTML presets give `thead`, `tbody` and `tfoot` Chrome's `vertical-align: middle`, which rows and cells inherit, and a cell that holds only text follows it.
- Cells with `vertical-align: baseline` line their first lines up on the row's deepest baseline, whatever their fonts, padding or borders.
- An element inside a row that is not a table cell sits in an anonymous cell, as CSS table fixup puts it.
- A table's `width: min-content`, `max-content`, `fit-content` and `stretch` size it from its column grid, where they used to act as `auto`.

### Draw outlines as Chrome does

- An inline element's `outline` draws `double`, `groove`, `ridge`, `inset` and `outset`, which used to paint nothing.
- Dashed and dotted outlines start each edge on a dash, and a translucent dashed outline no longer darkens where its dashes overlap at the corners.
- PDF draws the `outline` of an inline element as one contour across line breaks, at the element's opacity.
- An outline around a box without `border-radius` keeps square corners under `outline-offset` and `outline-width`.
- A one-line inline outline paints like a box outline, so its 3D styles shade like a border.
- An inline outline wraps the element's border box on each line, including its padding, border and nested elements. `plain <b>bold</b> text` used to get no outline at all.
- A wrapped `solid` or `double` inline outline rounds its corners by the element's `border-radius`.
- A wrapped inline outline follows Chrome's shape. Its inner edge shrinks from the outer contour, dashed and dotted outlines round their corners, and 3D styles shade each edge with aliased mitred corners.
- An inline element's outline snaps its width and height from 1/64px layout units, so it no longer ends up 1px narrower than Chrome's.

### Show every `background-clip: text` span's background through nested text

When `background-clip: text` spans nest, each span now shows its background through all the text inside it, outer span first, as in Chrome. Before, only the innermost span's background showed. An inner span's own background now covers the outer span's clipped background, and `text-shadow` paints over the clipped background.

### Paint `box-shadow` as Chrome does

- The first shadow in a list sits on top in SVG and image output, as it already did in PDF.
- A spread shadow follows the outset-adjusted border radius. A square corner stays square, and a small radius grows less than the spread.
- SVG no longer paints an outer shadow under a translucent box.
- A PDF outer shadow no longer leaves a hairline around the box, and no longer fills the box when the offset moves the shadow clear of it.
- The image output places shadows at fractional offsets instead of rounding them toward zero.
- A blurred translucent `box-shadow` or `text-shadow` in PDF applies its color's alpha once. The bands that fake the blur used to stack it.
- The image output blurs the shadows of a scaled or rotated box by the transformed radius, as Skia maps a blur through the transform.

### Paint in CSS paint order, as Chrome does

- Positioned boxes and stacking contexts at `z-index: auto` paint above later in-flow siblings, and floats above in-flow block backgrounds, following CSS 2.1 Appendix E. A `position: relative` box nudged over the next block no longer disappears under it.
- Text paints after the backgrounds of every in-flow block and float in its stacking context, so overflowing text stays above the next block's background. Flex and grid items still paint whole, like inline blocks.
- A float inside text paints before that text.
- A box with `overflow: hidden` paints its text above the backgrounds of later siblings. A positioned box whose containing block sits outside it escapes its clip, even under an `opacity` in between.
- A filtered box inside an `overflow: hidden` parent stays inside the parent's edges in the image output.
- PDF places the content of a scaled or rotated box with `overflow: hidden` once, where it used to apply the box's transform twice.

### Lay out inline boxes as Chrome does

- A span's background, border and outline cover its own font's ascent and descent on every line, even when it holds only other spans or padding, instead of the whole `line-height`. A wrapped outline joins its lines only where they touch.
- A span's background and outline stop at the last glyph before a line break, like its text decoration.
- A span's left and right margins push the text beside it apart, on the parent's background, and a negative margin overlaps it with its neighbors.
- Each line grows to the block's own `line-height` and font, even when it holds only a span with a smaller line height. Under `line-height: normal`, a fallback font, such as an emoji font, grows the line by its own line spacing. Under any other line height it does not.
- A span with a larger font than its text makes its line as tall as Chrome does.
- `vertical-align` on an inline span moves its text, background and children, and grows the line to fit. `sub` and `super` shift by the parent's font size, percentages refer to the span's own line height, and `middle` uses the parent font's x-height. Offsets land on Chrome's 1/64px steps, and a `top` or `bottom` box aligns against the borders of the spans on its line.
- Text inside a span aligned off the baseline, like `sub` or `super`, no longer kerns against the text around it.
- A `background-image` on a span paints across its lines as one continuous strip, and `background-clip: text` shows it through the glyphs, so gradient text inside a heading no longer disappears.

### Keep a text shadow on the page of its line

A text shadow offset past a page cut was assigned to the next page and drawn above its top edge, so it vanished from both pages. It now follows the line it shadows.

### Sum a `calc()`'s absolute lengths into pixels

`calc()` now adds `cm`, `mm`, `in`, `pt`, `pc` and `q` into one pixel term, as CSS simplifies them, so an expression mixing several absolute units no longer fails to parse.

### Paint visible children of `visibility: hidden` elements

A `visibility: hidden` element now hides only its own box, text, image, and outline, so descendants that set `visibility: visible` paint, as browsers show them. `opacity: 0` and `display: none` still hide the whole subtree.

### Show a `background-clip: text` span's background through its text decorations

A span with `background-clip: text` now shows its background through its underlines, overlines and line-throughs as well as its glyphs, even when the decoration color is transparent, as in Chrome. Before, the decorations showed nothing.

### Match Chrome on which boxes a `background-clip: text` background shows through

The background now shows through the text of a box inside it at zero opacity or under a `scale(0)` transform, as in Chrome. It no longer shows through a float with its own opacity, transform or position, or an absolutely positioned box whose containing block is outside it, which Chrome leaves out.

### Draw text decorations as Chrome does

- Underlines, overlines and line-throughs round their top edge to the nearest pixel and their thickness down to a whole pixel, at least 1px.
- PDF applies a span's `opacity` to its text decorations.
- An overline rests on top of the text. A line-through sits a third of the ascent above the baseline instead of at the font's strikeout position.
- `text-underline-position: auto` puts the underline half its thickness, at least 1px, below the baseline. `from-font` keeps the font's underline position, `under` leaves a pixel below the em box, and a set `text-underline-offset` drops the `auto` gap.
- `text-decoration-thickness: from-font` uses the font's underline thickness for every line.
- `text-decoration: overline 12px red` keeps a thickness written right after the line keyword.
- A line spans the text exactly, with antialiased ends, instead of widening to whole pixels. A double or wavy line keeps its offset from the unrounded thickness.
- `text-decoration-skip-ink` cuts on whole device pixels, also cuts overlines, and looks for glyphs across the whole band a wavy or double line paints. It no longer cuts around CJK characters, Hangul, emoji, `/`, `\` or `_`.
- Text under `opacity`, `filter` or a blend mode keeps the part of a decoration line that reaches past its box.
- An underline sits against the baseline and font of the element that sets it, and a line-through takes its height from that element's font.
- A `text-decoration` reaches the text of every in-flow box inside the element that sets it, and nested decorations all draw. Inline blocks, floats, absolutely positioned boxes and outside list markers still stop it.
- A `list-style-position: inside` marker in a box at a fractional position snaps its decorations with the text beside it, instead of from the page origin.

### Paint SVG and PDF output like the image output

- A block's loose text no longer paints the block's background a second time, so a negative `z-index` child behind it stays visible.
- Spans that set their own `font-size` inside a `font-size: 0` block paint.
- PDF paints text that overflows a box with no width or height, and no longer writes `NaN` for a zero-sized run.
- PDF paints the content that overflows an `inline-block` with `height: 0`.
- SVG path data keeps the letter of a moveto that follows another, so the second move no longer becomes a stray line.

### Collapse white space as Chrome does

- `&nbsp;`, U+3000 and other spaces outside CSS's document white space keep their width under `white-space-collapse: collapse`.
- Collapsible spaces at the start and end of a paragraph drop out, such as indented HTML or `<span>Label </span>` inside a flex row.
- A float at the start of a line no longer keeps the space after it.
- `<br>` always starts a new line, even with the style presets off or `white-space` set to collapse newlines.
- A right-to-left line that ends in left-to-right words, or the reverse, hangs its line-end space past the edge and leaves it out of decorations and backgrounds.
- Under `white-space: pre-wrap`, a newline right after a space ends the line. `"A \nB"` used to render as one line.

### Paint backgrounds as Chrome does

- `background-repeat: space` spreads the leftover room so the first and last tiles touch the edges, and a single tile follows `background-position`. `round` rounds the tile count to the nearest whole number. Both keep tiling across a painting area larger than the positioning area.
- Shorter `background-size`, `-position`, `-repeat` and `-blend-mode` lists cycle over the layers instead of repeating their last value.
- SVG places tiles at exact positions and positions `background-clip: text` layers by `background-origin`. PDF no longer repeats a layer along an axis that does not repeat.
- `background-blend-mode` in the image output blends only with the box's own layers and color, not with what sits behind the box.
- Gradients sample each pixel at its center, so hard stops land on the same pixels as Chrome's. A tile of fractional size blends across its seams as Chrome's does.
- A repeating gradient whose stops all sit at one position paints solid in the last stop's color.
- A tile smaller than 1/64px paints nothing, as Chrome's `LayoutUnit` sizes truncate it to empty, instead of listing millions of tiles.
- `background-repeat: repeat bogus`, or a stray token after the last item of `background-size`, `-position` or `-blend-mode`, makes the declaration invalid, as in browsers.

### Paint borders as Chrome does

- Sides that differ in color or style meet along the corner diagonal, and adjacent sides of the same color leave no seam.
- A dashed side runs the full length of the box, so its dashes start at the corners.
- A `dotted` border up to 3px wide draws square dots spaced one dot apart and ends each side on a whole dot. Wider dotted lines keep round dots inside the box.
- `inset`, `outset`, `groove` and `ridge` darken the shadowed edges and keep the color on the lit ones. A very dark color lightens instead, so both edges stay visible.
- PDF draws dashed and dotted sides next to sides of other styles, which it used to leave out.
- `background-clip: border-area` keeps the background only where a dashed, dotted or double border paints, and no longer paints a translucent border's color twice in the image output.
- Square sides with mixed styles, colors or opacities meet with Chrome's miters. A uniform dashed or dotted border shows no diagonal seam, and translucent sides no longer blend twice where they overlap.
- Rounded borders with mixed colors, styles or opacities clip each side and cut their corners as Chrome does.
- A `double` side under 3px and a 1px `groove` or `ridge` side paint solid.
- A collapsed table border stays square even with `border-radius`, as the spec requires.
- Borders and padding-box clips with `corner-shape` values such as `bevel`, `scoop` and `notch` keep one thickness around each corner. Opposite concave corners that would overlap shrink as Chrome shrinks them.

### Fit text with `text-fit` as Chrome does

- Fixed `letter-spacing` and `word-spacing` keep their size when `text-fit` scales a line, and a fixed `line-height` keeps its height around the scaled glyphs.
- `-webkit-text-stroke`, `text-shadow` and text decorations scale with the line.
- A line within 2px of its box stays unscaled, counting its `text-indent` under `per-line` and `per-line-all`. A `grow` limit under 100% or a `shrink` limit over 100% stops the text from scaling.
- A line keeps its `text-indent`, and `center`, `right` and right-to-left lines land where Chrome puts them.
- Spans keep their `vertical-align` offsets and backgrounds instead of scaling them a second time.
- Text sits on the baseline Chrome paints it at, and its glyphs snap to the same pixel rows.
- A line whose fixed `letter-spacing` or `word-spacing` already fills its box stays unscaled, as in Chrome, where it used to vanish.

### Paint text shadows as Chrome does

- A `text-shadow` set on a `<span>` paints. Only the block's own shadow used to.
- Text shadows also shadow underlines, overlines and line-throughs, and the first listed shadow sits on top.
- Each run paints its underline and overline, then its text, then its line-through, before the next run, as CSS 2 orders them.
- PDF text shadows no longer repeat the shadowed words when the text is copied or extracted.
- Color bitmap glyphs, such as Noto Color Emoji, cast a shadow that follows the glyph's shape in image and SVG output.
- A blurred `text-shadow` fades out in PDF output, through the same stepped bands a blurred `box-shadow` uses, and so does the shadow its decorations cast.
- PDF shadows color glyphs, such as COLR and bitmap emoji, as a silhouette in the shadow color, blurred as Chrome blurs it.
- A wavy, dotted or dashed underline casts its shadow at the shadow's offset instead of losing it to the clip that shapes the line.

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
