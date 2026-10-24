## takumi-core@0.33.0

### Draw bitmap emoji in SVG text

SVG `<text>` now draws bitmap glyphs, such as Noto Color Emoji's, in their own colors and the text's `fill-opacity`, as HTML text does. These glyphs used to be left out.

### Pick an inline block's baseline with `baseline-source`

`baseline-source: first` lines an inline block up by its first line, and `last` by its last line. `auto` keeps the default, which is the last line of an inline block and the first baseline of an inline flex or grid container. A `vertical-align` declared after `baseline-source` resets it to `auto`, as Chrome does.

### Free the layout data of a page once it is drawn

`render_pages` and `render` free the layout data a page held, such as its text lines and cached layouts, once the page is drawn and no later page paints from it. The source tree and the tree of boxes stay in memory for the whole render, so peak memory still grows with the size of the document. A document that lays out to its end before the first page is drawn frees nothing early. A page that would paint from a released page fails with `Error::PageReleased` instead.

### Set overflow per logical axis with `overflow-inline` and `overflow-block`

`overflow-inline` sets the horizontal overflow and `overflow-block` sets the vertical overflow, as Chrome maps them in horizontal text.

### Align and decorate SVG text with the font metrics HTML text uses

SVG `<text>` now places `dominant-baseline`, `alignment-baseline`, and text decorations from the same font metrics as HTML text. `baseline-shift="sub"` and `"super"` move text by half the font's height, as Chromium does, and decoration lines are a tenth of the font size thick.

### Lay out `::before` and `::after` as flex and grid containers

A `::before` or `::after` with `display: flex` or `display: grid` now lays its content out as items, so `align-items`, `justify-content`, and `place-items` center a generated number or icon. Before, the box became a block and ignored them. Adjacent strings and `attr()` values in `content` form one item, as in Chrome.

### Break positioned boxes across the pages of flex, grid and table containers

In paged output, an absolutely positioned box in a flex, grid or table container now breaks across pages with its container, and one placed in a grid area that starts on a later page lands on that page.

### Repeat a fixed box from the page it starts on

In paged output, a `position: fixed` box now repeats on the page that holds its static position and on every page after it, no longer on the pages before.

### Size tables inside flex and grid containers as Chrome does

A table that is a flex or grid item now takes the size its container gives it.

### Style list rows with `first:`, `odd:`, and other structural variants in `tw`

The `tw` prop now applies structural variants, which match the element's place in the tree or, for `empty:`, its contents. They are `first:`, `last:`, `only:`, `odd:`, `even:`, the `*-of-type:` forms, `empty:`, `nth-3:` and its relatives, `nth-[2n+1]:`, `not-*:`, and arbitrary variants on the element such as `[&:nth-child(3)]:`. They combine with breakpoints, as in `md:first:mt-0`. Before, a class with one of these variants was ignored.

`:empty` in a stylesheet now also matches an element whose only children are text nodes with no characters, as in Chrome.

### Place outside list markers outside the list item's border box, as Chrome does

An outside marker now hangs from the list item's border edge, so padding or a border on the item moves the text but not the marker. A marker taller than the first line, such as a large `list-style-image`, now pushes the content down so the two share a baseline.

### Draw an SVG's bitmaps and filtered groups in a PDF without rasterizing

takumi-pdf embeds the PNG, JPEG, and WebP images inside an SVG as PDF images, and draws an SVG group without its filter. A CSS color `filter` on an SVG `<img>` or `background-image` recolors the SVG instead of rasterizing it, and takumi-svg writes an SVG's bitmaps as `<image>` elements. `SvgOp::Image` and `SvgOp::PushFilter` replace `SvgOp::Raster`, `SvgSource::vector_ops` no longer takes a raster scale, and resvg's renderer moves behind the `svg-raster` feature, on by default.

### Parse every `break-before`, `break-after`, and `break-inside` keyword

`break-before` and `break-after` now accept `avoid`, `avoid-page`, `avoid-column`, `column`, `recto`, and `verso`, and keep `left` and `right` as their own values. `break-inside` accepts `avoid-page` and `avoid-column`. In a PDF, `recto` and `verso` force a page break, and `break-inside: avoid-page` keeps a box on one page as `avoid` does. A PDF does not yet avoid a break between boxes for `break-before` or `break-after`.

### Support the `grid` and `grid-template` shorthands

`grid-template` and `grid` now set the grid longhands as browsers do, including the areas form such as `grid-template: "head head" 40px "side main" 1fr / 120px 1fr`. Track sizes also accept `min-content`, `max-content`, and `fit-content()`. A `grid-template-areas` row must now be a quoted string, and a value whose named areas do not form rectangles is rejected, as in browsers.

### Paint a block inside an inline box as Chrome does

A block inside an inline box, such as a `<div>` in a `<span>`, now paints its background before the line's inline boxes, and its positioned boxes stack with the rest of the page. A percentage height inside it now resolves against its containing block.

### Draw inline boxes that have no content

An inline element with no content, such as `<span style="padding: 0 10px; background: red"></span>`, now paints its background, border, and outline, as browsers do. It used to take its space on the line but paint nothing.

### Lay out tables with Chromium's table algorithm

Tables now follow Chromium's table layout instead of a grid approximation, so column widths, row heights, and collapsed borders match Chromium. `<col>` and `<colgroup>` size their columns and paint their backgrounds, and `visibility: collapse`, `empty-cells: hide`, and `display: inline-table` work. A `rowspan` now stops at the end of its row group.

### Apply the dimension limit to each GIF frame

A GIF frame wider or taller than 8192 pixels now fails to decode, as a GIF whose canvas is that large already does.

### Unset a property whose `var()` substitutes to nothing valid

A declaration such as `width: var(--missing)` now resets the property as `unset` does when the variable is missing or substitutes to an invalid value, as in Chrome. It used to be skipped, so an earlier value for the same property stayed in effect.

### Paint outside list markers as boxes of the page

An outside list marker now paints and stacks as Chrome paints it. In a tagged PDF, the text of a list item stays in one marked-content region instead of being split around its marker.

### Lay out a flex or grid container's text as an item beside its `::after`

An element with its own text and an `::after` now lays the text out as an anonymous flex or grid item, as it already did beside a `::before`. Before, the text sat on the container itself, so `align-items` and `justify-content` ignored it and the `::after` box landed on top of it.

### Set any style property with a builder method

`Style` now has one builder per longhand and shorthand, so `Style::default().font_size(32.0.px())` replaces `Style::default().with(StyleDeclaration::font_size(...))`. The `LengthUnit` trait adds `px`, `percent`, `em`, `rem`, `vw`, and `vh` to `f32`, each returning a `Length`. The `Style::with_<shorthand>` methods are deprecated in favor of the shorthand's builder of the same name.

### Draw synthetic bold, synthetic italic, and color emoji in SVG text

SVG `<text>` now draws the synthetic bold and italic HTML text gets for a face without that weight or style, and draws COLRv0 color glyphs such as Twemoji in color. These glyphs used to draw upright, at regular weight, or not at all.

### Lay out and paint a document a page at a time

`PagedLayout` lays a render tree out one `next_page` call at a time, `LayoutView` reads it while it goes on, and `PageStream` hands out the scene of each page once no later page can paint onto it. `Scene::enter`, `PagedScene::page_origin` and the `LayoutResults` methods `is_repeated_fragment`, `table_fragment`, `is_hidden_for_paint` and `is_line_clamp_clipped_float` are gone, in favor of `Scene::view`, `SceneRef::page_origin` and the same methods on `LayoutView`. `TextClip::of`, `TableDecorations::of` and `paint_collapsed_borders` read a `LayoutView`, and the last two take the `NodePaint`.

### Place margins, floats, and positioned boxes where Chrome does

Block layout now collapses margins and places floats, cleared boxes, and positioned boxes as Chrome does. Positions round to 1/64px, so text can move by up to 0.01px.

### Collect images from Tailwind `list-image-*` and `content-*`

`Node::image_urls` now returns the images that `list-image-[url(...)]` and `content-[url(...)]` reference, as it does when the same properties are set in `style`. Before, it returned only the images from Tailwind `bg-*` and `mask-*`.

### Decode images without the `image` crate

PNG and JPEG images now decode straight into RGBA through `png` and `zune-jpeg`, so `takumi-core` no longer depends on `image`. A 16-bit PNG channel now keeps its high byte instead of rounding to the nearest 8-bit value, and decode errors read differently. Raw bitmaps that SVG output embeds as PNG now compress with `png`'s fast setting, trading some size for encode time.

### Rasterize SVG images from the same ops as PDF

takumi-core now rasterizes an SVG from the `SvgOp` list with tiny-skia, so resvg's renderer is gone and the raster, PDF, and SVG backends draw one flattened tree. `SvgOp` gains anti-aliasing, the radial focal radius, and a `miter-clip` join, which resvg drew and the ops had dropped. `BlendMode::to_tiny_skia` replaces takumi-raster's private conversion.

### Expose `rgba_len` for sizing RGBA buffers

`resources::image_buffer::rgba_len` returns the byte length of a `width` by `height` RGBA buffer, or `None` when it overflows `usize`, so backends check sizes the same way the core does.

### Space SVG text as HTML text does

SVG `<text>` now applies `letter-spacing` and `word-spacing` with the same rules as HTML text. Nonzero `letter-spacing` turns off ligatures, and the spacing also follows the last letter, which moves text centered with `text-anchor="middle"`. `word-spacing` now applies to a leading space too, as Chromium does in SVG.

### Match an SVG's `<style>` rules like the page's own CSS

An SVG image's `<style>` sheet and `style` attributes now go through the selector engine takumi uses for HTML. Selectors such as `:nth-child()`, `:not()`, `:is()`, `:lang()`, and `~` now apply, where they used to drop the rule.

An `@property` descriptor marked `!important` is now ignored, as in Chrome, where its value used to keep the annotation.

### Break boxes inside a positioned inline box across pages

In paged output, an absolutely positioned box whose containing block is a `position: relative` inline box now moves to the next page when it starts past the page end, and breaks across pages when it is taller than the space left.

### Draw disc, circle, square, and disclosure list markers as shapes

These markers are now filled or stroked shapes sized from the marker's font, as Chrome draws them, in raster, SVG, PDF, and paint-tree output. They used to be text glyphs, so their size and position followed whichever font had the bullet character. An outside marker now sits as far from its item's text as it does in Chrome.

### Draw an outline that sets only `outline-style`

An element with `outline-style: solid` and no `outline-width` now draws a `medium` (3px) outline, as browsers do. It used to draw no outline unless another outline longhand, such as `outline-color`, was also set.

### Support the `all` shorthand

`all: initial`, `all: inherit`, and `all: unset` now set every property except `direction`, `unicode-bidi`, and custom properties, as in Chrome. `all: revert` is ignored, as `revert` is on every other property.

### Resolve `inherit`, `initial`, and `unset` on custom properties

A custom property set to `inherit`, `initial`, or `unset` now takes the value the keyword names, as in Chrome. `inherit` takes the parent's value, `initial` takes the `@property` initial value or leaves the property unset so `var()` uses its fallback, and `unset` acts as `inherit` or `initial` by whether the property inherits. Before, `var()` substituted the keyword itself, so `--w: inherit; width: var(--w)` became `width: inherit`.

