use napi::bindgen_prelude::*;
use napi_derive::napi;
use takumi::{
  GlobalContext,
  layout::node::NodeKind,
  parley::{FontWeight, GenericFamily, fontique::FontInfoOverride},
  rendering::ImageOutputFormat,
  resources::image::load_image_source_from_bytes,
};

use crate::{
  FontInput, buffer_from_object, buffer_slice_from_object, deserialize_with_tracing,
  load_font_task::LoadFontTask, map_error, measure_task::MeasureTask,
  put_persistent_image_task::PutPersistentImageTask, render_animation_task::RenderAnimationTask,
  render_task::RenderTask,
};

#[napi(object)]
pub struct MeasuredTextRun {
  pub text: String,
  pub x: f64,
  pub y: f64,
  pub width: f64,
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

#[napi(object)]
pub struct MeasuredNode {
  pub width: f64,
  pub height: f64,
  #[napi(ts_type = "[number, number, number, number, number, number]")]
  pub transform: Vec<f64>,
  pub children: Vec<MeasuredNode>,
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

#[napi]
pub struct Renderer {
  global: GlobalContext,
}

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
  /// The device pixel ratio.
  /// @default 1.0
  pub device_pixel_ratio: Option<f64>,
}

#[napi(object)]
pub struct AnimationFrameSource<'ctx> {
  #[napi(ts_type = "AnyNode")]
  pub node: Object<'ctx>,
  pub duration_ms: u32,
}

#[napi(object)]
pub struct RenderAnimationOptions {
  pub draw_debug_border: Option<bool>,
  pub width: u32,
  pub height: u32,
  pub format: Option<AnimationOutputFormat>,
}

#[napi(string_enum)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AnimationOutputFormat {
  webp,
  apng,
}

#[napi(string_enum)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
  webp,
  png,
  jpeg,
  /// @deprecated Use lowercase `webp` instead, may be removed in the future
  WebP,
  /// @deprecated Use lowercase `jpeg` instead, may be removed in the future
  Jpeg,
  /// @deprecated Use lowercase `png` instead, may be removed in the future
  Png,
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

