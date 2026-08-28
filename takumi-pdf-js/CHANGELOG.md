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
