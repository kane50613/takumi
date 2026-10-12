## takumi@2.15.0

### Blend `color-dodge` and `color-burn` the same on every platform

### Apply filters as Chrome does

- `blur()`, `backdrop-filter` and `text-shadow` blur with Chrome's Gaussian, at full resolution up to a 135px standard deviation.
- `drop-shadow()` reads its blur as the standard deviation, as CSS defines it. Image output used to halve it.
- A colored `drop-shadow()` fades out instead of painting an opaque halo.
- `drop-shadow()` rounds a fractional offset down, so `10.5px` lands where Chrome puts it.
- `grayscale()` and `sepia()` above 100% act as 100%.
- SVG keeps a fractional `hue-rotate()` angle.

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

### Support `background-blend-mode` in SVG output

Each background layer blends with the layers and color beneath it, and nothing behind the box takes part.

### Skip text in `:first-child` and sibling selectors

`:first-child`, `:last-child`, `:nth-child()`, `+` and `~` skip text nodes, as browsers do. `td:first-child` now matches in indented HTML.

### Draw outlines as Chrome does

- An inline element's outline draws `double`, `groove`, `ridge`, `inset` and `outset`.
- An inline outline wraps the element's border box on each line, nested elements included. `plain <b>bold</b> text` now gets one.
- A wrapped inline outline follows Chrome's shape, rounds by `border-radius`, and shades 3D styles per edge.
- A one-line inline outline paints like a box outline.
- Inline outlines are no longer 1px narrower than Chrome's.
- Dashed and dotted outlines start each edge on a dash, and translucent ones no longer darken at the corners.
- A box without `border-radius` keeps square outline corners.
- PDF draws an inline outline as one contour across lines, at the element's opacity.

### Rotate by quarter turns exactly

`rotate: 90deg`, `180deg` and `270deg` build an exact matrix, as Chrome does. A card with an upside-down image renders 36% faster. A few edge pixels change by one level.

### Paint borders on inline spans

A `display: inline` span paints its `border` on each line. A wrapped span leaves out the side at each break, as browsers do.

### Decode `data:` images once per renderer

A template that inlines its logo or background as a `data:` URI decodes it once, not on every render. Real cards with inlined images render up to 92% faster.

Rust callers pass their `ResourceCache` to `RenderOptions::builder().resource_cache(...)`. `ResourceCache` now implements `Clone`, and clones share one store.

### Start a stacking context for any transform and `position: fixed`

Any transform property, even `translate: 0 0`, and `position: fixed` start a stacking context, as in Chrome. The box paints above later in-flow boxes and keeps its `z-index: -1` children above its background.

### Skip unknown at-rules in `StyleSheet::parse`

An at-rule takumi does not implement, such as `@font-face` or `@page`, is dropped instead of failing the whole sheet.

### Apply `backdrop-filter` as Chrome does

- The filtered backdrop draws over the original, so `opacity()` no longer punches a hole.
- With both `clip-path` and `mask-image`, the backdrop shows only where both let it through.
- A blurred backdrop reads only what is inside the element, so content just outside no longer bleeds in.

### Escape text inside JSX `<svg>` elements

`fromJsx` escapes text inside `<svg>`, so it cannot inject markup. It throws on invalid XML names and on `style` values that would end their own declaration, such as `fill: "red;stroke:blue"`.

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

### Get what to draw from JSX, HTML, or a node tree

`paint()` lays out the input and returns a `PaintTree` instead of pixels. `nodes` lists each box, text run and image with its CSS resolved. `steps` lists what to draw, in paint order.

### Write smaller JPEG files

JPEG output uses optimized Huffman tables. Text and UI images come out about 29% smaller at the same quality, and photos about 4% smaller.

### Show `background-clip: text` through the text Chrome shows it through

- The background shows through the text of child blocks, inline-blocks, floats and positioned children, not just the box's own text.
- Nested `background-clip: text` spans each show their background, outer span first.
- The background shows through underlines, overlines and line-throughs too.
- It shows through text at `opacity: 0` or `scale(0)`, and skips the floats and positioned boxes Chrome skips.
- `text-shadow` paints on top of the clipped background, and a child's own background covers it.
- A clipped background renders about 30% faster and peaks 35% lower in memory.

### Register WOFF2 fonts faster

Registering a WOFF2 font takes about 25% less time, so a cold start with a large CJK font is ready sooner.

### Write smaller lossy WebP

Lossy WebP files are about a quarter smaller at the same quality, with libwebp's method 2. It encodes about a quarter slower, then 13% faster again on two threads.

### Build `takumi` with fewer Cargo features

- `ImageSource::Animated` needs `gif`, `png`, or `webp`. A GIF still reports its size without its decoder.
- `png` is on by default through `image-decoding`. Without it, PNG sources report their size but cannot draw, PNG bitmap glyphs are skipped, and `ImageBuffer::encode_png` is gone.
- `svg-sizing` reads an SVG's `width`, `height`, and `viewBox` without the renderer. `svg` includes it.
- The raster backend builds without `svg`. Only SVG sources need it.

### Fit text with `text-fit` as Chrome does

- Fixed `letter-spacing`, `word-spacing` and `line-height` keep their size when a line scales.
- Text strokes, shadows and decorations scale with the line.
- A line within 2px of its box stays unscaled, and so does one that spacing already fills.
- `grow` limits under 100% and `shrink` limits over 100% stop scaling.
- Indented, centered, right-aligned and right-to-left lines land where Chrome puts them, on the same baseline.
- Span offsets and backgrounds no longer scale twice.
- With `text-overflow: ellipsis`, a line that fits once scaled gets no ellipsis.

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

### Encode GIF and decode WebP animations faster

- Native builds pick GIF palettes in parallel. A 24-frame 1200x630 GIF encodes 2.5x faster, byte for byte the same.
- Native builds decode animated WebP with libwebp, about 2.5x faster. The Node.js binary is 180KB smaller.

### Mix absolute units in `calc()`

`calc()` now adds `cm`, `mm`, `in`, `pt`, `pc` and `q` together, so mixing them no longer fails to parse.

### Draw glyph outlines through `draw_outline`

`base::resources::glyph::draw_outline` replaces `ErasedPen`. It draws a skrifa `OutlineGlyph` through `&mut dyn OutlinePen`.

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

### Render faster

Real cards render up to:

- 69% faster with downscaled photos, and 3 times faster with photos cropped by `object-fit: cover`.
- 65% faster with translucent overlays.
- 50% faster with nested clips or large blurred backgrounds.
- 30% faster when text-heavy.
- 14-32% faster with big blurred backgrounds, which blur on several threads in native builds.
- 11% faster when the card ends translucent.

Also:

- Text is shaped and measured once per node.
- Rotated and scaled glyphs come from the glyph cache. Their edges may move by up to 1/8px, as they now snap the way Chrome's do.
- Shapes and glyphs outside the image are skipped.
- A stylesheet is indexed once when parsed, so passing a whole site's CSS no longer slows every render.

### Cascade custom properties as browsers do

- `--gap: 8px !important` marks the declaration important instead of substituting `8px !important`.
- An `@property` rule with a typed `syntax` and no `initial-value` is ignored. `syntax` no longer keeps its quotes.
- An author's own `--tw-` property inherits like any other.
- A `--tw-*` property registered with `@property` keeps its initial value, so Tailwind's compiled gradients paint.

### Read painted values from a node tree

