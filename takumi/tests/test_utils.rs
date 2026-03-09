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
  layout::{DEFAULT_FONT_SIZE, Viewport, node::NodeKind},
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

const FIXTURE_DEVICE_PIXEL_RATIO: f32 = 0.75;

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

pub const fn create_test_viewport_with_size(width: u32, height: u32) -> Viewport {
  Viewport {
    width: Some((width as f32 * FIXTURE_DEVICE_PIXEL_RATIO) as u32),
    height: Some((height as f32 * FIXTURE_DEVICE_PIXEL_RATIO) as u32),
    device_pixel_ratio: FIXTURE_DEVICE_PIXEL_RATIO,
    font_size: DEFAULT_FONT_SIZE,
  }
}

pub fn create_test_viewport() -> Viewport {
  create_test_viewport_with_size(1200, 630)
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

impl IntoAnimationFixtureFrames<'_> for Vec<NodeKind> {
  fn into_frames(self, frame_duration_ms: u32) -> Vec<AnimationFrame> {
    let viewport = create_test_viewport();

    build_animation_frames(
      self
        .into_iter()
        .enumerate()
        .map(|(index, node)| {
          let time_ms = (index as u64) * u64::from(frame_duration_ms);

          (
            RenderOptionsBuilder::default()
              .viewport(viewport)
              .node(node)
              .time_ms(time_ms)
              .global(&CONTEXT)
              .build()
              .unwrap(),
            frame_duration_ms,
          )
        })
        .collect(),
    )
  }
}

impl<'g> IntoAnimationFixtureFrames<'g> for Vec<RenderOptions<'g, NodeKind>> {
  fn into_frames(self, frame_duration_ms: u32) -> Vec<AnimationFrame> {
    build_animation_frames(
      self
        .into_iter()
        .map(|options| (options, frame_duration_ms))
        .collect(),
    )
  }
}

fn build_animation_frames(options: Vec<(RenderOptions<'_, NodeKind>, u32)>) -> Vec<AnimationFrame> {
  options
    .into_par_iter()
    .map(|(options, duration_ms)| AnimationFrame::new(render(options).unwrap(), duration_ms))
    .collect()
}
