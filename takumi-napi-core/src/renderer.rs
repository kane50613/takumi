use napi::bindgen_prelude::*;
use napi_derive::napi;
use takumi::{GlobalContext, ImageStore, Viewport, rendering::ImageOutputFormat};

use crate::{
  load_font_task::LoadFontTask, put_persistent_image_task::PutPersistentImageTask,
  render_task::RenderTask,
};

#[napi]
#[derive(Default)]
pub struct Renderer(GlobalContext);

#[napi(object)]
pub struct RenderOptions {
  pub width: u32,
  pub height: u32,
  pub format: Option<OutputFormat>,
  pub quality: Option<u8>,
}

#[napi(string_enum)]
pub enum OutputFormat {
  WebP,
  Png,
  Jpeg,
}

impl From<OutputFormat> for ImageOutputFormat {
  fn from(format: OutputFormat) -> Self {
    match format {
      OutputFormat::Png => ImageOutputFormat::Png,
      OutputFormat::Jpeg => ImageOutputFormat::Jpeg,
      OutputFormat::WebP => ImageOutputFormat::WebP,
    }
  }
}

#[napi(object)]
#[derive(Default)]
pub struct ConstructRendererOptions {
  pub debug: Option<bool>,
}

#[napi]
impl Renderer {
  #[napi(constructor)]
  pub fn new(options: Option<ConstructRendererOptions>) -> Self {
    let options = options.unwrap_or_default();

    Self(GlobalContext {
      draw_debug_border: options.debug.unwrap_or_default(),
      ..Default::default()
    })
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn put_persistent_image_async(
    &self,
    src: String,
    data: ArrayBuffer,
    signal: Option<AbortSignal>,
  ) -> AsyncTask<PutPersistentImageTask> {
    AsyncTask::with_optional_signal(
      PutPersistentImageTask {
        src: Some(src),
        context: &self.0,
        buffer: data.to_vec(),
      },
      signal,
    )
  }

  #[napi(ts_return_type = "Promise<number>")]
  pub fn load_font_async(
    &self,
    data: ArrayBuffer,
    signal: Option<AbortSignal>,
  ) -> AsyncTask<LoadFontTask> {
    AsyncTask::with_optional_signal(
      LoadFontTask {
        context: &self.0,
        buffers: vec![data.to_vec()],
      },
      signal,
    )
  }

  #[napi(ts_return_type = "Promise<number>")]
  pub fn load_fonts_async(
    &self,
    fonts: Vec<ArrayBuffer>,
    signal: Option<AbortSignal>,
  ) -> AsyncTask<LoadFontTask> {
    AsyncTask::with_optional_signal(
      LoadFontTask {
        context: &self.0,
        buffers: fonts.into_iter().map(|buf| buf.to_vec()).collect(),
      },
      signal,
    )
  }

  #[napi]
  pub fn clear_image_store(&self) {
    self.0.persistent_image_store.clear();
  }

  #[napi(
    ts_args_type = "source: { type: string }, options: RenderOptions, signal?: AbortSignal",
    ts_return_type = "Promise<Buffer>"
  )]
  pub fn render_async(
    &self,
    env: Env,
    source: Object,
    options: RenderOptions,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<RenderTask>> {
    let node = env.from_js_value(source)?;

    Ok(AsyncTask::with_optional_signal(
      RenderTask {
        node: Some(node),
        context: &self.0,
        viewport: Viewport::new(options.width, options.height),
        format: options.format.unwrap_or(OutputFormat::Png).into(),
        quality: options.quality,
      },
      signal,
    ))
  }
}
