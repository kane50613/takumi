use std::{
  borrow::Cow,
  fs::File,
  io::Read,
  path::{Path, PathBuf},
  sync::{Arc, LazyLock},
};

use image::{RgbaImage, load_from_memory};
use parley::{GenericFamily, fontique::FontInfoOverride};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use takumi::{
  GlobalContext,
  layout::{Viewport, node::NodeKind},
  rendering::{
    AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame, ImageOutputFormat,
    RenderOptions, RenderOptionsBuilder, encode_animated_gif, encode_animated_png,
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
    "fonts/twemoji/TwemojiMozilla-colr.woff2",
    "Twemoji Mozilla",
    GenericFamily::Emoji,
  ),
  (
    "fonts/archivo/Archivo-VariableFont_wdth,wght.ttf",
    "Archivo",
    GenericFamily::SansSerif,
  ),
  (
    "fonts/sil/scheherazade-new-v17-arabic-regular.woff2",
    "Scheherazade New Test",
    GenericFamily::Serif,
  ),
  (
    "fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
    "Noto Sans TC",
    GenericFamily::SansSerif,
  ),
  (
    "fonts/noto-sans/noto-sans-devanagari-v30-devanagari-regular.woff2",
    "Noto Sans Devanagari",
    GenericFamily::Serif,
  ),
  (
    "fonts/poppins/poppins-v24-devanagari_latin-regular.woff2",
    "Poppins",
    GenericFamily::SansSerif,
  ),
  (
    "fonts/poppins/poppins-v24-devanagari_latin-700.woff2",
    "Poppins Bold",
    GenericFamily::SansSerif,
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

  let mut luma_cover_image_data = Vec::new();
  File::open(assets_path(
    "images/luma-cover-0dfbf65d-0f58-4941-947c-d84a5b131dc0.jpeg",
  ))
  .unwrap()
  .read_to_end(&mut luma_cover_image_data)
  .unwrap();

  context.persistent_image_store.insert(
    "assets/images/luma.svg".to_string(),
    parse_svg_str(&luma_image_data).unwrap(),
  );

  context.persistent_image_store.insert(
    "assets/images/luma-cover-0dfbf65d-0f58-4941-947c-d84a5b131dc0.jpeg".to_string(),
    Arc::new(ImageSource::Bitmap(
      load_from_memory(&luma_cover_image_data)
        .unwrap()
        .into_rgba8(),
    )),
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
        font_data.into(),
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

pub static CONTEXT: LazyLock<GlobalContext> = LazyLock::new(create_test_context);

#[allow(dead_code)]
pub fn run_fixture_test(node: NodeKind, fixture_name: &str) {
  let viewport = create_test_viewport();
  let options = RenderOptionsBuilder::default()
    .viewport(viewport)
    .node(node)
    .global(&CONTEXT)
    .build()
    .unwrap();

  run_fixture_test_with_options(options, fixture_name);
}

#[allow(dead_code)]
pub fn run_fixture_test_with_options(options: RenderOptions<'_, NodeKind>, fixture_name: &str) {
  let image = render(options).unwrap();

  save_image(
    image,
    format!("tests/fixtures-generated/{}.webp", fixture_name),
    ImageOutputFormat::WebP,
  );
}

fn save_image<P: AsRef<Path>>(image: RgbaImage, path: P, format: ImageOutputFormat) {
  let path = path.as_ref();

  let mut file = File::create(path).unwrap();

  write_image(Cow::Owned(image), &mut file, format, None).unwrap();
}

#[allow(dead_code)]
pub fn run_webp_animation_test(
  nodes: Vec<(NodeKind, u32)>,
  fixture_name: &str,
  options: AnimatedWebpOptions,
) {
  assert!(!nodes.is_empty());

  let viewport = create_test_viewport();
  let frames = build_animation_frames(nodes, viewport);

  let fixture_path = format!("tests/fixtures-generated/{}", fixture_name);
  let mut out = File::create(fixture_path).unwrap();
  encode_animated_webp(Cow::Owned(frames), &mut out, options).unwrap();
}

#[allow(dead_code)]
pub fn run_png_animation_test(
  nodes: Vec<(NodeKind, u32)>,
  fixture_name: &str,
  options: AnimatedPngOptions,
) {
  assert!(!nodes.is_empty());

  let viewport = create_test_viewport();
  let frames = build_animation_frames(nodes, viewport);

  let fixture_path = format!("tests/fixtures-generated/{}", fixture_name);
  let mut out = File::create(fixture_path).unwrap();
  encode_animated_png(&frames, &mut out, options).unwrap();
}

#[allow(dead_code)]
pub fn run_gif_animation_test(
  nodes: Vec<(NodeKind, u32)>,
  fixture_name: &str,
  options: AnimatedGifOptions,
) {
  assert!(!nodes.is_empty());

  let viewport = create_test_viewport();
  let frames = build_animation_frames(nodes, viewport);

  let fixture_path = format!("tests/fixtures-generated/{}", fixture_name);
  let mut out = File::create(fixture_path).unwrap();
  encode_animated_gif(Cow::Owned(frames), &mut out, options).unwrap();
}

fn build_animation_frames(nodes: Vec<(NodeKind, u32)>, viewport: Viewport) -> Vec<AnimationFrame> {
  nodes
    .into_par_iter()
    .map(|(node, duration_ms)| {
      AnimationFrame::new(
        render(
          RenderOptionsBuilder::default()
            .viewport(viewport)
            .node(node)
            .global(&CONTEXT)
            .build()
            .unwrap(),
        )
        .unwrap(),
        duration_ms,
      )
    })
    .collect()
}
