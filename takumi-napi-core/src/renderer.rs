use std::{
  collections::HashMap,
  sync::{Arc, OnceLock, RwLock},
};

use napi::bindgen_prelude::*;
use napi_derive::napi;
use parley::{GenericFamily, fontique::FontInfoOverride};
use rayon::prelude::*;
use takumi_core::{
  Fonts,
  layout::{node::Node, style::KeyframesRule as CoreKeyframesRule},
  resources::{
    font::FontResource,
    image::{ImageCache, ImageCacheMode as CoreImageCacheMode, ImageSource as LoadedImageSource},
  },
};
use takumi_raster::{
  DitheringAlgorithm as CoreDitheringAlgorithm, OutputFormat as RasterOutputFormat, Quality,
};

use crate::{
  De, deserialize_with_tracing, encode_frames_task::EncodeFramesTask, load_font_task::LoadFontTask,
  map_error, measure_task::MeasureTask, parse_font_input,
  render_animation_task::RenderAnimationTask, render_task::RenderTask,
};

/// Represents a single run of text in a measured node.
#[napi(object)]
pub struct MeasuredTextRun {
  /// The text content of the run.
  pub text: String,
  /// The inline x-coordinate of the run.
  pub x: f64,
  /// The inline y-coordinate of the run.
  pub y: f64,
  /// The width of the run.
  pub width: f64,
  /// The height of the run.
  pub height: f64,
}

impl From<takumi_raster::MeasuredTextRun> for MeasuredTextRun {
  fn from(run: takumi_raster::MeasuredTextRun) -> Self {
    Self {
      text: run.text,
      x: run.x as f64,
      y: run.y as f64,
      width: run.width as f64,
      height: run.height as f64,
    }
  }
}

/// Represents a node that has been measured, including its layout information.
#[napi(object)]
pub struct MeasuredNode {
  /// The measured width of the node.
  pub width: f64,
  /// The measured height of the node.
  pub height: f64,
  /// The transformation matrix of the node.
  #[napi(ts_type = "[number, number, number, number, number, number]")]
  pub transform: Vec<f64>,
  /// The children of the node.
  pub children: Vec<MeasuredNode>,
  /// The text runs within the node.
  pub runs: Vec<MeasuredTextRun>,
}

impl From<takumi_raster::MeasuredNode> for MeasuredNode {
  fn from(node: takumi_raster::MeasuredNode) -> Self {
    Self {
      width: node.width as f64,
      height: node.height as f64,
      transform: node.transform.iter().map(|&x| x as f64).collect(),
      children: node.children.into_iter().map(Into::into).collect(),
      runs: node.runs.into_iter().map(Into::into).collect(),
    }
  }
}

/// The main renderer for Takumi image rendering engine (Node.js version).
#[napi]
pub struct Renderer {
  pub(crate) state: Arc<RwLock<RendererState>>,
}

pub(crate) struct RendererState {
  pub(crate) fonts: Fonts,
  pub(crate) image_cache: ImageCache,
}

impl RendererState {
  /// Decodes the per-call image buffers into a `src`-keyed map.
  pub(crate) fn decode_images(
    &self,
    images: HashMap<Arc<str>, (Buffer, ImageCacheMode)>,
  ) -> Result<HashMap<Arc<str>, LoadedImageSource>> {
    let mut map = HashMap::new();

    for (src, (buffer, mode)) in images {
      let decoded = self
        .image_cache
        .get_or_decode(&buffer, mode.into())
        .map_err(map_error)?;

      map.insert(src, decoded);
    }

    Ok(map)
  }
}

pub(crate) fn deserialize_keyframes(keyframes: Option<Object>) -> Result<Vec<CoreKeyframesRule>> {
  match keyframes {
    Some(keyframes) => {
      let mut deserializer = De::new(&keyframes);
      takumi_core::keyframes::deserialize_keyframes(&mut deserializer)
        .map_err(|error: napi::Error| Error::from_reason(error.to_string()))
    }
    None => Ok(Vec::new()),
  }
}

