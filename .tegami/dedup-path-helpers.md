---
packages:
  "cargo:takumi-core": minor
---

### Share one path-builder helper between layout and raster

`takumi-core` and `takumi-raster` each kept a private single-impl trait wrapping `Vec<PathCommand>` pushes, plus two more one-method traits that existed only for method-call syntax. The push helpers now live once as the public `takumi_core::geometry::PathBuilder` trait; the private traits are gone. Rendered output is unchanged.
