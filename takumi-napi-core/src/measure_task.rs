use std::mem::take;
use std::sync::RwLock;
use std::{collections::HashMap, sync::Arc};

use napi::bindgen_prelude::*;
use takumi::{
  layout::style::StyleSheet,
  layout::{DEFAULT_DEVICE_PIXEL_RATIO, Viewport, node::Node},
  rendering::measure_layout,
  resources::image::ImageSource as LoadedImageSource,
};

use crate::{
  buffer_from_object, map_error, parse_stylesheet,
  renderer::{MeasuredNode, RenderOptions, RendererState, deserialize_keyframes},
};

pub struct MeasureTask {
  pub node: Option<Node>,
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub viewport: Viewport,
  pub time_ms: u64,
  pub stylesheet: StyleSheet,
  pub fetched_resources: HashMap<Arc<str>, Buffer>,
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
      viewport: Viewport::new(options.width, options.height).with_device_pixel_ratio(
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
      fetched_resources: options
        .fetched_resources
        .unwrap_or_default()
        .into_iter()
        .map(|image| Ok((Arc::from(image.src), buffer_from_object(env, image.data)?)))
        .collect::<Result<_>>()?,
    })
  }
}

impl Task for MeasureTask {
  type Output = takumi::rendering::MeasuredNode;
  type JsValue = MeasuredNode;

  fn compute(&mut self) -> Result<Self::Output> {
    let Some(node) = self.node.take() else {
      unreachable!()
    };

    let initialized_images = self
      .fetched_resources
      .iter()
      .map(|(k, v)| {
        Ok((
          k.clone(),
          LoadedImageSource::from_bytes(v).map_err(map_error)?,
        ))
      })
      .collect::<Result<HashMap<_, _>, _>>()?;

    let state = self
      .state
      .read()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    let options = takumi::rendering::RenderOptions::builder()
      .viewport(self.viewport)
      .fetched_resources(initialized_images)
      .stylesheet(take(&mut self.stylesheet))
      .time_ms(self.time_ms)
      .node(node)
      .global(&state.global)
      .build();

    measure_layout(options).map_err(map_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