/// Options for rendering an image.
#[napi(object)]
#[derive(Default)]
pub struct RenderOptions<'env> {
  /// The width of the image. If not provided, the width will be automatically calculated based on the content.
  pub width: Option<u32>,
  /// The height of the image. If not provided, the height will be automatically calculated based on the content.
  pub height: Option<u32>,
  /// The format of the image.
  pub format: Option<OutputFormat>,
  /// The quality of lossy formats (0-100). For JPEG; for WebP it selects lossy
  /// encoding unless `lossless` is set.
  pub quality: Option<u8>,
  /// Encode WebP losslessly. Defaults to lossless when neither `quality` nor
  /// `lossless` is given.
  pub lossless: Option<bool>,
  /// Whether to draw debug borders.
  pub draw_debug_border: Option<bool>,
  /// Images keyed by `src`, each carrying raw bytes.
  pub images: Option<Vec<ImageSource<'env>>>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// Structured keyframes to register alongside stylesheets.
  #[napi(ts_type = "Keyframes")]
  pub keyframes: Option<Object<'env>>,
  /// The device pixel ratio.
  /// @default 1.0
  pub device_pixel_ratio: Option<f64>,
  /// The animation timeline time in milliseconds.
  pub time_ms: Option<i64>,
  /// The output dithering algorithm.
  pub dithering: Option<DitheringAlgorithm>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  /// Defaults to all registered families in registration order.
  pub font_families: Option<Vec<String>>,
}

#[napi(string_enum)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DitheringAlgorithm {
  #[napi(value = "none")]
  None,
  #[napi(value = "ordered-bayer")]
  OrderedBayer,
  #[napi(value = "floyd-steinberg")]
  FloydSteinberg,
}

impl From<DitheringAlgorithm> for CoreDitheringAlgorithm {
  fn from(dithering: DitheringAlgorithm) -> Self {
    match dithering {
      DitheringAlgorithm::None => Self::None,
      DitheringAlgorithm::OrderedBayer => Self::OrderedBayer,
      DitheringAlgorithm::FloydSteinberg => Self::FloydSteinberg,
    }
  }
}

/// Represents a single frame in a precomputed animation sequence.
#[napi(object)]
pub struct AnimationFrameSource<'ctx> {
  /// The node tree to render for this frame.
  #[napi(ts_type = "Node")]
  pub node: Object<'ctx>,
  /// The duration of this frame in milliseconds.
  pub duration_ms: u32,
}

/// Represents a single scene in a sequential animation timeline.
#[napi(object)]
pub struct AnimationSceneSource<'ctx> {
  /// The node tree to render for this scene.
  #[napi(ts_type = "Node")]
  pub node: Object<'ctx>,
  /// The duration of this scene in milliseconds.
  pub duration_ms: u32,
}

/// Options for rendering a sequential scene animation.
#[napi(object)]
pub struct RenderAnimationOptions<'env> {
  /// The scenes to render sequentially.
  pub scenes: Vec<AnimationSceneSource<'env>>,
  /// Whether to draw debug borders around layout elements.
  pub draw_debug_border: Option<bool>,
  /// The width of each frame in pixels.
  pub width: u32,
  /// The height of each frame in pixels.
  pub height: u32,
  /// The output animation format (WebP, APNG, or GIF).
  pub format: Option<AnimationOutputFormat>,
  /// The quality of lossy WebP (0-100). Ignored for APNG and GIF, and when
  /// `lossless` is set.
  pub quality: Option<u8>,
  /// Encode WebP losslessly. Defaults to lossless when neither `quality` nor
  /// `lossless` is given. Ignored for APNG and GIF.
  pub lossless: Option<bool>,
  /// Frames per second for timeline sampling.
  pub fps: u32,
  /// Images keyed by `src`, each carrying raw bytes.
  pub images: Option<Vec<ImageSource<'env>>>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// The device pixel ratio.
  /// @default 1.0
  pub device_pixel_ratio: Option<f64>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  /// Defaults to all registered families in registration order.
  pub font_families: Option<Vec<String>>,
}

