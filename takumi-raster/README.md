# takumi-raster

<!-- cargo-rdme start -->

Raster (tiny-skia) painting backend for takumi: canvas, drawing, filters, and
the [`render`] entry point. Used via the `takumi` umbrella (`takumi::raster`) or
directly.

Imports the `takumi-core` root privately so painting code resolves
`crate::layout`, `crate::resources`, `crate::Result`, etc. against the shared
core. Base types are _not_ re-exported from here; reach them through
`takumi::base` (or `takumi_core` directly).

<!-- cargo-rdme end -->
