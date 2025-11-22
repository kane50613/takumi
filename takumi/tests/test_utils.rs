use std::{
  fs::File,
  io::{BufWriter, Read},
  path::{Path, PathBuf},
  sync::Arc,
};

use image::load_from_memory;
use parley::{GenericFamily, fontique::FontInfoOverride};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use takumi::{
  GlobalContext,
  layout::{Viewport, node::NodeKind},
  rendering::{
    AnimationFrame, ImageOutputFormat, RenderOptionsBuilder, encode_animated_png,
    encode_animated_webp, render, write_image,
  },
  resources::image::{ImageSource, parse_svg_str},
};

fn assets_path(path: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../assets/")
    .join(path)
    .to_path_buf()
}

const TEST_FONTS: &[(&str, &str, GenericFamily)] = &[
  (
    "fonts/geist/Geist[wght].woff2",
    "Geist",
    GenericFamily::SansSerif,
  ),
  (
    "fonts/geist/GeistMono[wght].woff2",
    "Geist Mono",
    GenericFamily::Monospace,
  ),
  (
    "fonts/noto-sans/NotoColorEmoji.ttf",
    "Noto Color Emoji",
    GenericFamily::Emoji,
  ),
];

fn create_test_context() -> GlobalContext {
  let mut context = GlobalContext::default();

  let mut yeecord_image_data = Vec::new();
  File::open(assets_path("images/yeecord.png"))
    .unwrap()
    .read_to_end(&mut yeecord_image_data)
    .unwrap();

  let mut luma_image_data = String::new();
  File::open(assets_path("images/luma.svg"))
    .unwrap()
    .read_to_string(&mut luma_image_data)
    .unwrap();

  context.persistent_image_store.insert(
    "assets/images/yeecord.png".to_string(),
    Arc::new(ImageSource::Bitmap(
      load_from_memory(&yeecord_image_data).unwrap().into_rgba8(),
    )),
  );

  context.persistent_image_store.insert(
    "assets/images/luma.svg".to_string(),
    parse_svg_str(&luma_image_data).unwrap(),
  );

  for (font, name, generic) in TEST_FONTS {
    let mut font_data = Vec::new();
    File::open(assets_path(font))
      .unwrap()
      .read_to_end(&mut font_data)
      .unwrap();

    context
      .font_context
      .load_and_store(
        &font_data,
        Some(FontInfoOverride {
          family_name: Some(name),
          ..Default::default()
        }),
        Some(*generic),
      )
      .unwrap();
  }

  context
}

pub fn create_test_viewport() -> Viewport {
  (1200, 630).into()
}

/// Helper function to run style width tests
#[allow(dead_code)]
pub fn run_style_width_test(node: NodeKind, fixture_path: &str) {
  let context = create_test_context();
  let viewport = create_test_viewport();

  let image = render(
    RenderOptionsBuilder::default()
      .viewport(viewport)
      .node(node)
      .global(&context)
      .build()
      .unwrap(),
  )
  .unwrap();

  let path = Path::new(fixture_path);

  let mut file = File::create(path).unwrap();
  let mut buf = BufWriter::new(&mut file);

  write_image(&image, &mut buf, ImageOutputFormat::WebP, None).unwrap();
}

#[allow(dead_code)]
pub fn run_webp_animation_test(
  nodes: Vec<(NodeKind, u32)>,
  fixture_path: &str,
  blend: bool,
  dispose: bool,
  loop_count: Option<u16>,
) {
  assert_ne!(nodes.len(), 0);

  let context = create_test_context();
  let viewport = create_test_viewport();

  let frames: Vec<_> = nodes
    .into_par_iter()
    .map(|(node, duration_ms)| {
      AnimationFrame::new(
        render(
          RenderOptionsBuilder::default()
            .viewport(viewport)
            .node(node)
            .global(&context)
            .build()
            .unwrap(),
        )
        .unwrap(),
        duration_ms,
      )
    })
    .collect();

  let mut out = File::create(fixture_path).unwrap();
  encode_animated_webp(&frames, &mut out, blend, dispose, loop_count).unwrap();
}

#[allow(dead_code)]
pub fn run_png_animation_test(
  nodes: Vec<(NodeKind, u32)>,
  fixture_path: &str,
  loop_count: Option<u16>,
) {
  assert_ne!(nodes.len(), 0);

  let context = create_test_context();
  let viewport = create_test_viewport();

  let frames: Vec<_> = nodes
    .into_par_iter()
    .map(|(node, duration_ms)| {
      AnimationFrame::new(
        render(
          RenderOptionsBuilder::default()
            .viewport(viewport)
            .node(node)
            .global(&context)
            .build()
            .unwrap(),
        )
        .unwrap(),
        duration_ms,
      )
    })
    .collect();

  let mut out = File::create(fixture_path).unwrap();
  encode_animated_png(&frames, &mut out, loop_count).unwrap();
}
