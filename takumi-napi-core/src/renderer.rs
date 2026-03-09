use std::{
  borrow::Cow,
  collections::HashSet,
  sync::{Arc, RwLock},
};

use napi::bindgen_prelude::*;
use napi_derive::napi;
use takumi::{
  GlobalContext,
  layout::{node::NodeKind, style::KeyframesRule as CoreKeyframesRule},
  parley::{FontWeight, GenericFamily, fontique::FontInfoOverride},
  rendering::{DitheringAlgorithm as CoreDitheringAlgorithm, ImageOutputFormat},
  resources::image::load_image_source_from_bytes,
};
use xxhash_rust::xxh3::Xxh3DefaultBuilder;

use crate::{
  De, FontInput, buffer_from_object, buffer_slice_from_object, deserialize_with_tracing,
  encode_frames_task::EncodeFramesTask, load_font_task::LoadFontTask, map_error,
  measure_task::MeasureTask, put_persistent_image_task::PutPersistentImageTask,
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

impl From<takumi::rendering::MeasuredTextRun> for MeasuredTextRun {
  fn from(run: takumi::rendering::MeasuredTextRun) -> Self {
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

impl From<takumi::rendering::MeasuredNode> for MeasuredNode {
  fn from(node: takumi::rendering::MeasuredNode) -> Self {
    Self {
      width: node.width as f64,
      height: node.height as f64,
      transform: node.transform.iter().map(|&x| x as f64).collect(),
      children: node.children.into_iter().map(Into::into).collect(),
      runs: node.runs.into_iter().map(Into::into).collect(),
    }
  }
}

#[derive(PartialEq, Eq, Hash)]
pub(crate) struct ImageCacheKey {
  pub src: Box<str>,
  pub data_hash: u64,
}

/// The main renderer for Takumi image rendering engine (Node.js version).
#[napi]
pub struct Renderer {
  pub(crate) state: Arc<RwLock<RendererState>>,
}

pub(crate) struct RendererState {
  pub(crate) global: GlobalContext,
  pub(crate) persistent_image_cache: HashSet<ImageCacheKey, Xxh3DefaultBuilder>,
}

pub(crate) fn deserialize_keyframes(keyframes: Option<Object>) -> Result<Vec<CoreKeyframesRule>> {
  match keyframes {
    Some(keyframes) => {
      let mut deserializer = De::new(&keyframes);
      takumi::keyframes::deserialize_keyframes(&mut deserializer)
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
  /// The quality of JPEG format (0-100).
  pub quality: Option<u8>,
  /// Whether to draw debug borders.
  pub draw_debug_border: Option<bool>,
  /// The fetched resources to use.
  pub fetched_resources: Option<Vec<ImageSource<'env>>>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// Structured keyframes to register alongside stylesheets.
  #[napi(
    ts_type = "{ name: string; keyframes: { offsets: number[]; declarations: Record<string, unknown> }[] }[] | Keyframes"
  )]
  pub keyframes: Option<Object<'env>>,
  /// The device pixel ratio.
  /// @default 1.0
  pub device_pixel_ratio: Option<f64>,
  /// The animation timeline time in milliseconds.
  pub time_ms: Option<i64>,
  /// The output dithering algorithm.
  pub dithering: Option<DitheringAlgorithm>,
}

#[napi(string_enum)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DitheringAlgorithm {
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
  #[napi(ts_type = "AnyNode")]
  pub node: Object<'ctx>,
  /// The duration of this frame in milliseconds.
  pub duration_ms: u32,
}

/// Represents a single scene in a sequential animation timeline.
#[napi(object)]
pub struct AnimationSceneSource<'ctx> {
  /// The node tree to render for this scene.
  #[napi(ts_type = "AnyNode")]
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
  /// The quality of WebP format (0-100). Ignored for APNG and GIF.
  pub quality: Option<u8>,
  /// Frames per second for timeline sampling.
  pub fps: u32,
  /// The fetched resources to use.
  pub fetched_resources: Option<Vec<ImageSource<'env>>>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// The device pixel ratio.
  /// @default 1.0
  pub device_pixel_ratio: Option<f64>,
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
  /// The quality of WebP format (0-100). Ignored for APNG and GIF.
  pub quality: Option<u8>,
  /// The fetched resources to use.
  pub fetched_resources: Option<Vec<ImageSource<'env>>>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// The device pixel ratio.
  /// @default 1.0
  pub device_pixel_ratio: Option<f64>,
}

/// Output format for animated images.
#[napi(string_enum)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AnimationOutputFormat {
  /// Animated WebP format.
  webp,
  /// Animated PNG format.
  apng,
  /// Animated GIF format.
  gif,
}

