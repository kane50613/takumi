use std::{
  collections::HashMap,
  mem::take,
  sync::{Arc, RwLock},
};

use napi::bindgen_prelude::*;
use takumi_core::layout::{DEFAULT_DEVICE_PIXEL_RATIO, Viewport, node::Node, style::StyleSheet};
use takumi_raster::measure;

use crate::{
  buffer_from_object, map_error, parse_stylesheet,
  renderer::{ImageCacheMode, MeasuredNode, RenderOptions, RendererState, deserialize_keyframes},
};

pub struct MeasureTask {
  pub node: Option<Node>,
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub viewport: Viewport,
  pub time_ms: u64,
  pub stylesheet: StyleSheet,
  pub images: HashMap<Arc<str>, (Buffer, ImageCacheMode)>,
  pub font_families: Option<Vec<String>>,
}

impl MeasureTask {
  pub(crate) fn from_options(
    env: Env,
    node: Node,
    options: RenderOptions,
    state: Arc<RwLock<RendererState>>,
  ) -> Result<Self> {
    Ok(MeasureTask {
      node: Some(node),
      state,
      viewport: Viewport::new((options.width, options.height)).with_device_pixel_ratio(
        options
          .device_pixel_ratio
          .map(|ratio| ratio as f32)
          .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
      ),
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

impl Task for MeasureTask {
  type Output = takumi_raster::MeasuredNode;
  type JsValue = MeasuredNode;

  fn compute(&mut self) -> Result<Self::Output> {
    let Some(node) = self.node.take() else {
      unreachable!()
    };

    let state = self
      .state
      .read()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    let initialized_images = state.decode_images(take(&mut self.images))?;

    let options = takumi_raster::RenderOptions::builder()
      .viewport(self.viewport)
      .images(initialized_images)
      .stylesheet(take(&mut self.stylesheet))
      .time_ms(self.time_ms)
      .node(node)
      .fonts(&state.fonts)
      .font_families(take(&mut self.font_families))
      .build();

    measure(options).map_err(map_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
