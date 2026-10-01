## takumi-pdf@0.16.0

### Paint SVG and PDF output like the image output

- A block's loose text no longer paints the block's background a second time, so a negative `z-index` child behind it stays visible.
- Spans that set their own `font-size` inside a `font-size: 0` block paint.
- PDF paints text that overflows a box with no width or height, and no longer writes `NaN` for a zero-sized run.
- PDF paints the content that overflows an `inline-block` with `height: 0`.
- SVG path data keeps the letter of a moveto that follows another, so the second move no longer becomes a stray line.

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

### Paint `box-shadow` as Chrome does

- The first shadow in a list sits on top in SVG and image output, as it already did in PDF.
- A spread shadow follows the outset-adjusted border radius. A square corner stays square, and a small radius grows less than the spread.
- SVG no longer paints an outer shadow under a translucent box.
- A PDF outer shadow no longer leaves a hairline around the box, and no longer fills the box when the offset moves the shadow clear of it.
- The image output places shadows at fractional offsets instead of rounding them toward zero.
- A blurred translucent `box-shadow` or `text-shadow` in PDF applies its color's alpha once. The bands that fake the blur used to stack it.
- The image output blurs the shadows of a scaled or rotated box by the transformed radius, as Skia maps a blur through the transform.

### Render decorated inline images in tagged PDFs

A tagged PDF panicked with "can't start marked content twice" when an inline `<img>` had a background, border, or box shadow. The decorations now paint as artifacts outside the image's own tag.

### Paint text shadows as Chrome does

- A `text-shadow` set on a `<span>` paints. Only the block's own shadow used to.
- Text shadows also shadow underlines, overlines and line-throughs, and the first listed shadow sits on top.
- Each run paints its underline and overline, then its text, then its line-through, before the next run, as CSS 2 orders them.
- PDF text shadows no longer repeat the shadowed words when the text is copied or extracted.
- Color bitmap glyphs, such as Noto Color Emoji, cast a shadow that follows the glyph's shape in image and SVG output.
- A blurred `text-shadow` fades out in PDF output, through the same stepped bands a blurred `box-shadow` uses, and so does the shadow its decorations cast.
- PDF shadows color glyphs, such as COLR and bitmap emoji, as a silhouette in the shadow color, blurred as Chrome blurs it.
- A wavy, dotted or dashed underline casts its shadow at the shadow's offset instead of losing it to the clip that shapes the line.

### Draw double, dotted, dashed, and wavy text decorations

`text-decoration-style` and the `text-decoration` shorthand now accept `double`, `dotted`, `dashed`, and `wavy`, drawn with Blink's spacing and wave shape, and Tailwind gains `decoration-solid`, `decoration-double`, `decoration-dotted`, `decoration-dashed`, and `decoration-wavy`.

### Scale gradient transforms to the page unit

A gradient inside a tiling pattern was built in pixels while its matrix resolves against the page in points, so every gradient drew 4/3 too large and a radial or conic one also landed off-centre.

### Read node trees faster

Each node's keys are read once instead of being buffered first. A key another node type owns that comes before `type` is now parsed, so a malformed one is an error instead of being ignored.

### Paint in CSS paint order, as Chrome does

- Positioned boxes and stacking contexts at `z-index: auto` paint above later in-flow siblings, and floats above in-flow block backgrounds, following CSS 2.1 Appendix E. A `position: relative` box nudged over the next block no longer disappears under it.
- Text paints after the backgrounds of every in-flow block and float in its stacking context, so overflowing text stays above the next block's background. Flex and grid items still paint whole, like inline blocks.
- A float inside text paints before that text.
- A box with `overflow: hidden` paints its text above the backgrounds of later siblings. A positioned box whose containing block sits outside it escapes its clip, even under an `opacity` in between.
- A filtered box inside an `overflow: hidden` parent stays inside the parent's edges in the image output.
- PDF places the content of a scaled or rotated box with `overflow: hidden` once, where it used to apply the box's transform twice.

### Paint visible children of `visibility: hidden` elements

A `visibility: hidden` element now hides only its own box, text, image, and outline, so descendants that set `visibility: visible` paint, as browsers show them. `opacity: 0` and `display: none` still hide the whole subtree.

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

### Place absolutely positioned boxes as Chrome does

- An `auto`-width absolute box wraps its content to the containing block's width minus its insets and margins, so `left: 50%` text no longer runs past the right edge.
- An absolute box inside a paragraph keeps the text around it on one line and starts where it sits in that line.
- An absolute box inside a `position: relative` span takes its offsets from that span's box, and no longer lets a line break inside the word around it.

