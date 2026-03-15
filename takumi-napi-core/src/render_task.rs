use std::sync::RwLock;
use std::{borrow::Cow, mem::take};
use std::{collections::HashMap, sync::Arc};

use napi::bindgen_prelude::*;
use takumi::layout::node::Node;
use takumi::{
  layout::style::StyleSheet,
  layout::{DEFAULT_DEVICE_PIXEL_RATIO, Viewport},
  rendering::{DitheringAlgorithm, render, write_image},
  resources::image::ImageSource as LoadedImageSource,
};

use crate::{
  ExternalMemoryAccountable, buffer_from_object, map_error, parse_stylesheet,
  renderer::{OutputFormat, RenderOptions, RendererState, deserialize_keyframes},
};

pub struct RenderTask {
  pub draw_debug_border: bool,
  pub node: Option<Node>,
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub viewport: Viewport,
  pub format: OutputFormat,
  pub quality: Option<u8>,
  pub dithering: DitheringAlgorithm,
  pub time_ms: u64,
  pub stylesheet: StyleSheet,
  pub fetched_resources: HashMap<Arc<str>, Buffer>,
}

impl RenderTask {
  pub(crate) fn from_options(
    env: Env,
    node: Node,
    options: RenderOptions,
    state: Arc<RwLock<RendererState>>,
  ) -> Result<Self> {
    Ok(RenderTask {
      node: Some(node),
      state,
      viewport: Viewport::new(options.width, options.height).with_device_pixel_ratio(
        options
          .device_pixel_ratio
          .map(|ratio| ratio as f32)
          .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
      ),
      format: options.format.unwrap_or(OutputFormat::Png),
      quality: options.quality,
      dithering: options.dithering.map(Into::into).unwrap_or_default(),
      time_ms: options.time_ms.unwrap_or_default().max(0) as u64,
      draw_debug_border: options.draw_debug_border.unwrap_or_default(),
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

impl Task for RenderTask {
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
          LoadedImageSource::from_bytes(v).map_err(map_error)?,
        ))
      })
      .collect::<Result<HashMap<_, _>, _>>()?;

    let state = self
      .state
      .read()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    let image = render(
      takumi::rendering::RenderOptions::builder()
        .viewport(self.viewport)
        .fetched_resources(initialized_images)
        .stylesheet(take(&mut self.stylesheet))
        .time_ms(self.time_ms)
        .dithering(self.dithering)
        .node(node)
        .global(&state.global)
        .draw_debug_border(self.draw_debug_border)
        .build(),
    )
    .map_err(map_error)?;

    if self.format == OutputFormat::Raw {
      return Ok(image.into_raw());
    }

    let mut buffer = Vec::new();

    write_image(
      Cow::Owned(image),
      &mut buffer,
      self.format.into(),
      self.quality,
    )
    .map_err(map_error)?;

    Ok(buffer)
  }

  fn resolve(&mut self, mut env: Env, output: Self::Output) -> Result<Self::JsValue> {
    // Account external memory to V8's garbage collector
    // This enables V8 to collect memory based on actual memory pressure
    output.account_external_memory(&mut env)?;
    Ok(output.into())
  }
}
