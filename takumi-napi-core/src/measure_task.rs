use std::sync::RwLock;
use std::{collections::HashMap, sync::Arc};

use napi::bindgen_prelude::*;
use takumi::{
  layout::{DEFAULT_DEVICE_PIXEL_RATIO, DEFAULT_FONT_SIZE, Viewport, node::NodeKind},
  rendering::{RenderOptionsBuilder, measure_layout},
  resources::image::load_image_source_from_bytes,
};

use crate::{
  buffer_from_object, map_error,
  renderer::{MeasuredNode, RenderOptions, RendererState},
};

pub struct MeasureTask {
  pub node: Option<NodeKind>,
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub viewport: Viewport,
  pub time_ms: u64,
  pub stylesheets: Option<Vec<String>>,
  pub fetched_resources: HashMap<Arc<str>, Buffer>,
}

impl MeasureTask {
  pub(crate) fn from_options(
    env: Env,
    node: NodeKind,
    options: RenderOptions,
    state: Arc<RwLock<RendererState>>,
  ) -> Result<Self> {
    Ok(MeasureTask {
      node: Some(node),
      state,
      viewport: Viewport {
        width: options.width,
        height: options.height,
        font_size: DEFAULT_FONT_SIZE,
        device_pixel_ratio: options
          .device_pixel_ratio
          .map(|ratio| ratio as f32)
          .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
      },
      time_ms: options.time_ms.unwrap_or_default().max(0) as u64,
      stylesheets: options.stylesheets,
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
          load_image_source_from_bytes(v).map_err(map_error)?,
        ))
      })
      .collect::<Result<HashMap<_, _>, _>>()?;

    let state = self
      .state
      .read()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    let options = RenderOptionsBuilder::default()
      .viewport(self.viewport)
      .fetched_resources(initialized_images)
      .stylesheets(self.stylesheets.clone().unwrap_or_default())
      .time_ms(self.time_ms)
      .node(node)
      .global(&state.global)
      .build()
      .map_err(map_error)?;

    measure_layout(options).map_err(map_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