- With takumi-core's `paint-tree` feature, `takumi_core::paint_tree::paint_tree` returns decorations, image placement and shaped text in paint order, in device pixels.
- `SceneRequest::build` replaces `build_stacking_contexts`. Set `paint_bounds: false` to skip paint bounds and the text shaping they need.
- `Fonts::face_family` names a shaped run's registered family.

### Apply `text-overflow` only to a box that clips

`text-overflow: ellipsis` needs `overflow` other than `visible`, as in Chrome. Add `overflow: hidden` where you relied on an ellipsis without it. `line-clamp` still adds its own.

### ⚠️ Reach custom properties through one type on `ComputedStyle`

`custom_properties` and `registered_custom_properties` are now one `custom_properties: CustomProperties` field. Read a value with `style.custom_properties.get(name)`, which returns `Option<&str>`. `..Default::default()` still works.

### Render in less memory

- An 11,000-row table renders in about 110MB less memory, since the layout tree is freed once layout finishes.
- Anonymous boxes share one layout style, and scenes give back unused list room, so large tables use less memory.
- A clip that covers the whole image, such as `overflow: hidden` on the root, no longer keeps a second mask.
- A full-width `blur-3xl` on a 1200×630 image peaks 38% lower.

### End clamped text with an ellipsis

`line-clamp` and `-webkit-line-clamp` end the last line with `…`, as Chrome does, so Tailwind's `line-clamp-*` looks as it does in a browser. `line-clamp: 2 none` cuts without one.

### Encode PNG faster and smaller

- A PNG of at most 256 colors, as most text cards are, is stored with a palette. Real cards come out about 37% smaller, with the same pixels.
- Large PNGs deflate in up to eight segments at once. A 1200×630 card encodes about 2.5 times as fast, and a 1200×630 photo in 6ms instead of 18ms.
- Every machine writes the same bytes.
- APNG uses the same path. A 24-frame 1200x630 animation encodes 5x faster and comes out slightly smaller.
- A shorter match search and a truer settings sample cut encoding time by 10% and 4% across real templates. Files stay within 0.2% of their size.

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

## takumi@2.14.0

### Clip overflow at the padding box

`overflow: hidden`, `overflow: clip` and `contain: paint` now clip at the padding edge, as CSS specifies. Content that spilled into a padded box's padding used to be cut at the content edge.

### Size boxes from their content or available space

`width` and `height` now accept `min-content`, `max-content`, `fit-content`, `fit-content(<length-percentage>)`, and `stretch`.

```css
.card {
  width: fit-content(20rem);
}
```

This fits the card to its content with a preferred limit of `20rem`. Its min-content size remains the lower bound.

`flex-basis` accepts the same values, plus `content` to size a flex item from its content regardless of its main size property.

Tailwind utilities map to the new values:

| Value         | Utilities                    |
| ------------- | ---------------------------- |
| `min-content` | `w-min`, `h-min`, `size-min` |
| `max-content` | `w-max`, `h-max`, `size-max` |
| `fit-content` | `w-fit`, `h-fit`, `size-fit` |
| `content`     | `basis-content`              |

`min-width`, `max-width`, `min-height`, and `max-height` do not accept the new keywords. Their existing length and percentage values still work.

### Text wraps at the width layout measured

A box with a fractional width wrapped its text against the pixel-snapped content box at paint, so a nearly full line pushed its last word onto a line the layout never reserved and it overlapped the block below.

### Upgrade taffy to 0.14

Layout picks up taffy's flexbox, grid and block fixes from 0.12 through 0.14.

### Paint text that overflows a zero-size box

Text inside a zero-size box now renders in SVG output like it does in raster output.

### Add balanced flex wrapping, flow-root, self-alignment, and containment

Spread flex items across balanced lines and set a minimum line count:

```css
.cards {
  display: flex;
  flex-wrap: balance;
  flex-line-count: 2;
}
```

`balance` also combines with `wrap` or `wrap-reverse`, in either order. It cannot combine with `nowrap`.

- `display: flow-root` makes the box a block formatting context root.
- `align-items`, `align-self`, `justify-items`, and `justify-self` accept `self-start` and `self-end` to align to the item's own start or end. `justify-content` and `align-content` reject them.
- `contain` accepts `none`, `content`, or a space-separated combination of `layout`, `style`, and `paint`, without duplicates. `content` means `layout style paint`.

`contain: paint` clips descendants to the padding box. Either `layout` or `paint` creates a stacking context and a containing block for absolute and fixed descendants.

Size containment is not implemented, so `size`, `inline-size`, and `strict` cause a parse error. `style` parses but has nothing to scope: Takumi has no author-facing counters, and list-item ordinals do not restart at the boundary.

Containment is a correctness feature here, not a performance hint. Takumi renders once, so there is no relayout to skip. Paint containment adds a stacking context and a clip layer per box.

### Take an anonymous box's border width down to its used value

An anonymous box skipped the used-value pass, so it reported the initial `medium` border width even though it has no border style to render it with.

### Place a measured inline box against its parent's content box

An inline box's transform omitted the parent's border and padding, so it was reported at the border-box origin while every other measured node carries an absolute transform.

### Align flex items to the first line of a wrapped item

A flex item that wraps onto several lines now offers the baseline of its first line to `align-items: baseline`, instead of its bottom edge.

### Reject repeated alignment overflow prefixes

Reject repeated `safe` and `unsafe` prefixes in CSS alignment values.

### Decode character references in `fromHtml` image sources

An `img` whose `src` holds `&amp;` now yields the decoded URL, matching `attributes.src`.

### Size a replaced element the way its source states it

An `<svg>` carrying only a `viewBox` states a ratio and no size, and as an inline box it took a size the ratio never stated. `box-sizing: border-box` counted an image's padding and border twice, so the picture came out taller than its box. A source stating neither a size nor a ratio had one synthesised from the default object size, so `width` alone moved `height` with it.

### Keep balanced lines aligned within the container

`text-wrap: balance` aligned lines against the narrowed balancing width, so RTL, `text-align: right`, and centered paragraphs drifted toward the left edge.

### Render canvases up to 64 megapixels

The canvas budget is now 64 megapixels, up from 16. A 7680 × 4320 banner or a 4096 × 16384 page fits.

Over the budget, `render` fails with `InvalidViewport`. The message now states the limit.

### Keep decoded photos in the resource cache

The default `cacheMaxBytes` is now 64 MiB, and one decoded image can use the whole budget. The cache used to split its budget into many small shards and never kept an image larger than one shard, so most photos were decoded again on every render.

### Size a table's remaining auto columns by their content

Auto columns next to a column with a declared width now share the free width in proportion to their max-content widths, as they already did when every column was auto.

### Encode photographic PNG output faster and smaller

PNG output now picks its encoder settings from the image. Photo backgrounds encode about twice as fast and around 30% smaller. Flat art keeps its bytes. Animated PNG picks its settings from the first frame.

### Stop anonymous boxes inheriting their parent's box model

Bare text inside a padded `display: flex` container measured against a content box narrowed by the parent's padding again, so a line that fit was laid out as two.

### Balance text beside floats against the container

`text-wrap: balance` searched for its width with the floats squeezed into that narrower width, so paragraphs beside a float ended up barely balanced.

### Resolve viewport units against the page area in paged output

`100vh` in paged content was `0` because the content column lays out at unbounded height; it now equals the page area height, as in print media.

## takumi@2.9.1

### Place an inset `box-shadow` in the padding box

