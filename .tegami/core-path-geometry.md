---
packages:
  takumi-core: minor
  takumi-raster: patch
  takumi-svg: patch
---

# takumi-core no longer depends on tiny-skia or tiny-skia-path

`takumi-core`, `takumi-raster`, and `takumi-svg` no longer pull in tiny-skia or tiny-skia-path. `PathCommand::build_path` returns a `takumi_core::path::Path`, `SvgFilters::region` returns a `takumi_core::geometry::Rect<f32>`, and the colour glyph gradients in `ColorPaint` carry their `transform` as an `Affine`. The conversions between takumi types and tiny-skia's `Transform`, `Pixmap`, `PixmapRef`, `BlendMode`, and `FilterQuality` are gone.

#### Migration

Build the tiny-skia values you still need yourself. A transform converts field by field, and a pixmap reads the buffer's premultiplied RGBA bytes.

```diff
- let transform = tiny_skia_path::Transform::from(affine);
+ let transform = tiny_skia_path::Transform::from_row(affine.a, affine.b, affine.c, affine.d, affine.x, affine.y);
- let pixmap = tiny_skia::PixmapRef::try_from(&buffer).ok();
+ let pixmap = tiny_skia::PixmapRef::from_bytes(buffer.data(), buffer.width(), buffer.height());
```

`BlendMode` and `ImageScalingAlgorithm` have no tiny-skia counterpart, so match the variants you use. `PlusLighter` maps to `Plus`, and `Pixelated` maps to `Nearest`.
