---
packages:
  "takumi": minor
---

# Size an SVG from its root element without the renderer

A new `svg-size` feature in `takumi-core` reads an SVG's `width`, `height` and `viewBox` with roxmltree and svgtypes, following the same rules usvg applies, so a build that only lays out SVG images can leave the vendored renderer out. `svg` implies it. `takumi-bindings-common` now forwards `svg` as a default feature instead of forcing it.
