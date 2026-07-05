# Takumi

<!-- cargo-rdme start -->

Takumi renders a UI component tree to an image.

This crate is the facade. The entry-point functions live at the crate root;
the curated, stable types live in [`prelude`]. Glob the prelude, build a node
tree, and call [`render`].

The backend crates carry a much larger surface (layout glue, paint internals,
cross-crate plumbing) that is `pub` only so sibling crates can share it. The
facade does not re-export it. Enable the `unstable` feature to reach it under
[`unstable`]; nothing there follows semver.

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
- `svg-backend`: vector SVG output backend ([`render_svg`]).
- `woff2`: WOFF2 font support.
- `woff`: WOFF font support.
- `rayon`: parallelism in the raster backend; needs `raster-backend`.
- `unstable`: re-export the backend crates under [`unstable`]; no semver.

<!-- cargo-rdme end -->