### Wrap text around floats outside its block

Text now flows beside a float that an earlier block placed, as Chrome lays it out, instead of running over it. A `line-clamp` container sits beside such a float as a block formatting context of its own, and clips the floats inside it at its last shown line. A positioned box that was inline sits where Chrome puts it among blocks and floats, `text-align` and `text-indent` included.

### Line up inline blocks and inline images where Chrome does

An inline block now sits on the baseline of its last line as Chrome places it, also when its `height` cuts that line off, and a percentage size inside an inline box resolves against its containing block as Chrome resolves it.

### Resolve conflicting `tw` utilities in Tailwind's order

Two `tw` utilities or `@apply` entries that set the same property now resolve as Tailwind's compiled CSS does, whatever their order. `pt-2 p-4` and `p-4 pt-2` both pad the top by `pt-2`, where before the later class won. To keep the old behavior, merge the class list first with a tool such as `tailwind-merge`.

### Style `::before` and `::after` from `tw` with `before:`, `after:`, and `content-[…]`

The `tw` prop now takes `before:` and `after:` variants, which style the element's `::before` and `::after`, and `content-['…']`, `content-[attr(…)]`, and `content-none` set what they show. They combine with structural variants and breakpoints, as in `first:before:content-['→']`. Like Tailwind, a `before:` or `after:` utility makes the box even without `content-*`.

### Size and place flex items where Chrome does

Flex and `-webkit-box` containers now size, wrap, and align their items as Chrome does.

### Keep an inline box inline when it holds a block

A block inside an inline box, such as a `<div>` in a `<span>`, now sits on a line of its own while the inline box keeps its lines before and after it, as in Chrome. The inline box used to turn into a block, so its background and border wrapped the whole thing.

### Center a block's content with `align-content`

`align-content: center` and `end` now move the content of a block container with a fixed height, as Chrome does, and `space-around` and `space-evenly` center it. Content that overflows stays at the top unless the value says `unsafe`, as in `unsafe end`. A container with an `align-content` other than `normal` becomes a block formatting context. `align-content` and `justify-content` now keep `unsafe` and accept `flow-start` and `flow-end`. Table cells honor `unsafe` too.

### Match and shape SVG text fonts as HTML text does

SVG `<text>` now picks fonts with the CSS font matching HTML text uses, falls back per character through the same font list, and shapes through the same shaper. `font-variant="small-caps"` now synthesizes small capitals for a font without them. `font-weight`, `font-style`, and `font-stretch` pick faces and variation axes as CSS does.

### Size and place grid items where Chrome does

Grid containers now place, size, and align their items as Chrome does, and an absolutely positioned child of a grid takes its grid area. Line names after the last track of a `repeat()` no longer drop.

### Share a shaped run's font bytes without copying them

`ShapedRun::font_blob` is now public, and `FontBlob::shared_data` returns the font's bytes as a shared `Arc`, so a backend can keep them without copying the file.

### Stack boxes inside inline blocks and inline floats as Chrome does

A positioned box or a `z-index` inside an inline block or a float of a line now stacks with the rest of the page, as Chrome stacks it, and `measure` reports what an inline block holds.

## takumi-core@0.32.0

### Match fonts once per style in a render

Each run of inline text matched its fonts and fallback faces again, even when an earlier run had the same font style. Runs with the same font style now share one fallback list for the whole render.

### Shape SVG text with the registry's faces

SVG `<text>` shapes with the faces the font registry holds, sharing their shaping tables instead of building new ones for each text chunk.

### List the images a stylesheet references

`StyleSheet::image_urls` lists the `url()`s its rules and keyframes reference, as `Node::image_urls` does for a node tree, so a caller can load them before rendering.

### Read `ch` and normal line height from the font that draws

`ch`, `line-height: normal`, and an inline box's ascent now come from the same primary font the text draws with, at its variation instance. With `font-optical-sizing: auto`, `ch` used the parent's font size to pick the optical size, so a `10ch` box in a 64px Inter came out 11px wider than in Chrome.

### Honor emoji and text variation selectors in font fallback

Since 2.17.0, the first font with a glyph drew a character whatever variation selector followed it, so `‼️` came out monochrome after a text font and `‼︎` in color after an emoji font. A font that lacks a variation sequence now hands it to a later font that draws it the way it asks, as Chrome does, and only when none does does the base character's glyph stand in.

### Lay out nested inline blocks in linear time

Each level of nested `inline-block` boxes laid out the level below it several times over, so a few levels deep took minutes. An inline block now keeps the layout it took in each available space.

### Share the font registry between renders

A render's font snapshot shares the registry instead of copying it, and SVG `<text>` builds its face store once per set of registered fonts instead of once per render.

### Skip an SVG too large to rasterize instead of failing the render

An SVG image or background that would rasterize past the pixel budget, such as `background-size: cover` on an SVG with an extreme `viewBox`, is now left undrawn. The whole render used to fail with `InvalidPixmapSize`.

### Generate `::before` and `::after` whose `content` is empty

A pseudo-element with `content: ""` now gets a box, so a clearfix, a decorative bar, or an absolutely positioned overlay drawn by one shows up. It used to be dropped because it held no text.

### Speed up `feMorphology`

`feMorphology` read every pixel of its radius rectangle for each output pixel. It now sweeps rows, then columns, with the same result. A large radius runs several times faster.

### Keep a background whose shorthand names an attachment

`background: red url(a.png) no-repeat fixed` used to drop the whole declaration over `fixed`. The shorthand now accepts `scroll`, `fixed`, and `local`; a still image paints them alike.

### Keep a line whole around a `display: none` element

A `display: none` element inside a line of text, such as `Only <span style="display: none">sold out</span>$20`, no longer breaks the line in two. Such an element now gets no box at all, so `measure()` leaves it out of the children it returns.

### Reject negative padding and sizes

A negative `padding`, `width`, `height`, `min-*`, or `max-*`, including their logical forms, is now invalid, as in browsers: a stylesheet drops it and keeps the earlier declaration, and a style object reports it as an invalid value instead of using it.

### Parse `flex: none` as `0 0 auto`

`flex: none` now keeps a flex item from growing or shrinking, as CSS defines it. It used to reset the item to `0 1 auto`, so the item still shrank to fit its container.

### Support `white-space: break-spaces`

`white-space: break-spaces` and `white-space-collapse: break-spaces` keep every space and let a line wrap after any of them, so spaces that reach the end of a line wrap onto the next instead of hanging past it.

### Keep scene bounds past the canvas's top and left edges

`SceneBounds` fields are now `i32`, and a node that reaches past the canvas's top or left edge gets negative bounds instead of bounds clamped to zero.

### Hide images an ellipsis cuts off

Since 2.17.0, an inline image at or past a `text-overflow: ellipsis` cut was still painted, over the ellipsis or in its place. It is now left out, and an ellipsis that lands right after an image paints as well.

### Shape every size of a face with one font

A face builds its shaping tables once while it stays registered, instead of once for each size, variation, and render that uses it.

### Support the `ch` unit

`ch` lengths, alone or in `calc()`, resolve to the advance of the `0` glyph in the first font that covers it, or half the font size when that font has no `0`.

## takumi-core@0.31.0

### Wrap loose table parts in anonymous tables

A `display: table-row` or `table-cell` outside a table, with text or inline elements in it, made rendering panic. Loose table parts now sit in an anonymous table, cells outside a row and a row group's other children in an anonymous row, and anything else in a row in an anonymous cell, as Chrome builds them. A table part inside a flex or grid container lays out as a block.

### Support `line-break`

`lineBreak` sets how strictly lines break around CJK punctuation and small kana: `strict` keeps a small kana with the character before it, while `normal` and `loose` break before it. `anywhere` breaks between any two characters, mid-word included.

### Support the remaining logical properties

The block-axis longhands (`margin-block-start`, `inset-block-end`, …), the logical sizes (`inline-size`, `max-block-size`, …), and the logical borders and corner radii (`border-inline-start`, `border-block-color`, `border-start-end-radius`, …) now parse. Inline start and end follow `direction`, and `inset-inline` and `border-inline-width` now do too.

### Support `text-justify`

`textJustify` takes `auto`, `none`, `inter-word`, and `inter-character`. With `text-align: justify`, `inter-word` widens only the spaces, `inter-character` spaces out the letters too, and `none` leaves the line unjustified.

### Support `hyphens` and `hyphenate-character`

`hyphens: none` stops lines from breaking at soft hyphens (U+00AD). `hyphenateCharacter` sets the string a line ends with where it breaks at one. `hyphens: auto` breaks only at soft hyphens, as `manual` does, because takumi has no hyphenation dictionaries.

### Match fonts as Chrome does

Within a family, a renderer now picks the face closest to `font-weight`, `font-stretch`, and `font-style` with the CSS font matching algorithm, as Chrome does. A variable face takes its `wght`, `wdth`, and `slnt` from the style, and a family split by `unicodeRange` keeps every subset for fallback.

### Draw an inline image inside its margins

An `<img>` or other replaced element in a line of text was drawn across its margins, stretched and with its border and background reaching into them. It now draws in its border box, and its margins stay empty space on the line.

### Transform text case as browsers do

`text-transform: capitalize` no longer lowercases the rest of each word, and a word split across inline elements is capitalized once. `uppercase` and `lowercase` follow the `lang` of Turkish, Azerbaijani, Greek, Lithuanian, and Dutch text, and uppercased Georgian keeps its usual letters.

### Drop line and column from CSS value errors

`cssparser` 0.38 no longer tracks where a parse error happened, so an invalid style value reports the token it stopped at but no longer its line and column.

### Shape with HarfBuzz 14.5.1

Shaping moves to harfrust 0.14, which matches HarfBuzz 14.5.1: Bengali ra-phala such as গ্র joins again, marks attach correctly after a ligated multiple substitution, and Hangul runs apply `calt` to the characters around them. A PDF now draws a glyph from a font's SVG table only when that glyph has SVG data.

### Support `unicode-bidi`

`unicodeBidi` with `direction` sets how a box's text joins the bidi algorithm: `embed` and `isolate` give an inline box its own direction, `bidi-override` and `isolate-override` lay its characters out in that direction whatever they are, and `plaintext` lets each paragraph's first strong character pick its direction.

### Space inside list markers as Chromium does

A `list-style-position: inside` marker now lays out as an inline box, as Chromium's does. A `disc`, `circle`, or `square` marker starts 1px earlier. A `disclosure-open` or `disclosure-closed` marker keeps a 0.4em gap from the text instead of 1em.

### Parse an image list that starts with `none`

`background-image: none, url(a.png)` and `mask-image: none, none` were dropped as invalid. A list may now start with `none`.

### Support `full-width` and `full-size-kana` in `text-transform`

`textTransform` takes `full-width`, which puts characters in their full-width forms, and `full-size-kana`, which puts small kana in their full-size forms, alone or with a case: `uppercase full-width`. A space becomes U+3000 only where white space is preserved, as in Chrome.

In Rust, `TextTransform` is now a struct of a `TextTransformCase` and the two flags; `TextTransformCase` converts into it.

### Support `font-language-override`

`fontLanguageOverride` takes an OpenType language system tag, such as `"TRK"`, which picks glyphs and localized forms in place of the system the content language maps to: `lang="ja"` with `font-language-override: "KOR"` draws Korean forms.