/// Options for encoding a precomputed frame sequence.
#[napi(object)]
pub struct EncodeFramesOptions<'env> {
  /// Whether to draw debug borders around layout elements.
  pub draw_debug_border: Option<bool>,
  /// The width of each frame in pixels.
  pub width: u32,
  /// The height of each frame in pixels.
  pub height: u32,
  /// The output animation format (WebP, APNG, or GIF).
  pub format: Option<AnimationOutputFormat>,
  /// The quality of lossy WebP (0-100). Ignored for APNG and GIF, and when
  /// `lossless` is set.
  pub quality: Option<u8>,
  /// Encode WebP losslessly. Defaults to lossless when neither `quality` nor
  /// `lossless` is given. Ignored for APNG and GIF.
  pub lossless: Option<bool>,
  /// Images keyed by `src`, each carrying raw bytes.
  pub images: Option<Vec<ImageSource<'env>>>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// The device pixel ratio.
  /// @default 1.0
  pub device_pixel_ratio: Option<f64>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  /// Defaults to all registered families in registration order.
  pub font_families: Option<Vec<String>>,
}

/// Output format for animated images.
#[napi(string_enum = "lowercase")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AnimationOutputFormat {
  /// Animated WebP format.
  WebP,
  /// Animated PNG format.
  Apng,
  /// Animated GIF format.
  Gif,
}

/// Output format for static images.
#[napi(string_enum = "lowercase")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
  /// WebP format.
  WebP,
  /// PNG format.
  Png,
  /// JPEG format.
  Jpeg,
  /// ICO format.
  Ico,
  /// Raw pixels format.
  Raw,
}

impl OutputFormat {
  /// Maps to a raster [`RasterOutputFormat`]. WebP is lossless unless a `quality`
  /// is supplied with `lossless` unset; JPEG folds `quality` (default 75).
  pub(crate) fn into_image_output_format(
    self,
    quality: Option<u8>,
    lossless: Option<bool>,
  ) -> RasterOutputFormat {
    match self {
      OutputFormat::WebP if webp_lossless(quality, lossless) => RasterOutputFormat::WebPLossless,
      OutputFormat::WebP => RasterOutputFormat::WebP {
        quality: quality.map_or_else(Quality::default, Quality::new),
      },
      OutputFormat::Jpeg => RasterOutputFormat::Jpeg {
        quality: quality.map_or_else(Quality::default, Quality::new),
      },
      OutputFormat::Png => RasterOutputFormat::Png,
      OutputFormat::Ico => RasterOutputFormat::Ico,
      // SAFETY: It's handled in the render task
      OutputFormat::Raw => unreachable!(),
    }
  }
}

/// WebP is lossless when explicitly requested or when no `quality` is given.
pub(crate) fn webp_lossless(quality: Option<u8>, lossless: Option<bool>) -> bool {
  lossless.unwrap_or(quality.is_none())
}

/// Cache policy for a decoded image. Defaults to `"auto"`.
#[napi(string_enum = "lowercase")]
#[derive(Clone, Copy, Default)]
pub enum ImageCacheMode {
  /// Cache the decoded image for reuse (evictable).
  #[default]
  Auto,
  /// Skip the decoded-image cache.
  None,
}

impl From<ImageCacheMode> for CoreImageCacheMode {
  fn from(mode: ImageCacheMode) -> Self {
    match mode {
      ImageCacheMode::Auto => Self::Auto,
      ImageCacheMode::None => Self::None,
    }
  }
}

/// An image source with its URL and raw data.
#[napi(object)]
pub struct ImageSource<'ctx> {
  /// The source URL of the image.
  pub src: String,
  /// The raw image data (Uint8Array or ArrayBuffer).
  #[napi(ts_type = "Uint8Array | ArrayBuffer")]
  pub data: Object<'ctx>,
  /// Cache policy for the decoded image. Defaults to `"auto"`.
  pub cache: Option<ImageCacheMode>,
}

const EMBEDDED_FONTS: &[(&[u8], &str, GenericFamily)] = &[
  (
    include_bytes!("../../assets/fonts/geist/Geist[wght].woff2"),
    "Geist",
    GenericFamily::SansSerif,
  ),
  (
    include_bytes!("../../assets/fonts/geist/GeistMono[wght].woff2"),
    "Geist Mono",
    GenericFamily::Monospace,
  ),
];

static DEFAULT_FONTS: OnceLock<Fonts> = OnceLock::new();

