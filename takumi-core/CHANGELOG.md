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
