use std::{
  borrow::Cow,
  env,
  fs::{File, create_dir_all, remove_file, write},
  io::Read,
  path::{Path, PathBuf},
  sync::LazyLock,
};

use image::RgbaImage;
use parley::{GenericFamily, fontique::FontInfoOverride};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use takumi::{
  GlobalContext,
  layout::{Viewport, node::Node},
  rendering::{
    AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame, ImageOutputFormat,
    RenderOptions, encode_animated_gif, encode_animated_png, encode_animated_webp, render,
    write_image,
  },
  resources::{font::FontResource, image::ImageSource},
};

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

fn create_test_context() -> GlobalContext {
  let mut context = GlobalContext::default();

  for image_path in IMAGES {
    let mut image_data = Vec::new();
    File::open(repo_base_path(image_path))
      .unwrap()
      .read_to_end(&mut image_data)
      .unwrap();

    let image = ImageSource::from_bytes(&image_data).unwrap();
    context
      .persistent_image_store
      .insert(image_path.to_string(), image);
  }

  for (font, name, generic) in TEST_FONTS {
    let mut font_data = Vec::new();
    File::open(repo_base_path(font))
      .unwrap()
      .read_to_end(&mut font_data)
      .unwrap();

    context
      .font_context
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

pub static CONTEXT: LazyLock<GlobalContext> = LazyLock::new(create_test_context);

#[allow(dead_code)]
pub fn run_fixture_test(node: Node, fixture_name: &str) {
  let viewport = create_test_viewport();
  let options = RenderOptions::builder()
    .viewport(viewport)
    .node(node)
    .global(&CONTEXT)
    .build();

  run_fixture_test_with_options(options, fixture_name);
}

#[allow(dead_code)]
pub fn run_fixture_test_with_options(options: RenderOptions<'_>, fixture_name: &str) {
  let viewport_width = options.viewport().size.width.unwrap_or(1200) as u32;
  let viewport_height = options.viewport().size.height.unwrap_or(630) as u32;

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

  let image = render(options).unwrap();
  let golden_path = format!("tests/fixtures-generated/{fixture_name}.webp");

  if env::var("CI").is_ok() {
    let expected_image = match image::open(&golden_path) {
      Ok(img) => img.to_rgba8(),
      Err(err) => panic!("Golden image missing or invalid at {golden_path}: {err}"),
    };

    if let Some(diff_image) = run_pixelmatch(&image, &expected_image) {
      let actual_path = format!("tests/fixtures-generated/{fixture_name}.actual.webp");
      let diff_path = format!("tests/fixtures-generated/{fixture_name}.diff.webp");

      save_image(image, &actual_path, ImageOutputFormat::WebP);
      save_image(diff_image, &diff_path, ImageOutputFormat::WebP);

      panic!(
        "Visual regression test failed for fixture '{fixture_name}'!\n\
         - Golden image: {golden_path}\n\
         - Fresh actual image: {actual_path}\n\
         - Pixel diff highlight: {diff_path}\n\
         Please inspect the diff and update the golden image locally (without CI=true)."
      );
    }
  } else {
    save_image(image, &golden_path, ImageOutputFormat::WebP);
    remove_file(format!(
      "tests/fixtures-generated/{fixture_name}.actual.webp"
    ))
    .ok();
    remove_file(format!("tests/fixtures-generated/{fixture_name}.diff.webp")).ok();
  }
}

fn run_pixelmatch(actual: &RgbaImage, expected: &RgbaImage) -> Option<RgbaImage> {
  if actual.dimensions() != expected.dimensions() {
    let mut diff = RgbaImage::new(
      actual.width().max(expected.width()),
      actual.height().max(expected.height()),
    );
    for pixel in diff.pixels_mut() {
      *pixel = image::Rgba([255, 0, 0, 255]);
    }
    return Some(diff);
  }

  let (w, h) = actual.dimensions();
  let mut diff = RgbaImage::new(w, h);
  let mut mismatch_count = 0;

  for y in 0..h {
    for x in 0..w {
      let p1 = actual.get_pixel(x, y);
      let p2 = expected.get_pixel(x, y);

      if p1 != p2 {
        mismatch_count += 1;
        diff.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
      } else {
        let r = (p1[0] as f32 * 0.3 + 255.0 * 0.7) as u8;
        let g = (p1[1] as f32 * 0.3 + 255.0 * 0.7) as u8;
        let b = (p1[2] as f32 * 0.3 + 255.0 * 0.7) as u8;
        diff.put_pixel(x, y, image::Rgba([r, g, b, 255]));
      }
    }
  }

  if mismatch_count > 0 { Some(diff) } else { None }
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
              .global(&CONTEXT)
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
