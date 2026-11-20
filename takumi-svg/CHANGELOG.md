## takumi-svg@0.4.0

### `paint-order: stroke` draws `-webkit-text-stroke` under the text

On HTML text, `paint-order: stroke` or `stroke fill` now draws the stroke first and the fill on top, as Chrome does, so a thick outline no longer eats into the letters. Images, SVG output, PDFs, and the paint tree all follow it.

```css
h1 {
  -webkit-text-stroke: 8px black;
  paint-order: stroke;
}
```

- The fill and the stroke each paint over a whole run, as in Chrome. With `normal`, a letter's fill no longer covers the previous letter's stroke where they overlap.
- `PaintOrder::text_phases` returns the order, and `InlineBrush` carries the run's `paint_order`.

### A positioned or stacking inline element paints as a layer of its own

A `display: inline` element now paints as a layer of its own, as Chrome paints it, when any of these hold. At `opacity: 0` it paints nothing at all, whatever else holds.

- `position` is not `static`
- `opacity` is below 1
- `filter` or `backdrop-filter` is not `none`
- `mix-blend-mode` is not `normal`
- `isolation` is `isolate`
- `clip-path` or `mask` is not `none`

Its backgrounds, its text, and the boxes inside it, floats and inline-blocks included, composite together after the content around it. Before, `opacity` faded each line and each text run on its own, and the other properties did nothing.

```html
<!-- Before: each line and each text run fades on its own -->
<!-- After: the whole element fades as one layer -->
<span style="opacity: 0.5; background: gold">A span that wraps onto a second line</span>
```

- The layer's box is the union of the element's line boxes. This approximates Chrome, where each line's box stays apart.
- A text node with a style of its own now opens an inline box as a span does.
- In the paint tree, such an element is a box with `effects`, and its line boxes and its own text node are its children.
- `NodePaint::inline_layer`, `InlineLayer`, and `OwnContent::InlineLayer` are new.

### Migration

`InlineBrush` no longer has an `opacity` field, since an inline element's `opacity` no longer applies per text run. It applies once, to the layer of the `NodePaint` whose `inline_layer` is set. In the paint tree, that element's box holds the value in `effects.opacity`.

```diff
- let opacity = brush.opacity;
+ let opacity = match &node.kind {
+   NodeKind::Box { effects: Some(effects), .. } => effects.opacity,
+   _ => 1.0,
+ };
```

Here `node` is the element's `PaintNode` in the paint tree.

### One `Cache` holds what renders reuse, in one pool of bytes

`Cache` replaces `ResourceCache` in the Rust crates. Everything it keeps shares `max_bytes`, 128 MiB by default. Setting `images` or `glyphs` to `ImageCacheMode::None` keeps none of that kind.

```rust
let cache = CacheOptions::builder()
  .max_bytes(256 << 20)
  .images(ImageCacheMode::None)
  .build();

let options = RenderOptions::builder()
  .viewport(viewport)
  .node(node)
  .cache(cache.clone())
  .build();
```

`RenderOptions`, `SvgOptions`, `PdfOptions`, `MeasureOptions`, and `PaintTreeOptions` take it as `cache`. A render without one shares `Cache::shared()`. PDF renders now reuse decoded images, and SVG and paint renders cache `data:` images.

### Migration

`ResourceCache` and the `resource_cache` setter of `RenderOptions` and of the `takumi-pdf` options are removed. Pass the cache as `cache`.

```diff
- let cache = ResourceCache::new(256 << 20);
- let image = cache.get_or_decode(&bytes, ImageCacheMode::Auto)?;
- let sheet = cache.get_or_parse_stylesheet(sources);
+ let cache = CacheOptions::builder().max_bytes(256 << 20).build();
+ let image = cache.decode_image(&bytes, ImageCacheMode::Auto)?;
+ let sheet = cache.parse_stylesheet(sources);
```

### An `svg` element embedded in SVG output takes the host's color whenever it draws with it

In SVG output, an embedded `svg` element that draws with `currentColor` now carries the host's `color` on its root, so a viewer draws it as takumi does. Before, it got the color only when its markup spelled out `currentColor`.

### Migration

`SvgSource::source_with_current_color` is removed.

```diff
- let markup = source.source_with_current_color(color);
+ let markup = source.embedded_source(&host, viewport);
```

### An SVG image no longer takes the host's color, as in Chromium

An SVG drawn by `<img>`, `background-image`, or another image now draws `currentColor` in its own `color`, or black when it sets none. Only an `svg` element takes the `color` of the box it sits in, whether it is inline in HTML, a JSX `<svg>`, or a `Node` with the `svg` tag name.

```html
<div style="color: red">
  <!-- Red before and after -->
  <svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="currentColor" /></svg>
  <!-- Before: red. After: black -->
  <img src="circle.svg" />
</div>
```

### Migration

To keep an icon in the host's color, render it as an `svg` element, or give its `Node::image` the `svg` tag name.

```diff
- Node::image(source)
+ Node::image(source).with_tag_name("svg")
```

## takumi-svg@0.3.33

