use std::{borrow::Cow, fs::File};

use takumi::base::{
  Fonts,
  layout::{
    Viewport,
    node::Node,
    style::{Length::Px, Style, StyleDeclaration},
  },
  resources::font::FontResource,
};
use takumi::raster::{ImageOutputFormat, Quality, RenderOptions, render, write_image};

/// Renders a "Hello, {name}!" image and saves it to `output.webp`.
pub fn say_hello_to(name: &str) {
  // A `Fonts` holds the registered fonts; create one per application and share it
  // across renders (e.g. behind an `Arc`). takumi loads no system fonts by default,
  // so register custom fonts with `register` before rendering text.
  let mut fonts = Fonts::default();

  fonts
    .register(FontResource::new(include_bytes!(
      "../../../assets/fonts/geist/Geist[wght].woff2"
    )))
    .unwrap();

  let text = Node::text(format!("Hello, {name}!"))
    .with_style(Style::default().with(StyleDeclaration::font_size(Px(48.0).into())));
  let root = Node::container([text]);

  let options = RenderOptions::builder()
    .viewport(Viewport::new((1200, 630)))
    .node(root)
    .fonts(&fonts)
    .build();

  let image = render(options).unwrap();

  let mut file = File::create("output.webp").unwrap();
  write_image(
    Cow::Owned(image),
    &mut file,
    ImageOutputFormat::WebP {
      quality: Quality::new(100),
    },
  )
  .unwrap();
}
