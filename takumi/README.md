# Takumi

<!-- cargo-rdme start -->

Takumi renders a UI component tree to an image.

This crate is the facade. The entry-point functions live at the crate root;
the curated, stable types live in [`prelude`](https://docs.rs/takumi/latest/takumi/prelude/). Glob the prelude, build a node
tree, and call [`render`](https://docs.rs/takumi/latest/takumi/fn.render.html).

## Example

```rust
use takumi::prelude::*;
use takumi::render;

let node = Node::container([Node::text("Hello, world!").with_style(
  Style::default().with(StyleDeclaration::font_size(Length::Px(32.0).into())),
)]);

// Reuse one font context across renders to share the decode cache.
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
```

## Feature flags

- `raster-backend` (default): raster rendering backend.
- `svg-source` (default): SVG image sources in the core and raster backend.
- `svg-backend`: vector SVG output backend (`render_svg`).
- `woff2`: WOFF2 font support.
- `woff`: WOFF font support.
- `image-decoding` (default): `jpeg`, `webp` and `gif` together.
- `jpeg`, `webp`, `gif`: one image source format each. PNG and ICO are always
  decoded.
- `rayon`: parallelism in the raster backend; needs `raster-backend`.
- `unstable`: re-export the backend crates with no semver guarantee.

<!-- cargo-rdme end -->
