# Takumi

<!-- cargo-rdme start -->

Takumi is a library with different parts to render your React components to images. This crate contains the core logic for layout, rendering.

Checkout the [Quick Start](https://takumi.kane.tw/docs) if you are looking for napi-rs / WASM bindings.

## Example

```rust
use takumi::{
  layout::{
    node::Node,
    Viewport,
    style::{Length::Px, Style, StyleDeclaration},
  },
  rendering::{render, RenderOptions},
  GlobalContext,
};

// Create a node tree with `Node::container` and `Node::text`
let node = Node::container([Node::text("Hello, world!").with_style(
  Style::default().with(StyleDeclaration::font_size(Px(32.0).into())),
)]);

// Create a context for storing resources, font caches.
// You should reuse the context to speed up the rendering.
let mut global = GlobalContext::default();

// Load fonts
global.font_context_mut().load_and_store(
  include_bytes!("../../assets/fonts/geist/Geist[wght].woff2").into(),
  None,
  None,
);

// Create a viewport
let viewport = Viewport::new(Some(1200), Some(630));

// Create render options
let options = RenderOptions::builder()
  .viewport(viewport)
  .node(node)
  .global(&global)
  .build();

// Render the layout to an `RgbaImage`
let image = render(options).unwrap();
```

## Feature Flags

- `woff2`: Enable WOFF2 font support.
- `woff`: Enable WOFF font support.
- `svg`: Enable SVG support.
- `rayon`: Enable rayon support.

## Credits

Takumi wouldn't be possible without the following works:

- [taffy](https://github.com/DioxusLabs/taffy) for the flex & grid layout.
- [image](https://github.com/image-rs/image) for the image processing.
- [parley](https://github.com/linebender/parley) for text layout.
- [swash](https://github.com/linebender/swash) for font shaping.
- [wuff](https://github.com/nicoburns/wuff) for woff/woff2 decompression.
- [resvg](https://github.com/linebender/resvg) for SVG parsing & rasterization.

<!-- cargo-rdme end -->
