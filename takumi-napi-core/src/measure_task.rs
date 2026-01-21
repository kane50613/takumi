use std::{collections::HashMap, sync::Arc};

use napi::bindgen_prelude::*;
use takumi::{
  GlobalContext,
  layout::{DEFAULT_DEVICE_PIXEL_RATIO, DEFAULT_FONT_SIZE, Viewport, node::NodeKind},
  rendering::{RenderOptionsBuilder, measure_layout},
  resources::image::load_image_source_from_bytes,
};

use crate::{
  buffer_from_object, map_error,
  renderer::{MeasuredNode, RenderOptions},
};

pub struct MeasureTask<'g> {
  pub node: Option<NodeKind>,
  pub global: &'g GlobalContext,
  pub viewport: Viewport,
  pub fetched_resources: HashMap<Arc<str>, Buffer>,
}

impl<'g> MeasureTask<'g> {
  pub fn from_options(
    env: Env,
    node: NodeKind,
    options: RenderOptions,
    global: &'g GlobalContext,
  ) -> Result<Self> {
    Ok(MeasureTask {
      node: Some(node),
      global,
      viewport: Viewport {
        width: options.width,
        height: options.height,
        font_size: DEFAULT_FONT_SIZE,
        device_pixel_ratio: options
          .device_pixel_ratio
          .map(|ratio| ratio as f32)
          .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
      },
      fetched_resources: options
        .fetched_resources
        .unwrap_or_default()
        .into_iter()
        .map(|image| Ok((Arc::from(image.src), buffer_from_object(env, image.data)?)))
        .collect::<Result<_>>()?,
    })
  }
}

impl Task for MeasureTask<'_> {
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

    let options = RenderOptionsBuilder::default()
      .viewport(self.viewport)
      .fetched_resources(initialized_images)
      .node(node)
      .global(self.global)
      .build()
      .map_err(map_error)?;

    measure_layout(options).map_err(map_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
