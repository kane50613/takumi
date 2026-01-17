use std::collections::HashMap;

use napi::bindgen_prelude::*;
use std::sync::Arc;
use takumi::{
  GlobalContext,
  layout::{DEFAULT_DEVICE_PIXEL_RATIO, DEFAULT_FONT_SIZE, Viewport, node::NodeKind},
  rendering::{RenderOptionsBuilder, render, write_image},
  resources::image::load_image_source_from_bytes,
};

use crate::{
  buffer_from_object, map_error,
  renderer::{OutputFormat, RenderOptions},
};

pub struct RenderTask<'g> {
  pub draw_debug_border: bool,
  pub node: Option<NodeKind>,
  pub global: &'g GlobalContext,
  pub viewport: Viewport,
  pub format: OutputFormat,
  pub quality: Option<u8>,
  pub fetched_resources: HashMap<Arc<str>, Buffer>,
}

impl<'g> RenderTask<'g> {
  pub fn from_options(
    env: Env,
    node: NodeKind,
    options: RenderOptions,
    global: &'g GlobalContext,
  ) -> Result<Self> {
    Ok(RenderTask {
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
      format: options.format.unwrap_or(OutputFormat::png),
      quality: options.quality,
      draw_debug_border: options.draw_debug_border.unwrap_or_default(),
      fetched_resources: options
        .fetched_resources
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| Ok((k, buffer_from_object(env, v)?)))
        .collect::<Result<_>>()?,
    })
  }
}

impl Task for RenderTask<'_> {
  type Output = Vec<u8>;
  type JsValue = Buffer;

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

    let image = render(
      RenderOptionsBuilder::default()
        .viewport(self.viewport)
        .fetched_resources(initialized_images)
        .node(node)
        .global(self.global)
        .draw_debug_border(self.draw_debug_border)
        .build()
        .map_err(map_error)?,
    )
    .map_err(map_error)?;

    if self.format == OutputFormat::raw {
      return Ok(image.into_raw());
    }

    let mut buffer = Vec::new();

    write_image(&image, &mut buffer, self.format.into(), self.quality).map_err(map_error)?;

    Ok(buffer)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
