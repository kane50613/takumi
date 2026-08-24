use std::{collections::HashMap, mem::take, sync::Arc};

use napi::bindgen_prelude::*;
use takumi_bindings_common::stylesheet;
use takumi_core::{
  layout::node::Node,
  style::{FontFamily, Lang, StyleSheet, Theme},
  viewport::Viewport,
};
use takumi_raster::measure;

use crate::{
  JsBytes, map_error,
  renderer::{
    ImageCacheMode, MeasuredNode, RenderOptions, RendererState, collect_images, decode_images,
    deserialize_keyframes, device_pixel_ratio, parse_lang,
  },
};

pub struct MeasureTask {
  pub(crate) node: Option<Node>,
  pub(crate) state: Arc<RendererState>,
  pub(crate) viewport: Viewport,
  pub(crate) time_ms: u64,
  pub(crate) stylesheet: Arc<StyleSheet>,
  pub(crate) images: HashMap<Arc<str>, (JsBytes, ImageCacheMode)>,
  pub(crate) font_families: Option<FontFamily>,
  pub(crate) lang: Option<Lang>,
}

impl MeasureTask {
  pub(crate) fn from_options(
    env: Env,
    node: Node,
    options: RenderOptions,
    state: Arc<RendererState>,
  ) -> Result<Self> {
    let stylesheet = stylesheet(
      &state.resource_cache,
      options.stylesheets,
      deserialize_keyframes(options.keyframes)?,
      Theme::from_unordered(options.theme.unwrap_or_default()),
    );

    Ok(MeasureTask {
      node: Some(node),
      state,
      viewport: Viewport::new((options.width, options.height))
        .with_device_pixel_ratio(device_pixel_ratio(options.device_pixel_ratio)),
      time_ms: options.time_ms.unwrap_or_default().max(0) as u64,
      stylesheet,
      images: collect_images(env, options.images)?,
      font_families: options.font_families.map(FontFamily::from_names),
      lang: parse_lang(options.lang)?,
    })
  }
}

impl Task for MeasureTask {
  type Output = takumi_raster::MeasuredNode;
  type JsValue = MeasuredNode;

  fn compute(&mut self) -> Result<Self::Output> {
    let Some(node) = self.node.take() else {
      unreachable!()
    };

    let fonts = self.state.fonts.load();

    let initialized_images = decode_images(&self.state.resource_cache, take(&mut self.images))?;

    let options = takumi_raster::RenderOptions::builder()
      .viewport(self.viewport)
      .images(initialized_images)
      .stylesheet(take(&mut self.stylesheet))
      .time_ms(self.time_ms)
      .node(node)
      .fonts(&fonts)
      .font_families(take(&mut self.font_families))
      .lang(take(&mut self.lang))
      .build();

    measure(options).map_err(map_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