### Correct filter and backdrop clipping

Ancestor overflow clips and an element's `clip-path` and `mask-image` now apply after filtering, preserving source pixels for blur and drop shadows. Rounded mask images follow the border box, and raster backdrop filters respect opacity, ancestor clips, and nested effect isolation. In `takumi-core`, the paint property tree exposes separate effect stages and filter clip boundaries through `EffectNode::kind` and `ClipNode::kind`, and `PropertySink::pop_clip` receives the clip being removed.

## takumi-svg@0.3.32

### Draw SVG images in SVG output with the render's fonts

An `<img>` of an SVG is now written into SVG output as paths, so its `<text>` uses the fonts the render registered and matches the PNG output. It used to embed the original markup, which the viewer drew in its own fallback font. An SVG with a filter or an embedded bitmap is still embedded as before.

### Draw SVG backgrounds and masks in SVG output with the render's fonts

A `background-image` or `mask-image` of an SVG `url(...)` is now written into SVG output as paths, like an `<img>` of an SVG. Its `<text>` uses the fonts the render registered and matches the PNG output. A tile too large to rasterize is skipped, as in PNG output.

### Draw SVG images at the size they take up

An SVG in an `<img>`, a background, or an inline `<svg>` now lays out at the size it is drawn at, as in Chrome, instead of drawing at its own size and stretching. A `viewBox` keeps its `preserveAspectRatio` in a box of another shape, viewport units and percentages measure that box, and `object-fit: cover` crops the SVG instead of squeezing it. An SVG without a `viewBox` still stretches, as in Chrome.

### Isolate an SVG group with `isolation: isolate`

A `<g style="isolation: isolate">` inside an SVG image or inline `<svg>` now composites its content as one group, as in Chrome, so a descendant's `mix-blend-mode` blends only with what the group paints instead of with the page behind it.

### Draw an SVG's bitmaps and filtered groups in a PDF without rasterizing

takumi-pdf embeds the PNG, JPEG, and WebP images inside an SVG as PDF images, and draws an SVG group without its filter. A CSS color `filter` on an SVG `<img>` or `background-image` recolors the SVG instead of rasterizing it, and takumi-svg writes an SVG's bitmaps as `<image>` elements. `SvgOp::Image` and `SvgOp::PushFilter` replace `SvgOp::Raster`, `SvgSource::vector_ops` no longer takes a raster scale, and resvg's renderer moves behind the `svg-raster` feature, on by default.

## takumi-svg@0.3.27

### Stop clipping blurred shadows in SVG output

Blurred `box-shadow` and `text-shadow` no longer end in a hard-edged square. Each blur filter now covers the image plus the `3σ` the blur reaches, as Chrome does.

## takumi-svg@0.3.23

### Share computed styles between boxes

Boxes share the parts of their computed style they leave unchanged, as Chrome's do. An 11,000-row table takes about a quarter less memory.

`ComputedStyle` fields now sit in groups named after Blink's, each behind an `Rc`. `RenderContext::style` is an `Rc<ComputedStyle>`, and `RenderContextBuilder::style` still takes a `Box<ComputedStyle>`.

```rust
let width = style.box_data.width;
let color = style.inherited_data.color;

Rc::make_mut(&mut style.box_data).width = Size::default();
```

## takumi-svg@0.3.19

### Place an inset `box-shadow` in the padding box

An inset `box-shadow` was placed against the border box instead of the padding box. On a box with a border the shadow fell at the border's position, where the opaque border covered it. The shadow is now placed against the padding box and appears just inside the border.

## takumi-svg@0.3.12

### Stroke the span that asked for it

`-webkit-text-stroke` was read off the element holding the text, so a `span` setting it for itself came out unstroked, and a nested one turning it off still got the parent's outline. The stroke now travels with the text run, in every backend.

## takumi-svg@0.3.11

### Paint a background colour from one place

`PaintDevice` and `FillShape` let shared painting code drive a rasterizer, an SVG writer or a PDF writer.

### Paint a `border-area` background under the border

A `background-clip: border-area` fill painted over the border, or replaced its colour, depending on the backend. It now paints under it.

### Draw every border ring from the shared painter

The SVG backend draws its borders through the shared painter. A `double` border fills two rings instead of stroking two centerlines.

### Place replaced content from one place

`object-fit` and `object-position` place replaced content from one place. An `object-position` past 100% now clips to the content box.

### Emit a text decoration as one rect

A text decoration emits as one positioned `<rect>` instead of a `<rect>` inside a `<g transform>`.

### Skip the ink an underline runs through, in every backend

`text-decoration-skip-ink` breaks an underline where the glyph outlines cross it, in every backend. A gap inside a letter stays a gap.

### Shade a 3D border in every backend

`inset`, `outset`, `groove` and `ridge` borders now shade their sides in the SVG and PDF backends, as the raster backend already did.

### Paint the outline above the content

An `outline` painted under the box's own text and images, so a negative `outline-offset` disappeared behind them. CSS 2.1 Appendix E paints the outline last, and every backend now does.