### Sum a `calc()`'s absolute lengths into pixels

`calc()` now adds `cm`, `mm`, `in`, `pt`, `pc` and `q` into one pixel term, as CSS simplifies them, so an expression mixing several absolute units no longer fails to parse.

### Lay out inline boxes as Chrome does

- A span's background, border and outline cover its own font's ascent and descent on every line, even when it holds only other spans or padding, instead of the whole `line-height`. A wrapped outline joins its lines only where they touch.
- A span's background and outline stop at the last glyph before a line break, like its text decoration.
- A span's left and right margins push the text beside it apart, on the parent's background, and a negative margin overlaps it with its neighbors.
- Each line grows to the block's own `line-height` and font, even when it holds only a span with a smaller line height. Under `line-height: normal`, a fallback font, such as an emoji font, grows the line by its own line spacing. Under any other line height it does not.
- A span with a larger font than its text makes its line as tall as Chrome does.
- `vertical-align` on an inline span moves its text, background and children, and grows the line to fit. `sub` and `super` shift by the parent's font size, percentages refer to the span's own line height, and `middle` uses the parent font's x-height. Offsets land on Chrome's 1/64px steps, and a `top` or `bottom` box aligns against the borders of the spans on its line.
- Text inside a span aligned off the baseline, like `sub` or `super`, no longer kerns against the text around it.
- A `background-image` on a span paints across its lines as one continuous strip, and `background-clip: text` shows it through the glyphs, so gradient text inside a heading no longer disappears.

### Clip as Chrome does

- A percentage corner radius in `clip-path: inset(... round ...)` resolves its vertical radius against the box height in the SVG output.
- A `path()` clip that cannot be parsed no longer hides its element in the image output.
- An element with more than one of `clip-path`, `mask-image` and a clipping `overflow` applies all of them in the image output.
- A rounded image clips to the curve of its content box, the border radius less the border and padding.
- `circle()` in `clip-path` and `offset-path` resolves a percentage radius against the box's diagonal over √2, and `closest-side` or `farthest-side` against all four sides.

### Measure content widths as Chrome does

A box sized to its content now takes its text's width rounded up to a 64th of a pixel, as Chrome's `LayoutUnit` does, rather than to a whole pixel. A shrink-to-fit box whose text wraps, such as a flex item, now takes the width it wraps at rather than its widest line.

`width`, `height` and `flex-basis` no longer accept `fit-content(<length-percentage>)`. Chrome treats it as invalid there and keeps it for grid tracks, so the declaration is now ignored.

### Fit text with `text-fit` as Chrome does

- Fixed `letter-spacing` and `word-spacing` keep their size when `text-fit` scales a line, and a fixed `line-height` keeps its height around the scaled glyphs.
- `-webkit-text-stroke`, `text-shadow` and text decorations scale with the line.
- A line within 2px of its box stays unscaled, counting its `text-indent` under `per-line` and `per-line-all`. A `grow` limit under 100% or a `shrink` limit over 100% stops the text from scaling.
- A line keeps its `text-indent`, and `center`, `right` and right-to-left lines land where Chrome puts them.
- Spans keep their `vertical-align` offsets and backgrounds instead of scaling them a second time.
- Text sits on the baseline Chrome paints it at, and its glyphs snap to the same pixel rows.
- A line whose fixed `letter-spacing` or `word-spacing` already fills its box stays unscaled, as in Chrome, where it used to vanish.

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

### Drop WOFF1 decoding from the wasm packages

`@takumi-rs/wasm` and `takumi-pdf` load TTF, OTF, and WOFF2 but no longer decode WOFF1. `@takumi-rs/core` keeps WOFF1.

### Shrink the wasm packages

Borders, outlines, text and decorations now compile once for every output format instead of once per backend. `@takumi-rs/wasm` is about 18 KB smaller gzipped, and `takumi-pdf` about 7 KB.

### Keep a text shadow on the page of its line

A text shadow offset past a page cut was assigned to the next page and drawn above its top edge, so it vanished from both pages. It now follows the line it shadows.

### Collapse white space as Chrome does