An inset `box-shadow` was placed against the border box instead of the padding box. On a box with a border the shadow fell at the border's position, where the opaque border covered it. The shadow is now placed against the padding box and appears just inside the border.

## takumi@2.7.0

### Restore JPEG, WebP and GIF decoding

`takumi` disables `takumi-core`'s default features and never re-enables image decoding, so every build decoded PNG and ICO only. A WebP source failed with `The image format could not be determined`. `image-decoding` is now a `takumi` feature as well, on by default, and it splits into `jpeg`, `webp` and `gif` for a build that wants one format and not the others. The napi and wasm bindings turn it on too.

## takumi@2.5.2

### Export `FontSource` from the prelude

`FontResource::new` takes anything that converts into a `FontSource`, and naming that type is the only way to reach `FontSource::from_static` or `from_shared`. It was missing from the prelude, so registering an `include_bytes!` face meant going through the semver-exempt `unstable` module.

## takumi@2.3.0

### Accept raw RGBA pixels as an image source

An image node's `src` takes `{ width, height, data, premultiplied? }` and renders the pixels without decoding; the `Bitmap` JSX helper wraps it as an `<img>`.

## takumi@2.0.0

### Refactor `font_families` and `lang` option type

Now both option takes resolved value instead of raw strings.

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

### Fix `fontFamilies` order being ignored

`fontFamilies` only fed the fallback bucket, never the root style, so text
picked whichever registered font resolved first instead of the requested
order. `FontFamily`'s default is now empty instead of a generic `sans-serif`
token, so an empty root style falls through to the fallback bucket directly.

### Fix buffer pool bucket capacity invariant

Release now buckets a buffer by the floor power of two its capacity guarantees,
and `acquire_dirty` reserves before `set_len`. A pooled buffer can no longer be
lengthened past its allocation.

### Skip mask copies and per-pixel mask lookups in canvas fast paths

Borrow the combined constraint mask directly when it already matches the
canvas viewport instead of copying it per draw, and hoist mask lookups out of
the per-pixel blit loops. Output is unchanged.

### Fix gradient stop snapping panic for oversized stop lists

Gradients with more color stops than the LUT can hold no longer panic. The
sample-index clamp and normalization passes stay within LUT bounds.

### Reuse per-scene state across animation frames

Compute each scene's font snapshot once and share its image and stylesheet
handles across frames instead of re-snapshotting and deep-cloning the whole
option tree per frame. Frame output is unchanged.

### Seal `parley::Layout` out of the inline-layout boundary

`BuiltInlineLayout::{layout, custom_inline_boxes}` are now private; the
measure-only walk moves into `BuiltInlineLayout::measure_runs`, returning
core-owned `MeasuredInlineRun`/`MeasuredInlineBox` (run text borrows the
layout). `get_parent_font_metrics`, `resolve_inline_line_metrics`,
`resolve_inline_line_states`, and `scale_text_fit_x` are no longer public.

### Add `takumi-html` for parsing HTML into a node tree

New `takumi-html` crate parses HTML + Tailwind markup into a node tree with
`from_html(source, FromHtmlOptions)`, mirroring the JS `fromHtml`. The `tw`,
`style`, `class`, `id`, `dir`, and `lang` attributes map to node styling and
metadata; `FromHtmlOptions` sets the `StylePresets` table and a `max_depth`
nesting cap. The `takumi` umbrella re-exports it under the `from-html` feature
as `takumi::from_html`, plus `Node::from_html` via the `FromHtml` prelude
trait.

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

### Drop `background-blend-mode` from the `background` shorthand

The `background` shorthand parsed a blend-mode token and reset
`background-blend-mode`, unlike browsers, where the shorthand touches neither. It
now leaves `background-blend-mode` alone; set it through the longhand. The
`blend_mode` field is gone from the `Background` shorthand value.

### Rename the `svg` feature to `svg-source`

`svg` and `svg-backend` read as the same thing at a glance despite gating
opposite directions (image-source input vs. render output). The umbrella's
input-side feature is now `svg-source`; `svg-backend` is unchanged.

### Make `:lang()` actually match

`:lang()` parsed but never matched, like every other pseudo-class the engine treats as
stateful. It needs no live state, only the `lang` attribute inherited up the tree, which a
static render already has. It now matches BCP-47 basic filtering (`:lang(zh-Hant)`, comma-separated
ranges, `*`) against the nearest ancestor-or-self with a `lang` set — the standards-based way to
route different fonts to different languages on the same page, e.g. `:lang(ja) { font-family:
"Noto Sans JP" }` alongside `:lang(zh-Hant) { font-family: "Noto Sans TC" }`.

### Represent the `none`/`normal` initial values of `max-*` and gaps

`max-width` and `max-height` are now a `MaxSize` value whose initial is `None`
(unbounded), instead of borrowing `Length`'s `auto`. `column-gap`, `row-gap`, and
the `gap` shorthand are now a `Gap` value whose initial is `Normal`. Rendering is
unchanged — `none` resolves like the old unbounded default and `normal` computes
to `0` — but the values now round-trip through `to_css` as `none`/`normal`.

### Cap image decode dimensions and GIF frame volume

Decoders reject images beyond 8192x8192 (via `image::Limits` for PNG and JPEG,
dimension checks for WebP) and GIFs beyond a total-frame pixel budget, stopping
decode-bomb OOM.

## takumi@2.0.0-rc.16

### Make the embedded font a true last resort

Both bindings now embed one font: a Latin Geist subset with a 400 to 700
weight axis (Geist Mono and Manrope are gone). It no longer claims the
`sans-serif` generic family and always sorts after registered fonts in
fallback selection, so generic families and unstyled text resolve to the fonts
you load. The new `FontResource::last_resort` marks a font's families to sort
after every normal registration.

## takumi@2.0.0-rc.15

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

## takumi@2.0.0-rc.13

### Skip mask copies and per-pixel mask lookups in canvas fast paths

Borrow the combined constraint mask directly when it already matches the
canvas viewport instead of copying it per draw, and hoist mask lookups out of
the per-pixel blit loops. Output is unchanged.

### Fix gradient stop snapping panic for oversized stop lists

Gradients with more color stops than the LUT can hold no longer panic. The
sample-index clamp and normalization passes stay within LUT bounds.

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

## takumi@2.0.0-rc.11

### Refactor `font_families` and `lang` option type

Now both option takes resolved value instead of raw strings.

## takumi@2.0.0-rc.10

### Drop `background-blend-mode` from the `background` shorthand

The `background` shorthand parsed a blend-mode token and reset
`background-blend-mode`, unlike browsers, where the shorthand touches neither. It
now leaves `background-blend-mode` alone; set it through the longhand. The
`blend_mode` field is gone from the `Background` shorthand value.

### Represent the `none`/`normal` initial values of `max-*` and gaps

`max-width` and `max-height` are now a `MaxSize` value whose initial is `None`
(unbounded), instead of borrowing `Length`'s `auto`. `column-gap`, `row-gap`, and
the `gap` shorthand are now a `Gap` value whose initial is `Normal`. Rendering is
unchanged — `none` resolves like the old unbounded default and `normal` computes
to `0` — but the values now round-trip through `to_css` as `none`/`normal`.

### Seal `parley::Layout` out of the inline-layout boundary

