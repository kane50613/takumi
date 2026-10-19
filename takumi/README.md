# Takumi

<!-- cargo-rdme start -->

Takumi lays out a node tree with CSS and renders it to an image, an animation, or SVG,
without a browser. Its text and inline layout port Chromium's Blink.

This crate is the stable entry point. The render functions live at the crate root, and the
types to build a tree and configure a render live in [`prelude`]. PDF output and the paint
tree ship as the npm packages [`takumi-pdf`](https://www.npmjs.com/package/takumi-pdf) and
[`takumi-paint`](https://www.npmjs.com/package/takumi-paint).

## Example

Build a node tree, register a font, and render a 1200 × 630 PNG:

```rust
use takumi::prelude::*;
use takumi::{render, write_image};

let node = Node::container([Node::text("Hello, world!").with_style(
  Style::default().with(StyleDeclaration::font_size(Length::Px(32.0).into())),
)]);

// Takumi loads no system fonts. Register fonts once and reuse `fonts` across renders.
let mut fonts = Fonts::default();
fonts.register(FontResource::new(include_bytes!(
  "../../assets/fonts/geist/Geist[wght].woff2"
)))?;

let options = RenderOptions::builder()
  .viewport(Viewport::new((1200, 630)))
  .node(node)
  .fonts(&fonts)
  .build();

let image = render(options)?;

let mut png = Vec::new();
write_image(&image, &mut png, OutputFormat::Png)?;
```

## Entry points

- [`render`] draws one frame. [`write_image`] encodes it as PNG, JPEG, WebP, or ICO.
- [`render_animation`] renders every frame of a scene timeline at a fixed frame rate.
  [`write_animation`] streams the frames into animated WebP, APNG, or GIF instead.
- [`measure`] lays out the tree and returns box sizes and text runs without drawing.
- `render_svg` writes vector SVG. It needs the `svg-backend` feature.
- `from_html` parses HTML into a node tree. It needs the `from-html` feature.

## Feature flags

- `raster-backend` (default) enables the raster rendering backend.
- `svg-source` (default) enables SVG image sources in the core and raster backend.
- `svg-backend` enables the vector SVG output backend (`render_svg`).
- `from-html` enables HTML parsing (`from_html`).
- `woff2` (default) enables WOFF2 font support.
- `woff` (default) enables WOFF font support.
- `image-decoding` (default) enables `png`, `jpeg`, `webp` and `gif` together.
- `png`, `jpeg`, `webp`, and `gif` each enable one image source format. A source whose
  decoder is off still lays out from its header size.
- `rayon` (default) adds parallelism in the raster backend. It needs `raster-backend`.
- `unstable` re-exports the backend crates with no semver guarantee.

<!-- cargo-rdme end -->
