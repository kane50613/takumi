# takumi-raster

<!-- cargo-rdme start -->

Raster painting backend for takumi, built on tiny-skia: canvas, drawing,
filters, and the [`render`] entry point.

Use it through the `takumi` umbrella. The render functions are re-exported at
the umbrella's crate root, and the crate itself is `takumi::unstable::raster`.
Core types are not re-exported here; reach them through `takumi_core` or
`takumi::unstable::base`.

<!-- cargo-rdme end -->
