use std::{
  borrow::Cow,
  collections::{BTreeMap, HashMap},
  fs::{File, create_dir_all, write},
  io::Read,
  path::{Path, PathBuf},
  sync::{Arc, LazyLock, OnceLock},
};

use rayon::iter::{IntoParallelIterator, ParallelIterator};
use takumi::{
  prelude::*, render, write_animated_gif, write_animated_png, write_animated_webp, write_image,
};
use takumi_core::resources::image::ResourceCache;
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
    GenericFamily::SANS_SERIF,
  ),
  (
    "assets/fonts/geist/GeistMono[wght].woff2",
    "Geist Mono",
    GenericFamily::MONOSPACE,
  ),
  (
    "assets/fonts/twemoji/TwemojiMozilla-colr.woff2",
    "Twemoji Mozilla",
    GenericFamily::EMOJI,
  ),
  (
    "assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf",
    "Archivo",
    GenericFamily::SANS_SERIF,
  ),
  (
    "assets/fonts/sil/scheherazade-new-v17-arabic-regular.woff2",
    "Scheherazade New Test",
    GenericFamily::SERIF,
  ),
  (
    "assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
    "Noto Sans TC",
    GenericFamily::SANS_SERIF,
  ),
  (
    "assets/fonts/cjk-locl-test/CJKLoclTest.woff2",
    "CJK Locl Test",
    GenericFamily::SANS_SERIF,
  ),
  (
    "assets/fonts/noto-sans/noto-sans-devanagari-v30-devanagari-regular.woff2",
    "Noto Sans Devanagari",
    GenericFamily::SERIF,
  ),
  (
    "assets/fonts/poppins/poppins-v24-devanagari_latin-regular.woff2",
    "Poppins",
    GenericFamily::SANS_SERIF,
  ),
  (
    "assets/fonts/poppins/poppins-v24-devanagari_latin-700.woff2",
    "Poppins Bold",
    GenericFamily::SANS_SERIF,
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
      .register(
        FontResource::new(font_data)
          .override_info(FontOverride {
            family_name: Some((*name).into()),
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

/// Test images, provided to renders as pre-fetched resources. Loaded through
/// an [`ResourceCache`] so fixtures exercise the same decode-at-draw-size path as
/// the renderer bindings.
pub static TEST_IMAGES: LazyLock<HashMap<Arc<str>, ImageSource>> = LazyLock::new(|| {
  let cache = ResourceCache::default();
  let images = IMAGES
    .iter()
    .map(|path| {
      let mut data = Vec::new();
      File::open(repo_base_path(path))
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
      (
        Arc::from(*path),
        cache.get_or_decode(&data, ImageCacheMode::Auto).unwrap(),
      )
    })
    .collect();
  CACHE.set(cache).ok().unwrap();
  images
});

/// Keeps the decode cache alive so lazily decoded sources keep their sized
/// entries across renders.
static CACHE: OnceLock<ResourceCache> = OnceLock::new();

#[allow(dead_code)]
pub fn attrs(pairs: &[(&str, &str)]) -> BTreeMap<Box<str>, Box<str>> {
  pairs
    .iter()
    .map(|(key, value)| ((*key).into(), (*value).into()))
    .collect()
}

#[allow(dead_code)]
pub fn run_fixture_test(node: Node, fixture_name: &str) {
  let viewport = create_test_viewport();
  let options = RenderOptions::builder()
    .viewport(viewport)
    .node(node)
    .fonts(&CONTEXT)
    .images(TEST_IMAGES.clone())
    .build();

  run_fixture_test_with_options(options, fixture_name);
}

#[allow(dead_code)]
pub fn run_fixture_test_with_options(options: RenderOptions<'_>, fixture_name: &str) {
  run_fixture_test_with_css(options, "", fixture_name);
}

/// Embeds `css` in the repro HTML; `RenderOptions` only carries the parsed
/// sheet, which cannot serialize back.
#[allow(dead_code)]
pub fn run_fixture_test_with_css(options: RenderOptions<'_>, css: &str, fixture_name: &str) {
  let viewport_width = options.viewport().size.width.unwrap_or(1200);
  let viewport_height = options.viewport().size.height.unwrap_or(630);

  create_dir_all("tests/fixtures-generated").ok();

  let node_html = options.node().to_html();

  // `from_html` is a normalizing importer (presets, collapse, text folding), so
  // round-tripping is a fixpoint: re-serializing a parsed tree reproduces it.
  // Disable presets/tw so the comparison sees only structure, not injected UA
  // styles.
  #[cfg(feature = "from-html")]
  {
    let options = FromHtmlOptions::builder()
      .presets(StylePresets::empty())
      .build();
    let round_tripped = Node::from_html(&node_html, options).expect("round-trip parse");
    assert_eq!(
      node_html,
      round_tripped.to_html(),
      "from_html round-trip diverged for {fixture_name}",
    );
  }

  let style_block = if css.is_empty() {
    String::new()
  } else {
    format!("\n  <style>{css}</style>")
  };
  let html_content = format!(
    r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>{fixture_name}</title>
  <base href="../../../">
  <link rel="stylesheet" href="takumi/tests/shared.css">{style_block}
</head>
<body style="width: {viewport_width}px; height: {viewport_height}px;">
  {node_html}
</body>
</html>"#
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
      .images(options.images().clone())
      .build(),
  ) {
    write(format!("tests/fixtures-generated/{fixture_name}.svg"), svg).unwrap();
  }

  let image = render(options).unwrap();
  let golden_path = format!("tests/fixtures-generated/{fixture_name}.webp");

  save_image(image, &golden_path, OutputFormat::WebPLossless);
}

fn save_image<P: AsRef<Path>>(image: Bitmap, path: P, format: OutputFormat) {
  let path = path.as_ref();

  let mut file = File::create(path).unwrap();

  write_image(&image, &mut file, format).unwrap();
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
        write_animated_webp(
          Cow::Owned(frames.clone()),
          &mut file,
          AnimatedWebpOptions::default(),
        )
        .unwrap();
      }
      AnimationFixtureFormat::Png => {
        write_animated_png(&frames, &mut file, AnimatedPngOptions::default()).unwrap();
      }
      AnimationFixtureFormat::Gif => {
        write_animated_gif(
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