`BuiltInlineLayout::{layout, custom_inline_boxes}` are now private; the
measure-only walk moves into `BuiltInlineLayout::measure_runs`, returning
core-owned `MeasuredInlineRun`/`MeasuredInlineBox` (run text borrows the
layout). `get_parent_font_metrics`, `resolve_inline_line_metrics`,
`resolve_inline_line_states`, and `scale_text_fit_x` are no longer public.

## takumi@2.0.0-rc.9

### Bump internal crates

Releasing v0 internal crates

## takumi@2.0.0-rc.8

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

## takumi@2.0.0-rc.6

### Rename the `svg` feature to `svg-source`

`svg` and `svg-backend` read as the same thing at a glance despite gating
opposite directions (image-source input vs. render output). The umbrella's
input-side feature is now `svg-source`; `svg-backend` is unchanged.

## takumi@2.0.0-rc.2

### Add `takumi-html` for parsing HTML into a node tree

New `takumi-html` crate parses HTML + Tailwind markup into a node tree with
`from_html(source, FromHtmlOptions)`, mirroring the JS `fromHtml`. The `tw`,
`style`, `class`, `id`, `dir`, and `lang` attributes map to node styling and
metadata; `FromHtmlOptions` sets the `StylePresets` table and a `max_depth`
nesting cap. The `takumi` umbrella re-exports it under the `from-html` feature
as `takumi::from_html`, plus `Node::from_html` via the `FromHtml` prelude
trait.

## takumi@2.0.0-beta.11

### Add CSS `offset-path` support

The path accepts `ray()`, the basic shapes `path()`/`circle()`/`ellipse()`/`polygon()`/`inset()`,
and a bare `<coord-box>`

`offset-distance`, `offset-rotate`, `offset-anchor`, `offset-position`, and the `offset` shorthand control placement.

`url()` references are not supported.

## takumi@2.0.0-beta.4

### Render text language-aware with the `lang` attribute

The `lang` attribute sets a node's language and inherits to its descendants; a
nested `lang` overrides, and an empty or invalid value clears it. The language
reaches the shaper as the locale, so the same code point draws its
language-specific glyph (Han unification across `ja` / `zh` / `ko`) and lines
break per language. Pass `lang` in the render options to set a default for the
whole tree.

## takumi@2.0.0-beta.2

### Expand subset font-family without borrowing the font context

A render holds the font context borrowed while it builds text layout, so per-element
`font-family` skipped subset-group expansion and routing relied on the fallback bucket, which
followed `fontique`'s hash iteration order. Content registered with `subset_of` (Rust) or
`subsetOf` (JS) could route to the wrong subset, and the result changed between renders.
Expansion now reads the subset groups without that borrow, and the fallback bucket follows
registration order.

## takumi@2.0.0-beta.1

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

## takumi@2.0.0-beta.0

### Rename render entry points and return a `Bitmap`

`measure_layout` becomes `measure`, `render_sequence_animation` becomes `render_animation`, and `ImageOutputFormat` becomes `OutputFormat`. `render` returns a `Bitmap` newtype instead of `image::RgbaImage`, and `write_image` takes `&Bitmap`.

### Make fonts and images explicit per-render resources

Drop the persistent image store and `GlobalContext`, and pass fonts and images per render. `registerFont` replaces `loadFont`/`loadFontSync`/`loadFonts`, each render takes a `fontFamilies` fallback chain, and `images` replaces `fetchedResources`.

### Align CSS handling with the spec

Fix the `border-*-width`/`outline-width` defaults, negative `scale`, the `position: static` default, and `line-clamp` longhand splitting.

### Split `takumi` into `takumi-core`, `takumi-raster`, and `takumi-svg` behind a re-export facade

### Minimize the public API

`takumi::prelude` exposes the stable data structures, entry-point functions sit at the crate root, the full backend crates move behind an `unstable` feature, and backend internals drop to `pub(crate)`.

### Rename the `raster` feature to `raster-backend`

This mirrors `svg-backend`, and `rayon` no longer enables it implicitly.

### Stop resolving `currentColor` in SVG images against the host color

### Model image output quality per format

`ImageOutputFormat::Jpeg`/`WebP` carry a `Quality`, a new `WebPLossless` variant replaces lossless WebP (a `lossless` flag in the napi/wasm bindings), and `write_image` drops its quality argument.

# takumi

## 1.8.7

## 1.8.6

### Patch Changes

- cfac305: Align empty `inline-block`, `inline-flex`, and `inline-grid` boxes to the baseline by their bottom margin edge
- 653f23c: Stop painting `outline` around the text of non-inline elements
- c1dd195: Scale `vertical-align: sub`/`super` by font size instead of line height
- 6f55980: Fix line-box height for `vertical-align: top`/`bottom` boxes taller than the line height

## 1.8.5

### Patch Changes

- b998cfd: Fix Tailwind per-side border colors and border-width rendering
- 1389d75: Support `safe`/`unsafe` overflow keywords on `align-items` and `justify-content` (taffy 0.11)
- Updated dependencies [b998cfd]
- Updated dependencies [1389d75]
  - takumi-css@0.1.2

## 1.8.4

### Patch Changes

- 72ee5dd: Fix vendor-prefixed property names not being resolved

## 1.8.3

## 1.8.2

### Patch Changes

- 041e5fd: Encode PNG/APNG with the `zlib-rs` deflate backend at level 7 to speed up and smaller file size

## 1.8.1

### Patch Changes

- d1ff41f: Skip painting zero-sized nodes instead of compositing them through a full-viewport offscreen canvas, fixing a severe slowdown for zero-sized nodes with `opacity`.

## 1.8.0

### Patch Changes

- bcac11f: Fix discrete animations to switch at 50% progress and correct `alternate` iteration parity
- bcac11f: Fix `filter: none`, `@media not (...)` / `only`, `blur()`, `repeat()`, and `@supports` parsing
- bcac11f: Ignore non-string custom property values instead of storing an empty string
- bcac11f: Resolve `direction` (keyword or `var()`) before logical properties in the same block
- bcac11f: Allow constructing `Flex` directly
- bcac11f: Guard `i16` grid-line negation and non-positive device-pixel-ratio
- bcac11f: Reduce shipped WebAssembly binary size (`opt-level` and `wasm-opt -Oz`)
- bcac11f: Keep equal-priority Tailwind utilities in source order

## 1.7.0

### Minor Changes

- ece3e03: Add `position: static` and `position: fixed`
- 4748c22: Support Tailwind `mask-` utility

### Patch Changes

- ef7e816: Fix absolutely positioned children being mispositioned inside a `display: block` `position: relative` container that has in-flow siblings
- 9952b65: Accept inline image bytes (`Uint8Array`/`ArrayBuffer`) for image node `src`.
- b41405d: Fix `background-image`/`mask-image` SVGs with only a `viewBox` to scale to the box
- b9b4359: Fix `tw` arbitrary URLs (`mask-[url(...)]`, `bg-[url(...)]`) broken by `:` and `_` parsing

## 1.6.0

### Minor Changes

- 9d9b65e: Render `::before` / `::after` pseudo-elements #703

## 1.5.1

### Patch Changes

- 2321dbf: Fix `rem` units double-applying device-pixel ratio on descendant elements

## 1.5.0

### Minor Changes

- 9dc58e3: Add `margin-inline-start`, `margin-inline-end`, `padding-inline-start`, `padding-inline-end` CSS longhands, resolved to physical sides via `direction`
- 9dc58e3: Audit Tailwind utilities against v4 spec and fix the drift

## 1.4.1

### Patch Changes

- 72c19fd: Fix `text-fit: grow` with `background-clip: text` and `-webkit-text-stroke`

