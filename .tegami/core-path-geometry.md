---
packages:
  takumi-core: minor
  takumi-raster: patch
  takumi-pdf: patch
---

# takumi-core no longer depends on tiny-skia-path

`takumi-core` builds paths with its own port of Skia's `SkPath` and `SkPathBuilder` in the new `takumi_core::path` module, so `tiny-skia-path` leaves the dependency tree along with the functions that returned its types.

- `PathCommand::build_path` returns a `takumi_core::path::Path`, and `SvgFilters::region` returns a `takumi_core::geometry::Rect<f32>`.
- The colour glyph gradients in `ColorPaint` carry their `transform` as an `Affine`.
- The conversions between `Affine` and `tiny_skia_path::Transform` are gone.

#### Migration

```diff
- let transform = tiny_skia_path::Transform::from(affine);
+ let transform = tiny_skia_path::Transform::from_row(affine.a, affine.b, affine.c, affine.d, affine.x, affine.y);
- let affine = Affine::from(transform);
+ let affine = Affine { a: transform.sx, b: transform.ky, c: transform.kx, d: transform.sy, x: transform.tx, y: transform.ty };
```
