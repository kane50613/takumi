---
packages:
  "cargo:takumi-core": minor
---

### Share one path-builder helper between layout and raster

`takumi-core` and `takumi-raster` each kept a private single-impl trait wrapping `Vec<PathCommand>` pushes (`BorderPath`, `PathBuilder`), plus two more one-method traits that existed only for method-call syntax. The push sugar now lives once as `takumi_core::geometry::PathBuilder`; the raster-only ellipse helper becomes a free `push_ellipse`, SVG path strings parse through `parse_svg_path_segments` directly, and border rasterization is a free `paint_border` instead of a trait method. Rendered output is unchanged.