## 1.4.0

### Minor Changes

- a607651: Support `:is()` / `:where()` and stop dropping rules that contain unsupported pseudo-classes/elements
- 15fdabb: Support `lh` and `rlh` CSS length units

### Patch Changes

- dc04e09: Drop whitespace-only text nodes in block containers #711
- 26f1276: Fixes `var()` function detection #712
- dc31096: **Fix `object-position` is not inherited**

## 1.3.0

### Minor Changes

- 832ecd2: Add `ToCss` trait
- c4f705e: Support relative color syntax #693

### Patch Changes

- e073060: Resolve `rem` against the root element's computed font-size
- 7dbff69: Drop whitespace-only text nodes in block-like HTML containers #695

## 1.2.1

### Patch Changes

- 1fb35bb: Fix text-fit with text-align
- fa9abe6: Fix opacity not applied in inline layout
- fa7c55c: Fix line height resolves percentage to unitless

## 1.2.0

### Minor Changes

- 0f84a52: Support `text-fit` property

## 1.1.2

### Patch Changes

- 903f0ea: Drop libm dependency

## 1.1.1

### Patch Changes

- adc48da: Treat absolute/floated children as out-of-flow for inline layout detection

## 1.1.0

### Minor Changes

- 785d760: Support more border styles

## 1.0.16

### Patch Changes

- 092b4fd: Support inline float flow

## 1.0.15

### Patch Changes

- 3be6898: Reduce path rendering allocation

## 1.0.14

## 1.0.13

## 1.0.12

### Patch Changes

- 151a56e: Fix inline replaced sizing with border-box and line-height baseline alignment

## 1.0.11

### Patch Changes

- b755451: Fix Tailwind font size 5xl above line height

## 1.0.10

### Patch Changes

- b5e38f8: Fix Tailwind leading parsing #665
- 7d91b9c: Fix image drawing offset #664

## 1.0.9

### Patch Changes

- 32fa381: Preserve first line when max height is smaller than line height

## 1.0.8

## 1.0.7

### Patch Changes

- 6e9b163: Fix stack overflow when inline-block presented

## 1.0.6

### Patch Changes

- 2c90eaa: Improve vertical alignment for inline boxes

## 1.0.5

## 1.0.4

### Patch Changes

- 818e2f9: Fix image downscale quality regression

## 1.0.3

### Patch Changes

- be5b03f: Skip drawing if paint bounds out of canvas viewport

## 1.0.2

### Patch Changes

- 17304ac: Fix paint bound calculation on inline content #644
- 49ce893: Fix `plus-darker` blending #643

## 1.0.1

### Patch Changes

- 25dd037: Fix opacity compositing on sub canvas

## 1.0.0

### Major Changes

