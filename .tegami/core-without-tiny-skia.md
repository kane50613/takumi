---
packages:
  takumi-core: minor
  takumi-raster: patch
  takumi-svg: patch
---

# takumi-core no longer depends on tiny-skia

`takumi-core`, `takumi-raster`, and `takumi-svg` no longer pull in tiny-skia's rasterizer. The conversions into tiny-skia types are gone with it:

- `TryFrom<&ImageBuffer>` for `tiny_skia::Pixmap` and `tiny_skia::PixmapRef`
- `TryFrom<BlendMode>` for `tiny_skia::BlendMode`
- `From<ImageScalingAlgorithm>` for `tiny_skia::FilterQuality`

#### Migration

Build a tiny-skia pixmap from the buffer's premultiplied RGBA bytes. An owned `Pixmap` takes a copy of them.

```diff
- let pixmap = tiny_skia::PixmapRef::try_from(&buffer).ok();
+ let pixmap = tiny_skia::PixmapRef::from_bytes(buffer.data(), buffer.width(), buffer.height());
- let owned = tiny_skia::Pixmap::try_from(&buffer).ok();
+ let owned = tiny_skia::IntSize::from_wh(buffer.width(), buffer.height())
+   .and_then(|size| tiny_skia::Pixmap::from_vec(buffer.data().to_vec(), size));
```

`BlendMode` and `ImageScalingAlgorithm` have no tiny-skia replacement, so map the variants you use yourself. `PlusLighter` maps to `Plus`, and `Pixelated` maps to `Nearest`.

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
