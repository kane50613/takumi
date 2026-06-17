use std::{
  borrow::Cow,
  collections::HashMap,
  fs::{File, create_dir_all, write},
  io::Read,
  path::{Path, PathBuf},
  sync::{Arc, LazyLock},
};

use image::RgbaImage;
use parley::{GenericFamily, fontique::FontInfoOverride};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use takumi::base::{
  Fonts,
  layout::{Viewport, node::Node},
  resources::{font::FontResource, image::ImageSource},
};
use takumi::raster::{
  AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame, ImageOutputFormat,
  RenderOptions, encode_animated_gif, encode_animated_png, encode_animated_webp, render,
  write_image,
};
use takumi_svg::{SvgOptions, render as svg_render};

fn repo_base_path(path: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../")
    .join(path)
    .to_path_buf()
}

const TEST_FONTS: &[(&str, &str, GenericFamily)] = &[
  (
    "assets/fonts/geist/Geist[wght].woff2",
    "Geist",
    GenericFamily::SansSerif,
  ),
  (
    "assets/fonts/geist/GeistMono[wght].woff2",
    "Geist Mono",
    GenericFamily::Monospace,
  ),
  (
    "assets/fonts/twemoji/TwemojiMozilla-colr.woff2",
    "Twemoji Mozilla",
    GenericFamily::Emoji,
  ),
  (
    "assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf",
    "Archivo",
    GenericFamily::SansSerif,
  ),
  (
    "assets/fonts/sil/scheherazade-new-v17-arabic-regular.woff2",
    "Scheherazade New Test",
    GenericFamily::Serif,
  ),
  (
    "assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
    "Noto Sans TC",
    GenericFamily::SansSerif,
  ),
  (
    "assets/fonts/noto-sans/noto-sans-devanagari-v30-devanagari-regular.woff2",
    "Noto Sans Devanagari",
    GenericFamily::Serif,
  ),
  (
    "assets/fonts/poppins/poppins-v24-devanagari_latin-regular.woff2",
    "Poppins",
    GenericFamily::SansSerif,
  ),
  (
    "assets/fonts/poppins/poppins-v24-devanagari_latin-700.woff2",
    "Poppins Bold",
    GenericFamily::SansSerif,
  ),
];

const IMAGES: &[&str] = &[
  "assets/images/yeecord.png",
  "assets/images/luma.svg",
  "assets/images/luma-cover-0dfbf65d-0f58-4941-947c-d84a5b131dc0.jpeg",
];

fn create_test_context() -> Fonts {
  let mut context = Fonts::default();

  for (font, name, generic) in TEST_FONTS {
    let mut font_data = Vec::new();
    File::open(repo_base_path(font))
      .unwrap()
      .read_to_end(&mut font_data)
      .unwrap();

    context
      .load_and_store(
        FontResource::new(font_data)
          .override_info(FontInfoOverride {
            family_name: Some(name),
            ..Default::default()
          })
          .generic_family(*generic),
      )
      .unwrap();
  }

  context
}

pub fn create_test_viewport() -> Viewport {
  Viewport::new((1200, 630))
}

pub static CONTEXT: LazyLock<Fonts> = LazyLock::new(create_test_context);

/// Test images, provided to renders as pre-fetched resources.
pub static TEST_IMAGES: LazyLock<HashMap<Arc<str>, ImageSource>> = LazyLock::new(|| {
  IMAGES
    .iter()
    .map(|path| {
      let mut data = Vec::new();
      File::open(repo_base_path(path))
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
      (Arc::from(*path), ImageSource::from_bytes(&data).unwrap())
    })
    .collect()
});

#[allow(dead_code)]
pub fn run_fixture_test(node: Node, fixture_name: &str) {
  let viewport = create_test_viewport();
  let options = RenderOptions::builder()
    .viewport(viewport)
    .node(node)
    .fonts(&CONTEXT)
    .fetched_resources(TEST_IMAGES.clone())
    .build();

  run_fixture_test_with_options(options, fixture_name);
}

