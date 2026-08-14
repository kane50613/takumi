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