/// Output format for static images.
#[napi(string_enum)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
  /// WebP format.
  webp,
  /// PNG format.
  png,
  /// JPEG format.
  jpeg,
  /// @deprecated Use lowercase `webp` instead, may be removed in the future
  WebP,
  /// @deprecated Use lowercase `jpeg` instead, may be removed in the future
  Jpeg,
  /// @deprecated Use lowercase `png` instead, may be removed in the future
  Png,
  /// Raw pixels format.
  raw,
}

impl From<OutputFormat> for ImageOutputFormat {
  fn from(format: OutputFormat) -> Self {
    match format {
      OutputFormat::WebP | OutputFormat::webp => ImageOutputFormat::WebP,
      OutputFormat::Jpeg | OutputFormat::jpeg => ImageOutputFormat::Jpeg,
      OutputFormat::Png | OutputFormat::png => ImageOutputFormat::Png,
      // SAFETY: It's handled in the render task
      OutputFormat::raw => unreachable!(),
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
}

/// Options for constructing a Renderer instance.
#[napi(object)]
#[derive(Default)]
pub struct ConstructRendererOptions<'ctx> {
  /// The images that needs to be preloaded into the renderer.
  pub persistent_images: Option<Vec<ImageSource<'ctx>>>,
  /// The fonts being used.
  #[napi(ts_type = "Font[] | undefined")]
  pub fonts: Option<Vec<Object<'ctx>>>,
  /// Whether to load the default fonts.
  /// If `fonts` are provided, this will be `false` by default.
  pub load_default_fonts: Option<bool>,
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

#[napi]
impl Renderer {
  /// Creates a new Renderer instance.
  #[napi(constructor)]
  pub fn new(env: Env, options: Option<ConstructRendererOptions>) -> Result<Self> {
    let options = options.unwrap_or_default();

    let load_default_fonts = options
      .load_default_fonts
      .unwrap_or_else(|| options.fonts.is_none());

    let mut global = GlobalContext::default();

    if load_default_fonts {
      for (font, name, generic) in EMBEDDED_FONTS {
        global
          .font_context
          .load_and_store(
            Cow::Borrowed(font),
            Some(FontInfoOverride {
              family_name: Some(name),
              ..Default::default()
            }),
            Some(*generic),
          )
          .map_err(map_error)?;
      }
    }

    let renderer = Self {
      state: Arc::new(RwLock::new(RendererState {
        global,
        persistent_image_cache: HashSet::default(),
      })),
    };

    if let Some(fonts) = options.fonts {
      for font in fonts {
        renderer.load_font_sync(env, font)?;
      }
    }

    if let Some(images) = options.persistent_images {
      let state = renderer
        .state
        .write()
        .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;
      for image in images {
        let buffer = buffer_slice_from_object(env, image.data)?;
        let image_source = load_image_source_from_bytes(&buffer).map_err(map_error)?;

        state
          .global
          .persistent_image_store
          .insert(image.src, image_source);
      }
    }

    Ok(renderer)
  }

  /// @deprecated This function does nothing.
  #[napi]
  pub fn purge_resources_cache(&self) {}

  /// @deprecated This function does nothing.
  #[napi]
  pub fn purge_font_cache(&self) {}

  /// @deprecated Use `putPersistentImage` instead (to align with the naming convention for sync/async functions).
  #[napi(
    ts_args_type = "src: string, data: Uint8Array | ArrayBuffer, signal?: AbortSignal",
    ts_return_type = "Promise<void>"
  )]
  pub fn put_persistent_image_async(
    &self,
    env: Env,
    src: String,
    data: Object,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<PutPersistentImageTask>> {
    self.put_persistent_image(env, src, data, signal)
  }

  /// Puts a persistent image into the renderer's internal store asynchronously.
  #[napi(
    ts_args_type = "src: string, data: Uint8Array | ArrayBuffer, signal?: AbortSignal",
    ts_return_type = "Promise<void>"
  )]
  pub fn put_persistent_image(
    &self,
    env: Env,
    src: String,
    data: Object,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<PutPersistentImageTask>> {
    let buffer = buffer_from_object(env, data)?;

    Ok(AsyncTask::with_optional_signal(
      PutPersistentImageTask {
        src: Some(src),
        state: Arc::clone(&self.state),
        buffer,
      },
      signal,
    ))
  }

  /// Loads a font synchronously.
  #[napi(ts_args_type = "font: Font")]
  pub fn load_font_sync(&self, env: Env, font: Object) -> Result<()> {
    let mut state = self
      .state
      .write()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;
    if let Ok(buffer) = buffer_slice_from_object(env, font) {
      state
        .global
        .font_context
        .load_and_store(Cow::Borrowed(&buffer), None, None)
        .map_err(map_error)?;

      return Ok(());
    }

    let buffer = font
      .get_named_property("data")
      .and_then(|buffer| buffer_slice_from_object(env, buffer))?;
    let font: FontInput = deserialize_with_tracing(font)?;

    let font_override = FontInfoOverride {
      family_name: font.name.as_deref(),
      style: font.style.map(|style| style.0),
      weight: font.weight.map(|weight| FontWeight::new(weight as f32)),
      axes: None,
      width: None,
    };

    state
      .global
      .font_context
      .load_and_store(Cow::Borrowed(&buffer), Some(font_override), None)
      .map_err(map_error)?;

    Ok(())
  }

  /// @deprecated Use `loadFont` instead (to align with the naming convention for sync/async functions).
  #[napi(
    ts_args_type = "data: Font, signal?: AbortSignal",
    ts_return_type = "Promise<number>"
  )]
  pub fn load_font_async(
    &self,
    env: Env,
    data: Object,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask>> {
    self.load_fonts(env, vec![data], signal)
  }

  /// Loads a font into the renderer asynchronously.
  #[napi(
    ts_args_type = "data: Font, signal?: AbortSignal",
    ts_return_type = "Promise<number>"
  )]
  pub fn load_font(
    &self,
    env: Env,
    data: Object,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask>> {
    self.load_fonts(env, vec![data], signal)
  }

  /// @deprecated Use `loadFonts` instead (to align with the naming convention for sync/async functions).
  #[napi(
    ts_args_type = "fonts: Font[], signal?: AbortSignal",
    ts_return_type = "Promise<number>"
  )]
  pub fn load_fonts_async(
    &self,
    env: Env,
    fonts: Vec<Object>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask>> {
    self.load_fonts(env, fonts, signal)
  }

  /// Loads multiple fonts into the renderer asynchronously.
  #[napi(
    ts_args_type = "fonts: Font[], signal?: AbortSignal",
    ts_return_type = "Promise<number>"
  )]
  pub fn load_fonts(
    &self,
    env: Env,
    fonts: Vec<Object>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask>> {
    let buffers = fonts
      .into_iter()
      .map(|font| {
        if let Ok(buffer) = buffer_from_object(env, font) {
          Ok((FontInput::default(), buffer))
        } else {
          let buffer = font
            .get_named_property("data")
            .and_then(|buffer| buffer_from_object(env, buffer))?;
          let font: FontInput = deserialize_with_tracing(font).map_err(map_error)?;

          Ok((font, buffer))
        }
      })
      .collect::<Result<Vec<_>>>()?;

    Ok(AsyncTask::with_optional_signal(
      LoadFontTask {
        state: Arc::clone(&self.state),
        buffers,
      },
      signal,
    ))
  }

  /// Clears the renderer's internal image store.
  #[napi]
  pub fn clear_image_store(&self) {
    if let Ok(state) = self.state.write() {
      state.global.persistent_image_store.clear();
    }
  }

  /// Renders a node tree into an image buffer asynchronously.
  #[napi(
    ts_args_type = "source: AnyNode, options?: RenderOptions, signal?: AbortSignal",
    ts_return_type = "Promise<Buffer>"
  )]
  pub fn render(
    &self,
    env: Env,
    source: Object,
    options: Option<RenderOptions>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<RenderTask>> {
    let node: NodeKind = deserialize_with_tracing(source)?;

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

  /// @deprecated Use `render` instead (to align with the naming convention for sync/async functions).
  #[napi(
    ts_args_type = "source: AnyNode, options?: RenderOptions, signal?: AbortSignal",
    ts_return_type = "Promise<Buffer>"
  )]
  pub fn render_async(
    &self,
    env: Env,
    source: Object,
    options: Option<RenderOptions>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<RenderTask>> {
    self.render(env, source, options, signal)
  }

  /// Measures a node tree and returns layout information asynchronously.
  #[napi(
    ts_args_type = "source: AnyNode, options?: RenderOptions, signal?: AbortSignal",
    ts_return_type = "Promise<MeasuredNode>"
  )]
  pub fn measure(
    &self,
    env: Env,
    source: Object,
    options: Option<RenderOptions>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<MeasureTask>> {
    let node: NodeKind = deserialize_with_tracing(source)?;

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
