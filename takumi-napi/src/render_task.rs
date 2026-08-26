use std::{collections::HashMap, mem::take, sync::Arc};

use napi::bindgen_prelude::*;
use takumi_bindings_common::stylesheet;
use takumi_core::{
  layout::node::Node,
  style::{FontFamily, Lang, StyleSheet},
  viewport::Viewport,
};
use takumi_raster::{DitheringAlgorithm, render, write_image};

use crate::{
  JsBytes, map_error,
  renderer::{
    ImageCacheMode, OutputFormat, RenderOptions, RendererState, collect_images, decode_images,
    deserialize_keyframes, device_pixel_ratio, parse_lang,
  },
};

pub struct RenderTask {
  pub(crate) draw_debug_border: bool,
  pub(crate) node: Option<Node>,
  pub(crate) state: Arc<RendererState>,
  pub(crate) viewport: Viewport,
  pub(crate) format: OutputFormat,
  pub(crate) quality: Option<u8>,
  pub(crate) lossless: Option<bool>,
  pub(crate) dithering: DitheringAlgorithm,
  pub(crate) time_ms: u64,
  pub(crate) stylesheet: Arc<StyleSheet>,
  pub(crate) images: HashMap<Arc<str>, (JsBytes, ImageCacheMode)>,
  pub(crate) font_families: Option<FontFamily>,
  pub(crate) lang: Option<Lang>,
}

impl RenderTask {
  pub(crate) fn from_options(
    env: Env,
    node: Node,
    options: RenderOptions,
    state: Arc<RendererState>,
  ) -> Result<Self> {
    let stylesheet = stylesheet(
      &state.resource_cache,
      options.css,
      deserialize_keyframes(options.keyframes)?,
      options.css_variables,
    );

    Ok(RenderTask {
      node: Some(node),
      state,
      viewport: Viewport::new((options.width, options.height))
        .with_device_pixel_ratio(device_pixel_ratio(options.device_pixel_ratio)),
      format: options.format.unwrap_or(OutputFormat::Png),
      quality: options.quality,
      lossless: options.lossless,
      dithering: options.dithering.map(Into::into).unwrap_or_default(),
      time_ms: options.time_ms.unwrap_or_default().max(0) as u64,
      draw_debug_border: options.draw_debug_border.unwrap_or_default(),
      stylesheet,
      images: collect_images(env, options.images)?,
      font_families: options.font_families.map(FontFamily::from_names),
      lang: parse_lang(options.lang)?,
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

    let fonts = self.state.fonts.load();

    let initialized_images = decode_images(&self.state.resource_cache, take(&mut self.images))?;

    let image = render(
      takumi_raster::RenderOptions::builder()
        .viewport(self.viewport)
        .images(initialized_images)
        .stylesheet(take(&mut self.stylesheet))
        .time_ms(self.time_ms)
        .dithering(self.dithering)
        .node(node)
        .fonts(&fonts)
        .font_families(take(&mut self.font_families))
        .lang(take(&mut self.lang))
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
