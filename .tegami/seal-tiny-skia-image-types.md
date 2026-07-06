---
packages:
  cargo:takumi-core:
    type: minor
---

### Seal `tiny_skia` and `image` types out of the public API

Glyph outline paths now use core-owned `geometry::PathCommand`/`geometry::Point`
instead of `tiny_skia::PathSegment`/`Point`. `ResolvedBitmapGlyph::pixmap`
(`tiny_skia::Pixmap`) is now `image: ImageBuffer`. `ImageBuffer::from_rgba`
(`Cow<RgbaImage>`) is now `from_rgba_bytes(Vec<u8>, width, height)`.
`layout::border::BorderProperties`'s path-building methods take
`Vec<geometry::PathCommand>` instead of `Vec<tiny_skia::PathSegment>`.