### Lay out inline text as Chrome does

Text and inline boxes now lay out with a port of Chromium's inline layout. `text-overflow: ellipsis` and `line-clamp` cut at a character instead of a whole word, a tab advances to the next tab stop, a float that no longer fits beside a line moves below it, and `text-wrap: balance` leaves a paragraph with floats unbalanced, as Chrome does.

### Support `text-align-last`

`textAlignLast` aligns the last line of a block, and a line before a forced break, apart from the others. `text-align: justify; text-align-last: justify` justifies every line, and `auto` keeps aligning a justified block's last line to the start.

### Support the `font` shorthand

`font: italic bold 16px/1.5 Geist, sans-serif` sets the style, weight, size, line height, and family in one declaration, and resets the other font properties it covers. System font keywords such as `caption` are not supported.

`font-weight` now rejects numbers outside 1 to 1000, and `font-family` rejects unquoted CSS-wide keywords and `default`, as browsers do.

### Release fonts a renderer no longer needs

`unregisterFont({ name })` removes a family with all its `unicodeRange` subsets, and `clearFonts()` goes back to the bundled fallback font. A later render that lists a released font registers it again. In Rust, use `Fonts::unregister_family`.

### Keep a keyframe when one of its declarations is invalid

`StyleSheet::parse_loosy` dropped a whole keyframe when one declaration in it failed to parse. Now it drops only that declaration, as it already did in style rules.

### Clamp with `-webkit-line-clamp` only in a vertical `-webkit-box`

As in browsers, `-webkit-line-clamp` now cuts lines only together with `display: -webkit-box` and `-webkit-box-orient: vertical`. Use `line-clamp` to cut lines on any block. `-webkit-box-orient` now sets the axis of a `-webkit-box` only, which ignores `flex-direction`, as in browsers.

## takumi-core@0.30.0

### Read a run's outlines one glyph at a time

`TextRun.outline()` is now `outlines()`, which returns each glyph's outline as SVG path data in the order of `glyphs`, with `null` for a glyph the run's color does not fill. A renderer can draw, animate, or hit-test each letter on its own. Replace `run.outline()` with `run.outlines().join("")`. In Rust, `PaintTree::run_outline` is now `run_outlines`.

### Map each glyph of a paint tree back to its text

A `Glyph` now has `cluster`: where its cluster starts in the run's `text`, in UTF-16 code units. Glyphs of one cluster share it, so a renderer can split a run into words or animate it a letter at a time without guessing from glyph positions.

## takumi-core@0.29.1

### Keep every line `line-clamp` allows

`line-clamp` now limits text by line count alone, as Chrome does. A paragraph that fits its clamp exactly no longer loses its last line to rounding, such as two lines of 42px text at `line-height: 1.04`.

## takumi-core@0.29.0

### Register font subsets with `unicodeRange`

A font now takes `unicodeRange`, the codepoint ranges its `@font-face` declares. It serves only those codepoints, as in Chrome, so each subset of a family registers under the same `name`:

```ts
const fonts = [
  { name: "Inter", data: latin, unicodeRange: [[0x0000, 0x00ff]] },
  { name: "Inter", data: cyrillic, unicodeRange: [[0x0400, 0x045f]] },
];
```

`googleFonts` returns its subsets this way. `subsetOf` and `subsetRank` are deprecated: `subsetOf` now only sets the family name, and `subsetRank` does nothing. In Rust, use `FontResource::unicode_range`.

### Turn off contextual alternates under letter spacing

Non-zero `letter-spacing` now also turns off contextual alternates (`calt`), as Chrome does. `font-variant-ligatures` can no longer turn ligatures back on while letters are spaced. `font-feature-settings` still overrides both.

### Upgrade taffy to 0.15

Layout runs on taffy 0.15, whose grid and flexbox passes are faster. The root box now sits at its own margins, as Chrome places it, instead of at the top left corner.

### Measure text ink once per text node

A text node with its own `display`, such as `flex`, no longer shapes and breaks its text a second time to measure ink bounds while the scene is built. Dashboards with many styled text labels render about 30% faster.

### Move inline elements by `position: relative`

An inline element with `position: relative` now moves by its `top`, `left`, `bottom` and `right`, along with its background, border, text decorations and inline-block children, as in Chrome. Tailwind's Preflight relies on this to raise `<sup>` and lower `<sub>`.

### Read Preflight from every Tailwind entry point

`@import "tailwindcss"` now turns Preflight on when it carries modifiers such as `source(none)` or `important`. `@import "tailwindcss/preflight.css"` turns it on too, while `tailwindcss/theme.css` and `tailwindcss/utilities.css` leave it off, as in Tailwind's setup without Preflight.

### Keep images at their natural size in grids and between insets

An image or SVG in a grid cell no longer stretches to the cell under the default alignment, and an absolutely positioned one no longer stretches between its insets. Both keep their natural size, as Chrome draws them.

### Lay out text with parley 0.12

Text layout moves to parley 0.12. Letter spacing turns off ligatures as browsers do, and text beside floats, cloned box decorations, list markers, and `text-fit` lines sit closer to where Chrome puts them. A family's coverage subsets now load together, which lets later releases drop the per-subset family names.

### Stop leaking a direction mark into PDF text

Paragraphs set their direction directly instead of starting with an invisible U+200E or U+200F mark, so text extracted from a PDF no longer contains it. A right-to-left paragraph that starts with an inline box before any text still gets the mark.

### Keep the line strut when no font family matches

A line that holds only an image or another inline box keeps the default font's strut when its `font-family` matches no registered font, as Chrome does. Such lines used to come out a few pixels short.

### Support `font-optical-sizing`

Variable fonts with an `opsz` axis now set it to the font size, as Chrome does, so fonts like Inter draw their display cut at large sizes. `font-optical-sizing: none` or an `opsz` value in `font-variation-settings` turns this off.

### Keep the static position of absolute boxes whose containing block is further up

An absolutely positioned box without insets now starts where it would have sat in its parent's flow, padding included, even when its containing block is an ancestor further up. It used to start at that ancestor's content edge. Chrome places it the same way.

### Synthesize small caps for fonts without them

`font-variant-caps` now synthesizes small capitals when the font lacks the caps feature, as Chrome does. Lowercase letters render as capitals at 70% size, so `small-caps` works with fonts like Geist. `font-synthesis: none` or the new `font-synthesis-small-caps: none` turns this off.

### Embed fonts at their `font-variation-settings` in PDF

PDF output and the paint tree now instance a variable font at the axes `font-variation-settings` sets. Before, they used the default instance, so glyphs drew at the wrong weight or width while the layout used the right one.

### Check small-caps support in each run's own font and script

`font-variant-caps` now checks the font that draws each run, and that font's caps features for the run's script, as Chrome does. A fallback font keeps the small capitals it ships, and a font that lists them only for some scripts synthesizes them for the rest.

## takumi-core@0.28.0

### Keep no fragment items for hidden subtrees

Laying out a `display: none` box no longer keeps fragment items for the boxes under it, which paint never reads. A hidden 48-card panel costs 0.3 ms instead of 2.1 ms per render.

### Shape repeated text once per render

Text that several boxes repeat with the same style is shaped once per render again. A box that reused another's measurement used to drop the shared shape after drawing, so every later box shaped it anew.

### Composite pixels faster

Blending, masking and PNG scanline work run in 16-bit vector lanes, with the same pixels as before. Real templates render about 3% faster.

### Read repeated style values once

Reading a node tree parses each distinct style value once per property and reuses it wherever the tree repeats it. A 48-card dashboard reads its styles in 0.16 ms instead of 0.26 ms.

### Divide by 255 in 16 bits

`div_255` and `mul_div_255` replace `fast_div_255` and `fast_div_255_u32`. Both take bytes or byte products, so their arithmetic fits 16-bit vector lanes.

### Redraw rounded boxes faster

A rounded background drawn at the same place and size as in an earlier render reuses its rasterized mask from the glyph cache. A mask may take at most `glyph_cache::max_entry_bytes`, a 32nd of the budget. `PathCommand::digest` hashes a path for such keys.

### Rasterize inline SVG images once

An `<img>` whose `src` is SVG markup now goes through the render's image cache, as `data:` URIs already do. Repeated renders reuse its parsed tree and rasters instead of redrawing them. Real templates render about 3% faster.

### Parse inline styles faster

A CSS value that fails one way of parsing no longer builds an error message before trying the next. Building nodes from HTML with inline styles takes about 40% less time.

## takumi-core@0.27.0

### Lay out text faster

The glyph cache and per-render font metrics hash their keys with a faster hasher. Text-heavy layouts take about 6% less time.

### Lay out paragraphs with inline elements faster

A paragraph holding spans no longer lays its text out a second time to find its baseline. Large documents lay out about 8% faster.

### Lay out inline spans faster

An inline span works out its strut from its line height alone, without building its whole text style. Large documents with many spans lay out about 5% faster.

### Lay out Latin text faster

Text below U+02C7, such as Latin, no longer scans the CJK tables to decide whether underlines skip its ink. Large documents lay out about 3% faster.

### Look up glyphs faster

Each glyph's cache key now hashes 12 bytes instead of copying a 600-byte hasher. Large documents lay out about 2% faster.

### Write glyph outlines only when a run asks for them

`paint()` no longer writes every run's outline up front. `run.outline()` builds it when called. A 2,000-section document paints 3 times faster in about a third of the memory.

In Rust, `TextRun::outline` is replaced by `PaintTree::run_outline(node, run)`.

### Expand each font stack once per render

A `font-family` stack is matched against registered font subsets once per render, not on every text style. Large documents lay out about 4% faster.

### Write SVG output faster

SVG output and the paint tree's glyph outlines write numbers below 1,000 without float formatting, with the same bytes. A text-heavy SVG renders about twice as fast.

### Read each glyph's ink box once

A glyph's ink box is worked out when the glyph is resolved, not on every use. Large documents lay out about 7% faster.

`ResolvedOutlineGlyph::Plain` and `ResolvedOutlineGlyph::Color` gain an `ink` field, which `outline_ink` computes from their paths.

## takumi-core@0.26.0

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

### Refuse node trees nested deeper than 512 levels

A deeper tree now fails with `nodes nest deeper than 512 levels`, Blink's parser limit, instead of crashing the process. `takumi_html::DEFAULT_MAX_DEPTH` is now `takumi_core::layout::node::MAXIMUM_DOM_TREE_DEPTH`. `takumi::DEFAULT_MAX_DEPTH` keeps its name.

### Break pages as Chrome prints them

- A page break falls between line boxes. A paragraph taller than a page no longer leaves the first page blank, and short paragraphs can split too.
- A line or image taller than a page moves to the next page, then continues over the pages after it.
- A box's top and bottom border and padding stay with its first and last line.
- A table row whose cell padding does not fit moves to the next page whole.

### Lay text out once

Painting, page breaking and PDF output reuse the lines layout made instead of shaping text again. A 1,500-page document renders in about a sixth less memory and a tenth less time.

`FragmentItems`, `OwnContent::fragment_items` and `RenderContext::release_shaped_text` are new.

### Lay out tables as Chrome does