- `&nbsp;`, U+3000 and other spaces outside CSS's document white space keep their width under `white-space-collapse: collapse`.
- Collapsible spaces at the start and end of a paragraph drop out, such as indented HTML or `<span>Label </span>` inside a flex row.
- A float at the start of a line no longer keeps the space after it.
- `<br>` always starts a new line, even with the style presets off or `white-space` set to collapse newlines.
- A right-to-left line that ends in left-to-right words, or the reverse, hangs its line-end space past the edge and leaves it out of decorations and backgrounds.
- Under `white-space: pre-wrap`, a newline right after a space ends the line. `"A \nB"` used to render as one line.

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

### Lay out tables as Chrome does

- A block-level `auto`-width table shrinks to fit its content instead of filling its container.
- Columns share the table's width through the CSS table width algorithm, so a table narrower than its content no longer squeezes a column to its minimum. Percentage, `min-width` and `max-width` cell widths constrain their columns instead of resizing the cell inside them.
- The HTML presets give `thead`, `tbody` and `tfoot` Chrome's `vertical-align: middle`, which rows and cells inherit, and a cell that holds only text follows it.
- Cells with `vertical-align: baseline` line their first lines up on the row's deepest baseline, whatever their fonts, padding or borders.
- An element inside a row that is not a table cell sits in an anonymous cell, as CSS table fixup puts it.
- A table's `width: min-content`, `max-content`, `fit-content` and `stretch` size it from its column grid, where they used to act as `auto`.

### Paint backgrounds as Chrome does

- `background-repeat: space` spreads the leftover room so the first and last tiles touch the edges, and a single tile follows `background-position`. `round` rounds the tile count to the nearest whole number. Both keep tiling across a painting area larger than the positioning area.
- Shorter `background-size`, `-position`, `-repeat` and `-blend-mode` lists cycle over the layers instead of repeating their last value.
- SVG places tiles at exact positions and positions `background-clip: text` layers by `background-origin`. PDF no longer repeats a layer along an axis that does not repeat.
- `background-blend-mode` in the image output blends only with the box's own layers and color, not with what sits behind the box.
- Gradients sample each pixel at its center, so hard stops land on the same pixels as Chrome's. A tile of fractional size blends across its seams as Chrome's does.
- A repeating gradient whose stops all sit at one position paints solid in the last stop's color.
- A tile smaller than 1/64px paints nothing, as Chrome's `LayoutUnit` sizes truncate it to empty, instead of listing millions of tiles.
- `background-repeat: repeat bogus`, or a stray token after the last item of `background-size`, `-position` or `-blend-mode`, makes the declaration invalid, as in browsers.

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

## takumi-pdf@0.11.1

### Type `tw` from the primitives entry

Move the `tw` JSX attribute augmentation into `takumi-pdf/primitives`, so importing only the primitives types it too.

## takumi-pdf@0.11.0

### Measure a band with the page count the cut produced

A header or footer band was measured once with three-digit stand-in counters, so a counter wider than three digits could wrap and get clipped, and a narrow band reserved margin it never used. The band now re-measures with the real page count until its height settles, up to three passes, the same way content counters already converge.

### Type the bundler entries' default export

`export *` does not forward a default export, so the wasm init default was untyped on every bundler entry.

### Add page counter primitives

`PageNumber`, `TotalPages`, and `TargetPageNumber` components wrap the counter class hooks, with a typed `format` prop for the supported `@counter-style` names.

### Repeat a table's header rows on every page

A `<thead>` paints again at the top of each page its table continues onto, per css-tables-3 repeated headers; a header taller than a quarter of the page does not repeat.

### Update the font stack

parley 0.11.1, skrifa 0.44, and write-fonts 0.50, with the hinting-gate fork rebased so a single fontations version serves layout and subsetting.

### Render HTML and CSS list markers in PDF

Paint generated list markers in PDF output, including nested, paginated, and tagged (`Lbl`) lists. Font subsetting counts the characters the predefined marker styles generate in every backend.

### Count the pages a repeated box prints on

A page counter filled only inside a `header` or `footer` band. A footer written into the document itself, as a `fixed` box, printed empty hooks, so the numbers had to come from a render option standing beside the document. A repeated box now lays out again for every page it draws on, with that page's numbers, so a component can carry its own footer.

### Match Blink's spacing between a bullet marker and its item

An outside bullet keeps Blink's fixed 7px gap on top of its suffix space, an inside bullet separates with `1em`, and the `square` style draws `▪`, approximating Blink's painted size.

### Drop `reversed`, gradient marker images, and `menu`/`dir` list counting

