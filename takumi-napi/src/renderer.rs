use std::{
  collections::HashMap,
  sync::{Arc, Mutex},
};

use arc_swap::ArcSwap;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use takumi_core::{
  Fonts,
  layout::node::Node,
  resources::image::{
    ImageCacheMode as CoreImageCacheMode, ImageSource as LoadedImageSource, ResourceCache,
  },
  style::{KeyframesRule as CoreKeyframesRule, Lang},
  viewport::DEFAULT_DEVICE_PIXEL_RATIO,
};
use takumi_raster::{
  DitheringAlgorithm as CoreDitheringAlgorithm, OutputFormat as RasterOutputFormat, Quality,
};

use crate::{
  De, JsBytes, deserialize_with_tracing, load_font_task::LoadFontTask, map_error,
  measure_task::MeasureTask, parse_font_input, render_animation_task::RenderAnimationTask,
  render_task::RenderTask, svg_render_task::SvgRenderTask,
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
  pub(crate) state: Arc<RendererState>,
}

pub(crate) struct RendererState {
  /// Wait-free reads via `fonts.load()`; registrations serialize on `font_write` and publish
  /// a fresh `Arc<Fonts>` via `store`. Renders in flight keep their old snapshot alive.
  pub(crate) fonts: ArcSwap<Fonts>,
  pub(crate) font_write: Mutex<()>,
  pub(crate) resource_cache: ResourceCache,
}

/// Decodes the per-call image buffers into a `src`-keyed map. The resource cache is
/// `quick_cache::sync` (internally locked, single-flight), so it needs no outer lock.
pub(crate) fn decode_images(
  resource_cache: &ResourceCache,
  images: HashMap<Arc<str>, (JsBytes, ImageCacheMode)>,
) -> Result<HashMap<Arc<str>, LoadedImageSource>> {
  let mut map = HashMap::new();

  for (src, (buffer, mode)) in images {
    let decoded = resource_cache
      .get_or_decode(buffer.as_ref(), mode.into())
      .map_err(map_error)?;

    map.insert(src, decoded);
  }

  Ok(map)
}

pub(crate) fn collect_images(
  env: Env,
  images: Option<Vec<ImageSource>>,
) -> Result<HashMap<Arc<str>, (JsBytes, ImageCacheMode)>> {
  images
    .unwrap_or_default()
    .into_iter()
    .map(|image| {
      Ok((
        Arc::from(image.src),
        (
          JsBytes::from_object(env, image.data)?,
          image.cache.unwrap_or_default(),
        ),
      ))
    })
    .collect()
}

pub(crate) fn parse_lang(lang: Option<String>) -> Result<Option<Lang>> {
  lang
    .as_deref()
    .map(Lang::parse)
    .transpose()
    .map_err(map_error)
}

pub(crate) fn device_pixel_ratio(ratio: Option<f64>) -> f32 {
  ratio
    .map(|ratio| ratio as f32)
    .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO)
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
  /// Design tokens for the `tw` prop, keyed by CSS custom property name.
  pub theme: Option<HashMap<String, String>>,
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
  /// Default BCP-47 language applied to the root, inherited by nodes without their own lang.
  pub lang: Option<String>,
}

/// Options for rendering a node tree to an SVG document. SVG is a vector
/// format, so the raster-only knobs (`format`, `quality`, `lossless`,
/// `dithering`, `drawDebugBorder`, `devicePixelRatio`) do not apply.
#[napi(object)]
#[derive(Default)]
pub struct SvgRenderOptions<'env> {
  /// The width of the viewport. If not provided, it is derived from content.
  pub width: Option<u32>,
  /// The height of the viewport. If not provided, it is derived from content.
  pub height: Option<u32>,
  /// Images keyed by `src`, each carrying raw bytes.
  pub images: Option<Vec<ImageSource<'env>>>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// Design tokens for the `tw` prop, keyed by CSS custom property name.
  pub theme: Option<HashMap<String, String>>,
  /// Structured keyframes to register alongside stylesheets.
  #[napi(ts_type = "Keyframes")]
  pub keyframes: Option<Object<'env>>,
  /// The animation timeline time in milliseconds.
  pub time_ms: Option<i64>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  /// Defaults to all registered families in registration order.
  pub font_families: Option<Vec<String>>,
  /// Default BCP-47 language applied to the root, inherited by nodes without their own lang.
  pub lang: Option<String>,
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

/// Represents a single scene in a sequential animation timeline.
#[napi(object)]
pub struct AnimationScene<'ctx> {
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
  pub scenes: Vec<AnimationScene<'env>>,
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
  /// Design tokens for the `tw` prop, keyed by CSS custom property name.
  pub theme: Option<HashMap<String, String>>,
  /// Structured keyframes to register alongside stylesheets.
  #[napi(ts_type = "Keyframes")]
  pub keyframes: Option<Object<'env>>,
  /// The device pixel ratio.
  /// @default 1.0
  pub device_pixel_ratio: Option<f64>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  /// Defaults to all registered families in registration order.
  pub font_families: Option<Vec<String>>,
  /// Default BCP-47 language applied to the root, inherited by nodes without their own lang.
  pub lang: Option<String>,
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

/// Options for constructing a [`Renderer`].
#[napi(object)]
#[derive(Default)]
pub struct RendererOptions {
  /// Byte budget shared by every cached resource — decoded images, SVG
  /// rasters, parsed stylesheets. `0` disables caching.
  /// @default 16 MiB
  pub cache_max_bytes: Option<f64>,
}

#[napi]
impl Renderer {
  /// Creates a new Renderer instance.
  #[napi(constructor)]
  pub fn new(env: Env, options: Option<RendererOptions>) -> Result<Self> {
    crate::pool::register_cleanup(&env);

    Ok(Self {
      state: Arc::new(RendererState {
        fonts: ArcSwap::from_pointee(takumi_bindings_common::default_fonts().map_err(map_error)?),
        font_write: Mutex::new(()),
        resource_cache: match options.and_then(|options| options.cache_max_bytes) {
          Some(bytes) => ResourceCache::new(bytes.max(0.0) as u64),
          None => ResourceCache::default(),
        },
      }),
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
    ts_return_type = "Promise<Buffer<ArrayBuffer>>"
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

  /// Renders a node tree into an SVG document string asynchronously.
  #[napi(
    ts_args_type = "source: Node, options?: SvgRenderOptions, signal?: AbortSignal",
    ts_return_type = "Promise<string>"
  )]
  pub fn render_svg(
    &self,
    env: Env,
    source: Object,
    options: Option<SvgRenderOptions>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<SvgRenderTask>> {
    let node: Node = deserialize_with_tracing(source)?;

    Ok(AsyncTask::with_optional_signal(
      SvgRenderTask::from_options(
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
    ts_return_type = "Promise<Buffer<ArrayBuffer>>"
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
}