- An `auto`-width table shrinks to fit its content.
- Columns share the width through the CSS table algorithm. Percentage, `min-width` and `max-width` cell widths size their columns.
- `width: min-content`, `max-content`, `fit-content` and `stretch` size a table from its columns.
- `thead`, `tbody` and `tfoot` default to `vertical-align: middle`, as in Chrome.
- `vertical-align: baseline` cells line up on the row's deepest baseline.
- A non-cell inside a row gets wrapped in an anonymous cell.
- Tables with more than 10,000 rows lay out every row. Rows past the 10,000th used to pile up.

### Lay out a node's inline content in one call

- `OwnContent::lay_out_inline` returns a `PaintedInline` with the node's spans, text and runs. Painting the node again in the same content box reuses it, unless an ellipsis cut the content.
- `PositionedInlineRun::resolved_glyphs` is a `ResolvedGlyphs`, an `FxHashMap` keyed by glyph id. Code that only calls `get` keeps working.

### Render in less memory

- An 11,000-row table renders in about 110MB less memory, since the layout tree is freed once layout finishes.
- Anonymous boxes share one layout style, and scenes give back unused list room, so large tables use less memory.
- A clip that covers the whole image, such as `overflow: hidden` on the root, no longer keeps a second mask.
- A full-width `blur-3xl` on a 1200×630 image peaks 38% lower.

## takumi-core@0.25.0

### Report a capped WebP animation as unfinished

A WebP stream cut at the frame cap now reports the same unfinished flag as APNG and GIF instead of claiming it ended.

### Rename the background layer inputs

`BackgroundLayerInput::resolve` and `BackgroundLayersInput::resolve` replace the `Resolve*` structs and free functions.

### Drop unused paint re-exports

`takumi_core::paint` no longer re-exports the tile position helpers, the gradient row states, or `LinearGradientFastPath`; a few layout helpers are crate-private now.

### Clip animation frames that start outside the canvas

A GIF or WebP frame whose origin lies beyond the logical screen no longer panics the decoder. The frame is clipped away instead, as browsers do.

### Build gradient tables through ColorLut

Gradient tiles carry one `lut: ColorLut`. `ResolvedGradientStop::resolve` and `ColorLut::new` replace the free functions.

### Move the border helpers onto their types

`side_bands`, `border_dash_pattern`, `inset_size`, and `rect_offset` are now methods on `BorderProperties`, `BorderStyle`, `Size`, and `Rect`.

### Name the values the paint helpers pass around

`BorderStyle::dash_pattern` returns a `BorderDash`. `resolve_inline_runs`, `inline_background_path`, `glyph_outlines`, and `run_decorations` are methods on `BuiltInlineLayout`, `InlineBackgroundFragment`, and `ShapedRun`.

### Read render settings through RenderContext methods

`time_ms`, `draw_debug_border`, and `dither_gradients` are methods now. `RenderContext::builder()` is unchanged.

## takumi-core@0.24.0

### Break lines where Chromium breaks them

Soft wrap opportunities follow Chromium's table instead of plain ICU, so
`ISBN-2026408` wraps after the hyphen the way a browser wraps it. Text under
`word-break: break-all` still breaks at every character.

### Apply `@media print` rules to PDF output

PDF renders now match the `print` media type, and image renders match
`screen`. `Viewport::media_target` picks which one a render resolves against.

### Round normal line heights like browsers

`line-height: normal` resolves from the primary font as
`round(ascent) + round(descent) + round(line gap)`, as Blink computes it, so
line stacks match browser pixel positions. Fallback-font runs still grow the
line to their own rounded height. Subset groups expand in registration order
when ranks tie, so the primary font is the first registered subset.

### Treat layout values a browser reads as equal as equal

Text outline fragments, uniform border widths, and the ellipsis overflow check
all compare against 1/64px now, the step Blink stores layout on. Each used to
carry its own tolerance, so the same difference counted three ways.

### Paint inline span backgrounds

A `display: inline` span fills its `background-color` under the text, one
rounded fragment per line. Horizontal padding reserves space on the line and
the fragment grows by it, so badge and pill markup renders instead of
silently dropping the background.

### Register multi-family font files in face order

A font file carrying several families (a ttc) registers them in face order.
Fallback selection no longer varies between renderers loading the same file.

### Match browser thickness for text-decoration auto

`text-decoration-thickness: auto` resolves to a tenth of the font size with a
1px floor, as Blink computes it; `from-font` keeps the font's own metric.

### Interpolate legacy-color gradients in sRGB

A gradient with only legacy color stops (hex, named, non-relative `rgb()`,
`hsl()`, `hwb()`) interpolates in sRGB, as browsers do. Modern stops keep
Oklab, and Tailwind gradient utilities pin `in oklab`.

### Combine media queries with `or` and nested groups

`@media (min-width: 600px) or (min-height: 900px)` and nested groups such as
`(width > 100px) and ((height < 500px) or (orientation: portrait))` now parse.
`not` applies inside a group as well.

### Stop decorating line-end whitespace

Underline, overline, and line-through no longer extend over the collapsed
whitespace at a line break, matching browsers for LTR text. RTL is
approximate: the trim lands on the visual right edge.

### Keep parsing a stylesheet past an unknown media query

An unknown media feature such as `(prefers-color-scheme: dark)` used to fail
the whole stylesheet. The query now matches nothing and the rest of the sheet
still applies, as the spec requires.

### Size corner-keyword radial gradients like browsers

An ellipse `radial-gradient` sized `farthest-corner` (the default) or
`closest-corner` passes through that corner, as browsers do; both keywords
previously stopped at the box sides. Side distances are measured absolute,
so a center outside the box keeps positive radii.

### Accept media query range syntax

`@media (width >= 768px)` and `@media (400px < height <= 700px)` now parse
alongside `min-width` and `max-width`. A bare `(width)` matches any non-zero
viewport width.

### Dither gradients under the dithering option

With `dithering` set, gradient fills add ordered Bayer noise before 8-bit
quantization, as Chrome does, so slow ramps stop banding. The old output pass
that reduced every pixel to 128 levels is gone; `floyd-steinberg` is now a
deprecated alias of `ordered-bayer`. The default output is unchanged.

### Match `resolution` and `aspect-ratio` media queries

`@media (resolution >= 2dppx)` reads the render's device pixel ratio, in
`dppx`, `x`, `dpi`, or `dpcm`. `@media (aspect-ratio >= 16/9)` reads the
viewport's ratio. Both take the range syntax and the `min-`/`max-` prefixes.

## takumi-core@0.23.1

### Apply text-fit to text with letter or word spacing

`text-fit` shrink and grow now scale text that sets `letter-spacing` or
`word-spacing`. Both used to disable the fit silently, so a spaced headline
overflowed instead of shrinking.

## takumi-core@0.22.0

### Stop drawing a hairline on borderless sides

A border on only some sides drew faint lines along the sides that had none, in PDF and in SVG. Those sides now draw nothing.

### Capture fixed and absolute descendants with transforms and filters

Transforms, transform properties, offset paths, filters, and backdrop filters now capture fixed and absolute descendants. Those descendants position against that ancestor, matching Blink.

### Add `units` for the absolute length constants

`takumi_core::units` exports the CSS absolute units in 96 dpi pixels, so page geometry and CSS lengths resolve through one set of constants instead of each crate rederiving them.

### Embed JPEG and WebP images

`images` took bytes in any raster format, but only PNG reached the page: a JPEG or a WebP failed the whole render. Both embed now, and a JPEG keeps its own compression instead of being decoded and re-encoded.

### Draw list markers for `<ol>` and `<ul>`

List items rendered with no bullet or number. A `display: list-item` box now generates a marker: `list-style-type`, `list-style-position` and `list-style-image` pick what it draws and where it sits, and `<ol start>` and `<li value>` set the count.

### Snap border and outline widths to whole device pixels

`border-*-width`, `outline-width` and `outline-offset` now follow the CSS `snap a length as a border width` rule. Anything thinner than one device pixel draws as one, so a hairline stops fading in and out with its position. The rest rounds toward zero, so `1.5px` draws as `1px`.

## takumi-core@0.21.0

### Honor emoji variation selectors in font selection

When emoji render from registered fonts (`emoji: "from-font"`), a codepoint followed by `U+FE0F` now picks a registered color font and one followed by `U+FE0E` a registered monochrome font. `font-family` order does not affect either. Bare codepoints keep following the stack, matching browsers.

### Honor `widows` and `orphans` at page breaks

A cut through a paragraph keeps at least `orphans` lines at the bottom of the page and `widows` lines at the top of the next. Both are inherited CSS properties and default to 2, the Chromium print default. Set both to 1 to disable the limits. Minimums the page cannot fit are dropped for that page.

## takumi-core@0.20.0

### Resolve `rem` against the viewport instead of the outermost node

`rem` used the computed `font-size` of the tree's outermost node, so a `text-2xl` there silently scaled every `rem` length below it, including the whole Tailwind spacing scale. `rem` and `rlh` now resolve against the viewport's font size. A tree rooted at an `<html>` element keeps the old basis, since CSS makes that element the root; the JS `fromHtml` returns one for a full document.

## takumi-core@0.19.1

### Accept `text-decoration: none`

`none` is the initial value of `text-decoration-line`, but the parser rejected it and failed the whole render. Both `text-decoration` and `text-decoration-line` now parse `none` as no decoration.

## takumi-core@0.19.0

### Restore JPEG, WebP and GIF decoding

`takumi` disables `takumi-core`'s default features and never re-enables image decoding, so every build decoded PNG and ICO only. A WebP source failed with `The image format could not be determined`. `image-decoding` is now a `takumi` feature as well, on by default, and it splits into `jpeg`, `webp` and `gif` for a build that wants one format and not the others. The napi and wasm bindings turn it on too.

## takumi-core@0.18.1

### Replace the unmaintained `paste` with `pastey`

`paste` has been unmaintained since 2024, and `cargo audit` reports RUSTSEC-2024-0436 for it. `pastey` is a maintained fork with the same macro surface, so the generated code is unchanged.

## takumi-core@0.18.0

### Map a cluster's glyphs to its source text once

In Devanagari and other scripts that attach marks to a base letter, the base and its mark form separate clusters over the same source text. Every glyph claimed that whole range, so `मोटा` came out of the PDF text layer as `ममोटटा`. Overlapping glyphs now share one range and one `/ActualText`, and every glyph gets a codepoint mapping so a viewer without `/ActualText` support does not read a raw glyph index.

### Key text layout on the stroke width

Two passages of the same words in the same font shared one shaped layout, so a `-webkit-text-stroke` width set on the second was drawn at the first one's width.

### Widen a clipped background by the text stroke

A transparent `-webkit-text-stroke` reveals a ring of the background painted through the glyphs. In PDF that ring was missing: the background pass widened the coverage by the faux bold alone, so the output disagreed with the image and SVG backends.

### Stroke the span that asked for it

`-webkit-text-stroke` was read off the element holding the text, so a `span` setting it for itself came out unstroked, and a nested one turning it off still got the parent's outline. The stroke now travels with the text run, in every backend.

## takumi-core@0.17.0

### Decide an inline outline's stroke once

`SizedFontStyle::outline_stroke` says whether a `<span>`'s `outline` paints and how to dash it. Five items in `takumi_core::layout::inline` no longer leave the crate.