An `<ol reversed>` now counts up, a gradient `list-style-image` falls back to the counter style, and only `ul`/`ol` scope a list's count.

### Fill a page counter in the content

A `pageNumber` or `totalPages` hook outside a band stayed empty, and nothing said why. It now takes the page its box lands on, and a hook laid out inline takes the page of the box that holds it. Numbering the content lays the document out a second time, since the page a hook sits on is only known once the content is cut into pages. A document without such a hook pays nothing.

## takumi-pdf@0.10.0

### Start a page at the margin Chromium prints at

The default margin was 48px, a number with no source behind it. It is now the 1cm Chromium uses for `kDefaultMargins`, and an axis shorter than an inch keeps no margin at all, the way Chromium drops it rather than leave the page with nothing to print on. Pass `margin` to keep the old geometry.

### Close the gap between two boxes on a fractional parent

A box's position snapped to the pixel grid against its parent while its size snapped against the page. A parent sitting on a fraction, such as a container padded in points, pushed the two apart and left a hairline of background between boxes that should meet.

### Cut once at a forced break

The anonymous box a text child lays out in copied `break-before` and `break-after` from its parent. A padded box carrying `break-after: page` cut twice, once at its content edge and once at its border edge, and the padding between them landed on a blank page of its own.

## takumi-pdf@0.9.1

### Default an omitted margin side to `auto`

A side left out of the `margin` object used to sit flush with the paper edge, which put a band on that side straight over the content. It now defaults to `"auto"`, the same as every other side. Pass `0` to get the old behaviour.

## takumi-pdf@0.9.0

### Take every page-size keyword CSS defines

`size` knew `"a4"` and `"letter"`, so a receipt on A5 or a US legal contract meant working out the millimetres. It now takes all ten page-size keywords CSS Paged Media defines, ISO and JIS sheets alongside the US ones.

### Load the wasm binary in a browser bundle

Vite, webpack and Turbopack set the same export conditions for a browser build. All three resolved the Vite entry, whose `?url` import only works in Vite. Each package now exports `wasm-url`, which resolves the binary through `new URL(specifier, import.meta.url)`, the call Vite, webpack and Turbopack rewrite to the asset they emit. Pair it with `takumi-pdf/no-init`, or with the new `takumi-js/wasm/no-init`, which keeps the auto-init entry out of the bundle.

### Pick the Node entry when webpack targets Node

A webpack build for Node resolved the Vite entry, because both environments set the `module` condition and it is listed first. The build then failed on that entry's `?url` import, which only Vite reads. A `webpack` condition now routes webpack's Node target to the Node entry, and every other bundler keeps the entry it already resolved.

### Say what a failed render needs

A failed render threw the error's Rust shape, such as `MissingGlyphs("क (U+0915)")` or `DecodeError(Unsupported(UnsupportedError { format: Unknown }))`. Every error now reads as a sentence that names the fix, and the ones wrapping another error carry its message instead of its debug form.

### Size the page margin to its band

A band draws inside the page margin, and a margin shorter than the band left content running underneath it. `margin` now takes `"auto"` on any side and starts there, growing to the space that side's band needs and never dropping below the 48 it began at.

### Render from a Next.js route without configuring the bundler

Turbopack bundles a server route's imports, and it resolved `takumi-pdf` to the Vite entry, whose `?url` import only Vite reads. The build failed unless the package was listed in `serverExternalPackages`. `takumi-pdf/next` hands Turbopack the binary in the form it emits, on the Node runtime and the Edge runtime alike.

## takumi-pdf@0.8.1

### Ship without skrifa's hinting interpreter

Every draw is unhinted, but skrifa's TrueType hinting interpreter and autohinter survived dead-code elimination through runtime branches. A patched skrifa gates them behind a `hinting` feature, cutting ~240KB from the wasm binaries with identical rendering.

## takumi-pdf@0.8.0

### Repeat fixed boxes on every page

Fixed boxes outside transformed or filtered ancestors now lay out against the page area and paint on every page. Watermarks no longer stop at the first page.

### Reject a page that would print wrong

An image whose bytes will not decode used to leave a hole, and `filter: blur()` or `drop-shadow()` used to be dropped without a word. Both now stop the render and name what went wrong, the way an uncovered character already did.

### Set the paper color

`backgroundColor` takes a CSS color and paints it under everything on every page, margins included. A watermark with a negative `z-index` sits above it, so the paper no longer has to come from a box in the tree.

### Pick the WASM entry from the bundler's export condition