/// Returns a clone of the process-wide default font set, decoding the embedded
/// fonts once and sharing the decoded blobs across every renderer.
fn default_fonts() -> Result<Fonts> {
  if let Some(fonts) = DEFAULT_FONTS.get() {
    return Ok(fonts.clone());
  }

  let mut fonts = Fonts::default();
  let resources = crate::pool::install(|| {
    EMBEDDED_FONTS
      .par_iter()
      .map(|(font, name, generic)| {
        FontResource::new(*font)
          .override_info(FontInfoOverride {
            family_name: Some(*name),
            ..Default::default()
          })
          .generic_family(*generic)
          .into_resolved()
          .map_err(|e| Error::from_reason(format!("Failed to load default font: {e}")))
      })
      .collect::<Result<Vec<_>>>()
  })?;

  for resource in resources {
    drop(fonts.register(resource).map_err(map_error)?);
  }

  if DEFAULT_FONTS.set(fonts.clone()).is_err()
    && let Some(stored) = DEFAULT_FONTS.get()
  {
    return Ok(stored.clone());
  }

  Ok(fonts)
}

#[napi]
impl Renderer {
  /// Creates a new Renderer instance.
  #[napi(constructor)]
  pub fn new(env: Env) -> Result<Self> {
    crate::pool::register_cleanup(&env);

    Ok(Self {
      state: Arc::new(RwLock::new(RendererState {
        fonts: default_fonts()?,
        image_cache: ImageCache::default(),
      })),
    })
  }

  /// Register font into the renderer, returning the families produced.
  #[napi(
    js_name = "registerFont",
    ts_args_type = "fonts: Font, signal?: AbortSignal",
    ts_return_type = "Promise<RegisteredFamily[]>"
  )]
  pub fn register_font(
    &self,
    env: Env,
    font: Object,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask>> {
    let (info, buffer) = parse_font_input(env, font)?;

    Ok(AsyncTask::with_optional_signal(
      LoadFontTask {
        state: Arc::clone(&self.state),
        buffer,
        info,
      },
      signal,
    ))
  }

  /// Renders a node tree into an image buffer asynchronously.
  #[napi(
    ts_args_type = "source: Node, options?: RenderOptions, signal?: AbortSignal",
    ts_return_type = "Promise<Buffer>"
  )]
  pub fn render(
    &self,
    env: Env,
    source: Object,
    options: Option<RenderOptions>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<RenderTask>> {
    let node: Node = deserialize_with_tracing(source)?;

    Ok(AsyncTask::with_optional_signal(
      RenderTask::from_options(
        env,
        node,
        options.unwrap_or_default(),
        Arc::clone(&self.state),
      )?,
      signal,
    ))
  }

  /// Measures a node tree and returns layout information asynchronously.
  #[napi(
    ts_args_type = "source: Node, options?: RenderOptions, signal?: AbortSignal",
    ts_return_type = "Promise<MeasuredNode>"
  )]
  pub fn measure(
    &self,
    env: Env,
    source: Object,
    options: Option<RenderOptions>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<MeasureTask>> {
    let node: Node = deserialize_with_tracing(source)?;

    Ok(AsyncTask::with_optional_signal(
      MeasureTask::from_options(
        env,
        node,
        options.unwrap_or_default(),
        Arc::clone(&self.state),
      )?,
      signal,
    ))
  }

  /// Renders a sequential scene animation into a buffer asynchronously.
  #[napi(
    ts_args_type = "options: RenderAnimationOptions, signal?: AbortSignal",
    ts_return_type = "Promise<Buffer>"
  )]
  pub fn render_animation(
    &self,
    env: Env,
    options: RenderAnimationOptions,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<RenderAnimationTask>> {
    Ok(AsyncTask::with_optional_signal(
      RenderAnimationTask::from_options(env, options, Arc::clone(&self.state))?,
      signal,
    ))
  }

  /// Encodes a precomputed frame sequence into an animated image buffer asynchronously.
  #[napi(
    ts_args_type = "source: AnimationFrameSource[], options: EncodeFramesOptions, signal?: AbortSignal",
    ts_return_type = "Promise<Buffer>"
  )]
  pub fn encode_frames(
    &self,
    env: Env,
    source: Vec<AnimationFrameSource>,
    options: EncodeFramesOptions,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<EncodeFramesTask>> {
    let frames = source
      .into_iter()
      .map(|frame| Ok((deserialize_with_tracing(frame.node)?, frame.duration_ms)))
      .collect::<Result<Vec<_>>>()?;

    Ok(AsyncTask::with_optional_signal(
      EncodeFramesTask::from_options(env, frames, options, Arc::clone(&self.state))?,
      signal,
    ))
  }
}
