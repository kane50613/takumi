use std::{
  borrow::Cow,
  collections::HashMap,
  mem::take,
  sync::{Arc, RwLock},
};

use napi::bindgen_prelude::*;
use rayon::prelude::*;
use takumi_core::layout::{DEFAULT_DEVICE_PIXEL_RATIO, Viewport, node::Node};
use takumi_raster::{
  AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame, encode_animated_gif,
  encode_animated_png, encode_animated_webp, render,
};

use crate::{
  buffer_from_object, map_error, parse_stylesheet,
  renderer::{
    AnimationOutputFormat, EncodeFramesOptions, ImageCacheMode, ImageSource, RendererState,
    webp_lossless,
  },
};

pub struct EncodeFramesTask {
  pub frames: Option<Vec<(Node, u32)>>,
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub viewport: Viewport,
  pub format: AnimationOutputFormat,
  pub quality: Option<u8>,
  pub lossless: Option<bool>,
  pub draw_debug_border: bool,
  pub stylesheets: Option<Vec<String>>,
  pub images: HashMap<Arc<str>, (Buffer, ImageCacheMode)>,
  pub font_families: Option<Vec<String>>,
}

impl EncodeFramesTask {
  pub(crate) fn from_options(
    env: Env,
    frames: Vec<(Node, u32)>,
    options: EncodeFramesOptions,
    state: Arc<RwLock<RendererState>>,
  ) -> Result<Self> {
    Ok(Self {
      frames: Some(frames),
      state,
      viewport: Viewport::new((options.width, options.height)).with_device_pixel_ratio(
        options
          .device_pixel_ratio
          .map(|ratio| ratio as f32)
          .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
      ),
      format: options.format.unwrap_or(AnimationOutputFormat::WebP),
      quality: options.quality,
      lossless: options.lossless,
      draw_debug_border: options.draw_debug_border.unwrap_or_default(),
      stylesheets: options.stylesheets,
      images: options
        .images
        .unwrap_or_default()
        .into_iter()
        .map(|image: ImageSource<'_>| {
          Ok((
            Arc::from(image.src),
            (
              buffer_from_object(env, image.data)?,
              image.cache.unwrap_or_default(),
            ),
          ))
        })
        .collect::<Result<_>>()?,
      font_families: options.font_families,
    })
  }
}

impl Task for EncodeFramesTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    crate::pool::install(move || {
      const ENCODED_BYTES_PER_PIXEL_ESTIMATE: usize = 1;
      const FRAME_OVERHEAD_BYTES: usize = 128;
      const MAX_PREALLOC: usize = 4 * 1024 * 1024;

      let Some(frames) = self.frames.take() else {
        unreachable!()
      };
      let state = self
        .state
        .read()
        .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;
      let initialized_images = state.decode_images(take(&mut self.images))?;

      let viewport = self.viewport;
      let draw_debug_border = self.draw_debug_border;
      let font_families = take(&mut self.font_families);
      let stylesheet = parse_stylesheet(self.stylesheets.clone(), Vec::new())?;
      let frames = frames
        .into_par_iter()
        .map(|(node, duration_ms)| {
          Ok(AnimationFrame::new(
            render(
              takumi_raster::RenderOptions::builder()
                .viewport(viewport)
                .images(initialized_images.clone())
                .stylesheet(stylesheet.clone())
                .node(node)
                .fonts(&state.fonts)
                .font_families(font_families.clone())
                .draw_debug_border(draw_debug_border)
                .build(),
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

      match self.format {
        AnimationOutputFormat::WebP => {
          let mut options = AnimatedWebpOptions::default();
          options.lossless = webp_lossless(self.quality, self.lossless);
          if let Some(quality) = self.quality {
            options.quality = quality;
          }

          encode_animated_webp(Cow::Owned(frames), &mut buffer, options)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        }
        AnimationOutputFormat::Apng => {
          encode_animated_png(&frames, &mut buffer, AnimatedPngOptions::default())
            .map_err(|e| Error::from_reason(e.to_string()))?;
        }
        AnimationOutputFormat::Gif => {
          encode_animated_gif(
            Cow::Owned(frames),
            &mut buffer,
            AnimatedGifOptions::default(),
          )
          .map_err(|e| Error::from_reason(e.to_string()))?;
        }
      }

      Ok(buffer)
    })
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
