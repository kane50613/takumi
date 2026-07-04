---
packages:
  cargo:takumi-css: minor
---

### Move gradient tiles to the `paint` module and seal `tiny_skia`

`LinearGradientTile`, `RadialGradientTile`, `ConicGradientTile`, their
row-state and fast-path companions, and the `GradientOverlayTile` trait moved
out of the prelude-globbed `style` surface into `paint`. Backends reach them
via `paint`, keeping `tiny_skia` off the public API.