Bundling `takumi-pdf` broke initialization, because every environment resolved to the Node entry and that entry locates the binary from `import.meta.url`. Vite, Next, workerd and Bun now each get an entry that finds the binary where that bundler puts it.

### Embed JPEG and WebP images

`images` took bytes in any raster format, but only PNG reached the page: a JPEG or a WebP failed the whole render. Both embed now, and a JPEG keeps its own compression instead of being decoded and re-encoded.

## takumi-pdf@0.7.0

### Honor `widows` and `orphans` at page breaks

A cut through a paragraph keeps at least `orphans` lines at the bottom of the page and `widows` lines at the top of the next. Both are inherited CSS properties and default to 2, the Chromium print default. Set both to 1 to disable the limits. Minimums the page cannot fit are dropped for that page.

## takumi-pdf@0.6.0

### Tag the structure inside an inline-block

An inline-block lays out in a subtree of its own, and that subtree drew without tagging anything. A heading or a list nested inside one never became a structure element, and its text was folded into the paragraph around the box. The subtree now tags its nodes where the document tree expects them.

### Print the page a link points at

A node classed `targetPageNumber` now renders the page number of the element the nearest enclosing `href` points at, which is what a table of contents needs. Counter styles apply the same way they do on `pageNumber`, and a fragment naming no element renders nothing.

Page numbers only exist once the document is paginated, so a document using the hook is paginated again with the numbers in place, up to three times, until they stop moving.

### Fill text clipped to its background with an image

`background-clip: text` could paint a colour or a gradient through the glyphs, but not an image. The layer was dropped, and since the idiom pairs the clip with a transparent colour, the text came out invisible. An image layer now draws into a pattern the glyphs are filled with.

### Reject a character no registered font covers

A character outside every registered font shaped to `.notdef`. It painted nothing and left nothing in the text layer, so the page looked finished with the character quietly gone. Rendering now fails with `MissingGlyphs`, naming each character and its codepoint.

### Map a cluster's glyphs to its source text once

In Devanagari and other scripts that attach marks to a base letter, the base and its mark form separate clusters over the same source text. Every glyph claimed that whole range, so `मोटा` came out of the PDF text layer as `ममोटटा`. Overlapping glyphs now share one range and one `/ActualText`, and every glyph gets a codepoint mapping so a viewer without `/ActualText` support does not read a raw glyph index.

### Key text layout on the stroke width

Two passages of the same words in the same font shared one shaped layout, so a `-webkit-text-stroke` width set on the second was drawn at the first one's width.

### Widen a clipped background by the text stroke

A transparent `-webkit-text-stroke` reveals a ring of the background painted through the glyphs. In PDF that ring was missing: the background pass widened the coverage by the faux bold alone, so the output disagreed with the image and SVG backends.

### Let a stroke be as transparent as what it outlines

Faux bold outlines a glyph in the colour it fills, and `-webkit-text-stroke` outlines it in its own. Both took the colour without its alpha, so translucent text came out ringed in solid colour. Text under `background-clip: text` is transparent by design, which made this a black outline around every gradient-filled glyph.

### Keep a clipped background out of the text layer

`background-clip: text` drew the run twice, once to fill the background through the glyphs and once for the text itself. Both landed in the text layer, so extraction, search and copy returned the text doubled. The background pass now paints the glyph outlines, which cover the same pixels without adding a second run of text.

### Keep clipped-away content off every page

Content an `overflow` clip cut away still reached the file when it sat far enough down the page to land on a later one. A clip keeps it off the page, but not out of the text layer, so a redacted or collapsed section came back out of any tool that reads text: search, copy, an accessibility reader.

### Declare a passage written in another language

A `lang` attribute reached shaping and line breaking but never the output, so a document carrying Arabic or Hindi inside an English page declared only the document language. A screen reader read every passage in the document voice. Content whose language differs from the document's is now marked with that language.

### Stroke the span that asked for it

`-webkit-text-stroke` was read off the element holding the text, so a `span` setting it for itself came out unstroked, and a nested one turning it off still got the parent's outline. The stroke now travels with the text run, in every backend.

### Stop counting pages at twenty thousand

Content tall enough to cut into millions of pages walked the whole document once per page, with nothing to stop it. A render taking untrusted markup could be handed a document whose only purpose was to spend the renderer's memory. Rendering now fails with `TooManyPages` rather than trying.

### Give a link target something to point at under PDF/UA-2

