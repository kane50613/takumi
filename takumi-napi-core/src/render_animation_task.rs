use std::{
  borrow::Cow,
  collections::HashMap,
  sync::{Arc, RwLock},
};

use napi::bindgen_prelude::*;
use takumi::{
  layout::{DEFAULT_DEVICE_PIXEL_RATIO, DEFAULT_FONT_SIZE, Viewport, node::NodeKind},
  rendering::{
    AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, RenderOptionsBuilder,
    SequentialSceneBuilder, encode_animated_gif, encode_animated_png, encode_animated_webp,
    render_sequence_animation,
  },
  resources::image::load_image_source_from_bytes,
};

use crate::{
  ExternalMemoryAccountable, buffer_from_object, map_error,
  renderer::{AnimationOutputFormat, ImageSource, RenderAnimationOptions, RendererState},
};

pub struct RenderAnimationTask {
  pub scenes: Option<Vec<(NodeKind, u32)>>,
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub viewport: Viewport,
  pub format: AnimationOutputFormat,
  pub quality: Option<u8>,
  pub draw_debug_border: bool,
  pub stylesheets: Option<Vec<String>>,
  pub fetched_resources: HashMap<Arc<str>, Buffer>,
  pub fps: u32,
}

impl RenderAnimationTask {
  pub(crate) fn from_options(
    env: Env,
    scenes: Vec<(NodeKind, u32)>,
    options: RenderAnimationOptions,
    state: Arc<RwLock<RendererState>>,
  ) -> Result<Self> {
    if scenes.is_empty() {
      return Err(Error::new(
        Status::InvalidArg,
        "Expected at least one animation scene".to_owned(),
      ));
    }

    if options.fps == 0 {
      return Err(Error::new(
        Status::InvalidArg,
        "Expected fps to be greater than 0".to_owned(),
      ));
    }

    Ok(Self {
      scenes: Some(scenes),
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
      fps: options.fps,
    })
  }
}

impl Task for RenderAnimationTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    let Some(scenes) = self.scenes.take() else {
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
    let stylesheets = self.stylesheets.clone().unwrap_or_default();
    let scene_options = scenes
      .into_iter()
      .map(|(node, duration_ms)| {
        SequentialSceneBuilder::default()
          .duration_ms(duration_ms)
          .options(
            RenderOptionsBuilder::default()
              .viewport(self.viewport)
              .fetched_resources(initialized_images.clone())
              .stylesheets(stylesheets.clone())
              .node(node)
              .global(&state.global)
              .draw_debug_border(self.draw_debug_border)
              .build()
              .map_err(map_error)?,
          )
          .build()
          .map_err(map_error)
      })
      .collect::<Result<Vec<_>>>()?;
    let frames = render_sequence_animation(&scene_options, self.fps).map_err(map_error)?;

    if let Some(quality) = self.quality
      && quality > 100
    {
      return Err(Error::from_reason(format!(
        "Invalid WebP quality {quality}; expected a value in 0..=100"
      )));
    }

    let mut buffer = Vec::new();

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
