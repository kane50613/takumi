---
packages:
  "cargo:takumi-core": minor
---

### Encapsulate glyph pixmap and color-layer paths behind accessors

`ResolvedBitmapGlyph::pixmap` and `ResolvedColorLayer::paths` were public fields
exposing `tiny_skia` types. They are now `pub(crate)` with `pixmap()`/`paths()`
accessors.
