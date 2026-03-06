use std::{
  borrow::Cow,
  collections::HashMap,
  sync::{Arc, RwLock},
};

use napi::bindgen_prelude::*;
use rayon::prelude::*;
use takumi::{
  layout::{DEFAULT_DEVICE_PIXEL_RATIO, DEFAULT_FONT_SIZE, Viewport, node::NodeKind},
  rendering::{
    AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame,
    RenderOptionsBuilder, encode_animated_gif, encode_animated_png, encode_animated_webp, render,
  },
  resources::image::load_image_source_from_bytes,
};

use crate::{
  ExternalMemoryAccountable, buffer_from_object, map_error,
  renderer::{AnimationOutputFormat, EncodeFramesOptions, ImageSource, RendererState},
};

pub struct EncodeFramesTask {
  pub frames: Option<Vec<(NodeKind, u32)>>,
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub viewport: Viewport,
  pub format: AnimationOutputFormat,
  pub quality: Option<u8>,
  pub draw_debug_border: bool,
  pub stylesheets: Option<Vec<String>>,
  pub fetched_resources: HashMap<Arc<str>, Buffer>,
}

impl EncodeFramesTask {
  pub(crate) fn from_options(
    env: Env,
    frames: Vec<(NodeKind, u32)>,
    options: EncodeFramesOptions,
    state: Arc<RwLock<RendererState>>,
  ) -> Result<Self> {
    Ok(Self {
      frames: Some(frames),
      state,
      viewport: Viewport {
        width: Some(options.width),
        height: Some(options.height),
        font_size: DEFAULT_FONT_SIZE,
        device_pixel_ratio: options
          .device_pixel_ratio
          .map(|ratio| ratio as f32)
          .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
      },
      format: options.format.unwrap_or(AnimationOutputFormat::webp),
      quality: options.quality,
      draw_debug_border: options.draw_debug_border.unwrap_or_default(),
      stylesheets: options.stylesheets,
      fetched_resources: options
        .fetched_resources
        .unwrap_or_default()
        .into_iter()
        .map(|image: ImageSource<'_>| {
          Ok((Arc::from(image.src), buffer_from_object(env, image.data)?))
        })
        .collect::<Result<_>>()?,
    })
  }
}

impl Task for EncodeFramesTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    const ENCODED_BYTES_PER_PIXEL_ESTIMATE: usize = 1;
    const FRAME_OVERHEAD_BYTES: usize = 128;
    const MAX_PREALLOC: usize = 4 * 1024 * 1024;

    let Some(frames) = self.frames.take() else {
      unreachable!()
    };
    let initialized_images = self
      .fetched_resources
      .iter()
      .map(|(key, value)| {
        Ok((
          key.clone(),
          load_image_source_from_bytes(value).map_err(map_error)?,
        ))
      })
      .collect::<Result<HashMap<_, _>, _>>()?;
    let state = self
      .state
      .read()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    let viewport = self.viewport;
    let draw_debug_border = self.draw_debug_border;
    let stylesheets = self.stylesheets.clone().unwrap_or_default();
    let frames = frames
      .into_par_iter()
      .map(|(node, duration_ms)| {
        Ok(AnimationFrame::new(
          render(
            RenderOptionsBuilder::default()
              .viewport(viewport)
              .fetched_resources(initialized_images.clone())
              .stylesheets(stylesheets.clone())
              .node(node)
              .global(&state.global)
              .draw_debug_border(draw_debug_border)
              .build()
              .map_err(map_error)?,
          )
          .map_err(map_error)?,
          duration_ms,
        ))
      })
      .collect::<Result<Vec<_>, _>>()?;

    let estimated_capacity = if let Some(first) = frames.first() {
      let width = first.image.width() as usize;
      let height = first.image.height() as usize;
      let per_frame_estimate = width
        .saturating_mul(height)
        .saturating_mul(ENCODED_BYTES_PER_PIXEL_ESTIMATE)
        .saturating_add(FRAME_OVERHEAD_BYTES);
      per_frame_estimate
        .saturating_mul(frames.len())
        .saturating_add(44)
        .min(MAX_PREALLOC)
    } else {
      0
    };
    let mut buffer = Vec::with_capacity(estimated_capacity);

    if let Some(quality) = self.quality
      && quality > 100
    {
      return Err(Error::from_reason(format!(
        "Invalid WebP quality {quality}; expected a value in 0..=100"
      )));
    }

    match self.format {
      AnimationOutputFormat::webp => {
        let mut options = AnimatedWebpOptions::default();
        if let Some(quality) = self.quality {
          options.quality = quality;
        }

        encode_animated_webp(Cow::Owned(frames), &mut buffer, options)
          .map_err(|e| Error::from_reason(e.to_string()))?;
      }
      AnimationOutputFormat::apng => {
        encode_animated_png(&frames, &mut buffer, AnimatedPngOptions::default())
          .map_err(|e| Error::from_reason(e.to_string()))?;
      }
      AnimationOutputFormat::gif => {
        encode_animated_gif(
          Cow::Owned(frames),
          &mut buffer,
          AnimatedGifOptions::default(),
        )
        .map_err(|e| Error::from_reason(e.to_string()))?;
      }
    }

    Ok(buffer)
  }

  fn resolve(&mut self, mut env: Env, output: Self::Output) -> Result<Self::JsValue> {
    output.account_external_memory(&mut env)?;
    Ok(output.into())
  }
}
