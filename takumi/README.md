# Takumi

<!-- cargo-rdme start -->

Takumi renders UI component trees to images. This crate is a thin facade that
re-exports the backend-agnostic core ([`takumi_core`], as [`core`]) and the
rendering backends under namespaced modules: the raster backend
([`takumi_paint`], as [`paint`]) and the vector/SVG backend ([`takumi_svg`],
as [`svg`]).

## Example

```rust
use takumi::core::{
  GlobalContext,
  layout::{Viewport, node::Node, style::{Length::Px, Style, StyleDeclaration}},
  resources::font::FontResource,
};
use takumi::paint::{RenderOptions, render};

let node = Node::container([Node::text("Hello, world!").with_style(
  Style::default().with(StyleDeclaration::font_size(Px(32.0).into())),
)]);

let mut global = GlobalContext::default();

global.font_context.load_and_store(
  FontResource::new(include_bytes!("../../assets/fonts/geist/Geist[wght].woff2"))
);

let viewport = Viewport::new((1200, 630));

let options = RenderOptions::builder()
  .viewport(viewport)
  .node(node)
  .global(&global)
  .build();

let image = render(options).unwrap();
```

## Feature Flags

- `paint` (default): Enable the raster rendering backend, available as
  [`takumi::paint`](paint).
- `svg` (default): Enable SVG image-source support in the core and paint
  backend.
- `svg-backend`: Enable the vector/SVG output backend, available as
  [`takumi::svg`](svg). Opt-in.
- `woff2`: Enable WOFF2 font support.
- `woff`: Enable WOFF font support.
- `rayon`: Enable rayon-based parallelism in the raster backend (implies
  `paint`).

<!-- cargo-rdme end -->
