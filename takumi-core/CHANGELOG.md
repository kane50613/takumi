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