#[allow(dead_code)]
pub fn run_fixture_test_with_options(options: RenderOptions<'_>, fixture_name: &str) {
  let viewport_width = options.viewport().size.width.unwrap_or(1200);
  let viewport_height = options.viewport().size.height.unwrap_or(630);

  create_dir_all("tests/fixtures-generated").ok();

  let html_content = format!(
    r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>{}</title>
  <link rel="stylesheet" href="../shared.css">
</head>
<body style="width: {}px; height: {}px;">
  {}
</body>
</html>"#,
    fixture_name,
    viewport_width,
    viewport_height,
    options.node().to_html()
  );

  write(
    format!("tests/fixtures-generated/{fixture_name}.html"),
    html_content,
  )
  .unwrap();

  // Emit the vector SVG alongside the raster golden (best-effort: the SVG backend
  // does not cover every paint feature yet, so failures are skipped not fatal).
  if let Ok(svg) = svg_render(
    SvgOptions::builder()
      .node(options.node().clone())
      .viewport(*options.viewport())
      .fonts(options.fonts())
      .stylesheet(options.stylesheet().clone())
      .fetched_resources(options.fetched_resources().clone())
      .build(),
  ) {
    write(format!("tests/fixtures-generated/{fixture_name}.svg"), svg).unwrap();
  }

  let image = render(options).unwrap();
  let golden_path = format!("tests/fixtures-generated/{fixture_name}.webp");

  save_image(image, &golden_path, ImageOutputFormat::WebP);
}

fn save_image<P: AsRef<Path>>(image: RgbaImage, path: P, format: ImageOutputFormat) {
  let path = path.as_ref();

  let mut file = File::create(path).unwrap();

  write_image(Cow::Owned(image), &mut file, format, None).unwrap();
}

#[allow(dead_code)]
pub(crate) fn run_animation_fixture_test<'g, Frames>(
  frames: Frames,
  fixture_id: &str,
  duration_ms: u32,
  fps: u32,
) where
  Frames: IntoAnimationFixtureFrames<'g>,
{
  assert!(duration_ms > 0);
  assert!(fps > 0);

  let frame_duration_ms = ((1000.0 / fps as f32).round() as u32).max(1);
  let expected_frame_count = duration_ms.div_ceil(frame_duration_ms).max(1) as usize;
  let frames = frames.into_frames(frame_duration_ms);
  assert!(!frames.is_empty());
  assert_eq!(frames.len(), expected_frame_count);

  enum AnimationFixtureFormat {
    Webp,
    Png,
    Gif,
  }

  [
    AnimationFixtureFormat::Webp,
    AnimationFixtureFormat::Png,
    AnimationFixtureFormat::Gif,
  ]
  .into_par_iter()
  .for_each(|format| {
    let extension = match format {
      AnimationFixtureFormat::Webp => "webp",
      AnimationFixtureFormat::Png => "png",
      AnimationFixtureFormat::Gif => "gif",
    };
    let mut file =
      File::create(format!("tests/fixtures-generated/{fixture_id}.{extension}")).unwrap();

    match format {
      AnimationFixtureFormat::Webp => {
        encode_animated_webp(
          Cow::Owned(frames.clone()),
          &mut file,
          AnimatedWebpOptions::default(),
        )
        .unwrap();
      }
      AnimationFixtureFormat::Png => {
        encode_animated_png(&frames, &mut file, AnimatedPngOptions::default()).unwrap();
      }
      AnimationFixtureFormat::Gif => {
        encode_animated_gif(
          Cow::Owned(frames.clone()),
          &mut file,
          AnimatedGifOptions::default(),
        )
        .unwrap();
      }
    }
  });
}

pub(crate) trait IntoAnimationFixtureFrames<'g> {
  fn into_frames(self, frame_duration_ms: u32) -> Vec<AnimationFrame>;
}

impl IntoAnimationFixtureFrames<'_> for Vec<AnimationFrame> {
  fn into_frames(self, _: u32) -> Vec<AnimationFrame> {
    self
  }
}

impl IntoAnimationFixtureFrames<'_> for Vec<Node> {
  fn into_frames(self, frame_duration_ms: u32) -> Vec<AnimationFrame> {
    let viewport = create_test_viewport();

    build_animation_frames(
      self
        .into_iter()
        .enumerate()
        .map(|(index, node)| {
          let time_ms = (index as u64) * u64::from(frame_duration_ms);

          (
            RenderOptions::builder()
              .viewport(viewport)
              .node(node)
              .time_ms(time_ms)
              .fonts(&CONTEXT)
              .build(),
            frame_duration_ms,
          )
        })
        .collect(),
    )
  }
}

impl<'g> IntoAnimationFixtureFrames<'g> for Vec<RenderOptions<'g>> {
  fn into_frames(self, frame_duration_ms: u32) -> Vec<AnimationFrame> {
    build_animation_frames(
      self
        .into_iter()
        .map(|options| (options, frame_duration_ms))
        .collect(),
    )
  }
}

fn build_animation_frames(options: Vec<(RenderOptions<'_>, u32)>) -> Vec<AnimationFrame> {
  options
    .into_par_iter()
    .map(|(options, duration_ms)| AnimationFrame::new(render(options).unwrap(), duration_ms))
    .collect()
}
