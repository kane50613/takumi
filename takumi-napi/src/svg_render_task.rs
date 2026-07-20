use std::{collections::HashMap, mem::take, sync::Arc};

use napi::bindgen_prelude::*;
use takumi_core::{
  layout::node::Node,
  style::{FontFamily, Lang, StyleSheet},
  viewport::Viewport,
};

use crate::{
  buffer_from_object, map_error,
  renderer::{
    ImageCacheMode, RendererState, SvgRenderOptions, decode_images, deserialize_keyframes,
  },
};
use takumi_bindings_common::stylesheet;

pub struct SvgRenderTask {
  pub(crate) node: Option<Node>,
  pub(crate) state: Arc<RendererState>,
  pub(crate) viewport: Viewport,
  pub(crate) time_ms: u64,
  pub(crate) stylesheet: Arc<StyleSheet>,
  pub(crate) images: HashMap<Arc<str>, (Buffer, ImageCacheMode)>,
  pub(crate) font_families: Option<FontFamily>,
  pub(crate) lang: Option<Lang>,
}

impl SvgRenderTask {
  pub(crate) fn from_options(
    env: Env,
    node: Node,
    options: SvgRenderOptions,
    state: Arc<RendererState>,
  ) -> Result<Self> {
    let stylesheet = stylesheet(
      &state.resource_cache,
      options.stylesheets,
      deserialize_keyframes(options.keyframes)?,
    );

    Ok(SvgRenderTask {
      node: Some(node),
      state,
      viewport: Viewport::new((options.width, options.height)),
      time_ms: options.time_ms.unwrap_or_default().max(0) as u64,
      stylesheet,
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
      font_families: options.font_families.map(FontFamily::from_names),
      lang: options
        .lang
        .map(|lang| Lang::parse(&lang).map_err(map_error))
        .transpose()?,
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

    let fonts = self.state.fonts.load();

    let images = decode_images(&self.state.resource_cache, take(&mut self.images))?;

    takumi_svg::render(
      takumi_svg::SvgOptions::builder()
        .viewport(self.viewport)
        .images(images)
        .stylesheet(take(&mut self.stylesheet))
        .time_ms(self.time_ms)
        .node(node)
        .fonts(&fonts)
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