A link to `#some-id` names a structure element, and PDF/UA-2 requires every link inside a document to do so. Markup with nothing to say for itself, a plain `div` holding an id, left no element behind, so the link named one that was never written and the file failed validation while the render reported success.

### Count pages in more scripts

Page counters knew seven `@counter-style` names. They now know the digits of eighteen more scripts, from Devanagari and Thai to Tamil and Tibetan, and count through five alphabets including Latin letters, Greek, hiragana and katakana.

A face registered through `fonts` is kept only when its range covers something the page asks for, and a counter's characters appear nowhere in the document. A counter in a style other than decimal now keeps every registered face, so the one it needs survives.

## takumi-pdf@0.5.0

### Validate against PDF/UA-2

`tagged: "ua2"` writes PDF/UA-2, which pairs with PDF/A-4 and needs a document language.

### Draw inline images and containers

An `<img>` inside a paragraph now draws, and carries its `alt` into the structure tree.

### Follow a rounded axis with the `auto` one

`background-size` with one `auto` axis kept the size it was first given when `background-repeat: round` rescaled the other. The tile stopped matching the image's shape. It now follows, as it already did in the raster and SVG backends.

### Route shared codepoints to the subset that declares them

A Google Fonts subset encodes more than the `unicode-range` it was cut for, and the Cyrillic and Greek ones also carry the ASCII space and the Latin capitals. Selection took the first subset whose glyphs covered a character, in family-name order, so those codepoints left the Latin subset and every word split into separate runs. Subsets now rank by the range they declare, lowest first.

### Place replaced content from one place

`object-fit` and `object-position` place replaced content from one place. An `object-position` past 100% now clips to the content box.

### Ask a background layer once whether it paints

`BackgroundImage::paints` replaces the three spellings each backend had for the same question.

PDF used to treat a `url()` layer as unpaintable when built without the `images` feature, which skipped the whole background-image pass rather than that one layer.

### Skip the ink an underline runs through, in every backend

`text-decoration-skip-ink` breaks an underline where the glyph outlines cross it, in every backend. A gap inside a letter stays a gap.

### Report the measured tree's own width

`measure` handed back the width it laid the tree out against, so a box with `width: 100px` measured 793 on an A4 page. It now reports the size the tree itself took.

### Paint text decorations from one place

`paint_run_decorations` paints a run's underline, overline and line-through for every backend.

### Tag `<figure>` as a Figure

A `<figure>` becomes a `Figure` carrying its image's `alt`. The `<figcaption>` inside becomes a `Caption` child of it. Captions used to reach the document root, which no standard allows.

### Resolve inline boxes once, for every backend

`resolve_inline_box` places an inline box's replaced content or nested subtree, shared by the SVG and PDF backends.

### Shade a 3D border in every backend

`inset`, `outset`, `groove` and `ridge` borders now shade their sides in the SVG and PDF backends, as the raster backend already did.

### Correct the PDF 2.0 structure namespace

A tagged PDF/A-4 document now names its structure namespace `http://iso.org/pdf2/ssn`, the identifier ISO 32000-2 defines. The old one matched no known namespace, so PDF/UA-2 validators rejected every structure element in the file.

### Paint the outline above the content

An `outline` painted under the box's own text and images, so a negative `outline-offset` disappeared behind them. CSS 2.1 Appendix E paints the outline last, and every backend now does.

### Draw dashed, dotted and double borders in PDF

`dashed`, `dotted` and `double` borders and outlines now draw in PDF instead of falling back to solid.

## takumi-pdf@0.4.2

### Render the weight the text asked for

A variable font is embedded at the coordinates the run was shaped at, so `font-weight` and `font-stretch` reach the page instead of the font's default instance. A face with no bold or oblique of its own gets the same synthesized ones the raster renderer applies.

### Render HTML strings

`render()`, `measure()`, and the header and footer options accept HTML strings, the same input format as `takumi-js`. A `<style>` tag applies only to that render.

## takumi-pdf@0.4.1

### Attach files under PDF/A-4

`pdfa: "4f"` renders the PDF 2.0 archival level that takes attachments. The other `"4"` level still rejects them.

### Emit a structure tree PDF/UA accepts

Headings are renumbered by nesting depth, so a document that opens at `h2` or jumps from `h1` to `h4` no longer writes a tree the validator rejects. A list item outside a list now brings its own list. A heading whose text sits in child elements, such as `<h1>Plain <strong>bold</strong></h1>`, reaches the outline instead of being dropped, which used to fail a `tagged: "ua1"` render outright.

