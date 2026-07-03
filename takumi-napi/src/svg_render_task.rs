use std::{collections::HashMap, mem::take, sync::Arc};

use napi::bindgen_prelude::*;
use takumi_core::layout::{Viewport, node::Node, style::StyleSheet};

use crate::{
  buffer_from_object, map_error, parse_stylesheet,
  renderer::{
    ImageCacheMode, RendererState, SvgRenderOptions, decode_images, deserialize_keyframes,
  },
};

pub struct SvgRenderTask {
  pub(crate) node: Option<Node>,
  pub(crate) state: Arc<RendererState>,
  pub(crate) viewport: Viewport,
  pub(crate) time_ms: u64,
  pub(crate) stylesheet: StyleSheet,
  pub(crate) images: HashMap<Arc<str>, (Buffer, ImageCacheMode)>,
  pub(crate) font_families: Option<Vec<String>>,
  pub(crate) lang: Option<Arc<str>>,
}

impl SvgRenderTask {
  pub(crate) fn from_options(
    env: Env,
    node: Node,
    options: SvgRenderOptions,
    state: Arc<RendererState>,
  ) -> Result<Self> {
    Ok(SvgRenderTask {
      node: Some(node),
      state,
      viewport: Viewport::new((options.width, options.height)),
      time_ms: options.time_ms.unwrap_or_default().max(0) as u64,
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
      lang: options.lang.map(Arc::from),
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

    let images = decode_images(&self.state.image_cache, take(&mut self.images))?;

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
