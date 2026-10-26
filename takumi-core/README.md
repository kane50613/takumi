# takumi-core

<!-- cargo-rdme start -->

The layout engine shared by every takumi output. It holds the node tree, the CSS cascade,
layout, text shaping, and font and image resources. Its block, inline, flexbox, grid, and
table layout port Chromium's Blink.

Each backend builds on these types: [`takumi-raster`](https://docs.rs/takumi-raster) for
images, [`takumi-svg`](https://docs.rs/takumi-svg) for SVG, and the PDF and paint-tree
builds behind the `takumi-pdf` and `takumi-paint` npm packages.

Depend on the [`takumi`](https://docs.rs/takumi) crate instead. Its prelude re-exports the
stable subset of these types.

<!-- cargo-rdme end -->