## takumi-pdf@0.4.0

### Write shorter paths

Box decorations wrote every corner point twice and spelled out the closing edge that `h` draws anyway. Rectangles now use the `re` operator, and segments that go nowhere are dropped. A two-page invoice loses 12% of its bytes and renders about 3% faster.

### Close the CSS paint gaps against the raster backend

`outline`, `text-shadow`, `-webkit-text-stroke`, `url()` background and mask layers, `background-origin`, `background-clip` (including `text` and `border-area`) and `background-blend-mode` now paint.

- outlines ride the border machinery: offset outward, following the radius, no layout impact
- text shadows draw as shifted glyph passes under the text; PDF has no blur operator, so a blurred one draws sharp
- `background-clip: text` fills the glyphs with the background color and gradient layers, so gradient text stays selectable vector text
- url() layers rasterize like a filtered image and honor intrinsic sizing, so `background-size: auto`, `cover` and `contain` resolve like the raster backend

### Draw `box-shadow`

Outer and inset shadows now paint. The offset, spread and rounded corners are exact: the shadow is the border box spread and moved, with the box itself cut out by an even-odd fill so nothing paints under an opaque element.

PDF has no blur operator, so a blurred shadow is approximated by eight bands whose opacity follows the Gaussian edge coverage CSS specifies, with a standard deviation of half the blur radius. The shifted, unblurred shape stays fully opaque underneath, and a shadow with no blur draws as one exact fill.

Inset shadows draw inside the padding box, so a border neither carries shadow paint nor widens the shadow.

### Mark backgrounds and borders as artifacts

Backgrounds and borders were painted outside any tagged content sequence. PDF/UA-1 validators reported untagged content on every page that drew one. They are now artifacts, like header and footer bands.

### Write colours to four decimals

An eight-bit colour component divided by 255 printed as `0.047058824`, once per painted element. Four decimals resolve finer than the value it came from, and readers see the same colour.

### Custom XMP metadata

`metadata.xmp` takes namespaces to write into the XMP packet, for metadata the renderer knows nothing about. One is the `fx:` schema that turns a PDF/A-3 with an attached invoice into a Factur-X file.

- each schema carries a prefix, a namespace URI, and its properties
- every property is written as a value and described in the `pdfaExtension:schemas` entry PDF/A requires, so the two cannot drift apart
- a prefix, property name or namespace the XMP writer cannot serialize rejects the render instead of writing a broken packet

### Fade elements with `mask-image`

Gradient mask layers now apply, as a PDF soft mask holding the mask's own vector content. The masked element and its descendants stay vector: nothing is rasterized to fade an element out.

`mask-size`, `mask-position` and `mask-repeat` place the layers, the same way they place a background. `url()` mask sources are still ignored, and the mask is an alpha mask, which is what `mask-mode: match-source` resolves to for an image source.

### Clip elements with `clip-path`

`inset()`, `ellipse()`, `polygon()` and `path()` now clip an element and its decorations, as a real PDF clipping path rather than a rasterized mask.

`clip_shape_commands` in takumi-core resolves a basic shape to path commands, which is where the raster backend's copy of that geometry now lives too.

### Write less per embedded font

`/DW` now states the width most glyphs share, so `/W` only lists the ones that differ, which empties it almost entirely for monospaced and CJK faces. The `CIDSet` stream is gone: only PDF/A-1b asks for one, and that level is not offered. `/FontBBox` comes from the subset's own box instead of a pass over every glyph outline.

### Apply the color `filter` primitives

`grayscale`, `sepia`, `saturate`, `hue-rotate`, `invert`, `brightness`, `contrast` and `opacity` now apply. They are linear transforms of the source color, so they fold into the colors written to the page, including gradient stops, text and decoded image pixels, instead of rasterizing the element.

- filters apply in order, clamping between them as CSS requires, and an ancestor's filter runs after the element's own, like the group it wraps
- SVG images rasterize while a filter is active, since the transform applies to pixels
- shadows follow the filter too, like every other color the element paints
- `blur()` and `drop-shadow()` need a convolution and are still ignored, as are referenced SVG filters
- transforming each color before compositing matches compositing first only while the filtered content is opaque; overlapping translucent content differs

### Link to anchors inside the document

`<a href="#section">` now resolves to the element with that `id` and lands on the page holding it, so a table of contents works inside the PDF. A fragment matching no element is dropped rather than written as a link that goes nowhere.

