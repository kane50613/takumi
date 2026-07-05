---
packages:
  cargo:takumi-core:
    replay:
      - exit-prerelease(cargo:takumi-core)
---

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