### Build an inline layout request from the box it fills

`InlineLayoutRequest::in_content_box` and `in_available_space` build a request from the box it fills.

### Animate colours in sRGB

An animated or transitioned colour now interpolates in sRGB with premultiplied alpha, the space CSS Color 4 gives legacy colours, instead of Oklab. Midpoints between distant hues shift to match a browser.

### Paint a border ring from one place

`paint_border` fills a border ring, two for `double`, or strokes a uniform dashed or dotted one. `PaintDevice` gains `stroke_shape`. A fully transparent border no longer reaches the output.

### Paint a background colour from one place

`PaintDevice` and `FillShape` let shared painting code drive a rasterizer, an SVG writer or a PDF writer.

### Route shared codepoints to the subset that declares them

A Google Fonts subset encodes more than the `unicode-range` it was cut for, and the Cyrillic and Greek ones also carry the ASCII space and the Latin capitals. Selection took the first subset whose glyphs covered a character, in family-name order, so those codepoints left the Latin subset and every word split into separate runs. Subsets now rank by the range they declare, lowest first.

### Draw every border ring from the shared painter

The SVG backend draws its borders through the shared painter. A `double` border fills two rings instead of stroking two centerlines.

### Place replaced content from one place

`object-fit` and `object-position` place replaced content from one place. An `object-position` past 100% now clips to the content box.

### Ask a background layer once whether it paints

`BackgroundImage::paints` replaces the three spellings each backend had for the same question.

PDF used to treat a `url()` layer as unpaintable when built without the `images` feature, which skipped the whole background-image pass rather than that one layer.

### Resolve background layers once, for every backend

`takumi_core::layout::background` works out how many tiles a `background-image` layer paints and where each one goes. The raster and SVG backends each carried a copy of that arithmetic. Rasterizing a tile stays with the backend that draws pixels.

### Decide a box's paint in one place

`BoxPainter` decides what a box paints — its background shape, colour, shadows and outline — for every backend.

### Skip the ink an underline runs through, in every backend

`text-decoration-skip-ink` breaks an underline where the glyph outlines cross it, in every backend. A gap inside a letter stays a gap.

### Paint text decorations from one place

`paint_run_decorations` paints a run's underline, overline and line-through for every backend.

### Rotate hue from the same matrix everywhere

`takumi_core::filter::ColorMatrix` turns a colour-transforming `filter` function into the matrix Filter Effects defines for it. The raster backend had written the `hue-rotate` coefficients out a second time and rounded the angle to whole degrees first, so `hue-rotate(45.5deg)` rotated by 45.

### Collect a box's shadows once

`BoxPainter::shadows` resolves `box-shadow` and splits it into the layers inside the box and the ones outside, so a backend no longer walks the list itself.

A fully transparent shadow is dropped. Two backends used to keep it and paint nothing.

### Stack text shadows the way a browser does

The raster backend painted `text-shadow` in list order, putting the last one on top. CSS puts the first one there, so a stack of glows came out inverted. `SizedFontStyle::painted_text_shadows` walks them back to front for every backend, and drops the ones nobody sees.

### Resolve inline boxes once, for every backend

`resolve_inline_box` places an inline box's replaced content or nested subtree, shared by the SVG and PDF backends.

### Shade a 3D border in every backend

`inset`, `outset`, `groove` and `ridge` borders now shade their sides in the SVG and PDF backends, as the raster backend already did.

### Keep the rest of a `style` attribute when one declaration fails

A value this crate cannot read, such as `width: fit-content`, discarded every other declaration in the same `style` attribute. It now invalidates only itself, which is the recovery CSS asks for and what a `<style>` sheet already did.

### Paint the outline above the content

An `outline` painted under the box's own text and images, so a negative `outline-offset` disappeared behind them. CSS 2.1 Appendix E paints the outline last, and every backend now does.

### Draw dashed, dotted and double borders in PDF

`dashed`, `dotted` and `double` borders and outlines now draw in PDF instead of falling back to solid.

## takumi-core@0.16.0

### Clip elements with `clip-path`

`inset()`, `ellipse()`, `polygon()` and `path()` now clip an element and its decorations, as a real PDF clipping path rather than a rasterized mask.

`clip_shape_commands` in takumi-core resolves a basic shape to path commands, which is where the raster backend's copy of that geometry now lives too.

### Link to anchors inside the document

`<a href="#section">` now resolves to the element with that `id` and lands on the page holding it, so a table of contents works inside the PDF. A fragment matching no element is dropped rather than written as a link that goes nowhere.

`Node::id` is public, alongside the existing `href`, `alt` and `tag_name` accessors.

### Bound allocations and loops driven by untrusted input

Three denial-of-service paths are closed. SVG rasterization and canvas allocation now cap at 16M pixels and return an error past it, instead of aborting on a huge allocation. A `background-size` past `i32::MAX` no longer wraps a repeat step negative and loops forever.

## takumi-core@0.15.0

### Vendored resvg updated to 0.48.1

Pulls the upstream parser and filter fixes: nested `svg` transforms are no longer applied twice, a missing `width`/`height` is computed from the viewBox aspect ratio, `href` takes precedence over `xlink:href`, `fr` is inherited for radial gradients referenced via `href`, and oversized filter regions no longer panic.

### `Node::alt` keeps empty values

`alt()` now returns `Some("")` for an explicitly empty attribute, so callers can tell a decorative image apart from a missing `alt`.

## takumi-core@0.14.1

### Render bare `border-t`/`border-r`/`border-b`/`border-l`/`border-x`/`border-y`

The side utilities only parsed with a width suffix, so plain `border-b` drew nothing.

## takumi-core@0.14.0

### Add PDF hyperlinks, outline, and document metadata

Anchors with an `href` become clickable link annotations, at the box for block anchors and per text run for inline ones. An `outline: true` option builds PDF bookmarks from `h1`–`h6` headings, and a `metadata` option fills the document's title, description, authors, keywords, and creator.

## takumi-core@0.13.0

### Add corner-shape

`corner-shape` and its per-corner longhands render `round`, `squircle`, `bevel`, `scoop`, `notch`, `square`, and `superellipse(<number>)` corners, and interpolate in animations per the spec. The shape applies wherever `border-radius` does: borders, backgrounds, box shadows, masks, and overflow clipping. Corner curves use Chromium's superellipse approximation, so a squircle here matches one drawn by a browser.

## takumi-core@0.12.0

### Add box-decoration-break

`box-decoration-break: slice | clone` parses. The paged PDF backend uses it to decide how a box painted across page fragments closes its edges: `slice` (the default) leaves the edge at a break open, matching browser print behavior, while `clone` paints every fragment's complete borders and background. Cloned padding is paint-only and does not reserve layout space.

### Paint fractional border widths evenly

Layout rounding snapped border widths to whole pixels per edge, so a uniform 2.5px border could come out as a 2px top and a 3px bottom depending on where each edge landed on the pixel grid. Border widths now keep their fractional values through layout rounding and paint with coverage antialiasing, matching how browsers draw them.

### Add break-before, break-after, and break-inside

The three fragmentation properties parse with their page values: `break-before: page`, `break-after: page`, and `break-inside: avoid`. Raster and SVG output ignores them; the paged PDF backend consumes them. `ShapedRun` now carries its source text range and per-glyph cluster ranges for text-extraction backends, and the JPEG, WebP, and GIF decoders sit behind a new default-on `image-decoding` feature so slim builds can drop them.

## takumi-core@0.11.0

### Place the underline below descenders with text-underline-position

`text-underline-position` now parses and applies. `under` puts the underline at the bottom edge of the em box rather than at the font's underline metric, so it clears descenders. `auto` and `from-font` keep the font's underline metric, which is what the renderer already did. `left` and `right` are rejected, since they only mean something in vertical writing modes, which takumi does not support.

## takumi-core@0.10.0

### Register a font from bytes the caller already holds

`FontSource::from_shared` takes an `Arc<dyn AsRef<[u8]> + Send + Sync>` and passes it to the font system untouched, so a memory-mapped face stays paged from disk instead of being copied onto the heap, which for a CJK family is tens of megabytes that never reach the heap at all. WOFF and WOFF2 still decompress into a fresh buffer.

### Register a font straight out of the binary

`FontSource::from_static` takes a `&'static [u8]`, so an `include_bytes!` face is read where it already sits, in the read-only segment, and the caller writes no `Arc` of its own: the one held internally wraps the slice reference, never a copy of the font. Its blob id comes from the address and length instead of a hash of the content, so registering a 30 MiB CJK face no longer reads through every page of it and the face is paged in a glyph at a time.

## takumi-core@0.9.0

### Bound calc() depth, var() substitution size, and list interpolation length

Four places in the CSS value layer let caller-supplied text drive unbounded recursion or allocation. `calc()` recursed once per leading unary sign and once per nested `calc(`. `var()` substitution capped neither its nesting nor its total substituted bytes; its cycle guard is pushed and popped per reference, so it stops a property referencing itself but not fan-out, and `--n: var(--n-1)var(--n-1)` doubles per link. `RepeatToLcm` list interpolation allocated the full LCM of the two list lengths. A `calc()` resolving to NaN or infinity reached taffy unclamped, where every other `Length` arm is clamped on its way through `to_px`.

Release builds abort on panic, so a stack overflow or a failed allocation here took down the host process instead of returning an error. The limits match Blink: depth 100, 2 MiB of substituted text (the value the spec and Firefox use as well), and 1000 interpolated list entries.

### Replace `ResolvedGlyphPlacement` with `geometry::Placement`

`Placement` moves into `takumi_core::geometry` and takes over from `ResolvedGlyphPlacement`, which described the same four fields. `BuiltInlineLayout::resolved_glyphs` is now keyed to `Arc<ResolvedGlyph>`, so a glyph cache hit stops copying the outline commands.

### Key the glyph caches on font content instead of blob identity

`Blob::new` draws its id from a global counter, and that id is part of the key for the shared resolved-glyph and glyph-mask caches, as well as parley's shaping data cache. Registering the same face again produced a fresh id, so a second renderer, or one rebuilt to reclaim memory, missed every glyph the face had already resolved and filled the budget with entries nothing would hit again. The id is now a hash of the decoded font bytes, so identical faces share cache entries no matter how often they are registered.

### Couple the overflow axes and stop mixed overflow from blanking a node

`overflow-x` and `overflow-y` were read straight off the computed style, so `overflow-x: hidden` next to `overflow-y: visible` stayed mixed. CSS Overflow 3 says a `visible` axis paired with one that is neither `visible` nor `clip` computes to a scrolling value instead, which is why Chrome clips both axes there. `resolve_overflows` now applies that coupling, so the pair reaches layout, painting, and the SVG backend already resolved. `clip` next to `visible` is a legal combination and still passes through untouched.

That legal pair then hit a second bug. The mask builder marks an unclipped axis with `u32::MAX`, and the identity-transform fast path narrowed it with `as i32`, which truncates to `-1` rather than saturating. The clip rectangle came out empty, so the node rendered nothing at all. The comparison now clamps before narrowing, matching the rotated path, which had always compared in `u32`.

## takumi-core@0.8.0

### Share the glyph caches across worker threads