- 3b4f03d: **Removed `FetchTaskCollection`, switch to `Node::resource_urls` & `Style::resource_urls` instead.**
- 7da94c5: Remove `SpacePair::from_reversed_pair`
- 188079f: **Replaced `RenderOptionsBuilder` with `RenderOptions::builder()`**
  Switch to [typed-builder](https://docs.rs/typed-builder) for compile time options validation, no unwrap needed.
  Before:

  ```rust
  let options = RenderOptionsBuilder::default().build().unwrap();
  ```

  After:

  ```rust
  let options = RenderOptions::builder().build();
  ```

- 7f0b66b: **Removed `parse_svg_str`, use `SvgSource::from_str` instead.**
- cac231c: **Updated parameter type in `Viewport` constructor, removed `impl From<(u32, u32)>`**
- b0e13d8: **Private `ImageSource::size()`**
- 188079f: **Changed initial `display` value from `flex` to `inline`**

  This is to comply with [the CSSWG spec](https://drafts.csswg.org/css-display/#the-display-properties).

  You should update your code to use `display: flex` if you want to use flexbox.

- 4a114d5: Removed `detailed_css_error` feature
- 1373f0a: **Replace `TakumiError` with `takumi::error::Error`**
- 80535ba: Declare `border_style`, `border_color` as shorthand

### Minor Changes

- 7da94c5: Support `order`, `z-index` longhand, `flex-flow`, `place-items`, `place-content`, `place-self` shorthand
- 1ccf8a9: Support `direction`, `float`, `clear` properties
- 256ef21: Remove public `load_font` function
- 2b68b8a: Selects GIF frame based on `time_ms`
- 00013a8: Support repeating gradients
- b09ce0b: Support buffer input for image node `src` field
- 1373f0a: Support `ico` format
- b2e304a: Rework on internal rendering pipeline to be performant
- 14ac37b: Support `text-indent` property

### Patch Changes

- 7a79268: Fix linear gradient direction keywords handles incorrectly
- a118b5d: Add blending fast path, blur downscale scaling
- cd47ace: Add bilinear interpolation fast path
- 7a79268: Set default color interpolation method to Oklab
- 27e38bd: Fix `calc()` infinity scaler calculation
- ef692db: Remove `fast_image_resize` dependency
- b0e13d8: Fix DPR not applied when resolving image intrinsic size
- e1de442: Drop `fast_image_resize` with direct sampling approach
- 02c4000: Fix `background-image` layers drawing order
- dc6126d: Support `<calc-keyword>`
- 1aa4442: Optimize gradient performance
- 3d2eab2: Blockify node when `position: absolute` #572
- 80535ba: Support `border-top/right/bottom/left` shorthand properties

## 1.0.0-rc.17

## 1.0.0-rc.16

## 1.0.0-rc.15

### Minor Changes

- 2b68b8a: Selects GIF frame based on `time_ms`
- b09ce0b: Support buffer input for image node `src` field

## 1.0.0-rc.14

### Patch Changes

- cd47ace: Add bilinear interpolation fast path

## 1.0.0-rc.13

### Patch Changes

- a118b5d: Add blending fast path, blur downscale scaling

## 1.0.0-rc.12

### Patch Changes

- 1aa4442: Optimize gradient performance

## 1.0.0-rc.11

### Minor Changes

- 1ccf8a9: Support `direction`, `float`, `clear` properties
- b2e304a: Rework on internal rendering pipeline to be performant

## 1.0.0-rc.10

## 1.0.0-rc.9

### Major Changes

- 4a114d5: Removed `detailed_css_error` feature

### Patch Changes

- 7a79268: Fix linear gradient direction keywords handles incorrectly
- 7a79268: Set default color interpolation method to Oklab

## 1.0.0-rc.8

## 1.0.0-rc.7

## 1.0.0-rc.6

### Major Changes

- 7f0b66b: **Removed `parse_svg_str`, use `SvgSource::from_str` instead.**

## 1.0.0-rc.5

### Patch Changes

- ef692db: Remove `fast_image_resize` dependency

## 1.0.0-rc.4

## 1.0.0-rc.3

## 1.0.0-rc.2

## 1.0.0-rc.1

## 1.0.0-rc.0

### Minor Changes

- 14ac37b: Support `text-indent` property

## 1.0.0-beta.20

### Major Changes

- 3b4f03d: **Removed `FetchTaskCollection`, switch to `Node::resource_urls` & `Style::resource_urls` instead.**

## 1.0.0-beta.19

## 1.0.0-beta.18

## 1.0.0-beta.17

## 1.0.0-beta.16

## 1.0.0-beta.15

## 1.0.0-beta.14

## 1.0.0-beta.13

## 1.0.0-beta.12

## 1.0.0-beta.11

## 1.0.0-beta.10

## 1.0.0-beta.9

### Major Changes

- 7da94c5: Remove `SpacePair::from_reversed_pair`

### Minor Changes

- 7da94c5: Support `order`, `z-index` longhand, `flex-flow`, `place-items`, `place-content`, `place-self` shorthand

## 1.0.0-beta.8

### Patch Changes

- e1de442: Drop `fast_image_resize` with direct sampling approach

## 1.0.0-beta.7

### Major Changes

- cac231c: **Updated parameter type in `Viewport` constructor, removed `impl From<(u32, u32)>`**

## 1.0.0-beta.6

### Major Changes

- b0e13d8: **Private `ImageSource::size()`**

### Patch Changes

- b0e13d8: Fix DPR not applied when resolving image intrinsic size
- 02c4000: Fix `background-image` layers drawing order

## 1.0.0-beta.5

### Patch Changes

- 27e38bd: Fix `calc()` infnity scaler calculation

## 1.0.0-beta.4

### Patch Changes

- dc6126d: Support `<calc-keyword>`

## 1.0.0-beta.3

### Minor Changes

- 00013a8: Support repeating gradients

### Patch Changes

- 3d2eab2: Blockify node when `position: absolute` #572

## 1.0.0-beta.2

## 1.0.0-beta.1

### Minor Changes

- 256ef21: Remove public `load_font` function

## 1.0.0-beta.0

### Major Changes

- 188079f: **Replaced `RenderOptionsBuilder` with `RenderOptions::builder()`**

  Switch to [typed-builder](https://docs.rs/typed-builder) for compile time options validation, no unwrap needed.

  Before:

  ```rust
  let options = RenderOptionsBuilder::default().build().unwrap();
  ```

  After:

  ```rust
  let options = RenderOptions::builder().build();
  ```

- 188079f: **`GlobalContext` fields are now private**
- 188079f: **Changed initial `display` value from `flex` to `inline`**

  This is to comply with [the CSSWG spec](https://drafts.csswg.org/css-display/#the-display-properties).

  You should update your code to use `display: flex` if you want to use flexbox.

## 0.73.1

### Patch Changes

- 27dc8aa: Add `ImageSource::from_bytes` method, switch to `libwebp`
- 43c4ff8: Improved CSS error details

## 0.73.0

### Minor Changes

- be1a220: **Migrate to pure Node struct without generic support**

  Before:

  ```rust
  let mut node = NodeKind::Container(ContainerNode {
    children: Some(Box::from([
      NodeKind::Text(TextNode {
        text: "Hello, world!".to_string(),
        style: None,
        tw: None,
        preset: None,
        tag_name: None,
        class_name: None,
        id: None,
      }),
    ])),
    preset: None,
    style: None,
    tw: None,
    tag_name: None,
    class_name: None,
    id: Some("root".to_string()),
  });
  ```

  After:

  ```rust
  let node = Node::container([Node::text("Hello, world!")]).with_id("root");
  ```

### Patch Changes

- e6f3cf1: Fix negative offsets for oversized `background-position` #558

## 0.72.0

### Minor Changes

- c3e584d: **Remove `css_stylesheet_parsing` feature**
- 5cfa29f: bump MSRV to 1.91
- a7956fd: Remove `Y_FIRST` variable on `SpacePair` for simplicity

### Patch Changes

- 57163ec: Support `@layer`, `@property`, `@supports`
- aa85f72: Fix `gap` resolves incorrectly
- d6fde4c: Fix `font-family`, `font-variation-settings`, `font-feature-settings` parsing in stylesheets

## 0.71.7

### Patch Changes

- 384bc9d: Support Tailwind box/text shadow color
- 85970f3: Support CSS variables and `var()` function
- 9a5a542: Support `@media` CSS at-rule

## 0.71.6

### Patch Changes

- 1a111a6: Add `keyframes` render option

## 0.71.5

### Patch Changes

- 02d7e3a: Support `.ttc` fonts loading

## 0.71.4

### Patch Changes

- 1d284ac: Improve gradient hot paths

## 0.71.3

### Patch Changes

- a279b4c: Add `dithering` option for smoother gradients

## 0.71.2

### Patch Changes

- 5c9a2f6: Rasterize repeated background tiles before compositing

## 0.71.1

## 0.71.0

### Minor Changes

- 812029d: Support lossy webp animation rendering
- 0930cdb: Support CSS keyframe animation rendering and animation properties
- 812029d: Add `encode_animated_gif` method
- 0930cdb: **BREAKING CHANGE: Refactor public style API to declaration-based styles**

  Replace the old field-based / `CssValue`-driven style construction with `StyleDeclaration` and `style.with(...)`.

  Before:

  ```rust
  let style = StyleBuilder::default()
    .font_size(Some(48.0.into()))
    .margin(Sides([Px(4.0); 4]))
    .build()
    .unwrap();
  ```

  After:

  ```rust
  let style = Style::default()
    .with(StyleDeclaration::font_size(Px(48.0).into()))
    .with_margin(Sides([Px(4.0); 4]));
  ```

### Patch Changes

- 5ad65bf: Fix panic when nested inline nodes presented
- 1c9b8ac: Support background-size: auto
- 52f1dd5: Fix `background-position`, `mask-position` now default to `top left`

## 0.70.4

### Patch Changes

- 4895e79: Fix nested node's resource urls not being collected
- e200a64: Fix comma-separated list parsing for `background`, `box-shadow`, `text-shadow`
- c5465f7: Fix flex text node self-alignment

## 0.70.3

## 0.70.2

### Patch Changes

- edd121c: Use `libwebp-sys` crate for non-wasm targets webp encoding

## 0.70.1

### Patch Changes

- 36fba31: Strip `text` & `tspan` nodes from SVG

## 0.70.0

### Patch Changes

- 217b9b0: Refactor rendering to avoid stack overflow in deep nested nodes
- bf3b981: fix regression image node size measurement in absolute positioned container

## 0.69.5

### Patch Changes

- 9d9f43b: Support viewport & container length units

## 0.69.4

### Patch Changes

- 1058a19: Pass `currentColor` to svg
- 1dc8413: Support gradient color-space interpolation

## 0.69.3

### Patch Changes

- 8b9642e: Support `<length>` value in `vertical-align`
- 2dda025: Support `color-mix()` function

## 0.69.2

### Patch Changes

- 83a8198: Support `linear/radial/conic-gradient()` double-position color stops syntax

## 0.69.1

### Patch Changes

- a81238d: Fix intrinsic SVG sizing in absolute flex shrink-to-fit measurement

## 0.69.0

### Minor Changes

- 62525f9: Parses CSS stylesheets with new `css_stylesheet_parsing` feature

### Patch Changes

- 55ea202: fixes `plus-darker` blending regression #501

## 0.68.17

### Patch Changes

- f186186: improve gradient quality with dithering

## 0.68.16

### Patch Changes

- 757339e: remove png `try_collect_palette`, add compression control via `quality` field
- bd4d602: support gif decoding

## 0.68.15

### Patch Changes

- 2c6f5c3: optimize interpolation, blur performance

## 0.68.14

## 0.68.13

### Patch Changes

- 27429b0: improve blur performance

## 0.68.12

### Patch Changes

- d8d0fa8: parse Tailwind background image gradient

## 0.68.11

### Patch Changes

- f0120ae: refactor text ellipsis
- 60942e9: support Tailwind 4.2 new colors (Taupe, Mauve, Mist and Olive)
- bf54cac: implement `BufferPool` to reuse buffers

## 0.68.10

### Patch Changes

- f1e2c62: support `vertical-align` property
- 1e30ea7: support `text-decoration-thickness` property

## 0.68.9

### Patch Changes

- 2016a3c: fix nested inline-block caused infinite recursive calls

## 0.68.8

### Patch Changes

- b7d7570: fix box-shadow rendering
- 4b13519: support `overflow: clip`

## 0.68.7

### Patch Changes

- 9248c6e: support `inline-block`, `inline-flex`, `inline-grid` layout, closes #219
- 2639396: fix decoration rounding caused gap
- 91037be: fix style shorthand overrides

## 0.68.6

### Patch Changes

- 51a5bd5: support `text-decoration-skip-ink` property
- 642cf06: refactor `TextDecorationLines` to use `bitflags`, parses Tailwind `underline`, `overline`...
- 57d6594: fix text painting order for shadow, decoration and actual content to avoid overlapping
- 9215906: fix embolden and skew should avoid emojis
- 53069e4: refactor taffy tree structure, support `calc()`

## 0.68.5

### Patch Changes

- 8fcbb90: support `font-synthesis-weight` & `font-synthesis-style` properties for faux-bold & skew
- 07649be: fix text overline decoration should be drawn before text

## 0.68.4

### Patch Changes

- d6f1cce: use `libm` for text rendering
- 37864fd: support `outline` properties

## 0.68.3

### Patch Changes

- 585e1ba: support `background: conic-gradient()`
- 755b998: support `font-stretch` property
- 12ef8ce: loosen SVG check
- db327e6: fix overflow hidden constraint should include border radius mask
- 133c6a4: fix line height "normal" behavior
- 3191865: fix inheritance should store computed instead of initial value

## 0.68.2

## 0.68.1

### Patch Changes

- fb32121: parse `bg-none` for Tailwind `background-image` and improve isolation coverage

## 0.68.0

### Minor Changes

- 7684faa: refactor font loading to reduce buffer copying

## 0.67.3

### Patch Changes

- 0632470: unpremultiply SVG pixels after rasterization #446
- 1e9b23e: fix plus-lighter compositing #447

## 0.67.2

## 0.67.1

### Patch Changes

- 4d8955d: fix text wrap ignores original height & line clamp constraint #439

## 0.67.0

### Minor Changes

- 691df9d: support `mix-blend-mode` and `isolation` property

### Patch Changes

- 7e34727: support `visibility` property
- ba7aa93: fix prevent text balancer from forcing breaks #437

## 0.66.14

### Patch Changes

- 102c24a: include DPR when calculating `text-wrap: balance` #434

## 0.66.13

## 0.66.12

### Patch Changes

- 7389d6e: add font loading cache

## 0.66.11

### Patch Changes

- 8f28434: refactor inline box drawing & measure API

## 0.66.10

### Patch Changes

- e457042: fix line height should be resolved absolute value

## 0.66.9

### Patch Changes

- c917b6d: add `parse_tw_with_arbitrary` method
- 9929474: fix missing Tailwind `leading-*` keywords

## 0.66.8

### Patch Changes

- 28a8348: merge `draw_background_color` & `draw_background_image` implementation

## 0.66.7

## 0.66.6

### Patch Changes

- 2e7dbed: support `strokeLinejoin` property for text stroke

## 0.66.5

### Patch Changes

- c8244eb: fix text clip drawing order

## 0.66.4

### Patch Changes

- 5f2b5ac: add `WebkitTextFillColor` property
- 7eed4a1: support background clipping on text stroke

## 0.66.3

### Patch Changes

- 77c7107: fix `WebkitTextStroke` deserialize naming

## 0.66.2

### Patch Changes

- 058f87a: use `zune-core` & `zune-jpeg` dev branch version (not published to crates.io)

## 0.66.1

### Patch Changes

- 7e513fc: add `text-shadow-*`, `drop-shadow-*` tailwind properties parser

## 0.66.0

## 0.65.0

### Minor Changes

- 1319540: new `measure()` API

## 0.64.1

## 0.64.0

### Patch Changes

- 6571216: fix viewport check should include defined values

## 0.63.2

### Patch Changes

- 63088f4: make `background_color` field optional, draw background color on text spans #220

## 0.63.1

## 0.63.0

## 0.62.8

### Patch Changes

- b0a21a4: refactor opacity blending should be on render level

## 0.62.7

## 0.62.6

### Patch Changes

- a10f933: fix tailwind filter classes (blur, brightness, etc.) now append instead of replace

## 0.62.5

### Patch Changes

- dd1c0e1: fix tailwind `text-pretty` & `text-balance` not being parsed

## 0.62.4

## 0.62.3

## 0.62.2

### Patch Changes

- 57cca21: Improve backdrop filter performance
- 520f15d: Improve drop shadow performance and reduce allocation

## 0.62.1

### Patch Changes

- 5214274: refactor `overlay_image` to take any `GenericImageView` (avoid unnecessary `RgbaImage` recreation)

## 0.62.0

### Minor Changes

- 4675458: use `Box` slices instead of `Vec` to optimize memory

### Patch Changes

- 7849598: SIMD enhanced `interpolate_rgba`
- a774aa6: optimize filters to render using LUTs

## 0.61.1

### Patch Changes

- 19235dd: support AVX2 & AVX-512 SIMD blurring
- 8066f93: bump MSRV to 1.89

## 0.61.0

### Minor Changes

- c4bf981: enrich CSS error

  The error message is much more helpful now.

  > InvalidArg, invalid type: integer `123`, expected a value of 'currentColor' or \<color>; also accepts 'initial' or 'inherit'.

- 98e9254: support `backdrop-filter`

## 0.60.8

### Patch Changes

- 4c6bf92: fix text drawing bypasses overflow constrain check

## 0.60.7

### Patch Changes

- f07b7f5: switch to gaussian box blur, integer based alpha blending

## 0.60.6

### Patch Changes

- 7813b86: use bit masking for faster alpha quantiazation

## 0.60.5

### Patch Changes

- 12415ba: fix alpha blending precision

## 0.60.4

### Patch Changes

- 6f74c75: fix `try_collect_palette` collecting over 256 colors

## 0.60.3

### Patch Changes

- 5e1cb26: try collect png palette if possible

## 0.60.2

### Patch Changes

- 946fc9e: update ellipsis condition explicity check `text-overflow: ellipsis`

## 0.60.1

### Patch Changes

- 71ab744: Unify text node & inline logic

  Brings more unified and consistent ellipsis, transform, collapse, measurement behavior.

## 0.60.0

### Minor Changes

- ef3ec72: support `text-wrap: balance` & `pretty` (`text-wrap-style`)!

## 0.59.1

### Patch Changes

- c6b4eab: use stack blur algorithm
- 8f02159: add `sepia()` filter, tailwind `filter` parsers

## 0.59.0

### Minor Changes

- 13eca0e: rename `LengthUnit` to `Length` #347
- 4dee0c0: support `blur()`, `drop-shadow()` filter, premultiply alpha blending for shadows

## 0.58.0

### Minor Changes

- 0deafbd: decouple base Chromium styles (or customized from `defaultStylePresets`) from `style` field to independent `preset` field.

## 0.57.6

### Patch Changes

- 68e8fc2: fix inline style order should be greater than tailwind styles

## 0.57.5

### Patch Changes

- 9bf3333: disable font hinting, apply normalized coordinates to glyph scaler

## 0.57.4

### Patch Changes

- a8ebbba: remove redundant style property wrapper
- a8ebbba: fix `matrix()` function parsing
- a8ebbba: support `col`, `row` tailwind grid properties

## 0.57.3

### Patch Changes

- fa2f034: fix COLR layers blending

## 0.57.2

### Patch Changes

- 695f34a: fix passing opacity to COLR palette

## 0.57.1

### Patch Changes

- 61191b2: handles `background-size` for rasterized images
- 260dbd0: optimize `encode_animated_webp` to reduce allocation

## 0.57.0

### Minor Changes

- 42572bb: **Drop `avif` format support**

### Patch Changes

- 26173c5: add `create_background_image` fast path for exact one image

## 0.56.1

### Patch Changes

- f4d54fa: fix `opacity` should be applied to image as well
- 1972df9: fix `background-size` css parsing
- 1972df9: support `background`, `mask` shorthand

## 0.56.0

### Minor Changes

- 1ac44c4: `mask-image` behaves correctly like a "mask" now instead of overlay image.
- 1ac44c4: support `background-clip`

### Patch Changes

- c1260a2: `line-clamp` should has ellipsis if overflow

## 0.55.4

### Patch Changes

- 34bf0af: fix mask image on text drawing overflows

## 0.55.3

### Patch Changes

- cd93ee9: handles special case of `text-overflow: ellipsis` + `text-wrap: nowrap`

## 0.55.2

### Patch Changes

- 274c716: reuse masking buffer to avoid allocation

## 0.55.1

### Patch Changes

- 3df6648: use `RefCell` internally for scratch buffer

## 0.55.0

### Minor Changes

- 5e79e33: support COLR emoji font drawing (e.g. twemoji)

### Patch Changes

- 5e79e33: reuse buffer for masking to reduce allocation

## 0.54.3

## 0.54.2

### Patch Changes

- df1aa7e: update `parley` to `0.7`

## 0.54.1

### Patch Changes

- b16fd1b: fix whitespace keywords parsing

## 0.54.0

### Minor Changes

- e8ea400: refactor `TakumiError` struct and eliminate `unwrap()` calls

### Patch Changes

- e6a0934: Crate: fix justify-content css parse

## 0.53.1

### Patch Changes

- 29a575c: optimize `CssValue` deserialize implementation to reduce generated `Visitor` variant

## 0.53.0

### Minor Changes

- 7740504: drop `ts_rs` support
- 4623702: **`textStroke` related properties will have prefix `WebkitTextStroke`**

## 0.52.2

### Patch Changes

- 563bf31: optimize transform to reduce multiplications

## 0.52.1

### Patch Changes

- 3fa5c55: optimize tailwind parser function size

## 0.52.0

### Minor Changes

- ed409d4: refactor `overflow` & `clip-path` rendering to avoid extra allocations
- b9b0a85: speed up out of viewport image rendering

### Patch Changes

- ed409d4: make transform behave correctly

## 0.51.1

### Patch Changes

- eb26a60: fix `overflow`, `clip-path`, `background-position` deserialization

## 0.51.0

### Minor Changes

- 27ac6c5: support `devicePixelRatio` value

## 0.50.0

## 0.49.1

## 0.49.0

## 0.48.0

### Minor Changes

- c3f1b7d: support optional width/height

## 0.47.0

### Minor Changes

- 7d3dbf1: replace `csscolorparser` with `color` crate to support more color functions

## 0.46.6

## 0.46.5

## 0.46.4

### Patch Changes

- 37610e0: bump `csscolorparser` to 0.8

## 0.46.3

## 0.46.2

## 0.46.1

### Patch Changes

- 9365705: fix `justify-between`, `around`, `evenly` tailwind parsing

## 0.46.0

### Minor Changes

- 18bbc7c: support tailwind breakpoint & important parsing #273

## 0.45.3

### Patch Changes

- 3cf3867: fix `bg-size-[…]`, `bg-position-[…]` arbitrary value parsing

## 0.45.2

### Patch Changes

- d28e982: add `background-image` arbitrary value parsing
- 3c0243b: fix gradient step parsing
- 1ba2585: bump minimum rust version to 1.88
- 3c0243b: prevent panicing in font weight parsing

## 0.45.1

### Patch Changes

- 97ba495: fix `rounded` parsing

## 0.45.0

### Minor Changes

- 702c419: add tailwind parser
- 702c419: support `inline`/`block` for padding/margin/inset/border-width

## 0.44.0

### Minor Changes

- 368fc1c: Support `textWrap`, `textWrapMode`, `whiteSpace`, `whiteSpaceCollapse` properties

  **BREAKING CHANGE: by default text will collapse instead of preserve**, use `whiteSpace: pre;` to get the same behavior

## 0.43.1

## 0.43.0

## 0.42.0

### Minor Changes

- 44368b8: remove all Mutex/RwLock uses
- 44368b8: replace noise-v1 to use lighter hash function, only `opacity()` & `seed()` is supported

## 0.41.0

### Patch Changes

- 8318812: fix `PositionComponent` should be untagged

## 0.40.2

### Patch Changes

- 21a9988: add `word-break: break-word` as alias for `word-break: normal` + `overflow-wrap: anywhere`
- ddae1b5: fix `letter-spacing`, `word-spacing` should not divide by font size

## 0.40.1

### Patch Changes

- 8751a1b: fix fetch tasks collecting being overwritten

## 0.40.0

### Minor Changes

- ae7062f: support `clip-path`, `clip-rule`

### Patch Changes

- ae7062f: fix inline content not being clipped by overflow constraints

## 0.39.0

### Minor Changes

- 71ae4a5: use `data-url` crate, **remove `image_data_uri` feature**

### Patch Changes

- 71ae4a5: parallelize background image layers rendering

## 0.38.1

### Patch Changes

- 88a56ed: use faster noise crate `fastnoiselite`
- 88a56ed: use `crossbeam-channel`

## 0.38.0

### Minor Changes

- 7245e49: Add `FetchTask` for resources need to be fetch externally.

## 0.37.0

### Minor Changes

- 92f4dd8: support `opacity` property
- e6a1c39: refactor internal image/text measuring to match browser overflow behavior
- 0dfb76b: support overflow `hidden`, `visible`

## 0.36.2

### Patch Changes

- 568f76f: fix box shadow not being parsed

## 0.36.1

## 0.36.0

### Minor Changes

- 95715d0: support `filter` on images (except `blur()` and `drop-shadow()`)

## 0.35.2

### Patch Changes

- cac5444: remove glyph cache

## 0.35.1

## 0.35.0

### Minor Changes

- 264fa71: implement inline layout
- 264fa71: make all nodes' `style` field optional

### Patch Changes

- 12a2d3f: fix `aspect-ratio`, `flex-grow` numberic value parsing

## 0.34.0

### Minor Changes

- c06cdce: support `currentColor` keyword

### Patch Changes

- 7c402d8: setup npm trusted publisher

## 0.33.1

## 0.33.0

### Minor Changes

- 98755a7: **drop support for `debug` field, replace with `draw_debug_border` option in rendering functions**
- 5f15925: support `flex` shorthand property
- aa965bd: support `translate`, `rotate`, `scale` property
- 656be8d: support custom ellipsis character for `line-clamp`, `text-overflow`

### Patch Changes

- a9f3999: fix border width on image node that caused offset to be applied twice
