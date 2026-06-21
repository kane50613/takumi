use std::{
  collections::HashMap,
  mem::take,
  sync::{Arc, RwLock},
};

use napi::bindgen_prelude::*;
use takumi_core::layout::{Viewport, node::Node, style::StyleSheet};

use crate::{
  buffer_from_object, map_error, parse_stylesheet,
  renderer::{ImageCacheMode, RendererState, SvgRenderOptions, deserialize_keyframes},
};

pub struct SvgRenderTask {
  pub node: Option<Node>,
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub viewport: Viewport,
  pub time_ms: u64,
  pub stylesheet: StyleSheet,
  pub images: HashMap<Arc<str>, (Buffer, ImageCacheMode)>,
  pub font_families: Option<Vec<String>>,
}

impl SvgRenderTask {
  pub(crate) fn from_options(
    env: Env,
    node: Node,
    options: SvgRenderOptions,
    state: Arc<RwLock<RendererState>>,
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

    let state = self
      .state
      .read()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    let images = state.decode_images(take(&mut self.images))?;

    takumi_svg::render(
      takumi_svg::SvgOptions::builder()
        .viewport(self.viewport)
        .images(images)
        .stylesheet(take(&mut self.stylesheet))
        .time_ms(self.time_ms)
        .node(node)
        .fonts(&state.fonts)
        .font_families(take(&mut self.font_families))
        .build(),
    )
    .map_err(map_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}
