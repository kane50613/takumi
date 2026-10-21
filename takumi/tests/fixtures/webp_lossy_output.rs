use std::fs;

use takumi::{prelude::*, render, write_image};

use crate::test_utils::{CONTEXT, TEST_IMAGES, create_test_viewport, decode_rgba, generated_path};

const PHOTO: &str = "assets/images/luma-cover-0dfbf65d-0f58-4941-947c-d84a5b131dc0.jpeg";

/// Peak signal-to-noise ratio of `decoded` against `source`, both RGBA, over the color channels.
fn psnr(source: &[u8], decoded: &[u8]) -> f64 {
  let (squared, count) = source
    .as_chunks::<4>()
    .0
    .iter()
    .zip(decoded.as_chunks::<4>().0)
    .flat_map(|(source, decoded)| source[..3].iter().zip(&decoded[..3]))
    .fold((0.0, 0usize), |(squared, count), (&source, &decoded)| {
      let difference = f64::from(source) - f64::from(decoded);

      (squared + difference * difference, count + 1)
    });

  10.0 * (255.0 * 255.0 / (squared / count as f64)).log10()
}

/// A photo under white text, encoded as lossy WebP at the default quality and read back.
#[test]
fn webp_lossy_output() {
  let node = Node::container([
    Node::image(PHOTO).with_tw(
      "absolute inset-0 w-full h-full object-cover"
        .parse()
        .unwrap(),
    ),
    Node::text("Typography on a photograph".to_string()).with_tw(
      "relative px-8 py-4 rounded-2xl bg-black/60 text-[64px] font-bold text-white"
        .parse()
        .unwrap(),
    ),
  ])
  .with_tw(
    "relative flex w-full h-full items-center justify-center p-16 bg-black"
      .parse()
      .unwrap(),
  );
  let image = render(
    RenderOptions::builder()
      .viewport(create_test_viewport())
      .node(node)
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
      .build(),
  )
  .unwrap();
  let mut encoded = Vec::new();

  write_image(
    &image,
    &mut encoded,
    OutputFormat::WebP {
      quality: Quality::default(),
    },
  )
  .unwrap();
  fs::write(generated_path("webp_lossy_output.webp"), &encoded).unwrap();

  let (width, height, decoded) = decode_rgba(&encoded);

  assert_eq!((width, height), (image.width(), image.height()));
  assert!(psnr(image.as_raw(), &decoded) > 35.0);
}
