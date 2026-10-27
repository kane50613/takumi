use std::{collections::HashMap, mem::take, sync::Arc};

use napi::bindgen_prelude::*;
use takumi_bindings_common::{append_css, parse_lang, stylesheet, time_ms};
use takumi_core::{
  Fonts,
  style::{FontFamily, Lang, StyleSheet},
  viewport::Viewport,
};

use crate::{
  JsBytes, NodeSource, map_error,
  renderer::{
    ImageCacheMode, RendererState, SvgRenderOptions, collect_images, decode_images,
    deserialize_css, deserialize_keyframes,
  },
};

pub struct SvgRenderTask {
  pub(crate) node: Option<NodeSource>,
  pub(crate) state: Arc<RendererState>,
  /// The fonts as they were when the call was made, so a later change leaves this one alone.
  pub(crate) fonts: Arc<Fonts>,
  pub(crate) viewport: Viewport,
  pub(crate) time_ms: u64,
  pub(crate) stylesheet: Arc<StyleSheet>,
  pub(crate) images: HashMap<Arc<str>, (JsBytes, ImageCacheMode)>,
  pub(crate) font_families: Option<FontFamily>,
  pub(crate) lang: Option<Lang>,
}

impl SvgRenderTask {
  pub(crate) fn from_options(
    env: Env,
    mut node: NodeSource,
    options: SvgRenderOptions,
    state: Arc<RendererState>,
  ) -> Result<Self> {
    let stylesheet = stylesheet(
      &state.resource_cache,
      append_css(
        deserialize_css(options.css, options.stylesheets)?,
        node.take_css(),
      ),
      deserialize_keyframes(options.keyframes)?,
    )
    .map_err(map_error)?;

    Ok(SvgRenderTask {
      node: Some(node),
      fonts: state.fonts_for_render(options.font_families.as_deref())?,
      state,
      viewport: Viewport::new((options.width, options.height)),
      time_ms: time_ms(options.time_ms),
      stylesheet,
      images: collect_images(env, options.images)?,
      font_families: options.font_families.map(FontFamily::from_names),
      lang: parse_lang(options.lang.as_deref()).map_err(map_error)?,
    })
  }
}

impl Task for SvgRenderTask {
  type Output = String;
  type JsValue = String;

  fn compute(&mut self) -> Result<Self::Output> {
    let Some(node) = self.node.take() else {
      unreachable!()
    };
    let node = node.into_node()?;

    let images = decode_images(&self.state.resource_cache, take(&mut self.images))?;

    takumi_svg::render(
      takumi_svg::SvgOptions::builder()
        .viewport(self.viewport)
        .images(images)
        .stylesheet(take(&mut self.stylesheet))
        .time_ms(self.time_ms)
        .node(node)
        .fonts(&self.fonts)
        .font_families(take(&mut self.font_families))
        .lang(take(&mut self.lang))
        .build(),
    )
    .map_err(map_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}
