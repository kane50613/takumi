use std::{
  collections::HashMap,
  mem::take,
  sync::{Arc, RwLock},
};

use napi::bindgen_prelude::*;
use takumi_core::layout::{DEFAULT_DEVICE_PIXEL_RATIO, Viewport, node::Node, style::StyleSheet};
use takumi_raster::{DitheringAlgorithm, render, write_image};

use crate::{
  buffer_from_object, map_error, parse_stylesheet,
  renderer::{ImageCacheMode, OutputFormat, RenderOptions, RendererState, deserialize_keyframes},
};

pub struct RenderTask {
  pub(crate) draw_debug_border: bool,
  pub(crate) node: Option<Node>,
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub(crate) viewport: Viewport,
  pub(crate) format: OutputFormat,
  pub(crate) quality: Option<u8>,
  pub(crate) lossless: Option<bool>,
  pub(crate) dithering: DitheringAlgorithm,
  pub(crate) time_ms: u64,
  pub(crate) stylesheet: StyleSheet,
  pub(crate) images: HashMap<Arc<str>, (Buffer, ImageCacheMode)>,
  pub(crate) font_families: Option<Vec<String>>,
}

impl RenderTask {
  pub(crate) fn from_options(
    env: Env,
    node: Node,
    options: RenderOptions,
    state: Arc<RwLock<RendererState>>,
  ) -> Result<Self> {
    Ok(RenderTask {
      node: Some(node),
      state,
      viewport: Viewport::new((options.width, options.height)).with_device_pixel_ratio(
        options
          .device_pixel_ratio
          .map(|ratio| ratio as f32)
          .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
      ),
      format: options.format.unwrap_or(OutputFormat::Png),
      quality: options.quality,
      lossless: options.lossless,
      dithering: options.dithering.map(Into::into).unwrap_or_default(),
      time_ms: options.time_ms.unwrap_or_default().max(0) as u64,
      draw_debug_border: options.draw_debug_border.unwrap_or_default(),
      stylesheet: parse_stylesheet(
        options.stylesheets,
        deserialize_keyframes(options.keyframes)?,
      )?,
      images: options
        .images
        .unwrap_or_default()
        .into_iter()
        .map(|image| {
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

impl Task for RenderTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    let Some(node) = self.node.take() else {
      unreachable!()
    };

    let state = self
      .state
      .read()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    let initialized_images = state.decode_images(take(&mut self.images))?;

    let image = render(
      takumi_raster::RenderOptions::builder()
        .viewport(self.viewport)
        .images(initialized_images)
        .stylesheet(take(&mut self.stylesheet))
        .time_ms(self.time_ms)
        .dithering(self.dithering)
        .node(node)
        .fonts(&state.fonts)
        .font_families(take(&mut self.font_families))
        .draw_debug_border(self.draw_debug_border)
        .build(),
    )
    .map_err(map_error)?;

    if self.format == OutputFormat::Raw {
      return Ok(image.into_raw());
    }

    let mut buffer = Vec::new();

    write_image(
      &image,
      &mut buffer,
      self
        .format
        .into_image_output_format(self.quality, self.lossless),
    )
    .map_err(map_error)?;

    Ok(buffer)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