`Node::id` is public, alongside the existing `href`, `alt` and `tag_name` accessors.

### Place background layers

`background-size`, `background-position` and `background-repeat` now apply to gradient layers, which used to stretch across the whole box whatever those properties said.

- `repeat`, `space` and `round` become one PDF tiling pattern per layer, so a repeated gradient costs one shading rather than one per tile
- the positioning area is still the border box; `background-origin` is not read yet

### Bound repeating radial gradients

A repeating radial gradient whose stops all sit at one position expanded to millions of stops, since the period it tiles by collapsed to zero. The expansion now tiles at most 512 periods, stretching the period to keep covering the full radius.

### Pack the structure tree into an object stream

A tagged document wrote one small uncompressed dictionary per structure element, a third of a text-heavy file. They now share a single compressed object stream. A two-page invoice drops 31%, and the whole fixture suite 15%. Tagging, PDF/A and PDF/UA output are unchanged; veraPDF still passes every level.

## takumi-pdf@0.3.0

### Measure a tree without rendering

`measure` returns a tree's laid-out size in CSS px. With page options it lays out at the full-page width with counter hooks filled, exactly how `render` measures a header or footer band. The height tells you how much margin the band needs.

### File attachments

Attach files with `attachments`, the PDF/A-3 shape ZUGFeRD and Factur-X e-invoices use. `data` takes bytes or a UTF-8 string. `modificationDate` falls back to `metadata.creationDate`. Invalid `pdfa` combinations are TypeScript type errors. Without type checking they reject the render at runtime.

### Embed SVG image sources as vectors

SVG images previously rasterized at 2× their placed size, leaving small logos soft next to vector text. They now embed as real paths, gradients and clips, sharp at any zoom. Filters and bitmaps embedded inside an SVG still rasterize at 2×.

### PDF/A and tagged output

Output is now tagged by default, like Chromium's print-to-PDF. The structure tree comes from the HTML semantics: headings, paragraphs, figures with alt text, links, and header/footer artifacts. Decorative `<img alt="">` images are artifacts. `tagged: false` turns the tree off. `tagged: "ua1"` validates against PDF/UA-1.

Set `pdfa` to a level from `"2b"` to `"4"` to emit archival PDFs with an sRGB output intent and XMP metadata. The `a` levels require the structure tree and `metadata.creationDate`. A document that cannot conform rejects the render instead of writing a broken file.

## takumi-pdf@0.2.1

### Shrink the WebAssembly binary by 5%

Size-optimize the PDF serialization and font subsetting crates. The shipped wasm drops about 220KB with render speed and output bytes unchanged.

## takumi-pdf@0.2.0

### Draw header and footer bands in the page margins

Bands previously reserved their height inside the content window, so a footered document paginated earlier than Chrome's print output. They now lay out at full page width and draw in the margin areas with Chromium's 15pt edge inset. The content window always spans the full margin box.

## takumi-pdf@0.1.5

### Write page geometry in PDF points

Pages were sized in CSS px written as pt, so an A4 document came out 33% oversized when printed. Page size, annotations, and outline destinations now convert at 0.75 pt/px. Layout still runs in px.

### Fill the page content box like a browser body

A fit-content root resolved child percentage widths inconsistently across layout passes. Long documents could overlap trailing content and drop pages entirely.

## takumi-pdf@0.1.1

### Auto-height viewport

`viewport.height` is now optional. Omitting it sizes the single page to the laid-out content, like a thermal receipt.

### Render SVG image sources

SVG images passed via `images` came out as blank space; the backend only embedded bitmap sources. SVG sources now rasterize at twice their displayed size and embed like other images.

### Ship the `tw` prop type

Importing only `takumi-pdf` left JSX `tw` props failing to typecheck; the react module augmentation now ships with the package types.

## takumi-pdf@0.1.0

### Publish takumi-pdf, the wasm PDF package

`render(jsx)` turns a node tree or JSX into a paged PDF with selectable text and embedded subset fonts, on Node, Bun, and Cloudflare Workers. Options mirror Puppeteer's `page.pdf()`: `size` (`"a4"`, `"letter"`, `{ width, height }`), `landscape`, per-side margins, and repeating header/footer bands with Chromium-style `pageNumber`/`totalPages` class hooks and CSS counter styles, while `viewport` renders a fixed single page instead. Fonts, images, and stylesheets round out the options.
