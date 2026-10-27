use std::{collections::HashMap, mem::take, sync::Arc};

use napi::bindgen_prelude::*;
use takumi_bindings_common::{device_pixel_ratio, parse_lang, time_ms};
use takumi_core::{
  Fonts,
  style::{FontFamily, Lang},
  viewport::Viewport,
};
use takumi_raster::measure;

use crate::{
  JsBytes, NodeSource, RenderInput, map_error,
  renderer::{
    ImageCacheMode, MeasuredNode, RenderOptions, RendererState, collect_images, decode_images,
  },
};

pub struct MeasureTask {
  pub(crate) input: Option<RenderInput>,
  pub(crate) state: Arc<RendererState>,
  /// The fonts as they were when the call was made, so a later change leaves this one alone.
  pub(crate) fonts: Arc<Fonts>,
  pub(crate) viewport: Viewport,
  pub(crate) time_ms: u64,
  pub(crate) images: HashMap<Arc<str>, (JsBytes, ImageCacheMode)>,
  pub(crate) font_families: Option<FontFamily>,
  pub(crate) lang: Option<Lang>,
}

impl MeasureTask {
  pub(crate) fn from_options(
    env: Env,
    source: NodeSource,
    options: RenderOptions,
    state: Arc<RendererState>,
  ) -> Result<Self> {
    Ok(MeasureTask {
      input: Some(RenderInput::new(
        source,
        options.css,
        options.stylesheets,
        options.keyframes,
        options.emoji.as_deref(),
      )?),
      fonts: state.fonts_for_render(options.font_families.as_deref())?,
      state,
      viewport: Viewport::new((options.width, options.height)).with_device_pixel_ratio(
        device_pixel_ratio(options.device_pixel_ratio.map(|ratio| ratio as f32)),
      ),
      time_ms: time_ms(options.time_ms),
      images: collect_images(env, options.images)?,
      font_families: options.font_families.map(FontFamily::from_names),
      lang: parse_lang(options.lang.as_deref()).map_err(map_error)?,
    })
  }
}

impl Task for MeasureTask {
  type Output = takumi_raster::MeasuredNode;
  type JsValue = MeasuredNode;

  fn compute(&mut self) -> Result<Self::Output> {
    let Some(input) = self.input.take() else {
      unreachable!()
    };
    let (node, stylesheet) = input.resolve(&self.state.resource_cache)?;

    let initialized_images = decode_images(&self.state.resource_cache, take(&mut self.images))?;

    let options = takumi_raster::RenderOptions::builder()
      .viewport(self.viewport)
      .images(initialized_images)
      .stylesheet(stylesheet)
      .time_ms(self.time_ms)
      .node(node)
      .fonts(&self.fonts)
      .font_families(take(&mut self.font_families))
      .lang(take(&mut self.lang))
      .build();

    measure(options).map_err(map_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
