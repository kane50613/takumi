---
packages:
  takumi-core: minor
  takumi-raster: patch
  takumi-svg: patch
---

# takumi-core no longer depends on tiny-skia or tiny-skia-path

`takumi-core`, `takumi-raster`, and `takumi-svg` no longer pull in tiny-skia or tiny-skia-path. Paths are now `takumi_core::path::Path`, and the conversions into tiny-skia types are gone:

- `PathCommand::build_path` returns a `takumi_core::path::Path`, and `SvgFilters::region` returns a `takumi_core::geometry::Rect<f32>`.
- The colour glyph gradients in `ColorPaint` carry their `transform` as an `Affine`.
- The conversions between `Affine` and `tiny_skia_path::Transform` are gone.
- `TryFrom<&ImageBuffer>` for `tiny_skia::Pixmap` and `tiny_skia::PixmapRef` are gone.
- `TryFrom<BlendMode>` for `tiny_skia::BlendMode` and `From<ImageScalingAlgorithm>` for `tiny_skia::FilterQuality` are gone.

#### Migration

Build the tiny-skia values you still need yourself. A transform converts field by field.

```diff
- let transform = tiny_skia_path::Transform::from(affine);
+ let transform = tiny_skia_path::Transform::from_row(affine.a, affine.b, affine.c, affine.d, affine.x, affine.y);
- let affine = Affine::from(transform);
+ let affine = Affine { a: transform.sx, b: transform.ky, c: transform.kx, d: transform.sy, x: transform.tx, y: transform.ty };
```

A pixmap reads the buffer's premultiplied RGBA bytes. An owned `Pixmap` takes a copy of them.

```diff
- let pixmap = tiny_skia::PixmapRef::try_from(&buffer).ok();
+ let pixmap = tiny_skia::PixmapRef::from_bytes(buffer.data(), buffer.width(), buffer.height());
- let owned = tiny_skia::Pixmap::try_from(&buffer).ok();
+ let owned = tiny_skia::IntSize::from_wh(buffer.width(), buffer.height())
+   .and_then(|size| tiny_skia::Pixmap::from_vec(buffer.data().to_vec(), size));
```

`BlendMode` and `ImageScalingAlgorithm` have no tiny-skia counterpart, so map the variants you use yourself. `PlusLighter` maps to `Plus`, and `Pixelated` maps to `Nearest`.

```diff
- let mode = tiny_skia::BlendMode::try_from(blend_mode).ok();
+ let mode = match blend_mode {
+   BlendMode::Normal => Some(tiny_skia::BlendMode::SourceOver),
+   BlendMode::PlusLighter => Some(tiny_skia::BlendMode::Plus),
+   _ => None,
+ };
- let quality = tiny_skia::FilterQuality::from(algorithm);
+ let quality = match algorithm {
+   ImageScalingAlgorithm::Pixelated => tiny_skia::FilterQuality::Nearest,
+   _ => tiny_skia::FilterQuality::Bilinear,
+ };
```
