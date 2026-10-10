use std::fs;

use image::ImageFormat;
use takumi::{prelude::*, render, write_image};

use crate::test_utils::{CONTEXT, create_test_viewport, generated_path};

/// Flat art large enough to deflate in segments, read back pixel for pixel.
#[test]
fn png_output() {
  let node = Node::container([
    Node::text("Segmented deflate".to_string())
      .with_tw("text-[96px] font-bold text-white".parse().unwrap()),
    Node::text("Each band of rows compresses on its own thread.".to_string())
      .with_tw("text-[36px] text-indigo-100".parse().unwrap()),
  ])
  .with_tw(
    "flex flex-col w-full h-full justify-center gap-6 p-20 bg-linear-to-br from-indigo-600 to-fuchsia-600"
      .parse()
      .unwrap(),
  );
  let image = render(
    RenderOptions::builder()
      .viewport(create_test_viewport())
      .node(node)
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap();
  let mut encoded = Vec::new();

  write_image(&image, &mut encoded, OutputFormat::Png).unwrap();
  fs::write(generated_path("png_output.png"), &encoded).unwrap();

  let decoded = image::load_from_memory_with_format(&encoded, ImageFormat::Png)
    .unwrap()
    .to_rgba8();

  assert_eq!(decoded.dimensions(), (image.width(), image.height()));
  assert!(decoded.as_raw() == image.as_raw());
}
