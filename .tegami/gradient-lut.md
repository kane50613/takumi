---
packages:
  takumi-core:
    type: minor
---

### Build gradient tables through ColorLut

Gradient tiles carry one `lut: ColorLut` instead of `color_lut` and its dithering companions. `resolve_stops_along_axis` is `ResolvedGradientStop::resolve`, `build_color_lut_with_interpolation` is `ColorLut::new`, and the overlay fast path is `GradientOverlayTile::overlay_unconstrained`.