The glyph mask and resolved-glyph caches were thread-local maps under one process-wide byte counter, but eviction only pruned the inserting thread. An idle worker kept its share forever, so real retention multiplied with the thread pool (the emoji-bitmap growth noted in #1023). Both caches are now process-global `quick_cache` instances splitting the same 8 MiB budget: a glyph resolved on one thread is a hit on every thread, and eviction is global. `GlyphCache` methods take `&self` and `get` returns a clone; `set_glyph_cache_max_bytes` now applies to caches not yet used, so call it before the first render.

## takumi-core@0.7.0

### Move resolved-glyph types to `resources::glyph`

Glyph rasterization — resolving a shaped glyph to a bitmap or vector outline — now lives in its own `resources::glyph` module, split out of the font registry it was tangled with. `ResolvedGlyph`, `ResolvedOutlineGlyph`, and `ResolvedColorLayer` move there from `resources::font`; imports of those types need updating.

### Decode animated GIF frames on demand instead of holding the whole timeline

Once a render scrubbed past the first frame, an animated GIF decoded and kept every remaining frame, so the encoded bytes, the first frame, and all later frames stayed resident at once — and none of it counted against the image cache budget. Frames past the first now decode at draw size when they are sampled and drop right after, so a GIF holds only its encoded bytes and first frame. Output is unchanged.

### Stop untrusted SVG from reading local files via `<image href>`

An `<image>` or `<feImage>` element whose href is a filesystem path is no longer read from disk when parsing untrusted SVG markup or applying SVG filters. The string href resolver is disabled at both entry points, matching the nested-SVG path that already ignored external references. `data:` URI images keep working.

### Bound the thread-local glyph caches by bytes

The resolved-glyph and glyph-mask caches were thread-local maps capped at 4096 entries each: per-entry size was unbounded, the cap multiplied per worker thread, and overflowing flushed the whole map so hot glyphs paid the rebuild cost. Both caches now weigh entries in bytes against one process-wide 8 MiB budget, tunable through `takumi_core::resources::glyph_cache::set_glyph_cache_max_bytes`. Going over budget first drops entries no recent render touched, then only as many fresh ones as the overage requires; the map is never flushed whole. A retention test renders the same content 200 times and asserts live heap bytes stay flat, so budget regressions fail in CI.

### Halve peak memory for native WebP decode

Native WebP decode now writes straight into a caller-owned buffer via `WebPDecode` with external memory, dropping the extra full-frame copy `WebPDecodeRGBA` required. Already-RGBA sources decoded through the `image` crate also skip one transient full-frame clone (`into_rgba8`). Output is bit-identical.

### Cut wasted work in style matching

`record_matches` no longer pushes entries for empty declaration blocks, and the per-node ancestor bloom filters (one multi-kilobyte copy per node) are replaced by a single counting filter walked along the DFS ancestor chain. Rendered output is unchanged.

### Unify decoded resources behind one budgeted cache

Decoded images had a byte budget, but each SVG kept up to 32 rasterized pixmaps outside it, and every render re-parsed its stylesheets from scratch. `ImageCache` is now `ResourceCache`: SVG sources, their rasterized pixmaps, and parsed stylesheets all weigh against the same budget as decoded images. The default budget drops from 64 MiB to 16 MiB and becomes configurable — `new Renderer({ cacheMaxBytes })` in the bindings, `ResourceCache::new(max_bytes)` in Rust, with `0` disabling caching. SVG rasters and parsed stylesheets now also survive across renders, so a server re-rendering the same template stops re-rasterizing and re-parsing per request. Rust callers: `RenderOptions.stylesheet` is now `Arc<StyleSheet>`; pass `sheet.into()`.

### Skip the style-match arena allocations when no CSS rules apply

`match_stylesheets_view` now filters the stylesheet rules before it builds the per-node match buckets, and returns early when nothing survives. Renders driven only by inline styles or Tailwind classes (the common case) no longer allocate the per-node bucket vectors or walk the matcher for zero rules. Rendered output is unchanged.

### Decode downscaled WebP at the target size

Native WebP sources drawn smaller than their pixel size now decode through libwebp's `use_scaling`, so the full-size frame is never allocated (~6 MB instead of ~48 MB for a 12 MP hero). libwebp's rescaler replaces the in-house resampler on this path, which can shift pixels slightly; wasm keeps the full-decode path.

### Add `font-kerning` and `tab-size`

`font-kerning: auto | normal | none` toggles the shaper's `kern` feature; an explicit `font-feature-settings` still wins on a tag conflict. `tab-size: <number>` expands preserved tabs to that many spaces (default 8) before shaping. Preserved tabs previously reached the shaper as U+0009 and rendered a font-dependent glyph, so tab characters under `white-space: pre` now render correctly.

### Share one path-builder helper between layout and raster

`takumi-core` and `takumi-raster` each kept a private single-impl trait wrapping `Vec<PathCommand>` pushes, plus two more one-method traits that existed only for method-call syntax. The push helpers now live once as the public `takumi_core::geometry::PathBuilder` trait; the private traits are gone. Rendered output is unchanged.

## takumi-core@0.6.3

### Drop whitespace between absolute-only block siblings

When every element child of a block container was absolutely positioned, the whitespace text nodes from pretty-printed HTML formed an inline formatting context that swallowed the out-of-flow boxes, so none of them rendered. The whitespace drop now also runs when the only in-flow content is whitespace, keeping the absolute children in the layout.

### Serialize `text-fit: none` without its target and limit

The computed value of `text-fit` kept the target and limit keywords after `none`, so `none per-line 50%` round-tripped verbatim. Chromium drops both, since neither scales anything when the value is `none`. The serializer now stops after `none`.

### Keep whitespace collapse state across empty inline spans

An empty span with `white-space: pre` reset the cross-span collapse state, so a boundary space next to it could double up or vanish. Empty spans now leave the state untouched, matching Blink's opaque-to-collapsing empty text items.

### Honour per-element white-space when collapsing inline text

Inline whitespace collapsing read the block's white-space value for every span, so a `white-space: pre` child inside a normal-collapsing parent lost its spaces and line breaks. Each span now collapses against its own value. `<br>` also carries a `white-space: pre` preset, so its line break survives.

## takumi-core@0.6.2

### Ellipsize nowrap text without a break opportunity

`text-overflow: ellipsis` with `white-space: nowrap` only kicked in when the text could wrap, so a single long token was clipped with no ellipsis. Overflow detection now also checks the line's inline advance against the box, matching how Blink truncates any overflowing line at a character boundary.

## takumi-core@0.6.1

### Include glyph ink extents in text paint bounds

Text paint bounds only covered the advance × (ascent + descent) metrics box, so isolation surfaces (opacity, filters) clipped ink outside it: synthetic-italic overhang, faux-bold outset, and negative bearings. Node paint bounds now merge each glyph's ink extents.

## takumi-core@0.6.0

### Accept raw RGBA pixels as an image source

An image node's `src` takes `{ width, height, data, premultiplied? }` and renders the pixels without decoding; the `Bitmap` JSX helper wraps it as an `<img>`.

## takumi-core@0.5.1

### Feed filter layers to resvg without a PNG roundtrip

Hand the premultiplied layer pixels to the vendored resvg pipeline through a new raw image kind, dropping the unpremultiply + PNG encode/decode in `apply_svg_filter`.

### Fix inherited resvg filter and parser bugs

Correct the spotlight Y offset, drop-shadow sRGB double conversion and displacement-map premultiplied reads; guard href cycles, turbulence seed overflow, convolve-matrix size overflow and oversized blur/morphology radii; make `<switch>` skip text branches and paint fallbacks apply for non-paint-server references.

### Prune the dead text node chain from vendored resvg

Remove `Node::Text`, the text tree types and their render, clip and paint-server arms; the parser already dropped text elements with the text feature stripped.

### Route vendored resvg image decoding through the core decoders

SVG-embedded raster images now decode through the shared image pipeline, dropping the imagesize and zune-jpeg dependencies and tiny-skia's png-format feature.

### Apply filter references without building a render tree

Parse `<filter>` markup straight into resolved filters and run them on the layer pixels, skipping the synthetic document render.

### Vendor resvg into takumi-core

Replace the external resvg dependency with a vendored copy of usvg + resvg 0.47, stripped of the text, svgz, system-fonts, memmap-fonts and writer features and the CLI. Rendering output is unchanged.

### Stop blend isolation from clipping text descenders

Include plain text nodes' glyph ink in scene paint bounds; `mix-blend-mode` on a text node no longer cuts glyphs that overflow the line box. Bounds now report unknown instead of underestimating for styles whose ink extent is not measured (shadows, outlines, text strokes), falling back to full-viewport isolation.

### Sample conic gradients deterministically

Replace the per-pixel libm `atan2` in conic gradient sampling with Skia's `xy_to_unit_angle` polynomial, so every platform renders identical conic output and sampling gets cheaper.

## takumi-core@0.5.0

### Apply filter references without the base64 roundtrip

`apply_svg_filter` hands the layer to resvg through a custom href resolver as fast-compressed PNG bytes, dropping the base64 encode, data-URI decode, and multi-megabyte XML parse.

### Support SVG filter references in `filter`

`filter` and `backdrop-filter` accept `url(data:image/svg+xml,...)` with inline `<filter>` markup, mixing freely with filter functions. The raster backend executes the graph through resvg; the SVG backend emits the markup verbatim.

### Accept SVG sources without xmlns

Inline SVG image sources no longer need an `xmlns` declaration. Data-URI decoding and base64 encoding are shared through `resources::image` (new `to_data_url`).

## takumi-core@0.4.0

### Decode GIF frames on the raw `gif` decoder

Composite frames on one reused canvas instead of the `image` crate's per-frame allocations; skipped frames no longer allocate or premultiply.

### Decode GIF frames lazily

Decode only the first GIF frame up front and the rest on first sample past it; static renders no longer decode the whole animation. `GifSource::frame_at_time` returns `Arc<ImageBuffer>` and `RenderedImage::Borrowed` becomes `Sampled`.

### Claim generic font families from the JS font API

Font descriptors accept `generic` (e.g. `"monospace"`), so stacks like Tailwind's `font-mono` resolve to registered fonts without naming the family.

### Stream PNG decode at draw size

Non-interlaced PNGs decode row-by-row through a streaming resampler, so downscaling never materializes the full-size buffer. Output is byte-identical to decode-then-resize.

### Decode bitmaps at draw size

Bitmaps loaded through `ImageCache` stay encoded until draw time, decode scaled to the box they are drawn into, and cache per content and target size. Adds `ImageSource::Encoded`; `get_or_decode` returns it instead of `Bitmap` for PNG/JPEG/WebP. `cache: "none"` keeps returning an eagerly decoded `Bitmap`.

### Memoize GIF frames at draw size

Frames past the first decode scaled to the box the GIF is drawn into, so an animation's memoized timeline holds draw-sized frames instead of canvas-sized ones. Adds `GifSource::frame_at_time_covering`.

## takumi-core@0.3.3

### Ignore null style values

Skip `null` and `undefined` style declarations instead of failing to deserialize the style.

### Fix inline SVG data URIs truncated at `#`

Percent-escape `#` in data URI bodies so inline SVGs are not cut off at the first fragment delimiter.

## takumi-core@0.3.2

### Make `:lang()` actually match

`:lang()` parsed but never matched, like every other pseudo-class the engine treats as
stateful. It needs no live state, only the `lang` attribute inherited up the tree, which a
static render already has. It now matches BCP-47 basic filtering (`:lang(zh-Hant)`, comma-separated
ranges, `*`) against the nearest ancestor-or-self with a `lang` set — the standards-based way to
route different fonts to different languages on the same page, e.g. `:lang(ja) { font-family:
"Noto Sans JP" }` alongside `:lang(zh-Hant) { font-family: "Noto Sans TC" }`.

### Fix `fontFamilies` order being ignored

`fontFamilies` only fed the fallback bucket, never the root style, so text
picked whichever registered font resolved first instead of the requested
order. `FontFamily`'s default is now empty instead of a generic `sans-serif`
token, so an empty root style falls through to the fallback bucket directly.

## takumi-core@0.3.1

### Fix gradient stop snapping panic for oversized stop lists

Gradients with more color stops than the LUT can hold no longer panic. The
sample-index clamp and normalization passes stay within LUT bounds.

### Cap image decode dimensions and GIF frame volume

Decoders reject images beyond 8192x8192 (via `image::Limits` for PNG and JPEG,
dimension checks for WebP) and GIFs beyond a total-frame pixel budget, stopping
decode-bomb OOM.

## takumi-core@0.3.0

### Refactor `font_families` and `lang` option type

Now both option takes resolved value instead of raw strings.

## takumi-core@0.2.0

### Drop `background-blend-mode` from the `background` shorthand

The `background` shorthand parsed a blend-mode token and reset
`background-blend-mode`, unlike browsers, where the shorthand touches neither. It
now leaves `background-blend-mode` alone; set it through the longhand. The
`blend_mode` field is gone from the `Background` shorthand value.

### Seal the `parlance` font model out of the public `style` API

`#916` grepped only `parley::`, so it missed `parlance` (the parley font
model) leaking through `style`. This follow-up seals it:

`FontFeature`, `FontVariation`, and `Tag` are now takumi-owned structs,
replacing `parlance::tag::{FontFeature, FontVariation, Tag}` in
`ComputedStyle::font_feature_settings`/`font_variation_settings`.
`FontWeight::Absolute` holds a plain `f32` instead of
`parlance::font::FontWeight`. `ComputedStyle::lang` is now `Option<Lang>`, a
takumi-owned BCP-47 tag, instead of `Option<parlance::language::Language>`.
`FontStretch`, `FontStyle`, `FontWeight`, `FontFamily`, and
`resources::font::GenericFamily` lose their `From<_>`/`Into<_>` impls
targeting `parlance` types; the conversions are now `pub(crate)` inherent
methods (`into_parlance`/`to_parlance`/`from_parlance_generic`) called only
at the shaping boundary.

Also dropped two unused `From<FontFamily> for parlance::FontFamily` impls
with no callers in the crate.

### Seal remaining `taffy`/`parley` leaks out of the public `style` API

`Position`, `Display`, `AlignItems`, `JustifyContent`, `BoxSizing`, `Direction`,
`FlexDirection`, `FlexWrap`, `Overflow`, `TextAlign`, `GridPlacement`,
`GridAutoFlow`, `GridRepetitionCount`, and `GridTemplateAreas` no longer
implement `From<_>` for their `taffy`/`parley` counterparts; the conversions
are now `pub(crate)` inherent methods (`into_taffy`/`into_parley`).
`TextWrapMode`, `WordBreak`, and `OverflowWrap` lose their `parley` `From`
impls the same way. `ResolvedVerticalAlign::apply` is now `pub(crate)`.

`takumi-raster`: dropped the unused `From<OutputFormat> for image::ImageFormat`
impl, which pinned the `image` crate into the public API without being called
anywhere.

### Represent the `none`/`normal` initial values of `max-*` and gaps

`max-width` and `max-height` are now a `MaxSize` value whose initial is `None`
(unbounded), instead of borrowing `Length`'s `auto`. `column-gap`, `row-gap`, and
the `gap` shorthand are now a `Gap` value whose initial is `Normal`. Rendering is
unchanged — `none` resolves like the old unbounded default and `normal` computes
to `0` — but the values now round-trip through `to_css` as `none`/`normal`.

### Seal `tiny_skia` and `image` types out of the public API

Glyph outline paths now use core-owned `geometry::PathCommand`/`geometry::Point`
instead of `tiny_skia::PathSegment`/`Point`. `ResolvedBitmapGlyph::pixmap`
(`tiny_skia::Pixmap`) is now `image: ImageBuffer`. `ImageBuffer::from_rgba`
(`Cow<RgbaImage>`) is now `from_rgba_bytes(Vec<u8>, width, height)`.
`layout::border::BorderProperties`'s path-building methods take
`Vec<geometry::PathCommand>` instead of `Vec<tiny_skia::PathSegment>`.

### Seal `parley::Layout` out of the inline-layout boundary

`BuiltInlineLayout::{layout, custom_inline_boxes}` are now private; the
measure-only walk moves into `BuiltInlineLayout::measure_runs`, returning
core-owned `MeasuredInlineRun`/`MeasuredInlineBox` (run text borrows the
layout). `get_parent_font_metrics`, `resolve_inline_line_metrics`,
`resolve_inline_line_states`, and `scale_text_fit_x` are no longer public.

### Replace `parley::GlyphRun` with a core-owned `ShapedRun` at the paint boundary

`PositionedInlineRun::glyph_run` is now `ShapedRun` (owned glyphs, brush, metrics,
font data) instead of `parley::GlyphRun<'l, InlineBrush>`; `PositionedInlineRun`
and `InlineRunLayout` drop their lifetime. `run_decorations` takes `&ShapedRun`.

### Seal `taffy` geometry types out of the public API

`layout::border::BorderProperties`, `shadow::SizedShadow`, `layout::inline::InlineBoxItem`,
and the other geometry-touching public items now use core-owned
`geometry::{Size, Rect, Point}` instead of `taffy::{Size, Rect, Point}`.
`layout::tree::LayoutResults::layout` returns an owned `geometry::ComputedLayout`
instead of `&taffy::Layout`. `LayoutTree::compute_layout` and the paint scene
(`build_stacking_contexts`, `NodePaint`, `layout::tree::OrderedChild`) now use
core-owned `geometry::{AvailableSpace, NodeId}` instead of `taffy::{AvailableSpace,
NodeId}`; `NodeId::ROOT` replaces the removed `root_node_id()` accessors. `taffy`
remains the layout engine at the `compute_layout` internals and `Style`
construction.

## takumi-core@0.1.0

### Merge `takumi-css` into `takumi-core`

Fold the CSS parsing, cascade, value types, selector matching, and `@keyframes`
layer back into `takumi-core` and drop the separate `takumi-css` crate. The
`style` module is reachable through `layout::style` as before; `matching` is now
crate-private. Core builds at `opt-level = "z"`, shrinking the napi `.node` ~7%
and the wasm binary ~8% with no render-path regression.

### Hide internal style/layout items from the public API

Roughly 200 CSS value types, parsing helpers, and internal accessors across `style::properties`,
`style::stylesheets`, `style::tw`, `style::selector`, and `layout` are now crate-private; they were
never meant to be constructed or matched on directly. `StyleSheet::property_rules` and
`apply_stylesheet_animations` are crate-private; use `StyleSheet`'s public parsing/query surface
instead. Gradient direction/keyword helpers (`GradientKeywordDirection`, `HorizontalKeyword`,
`VerticalKeyword`) stay public since they're reachable through `LinearGradientDirection`.

### Render backdrop-filter in the SVG backend

SVG has no native backdrop source, so the backdrop is the scene replayed up to
the element, run through an SVG `<filter>` chain, then clipped to the border
box and attenuated by the element's mask and clip-path. Adds
`ComputedStyle::has_shape_mask` and `Filter::is_drop_shadow`.

### Drop `serde_bytes::ByteBuf` from `ImageSourceInput::Buffer`

The `Buffer` variant exposed `serde_bytes::ByteBuf` in the public API. It now
holds a `Vec<u8>` with `#[serde(with = "serde_bytes")]`, keeping the FFI
bytes wire format while keeping `serde_bytes` out of the surface.

### Match the Chromium UA stylesheet for default element styles

Parse the relative font keywords `bolder`/`lighter` (`font-weight`) and
`larger`/`smaller` (`font-size`), resolving to the values Chromium uses. Expand
the default element presets to cover lists, `sub`/`sup`, `ins`/`del`, forms,
`details`/`summary`, and `search`.

### Seal `parley` out of the font API

`FontResource::override_info` takes a takumi-owned `FontOverride` (family name,
weight, style, width, axes) instead of `parley`'s `FontInfoOverride`, and
`FontResource::generic_family` takes a takumi-owned `GenericFamily` newtype with
named constants (`GenericFamily::SANS_SERIF`, …) re-exported from the prelude.
`FontSource` is an opaque struct over raw bytes. Callers no longer depend on
`parley`.

### Make subset-group font selection deterministic

Subsets registered under one logical family (via `FontResource::subset_of`) were kept
in registration order. Callers commonly register fonts concurrently, so that order — and
therefore which subset won for a codepoint covered by more than one (e.g. overlapping
weight subsets, where the loser is faux-bolded) — varied per process. Identical input
could render to different bytes run to run.

Subsets are now held in a `BTreeSet`, ordered by their family name, so expansion and
selection no longer depend on registration timing. Same input renders identically.

### Rename `resource` naming to `image`

`Node::resource_urls`/`Style::resource_urls`/`StyleDeclarationBlock::resource_urls` are now
`image_urls`, and `ImageResourceError` is now `ImageError` — everything they cover is image
loading, so the names say so.

### Mark public enums `#[non_exhaustive]`

The node and image enums (`NodeKind`, `ImageSource`, `ImageSourceInput`,
`ImageCacheMode`), the property identifiers (`LonghandId`, `ShorthandId`,
`PropertyId`, `StyleDeclaration`), and the spec-tracking value enums (`BlendMode`,
`Filter`, `BasicShape`, `ContentValue`, `TextTransform`, `WhiteSpaceCollapse`,
`OffsetPath`, `ImageScalingAlgorithm`, `Position`) can gain variants without a
breaking change. Match them with a wildcard arm.

### Serialize filter, grid track, and gradient values as valid CSS

`filter`/`backdrop-filter` and grid track lists were comma-joined
(`blur(3px), grayscale(0.5)`, `50px, 100px`) where CSS wants spaces, via the
shared `Vec` serializer. `ToCss` now carries a per-type `LIST_SEPARATOR`
(default `, `), overridden to a space for `Filter` and `GridTemplateComponent`.
Linear, radial, and conic gradients also placed the color-interpolation method
after a comma (`to right, in srgb`); it now sits in the leading clause
(`to right in srgb`). The output re-parses instead of dropping these
properties.

### Seal `selectors`, `tiny_skia`, `image`, and `smallvec` out of the public API

Selector matching moved into a `matching` module generic over a caller-supplied
`MatchableNode`; `CssRule`, `LayerName`, `Ident`, `SelectorImpl`, `PseudoClass`,
and `PseudoElement` are crate-private, keeping `selectors` off the public API.
The `From<Color>` conversions to `image::Rgba` and
`tiny_skia::PremultipliedColorU8` are gone; the raster backend converts
internally. `StyleDeclarationBlock::declarations` and
`DeclarationImportance::custom_properties` are crate-private with `len`/`is_empty`
accessors, dropping `smallvec` from the API. Gradient LUT, tile-position, and
transfer-table helpers plus the gradient tile types moved into a `paint` module
the backends use directly, keeping `tiny_skia` off the prelude.

### Seal `cssparser` and parse values via `FromCssStr`

`FromCss`, `ParseResult`, `CssToken`, `CssSyntaxKind`, and `CssExpectedMessage`
are now `pub(crate)`, keeping `cssparser` off the public API. Parse CSS value
types from strings through the new `FromCssStr` trait
(`Length::from_css_str("12px")`), which returns an owned `ParseError`
(`PartialEq`/`Eq`). The value-list types are plain aliases rather than newtypes:
`Filters` = `Vec<Filter>`, `GridTemplateComponents` = `Vec<GridTemplateComponent>`,
and `BackgroundImages`/`BackgroundSizes`/`BackgroundRepeats`/`PositionValues` =
`Box<[_]>`.

### Remove taffy/parley/image types from the public API

`Affine::transform_point`, `ComputedStyle::local_transform`/`has_non_identity_transform`,
`OffsetAnchor::resolve`, `PositionValue::to_point`, and `BorderRadiusPair::to_px` now take
separate `width`/`height` (or `x`/`y`) `f32` params and return tuples instead of `taffy::Point`/
`taffy::Size`. `Affine::decompose_translation` is removed; read `.x`/`.y` directly.
`SizingContext::container_size` is private; use the new `set_container_size` setter.
`ComputedStyle::to_taffy_style`/`creates_stacking_context`, `LineHeight::into_parley`,
`Float::resolve`/`Clear::resolve` are now crate-private. Dropped the `From<image::RgbaImage>`
impls for `ImageSource`/`ImageData`; convert through `ImageBuffer::from_rgba` instead.
`style::fast_div_255`/`fast_div_255_u32` moved under `style::math`. `NodePaint::container_size`
and `build_stacking_contexts` take `(Option<f32>, Option<f32>)` tuples instead of `taffy::Size`,
and the new `Error::InvalidLayoutNode` variant replaces the `From<taffy::TaffyError>` conversion.

### Hide the encoder and layout crates behind opaque `Error` variants

`Error::PngError`, `WebPEncodingError`, `GifEncodingError`, `ImageError`, and
`LayoutError` exposed the `png`, `image_webp`, `gif`, `image`, and `taffy`
crates. They collapse into `Error::Encode` (an opaque boxed source, built via
`Error::encode`) and `Error::Layout(String)`, so the public API no longer
tracks those crates' versions. `EmptyAnimationFrames` and
`MixedAnimationFrameDimensions` also drop their `format` fields.

### Stop exposing the parsed SVG tree as a public field

`SvgSource::tree` was a public `resvg::usvg::Tree` field, leaking `usvg` into
the API. It is now `pub(crate)`, with a `dimensions()` accessor for the canvas
size that callers actually need.

## takumi-core@0.1.0-rc.4

### Merge `takumi-css` into `takumi-core`

Fold the CSS parsing, cascade, value types, selector matching, and `@keyframes`
layer back into `takumi-core` and drop the separate `takumi-css` crate. The
`style` module is reachable through `layout::style` as before; `matching` is now
crate-private. Core builds at `opt-level = "z"`, shrinking the napi `.node` ~7%
and the wasm binary ~8% with no render-path regression.

### Remove taffy/parley/image types from the public API

`Affine::transform_point`, `ComputedStyle::local_transform`/`has_non_identity_transform`,
`OffsetAnchor::resolve`, `PositionValue::to_point`, and `BorderRadiusPair::to_px` now take
separate `width`/`height` (or `x`/`y`) `f32` params and return tuples instead of `taffy::Point`/
`taffy::Size`. `Affine::decompose_translation` is removed; read `.x`/`.y` directly.
`SizingContext::container_size` is private; use the new `set_container_size` setter.
`ComputedStyle::to_taffy_style`/`creates_stacking_context`, `LineHeight::into_parley`,
`Float::resolve`/`Clear::resolve` are now crate-private. Dropped the `From<image::RgbaImage>`
impls for `ImageSource`/`ImageData`; convert through `ImageBuffer::from_rgba` instead.
`style::fast_div_255`/`fast_div_255_u32` moved under `style::math`.

### Mark spec-tracking CSS value enums `#[non_exhaustive]`

`BlendMode`, `Filter`, `BasicShape`, `ContentValue`, `TextTransform`,
`WhiteSpaceCollapse`, `OffsetPath`, `ImageScalingAlgorithm`, and `Position` can
gain variants as their specs grow without a breaking change.

### Hide internal style/layout items from the public API

Roughly 200 CSS value types, parsing helpers, and internal accessors across `style::properties`,
`style::stylesheets`, `style::tw`, `style::selector`, and `layout` are now crate-private; they were
never meant to be constructed or matched on directly. `StyleSheet::property_rules` and
`apply_stylesheet_animations` are crate-private; use `StyleSheet`'s public parsing/query surface
instead. Gradient direction/keyword helpers (`GradientKeywordDirection`, `HorizontalKeyword`,
`VerticalKeyword`) stay public since they're reachable through `LinearGradientDirection`.

### Seal `parley` out of the font resource API

`FontResource::override_info` now takes a takumi-owned `FontOverride` (owned
family name, weight, style, width, axes) instead of `parley`'s
`FontInfoOverride`. `FontSource` is an opaque struct over raw bytes rather than
an enum exposing a `parley` blob. Callers no longer depend on `parley`.

### Rename `resource` naming to `image`

`Node::resource_urls`/`Style::resource_urls`/`StyleDeclarationBlock::resource_urls` are now
`image_urls`, and `ImageResourceError` is now `ImageError` — everything they cover is image
loading, so the names say so.

### Hide the encoder and layout crates behind opaque `Error` variants

`Error::PngError`, `WebPEncodingError`, `GifEncodingError`, `ImageError`, and
`LayoutError` exposed the `png`, `image_webp`, `gif`, `image`, and `taffy`
crates. They collapse into `Error::Encode` (an opaque boxed source, built via
`Error::encode`) and `Error::Layout(String)`, so the public API no longer
tracks those crates' versions. `EmptyAnimationFrames` and
`MixedAnimationFrameDimensions` also drop their `format` fields.

### Seal `cssparser` and parse values via `FromCssStr`

`FromCss`, `ParseResult`, `CssToken`, `CssSyntaxKind`, and `CssExpectedMessage`
are now `pub(crate)`, keeping `cssparser` off the public API. Parse CSS value
types from strings through the new `FromCssStr` trait
(`Length::from_css_str("12px")`), which returns an owned `ParseError`
(`PartialEq`/`Eq`). The value-list types are plain aliases rather than newtypes:
`Filters` = `Vec<Filter>`, `GridTemplateComponents` = `Vec<GridTemplateComponent>`,
and `BackgroundImages`/`BackgroundSizes`/`BackgroundRepeats`/`PositionValues` =
`Box<[_]>`.

## takumi-core@0.1.0-rc.1

### Make subset-group font selection deterministic

Subsets registered under one logical family (via `FontResource::subset_of`) were kept
in registration order. Callers commonly register fonts concurrently, so that order — and
therefore which subset won for a codepoint covered by more than one (e.g. overlapping
weight subsets, where the loser is faux-bolded) — varied per process. Identical input
could render to different bytes run to run.

Subsets are now held in a `BTreeSet`, ordered by their family name, so expansion and
selection no longer depend on registration timing. Same input renders identically.

## takumi-core@0.1.0-rc.0

### Drop `serde_bytes::ByteBuf` from `ImageSourceInput::Buffer`

The `Buffer` variant exposed `serde_bytes::ByteBuf` in the public API. It now
holds a `Vec<u8>` with `#[serde(with = "serde_bytes")]`, keeping the FFI
bytes wire format while keeping `serde_bytes` out of the surface.

### Own `GenericFamily` so callers don't depend on `parley`

`FontResource::generic_family` took a `parley::GenericFamily`, forcing callers
to add `parley` as a dependency. It now takes a takumi-owned `GenericFamily`
newtype exposing the families as named constants (`GenericFamily::SANS_SERIF`,
etc.), re-exported from the prelude.

### Mark the core node and image enums `#[non_exhaustive]`

`NodeKind`, `ImageSource`, `ImageSourceInput`, and `ImageCacheMode` can now gain
variants without a breaking change, so the surface stays stable across 1.0.

### Stop exposing the parsed SVG tree as a public field

`SvgSource::tree` was a public `resvg::usvg::Tree` field, leaking `usvg` into
the API. It is now `pub(crate)`, with a `dimensions()` accessor for the canvas
size that callers actually need.

## takumi-core@0.1.0-beta.6

### Support `text-underline-offset`

Add the `text-underline-offset` property, accepting `auto` or a `<length-percentage>` that shifts the underline away from the text. Percentages resolve against `1em`. Applies to the raster and SVG backends.

### Support `font-variant` properties

Add `font-variant` and its `font-variant-ligatures`, `font-variant-numeric`, `font-variant-east-asian`, `font-variant-caps`, and `font-variant-position` longhands. Each maps to OpenType features and resolves before `font-feature-settings`, which still wins on a tag conflict. `font-variant-alternates` and `font-variant-emoji` are out of scope, and missing features are not synthesized.

### Support `background-origin`

Add the `background-origin` property (`border-box`, `padding-box`, `content-box`), which sets the area that `background-position` and `background-size` resolve against. The `background` shorthand reads `<box>` values: the first sets origin and clip, a second overrides clip.

The initial value is `padding-box`, matching CSS, so backgrounds on bordered boxes now position against the padding box instead of the border box.

## takumi-core@0.1.0-beta.3

### Render text language-aware with the `lang` attribute

The `lang` attribute sets a node's language and inherits to its descendants; a
nested `lang` overrides, and an empty or invalid value clears it. The language
reaches the shaper as the locale, so the same code point draws its
language-specific glyph (Han unification across `ja` / `zh` / `ko`) and lines
break per language. Pass `lang` in the render options to set a default for the
whole tree.

## takumi-core@0.1.0-beta.2

### Expand subset font-family without borrowing the font context

A render holds the font context borrowed while it builds text layout, so per-element
`font-family` skipped subset-group expansion and routing relied on the fallback bucket, which
followed `fontique`'s hash iteration order. Content registered with `subset_of` (Rust) or
`subsetOf` (JS) could route to the wrong subset, and the result changed between renders.
Expansion now reads the subset groups without that borrow, and the fallback bucket follows
registration order.

## takumi-core@0.1.0-beta.1

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

## takumi-core@0.1.0-beta.0

### Split `takumi` into `takumi-core`, `takumi-raster`, and `takumi-svg` behind a re-export facade

### Minimize the public API

`takumi::prelude` exposes the stable data structures, entry-point functions sit at the crate root, the full backend crates move behind an `unstable` feature, and backend internals drop to `pub(crate)`.

### Rename the `raster` feature to `raster-backend`

This mirrors `svg-backend`, and `rayon` no longer enables it implicitly.