### Paint an outline above the box's children

A box with a negative `outline-offset` drew its outline under its own children, so a ring dragged inside the box disappeared behind them. The outline now paints after everything the box contains.

## takumi-svg@0.3.6

### Fill side border corners in vector backends

With per-side border colors and rounded corners, the SVG and PDF backends left the corner arcs unpainted: side fills used straight-edged polygons that stop short of the curve. Sides now fill with the same contour-following polygons the raster backend uses.

## takumi-svg@0.3.2

### Replace `ResolvedGlyphPlacement` with `geometry::Placement`

`Placement` moves into `takumi_core::geometry` and takes over from `ResolvedGlyphPlacement`, which described the same four fields. `BuiltInlineLayout::resolved_glyphs` is now keyed to `Arc<ResolvedGlyph>`, so a glyph cache hit stops copying the outline commands.

## takumi-svg@0.3.0

### Unify decoded resources behind one budgeted cache

Decoded images had a byte budget, but each SVG kept up to 32 rasterized pixmaps outside it, and every render re-parsed its stylesheets from scratch. `ImageCache` is now `ResourceCache`: SVG sources, their rasterized pixmaps, and parsed stylesheets all weigh against the same budget as decoded images. The default budget drops from 64 MiB to 16 MiB and becomes configurable — `new Renderer({ cacheMaxBytes })` in the bindings, `ResourceCache::new(max_bytes)` in Rust, with `0` disabling caching. SVG rasters and parsed stylesheets now also survive across renders, so a server re-rendering the same template stops re-rasterizing and re-parsing per request. Rust callers: `RenderOptions.stylesheet` is now `Arc<StyleSheet>`; pass `sheet.into()`.

## takumi-svg@0.2.0

### Anchor element filter regions to the border box

An invisible rect keeps a filtered element's region from collapsing when nothing inside it paints, matching the raster backend for filter-driven overlays like `feTurbulence` grain.

### Support SVG filter references in `filter`

`filter` and `backdrop-filter` accept `url(data:image/svg+xml,...)` with inline `<filter>` markup, mixing freely with filter functions. The raster backend executes the graph through resvg; the SVG backend emits the markup verbatim.

### Deduplicate glyph outlines in SVG output

Glyph outlines land in `<defs>` once and repeat as `<use>` references, shrinking text-heavy documents by 30–75%.

## takumi-svg@0.1.6

### Decode bitmaps at draw size

Bitmaps loaded through `ImageCache` stay encoded until draw time, decode scaled to the box they are drawn into, and cache per content and target size. Adds `ImageSource::Encoded`; `get_or_decode` returns it instead of `Bitmap` for PNG/JPEG/WebP. `cache: "none"` keeps returning an eagerly decoded `Bitmap`.

## takumi-svg@0.1.4

### Fix `fontFamilies` order being ignored

`fontFamilies` only fed the fallback bucket, never the root style, so text
picked whichever registered font resolved first instead of the requested
order. `FontFamily`'s default is now empty instead of a generic `sans-serif`
token, so an empty root style falls through to the fallback bucket directly.

## takumi-svg@0.1.0

### Render backdrop-filter in the SVG backend

SVG has no native backdrop source, so the backdrop is the scene replayed up to
the element, run through an SVG `<filter>` chain, then clipped to the border
box and attenuated by the element's mask and clip-path. Adds
`ComputedStyle::has_shape_mask` and `Filter::is_drop_shadow`.

## takumi-svg@0.1.0-rc.3

### Render backdrop-filter in the SVG backend

SVG has no native backdrop source, so the backdrop is the scene replayed up to
the element, run through an SVG `<filter>` chain, then clipped to the border
box and attenuated by the element's mask and clip-path. Adds
`ComputedStyle::has_shape_mask` and `Filter::is_drop_shadow`.

## takumi-svg@0.1.0-beta.5

### Fix `path()` ignoring device-pixel-ratio

`offset-path: path()` and `clip-path: path()` used the authored SVG coordinates verbatim while every other basic shape crossed the dpr boundary through `to_px`. At `devicePixelRatio != 1` the path was mis-scaled by `1/dpr`. The coordinates are now scaled into device space like the other shapes.

## takumi-svg@0.1.0-beta.2

### Render text language-aware with the `lang` attribute

The `lang` attribute sets a node's language and inherits to its descendants; a
nested `lang` overrides, and an empty or invalid value clears it. The language
reaches the shaper as the locale, so the same code point draws its
language-specific glyph (Han unification across `ja` / `zh` / `ko`) and lines
break per language. Pass `lang` in the render options to set a default for the
whole tree.

## takumi-svg@0.1.0-beta.1

### Match browser fake-bold for synthesized weights

Synthesize bold only at the CSS bold threshold (weight >= 600), so a medium weight keeps its regular face instead of being faux-bolded. Scale the synthetic stroke with text size — `1/24` easing to `1/32` per Skia — instead of a constant fraction, so large text is no longer over-emboldened.

## takumi-svg@0.1.0-beta.0

### Stop resolving `currentColor` in SVG images against the host color
