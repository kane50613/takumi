---
packages:
  cargo:takumi-css: minor
---

### Move backend paint helpers into a dedicated `paint` module

Gradient LUT, tile-position, and transfer-table helpers no longer glob into the
`style` re-export (and thus `takumi::prelude`). Cross-crate helpers live in a
dedicated `takumi_css::paint` module that the raster and SVG backends depend on
directly; the rest are `pub(crate)`.
