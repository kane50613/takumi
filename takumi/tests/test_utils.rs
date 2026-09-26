#![allow(
  dead_code,
  reason = "each test binary compiles this module and uses a subset"
)]

use std::{
  borrow::Cow,
  collections::HashMap,
  fs::{self, File},
  path::{Path, PathBuf},
  process::Command,
  result::Result,
  sync::{Arc, LazyLock, OnceLock},
};

use rayon::iter::{IntoParallelIterator, ParallelIterator};
use takumi::{
  measure, prelude::*, render, write_animated_gif, write_animated_png, write_animated_webp,
  write_image,
};
use takumi_core::resources::image::ResourceCache;
use takumi_svg::{SvgOptions, render as svg_render};

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

const TEST_VIEWPORT: (u32, u32) = (1200, 630);

pub const GENERATED_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures-generated");

pub static CONTEXT: LazyLock<Fonts> = LazyLock::new(create_test_context);

/// Test images, provided to renders as pre-fetched resources. Loaded through
/// an [`ResourceCache`] so fixtures exercise the same decode-at-draw-size path as
/// the renderer bindings.
pub static TEST_IMAGES: LazyLock<HashMap<Arc<str>, ImageSource>> = LazyLock::new(|| {
  let cache = ResourceCache::default();
  let images = IMAGES
    .iter()
    .map(|path| {
      let data = fs::read(repo_base_path(path)).unwrap();

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

/// Inclusive edges of the dark ink: left, top, right, bottom.
pub type InkBounds = (u32, u32, u32, u32);

pub fn repo_base_path(path: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../").join(path)
}

pub fn generated_path(file_name: &str) -> PathBuf {
  Path::new(GENERATED_DIR).join(file_name)
}

pub fn create_test_viewport() -> Viewport {
  Viewport::new(TEST_VIEWPORT)
}

pub fn render_node(node: Node, viewport: Viewport) -> Bitmap {
  render(
    RenderOptions::builder()
      .viewport(viewport)
      .node(node)
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap()
}

pub fn measure_with_css(node: Node, css: &str) -> MeasuredNode {
  measure(
    RenderOptions::builder()
      .viewport(create_test_viewport())
      .node(node)
      .stylesheet(StyleSheet::parse_loosy(css).into())
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap()
}

pub fn block(class: &str) -> Node {
  Node::container([])
    .with_class_name(class)
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)))
}

/// Text of every run in the subtree, in tree order.
pub fn run_texts(node: &MeasuredNode) -> Vec<&str> {
  node
    .runs
    .iter()
    .map(|run| run.text.as_str())
    .chain(node.children.iter().flat_map(run_texts))
    .collect()
}

/// Edges of the pixels dark enough to count as ink; `(u32::MAX, u32::MAX, 0, 0)`
/// when there are none.
pub fn ink_bounds(image: &Bitmap) -> InkBounds {
  let width = image.width();

  image
    .as_raw()
    .as_chunks::<4>()
    .0
    .iter()
    .enumerate()
    .filter(|(_, pixel)| pixel[3] > 0 && pixel[0].min(pixel[1]).min(pixel[2]) < 160)
    .fold(
      (u32::MAX, u32::MAX, 0, 0),
      |(left, top, right, bottom), (index, _)| {
        let (x, y) = (index as u32 % width, index as u32 / width);
        (left.min(x), top.min(y), right.max(x), bottom.max(y))
      },
    )
}

pub fn run_fixture_test(node: Node, fixture_name: &str) {
  let (viewport_width, viewport_height) = TEST_VIEWPORT;
  let options = RenderOptions::builder()
    .viewport(create_test_viewport())
    .node(node)
    .fonts(&CONTEXT)
    .images(TEST_IMAGES.clone())
    .build();
  let node_html = options.node().to_html();

  // `from_html` is a normalizing importer (presets, collapse, text folding), so
  // round-tripping is a fixpoint: re-serializing a parsed tree reproduces it.
  // Disable presets/tw so the comparison sees only structure, not injected UA
  // styles.
  let round_tripped = Node::from_html(
    &node_html,
    FromHtmlOptions::builder()
      .presets(StylePresets::empty())
      .build(),
  )
  .expect("round-trip parse");

  assert_eq!(
    node_html,
    round_tripped.to_html(),
    "from_html round-trip diverged for {fixture_name}",
  );

  let html_content = format!(
    r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <title>{fixture_name}</title>
    <base href="../../../" />
    <link rel="stylesheet" href="takumi/tests/shared.css" />
  </head>
  <body style="width: {viewport_width}px; height: {viewport_height}px">
    {node_html}
  </body>
</html>
"#
  );
  let html_path = generated_path(&format!("{fixture_name}.html"));

  fs::write(&html_path, html_content).unwrap();
  format_generated(&html_path);
  write_goldens(options, fixture_name).unwrap();
}

/// Writes the raster golden and, when the SVG backend can draw the fixture, the
/// vector one. The SVG backend does not cover every paint feature yet.
pub fn write_goldens(options: RenderOptions<'_>, fixture_name: &str) -> Result<(), String> {
  if let Ok(svg) = svg_render(
    SvgOptions::builder()
      .node(options.node().clone())
      .viewport(*options.viewport())
      .fonts(options.fonts())
      .stylesheet(options.stylesheet().clone())
      .images(options.images().clone())
      .build(),
  ) {
    fs::write(generated_path(&format!("{fixture_name}.svg")), svg)
      .map_err(|error| error.to_string())?;
  }

  let image = render(options).map_err(|error| format!("render: {error:?}"))?;
  let mut file = File::create(generated_path(&format!("{fixture_name}.webp")))
    .map_err(|error| error.to_string())?;

  write_image(&image, &mut file, OutputFormat::WebPLossless).map_err(|error| format!("{error:?}"))
}

/// Runs the repo's formatter over a generated fixture, so a test run leaves the
/// tree the way `bun lint` wants it. A checkout without `node_modules` skips.
pub fn format_generated(path: impl AsRef<Path>) {
  let path = path.as_ref();
  let binary = if cfg!(windows) { "oxfmt.exe" } else { "oxfmt" };
  let oxfmt = repo_base_path(&format!("node_modules/.bin/{binary}"));

  if !oxfmt.exists() {
    return;
  }

  let status = Command::new(&oxfmt)
    .arg(path)
    .status()
    .unwrap_or_else(|error| panic!("{} should run: {error}", oxfmt.display()));

  assert!(
    status.success(),
    "{} rejected {}",
    oxfmt.display(),
    path.display()
  );
}

pub(crate) fn run_animation_fixture_test<Frames: IntoAnimationFixtureFrames>(
  frames: Frames,
  fixture_id: &str,
  duration_ms: u32,
  fps: u32,
) {
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
    let mut file = File::create(generated_path(&format!("{fixture_id}.{extension}"))).unwrap();

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

pub(crate) trait IntoAnimationFixtureFrames {
  fn into_frames(self, frame_duration_ms: u32) -> Vec<AnimationFrame>;
}

impl IntoAnimationFixtureFrames for Vec<AnimationFrame> {
  fn into_frames(self, _: u32) -> Vec<AnimationFrame> {
    self
  }
}

impl IntoAnimationFixtureFrames for Vec<Node> {
  fn into_frames(self, frame_duration_ms: u32) -> Vec<AnimationFrame> {
    let viewport = create_test_viewport();
    let options: Vec<_> = self
      .into_iter()
      .enumerate()
      .map(|(index, node)| {
        let time_ms = (index as u64) * u64::from(frame_duration_ms);

        RenderOptions::builder()
          .viewport(viewport)
          .node(node)
          .time_ms(time_ms)
          .fonts(&CONTEXT)
          .build()
      })
      .collect();

    options
      .into_par_iter()
      .map(|options| AnimationFrame::new(render(options).unwrap(), frame_duration_ms))
      .collect()
  }
}

fn create_test_context() -> Fonts {
  let mut context = Fonts::default();

  for (font, name, generic) in TEST_FONTS {
    let font_data = fs::read(repo_base_path(font)).unwrap();

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
