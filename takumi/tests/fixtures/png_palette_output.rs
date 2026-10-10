use std::fs;

use image::ImageFormat;
use takumi::{prelude::*, render, write_image};

use crate::test_utils::{CONTEXT, create_test_viewport, generated_path};

/// A docs card of a few hundred colors at most, stored as a palette and read back pixel for pixel.
#[test]
fn png_palette_output() {
  let node = Node::container([
    Node::text("Configuring retries".to_string())
      .with_tw("text-[72px] font-bold text-white".parse().unwrap()),
    Node::text("Back off exponentially and cap each attempt's timeout.".to_string())
      .with_tw("text-[32px] text-neutral-400".parse().unwrap()),
  ])
  .with_tw(
    "flex flex-col w-full h-full justify-center gap-6 p-20 bg-neutral-950"
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
  fs::write(generated_path("png_palette_output.png"), &encoded).unwrap();

  // IHDR's color type, 3 for indexed color.
  assert_eq!(encoded[25], 3);

  let decoded = image::load_from_memory_with_format(&encoded, ImageFormat::Png)
    .unwrap()
    .to_rgba8();

  assert!(decoded.as_raw() == image.as_raw());
}