#[napi(object)]
pub struct ImageSource<'ctx> {
  pub src: String,
  #[napi(ts_type = "Uint8Array | ArrayBuffer")]
  pub data: Object<'ctx>,
}

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
            font,
            Some(FontInfoOverride {
              family_name: Some(name),
              ..Default::default()
            }),
            Some(*generic),
          )
          .map_err(map_error)?;
      }
    }

    if let Some(fonts) = options.fonts {
      for font in fonts {
        if let Ok(buffer) = buffer_slice_from_object(env, font) {
          global
            .font_context
            .load_and_store(&buffer, None, None)
            .map_err(map_error)?;

          continue;
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

        global
          .font_context
          .load_and_store(&buffer, Some(font_override), None)
          .map_err(map_error)?;
      }
    }

    let renderer = Self { global };

    if let Some(images) = options.persistent_images {
      for image in images {
        let buffer = buffer_slice_from_object(env, image.data)?;
        let image_source = load_image_source_from_bytes(&buffer).map_err(map_error)?;

        renderer
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
    &'_ self,
    env: Env,
    src: String,
    data: Object,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<PutPersistentImageTask<'_>>> {
    self.put_persistent_image(env, src, data, signal)
  }

  #[napi(
    ts_args_type = "src: string, data: Uint8Array | ArrayBuffer, signal?: AbortSignal",
    ts_return_type = "Promise<void>"
  )]
  pub fn put_persistent_image(
    &'_ self,
    env: Env,
    src: String,
    data: Object,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<PutPersistentImageTask<'_>>> {
    let buffer = buffer_from_object(env, data)?;

    Ok(AsyncTask::with_optional_signal(
      PutPersistentImageTask {
        src: Some(src),
        store: &self.global.persistent_image_store,
        buffer,
      },
      signal,
    ))
  }

  /// @deprecated Use `loadFont` instead (to align with the naming convention for sync/async functions).
  #[napi(
    ts_args_type = "data: Font, signal?: AbortSignal",
    ts_return_type = "Promise<number>"
  )]
  pub fn load_font_async(
    &'_ mut self,
    env: Env,
    data: Object,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask<'_>>> {
    self.load_fonts(env, vec![data], signal)
  }

  #[napi(
    ts_args_type = "data: Font, signal?: AbortSignal",
    ts_return_type = "Promise<number>"
  )]
  pub fn load_font(
    &'_ mut self,
    env: Env,
    data: Object,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask<'_>>> {
    self.load_fonts(env, vec![data], signal)
  }

  /// @deprecated Use `loadFonts` instead (to align with the naming convention for sync/async functions).
  #[napi(
    ts_args_type = "fonts: Font[], signal?: AbortSignal",
    ts_return_type = "Promise<number>"
  )]
  pub fn load_fonts_async(
    &'_ mut self,
    env: Env,
    fonts: Vec<Object>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask<'_>>> {
    self.load_fonts(env, fonts, signal)
  }

  #[napi(
    ts_args_type = "fonts: Font[], signal?: AbortSignal",
    ts_return_type = "Promise<number>"
  )]
  pub fn load_fonts(
    &'_ mut self,
    env: Env,
    fonts: Vec<Object>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask<'_>>> {
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
        context: &mut self.global,
        buffers,
      },
      signal,
    ))
  }

  #[napi]
  pub fn clear_image_store(&self) {
    self.global.persistent_image_store.clear();
  }

  #[napi(
    ts_args_type = "source: AnyNode, options?: RenderOptions, signal?: AbortSignal",
    ts_return_type = "Promise<Buffer>"
  )]
  pub fn render(
    &'_ self,
    env: Env,
    source: Object,
    options: Option<RenderOptions>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<RenderTask<'_>>> {
    let node: NodeKind = deserialize_with_tracing(source)?;

    Ok(AsyncTask::with_optional_signal(
      RenderTask::from_options(env, node, options.unwrap_or_default(), &self.global)?,
      signal,
    ))
  }

  /// @deprecated Use `render` instead (to align with the naming convention for sync/async functions).
  #[napi(
    ts_args_type = "source: AnyNode, options?: RenderOptions, signal?: AbortSignal",
    ts_return_type = "Promise<Buffer>"
  )]
  pub fn render_async(
    &'_ mut self,
    env: Env,
    source: Object,
    options: Option<RenderOptions>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<RenderTask<'_>>> {
    self.render(env, source, options, signal)
  }

  #[napi(
    ts_args_type = "source: AnyNode, options?: RenderOptions, signal?: AbortSignal",
    ts_return_type = "Promise<MeasuredNode>"
  )]
  pub fn measure(
    &'_ self,
    env: Env,
    source: Object,
    options: Option<RenderOptions>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<MeasureTask<'_>>> {
    let node: NodeKind = deserialize_with_tracing(source)?;

    Ok(AsyncTask::with_optional_signal(
      MeasureTask::from_options(env, node, options.unwrap_or_default(), &self.global)?,
      signal,
    ))
  }

  #[napi(
    ts_args_type = "source: AnimationFrameSource[], options: RenderAnimationOptions, signal?: AbortSignal",
    ts_return_type = "Promise<Buffer>"
  )]
  pub fn render_animation(
    &'_ self,
    source: Vec<AnimationFrameSource>,
    options: RenderAnimationOptions,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<RenderAnimationTask<'_>>> {
    let nodes = source
      .into_iter()
      .map(|frame| Ok((deserialize_with_tracing(frame.node)?, frame.duration_ms)))
      .collect::<Result<Vec<_>>>()?;

    Ok(AsyncTask::with_optional_signal(
      RenderAnimationTask {
        nodes: Some(nodes),
        context: &self.global,
        viewport: (options.width, options.height).into(),
        format: options.format.unwrap_or(AnimationOutputFormat::webp),
        draw_debug_border: options.draw_debug_border.unwrap_or_default(),
      },
      signal,
    ))
  }
}
