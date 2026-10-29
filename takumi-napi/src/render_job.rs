//! A render of a node tree: its source, the fonts it draws with, and its options.

use std::sync::Arc;

use napi::bindgen_prelude::*;
use takumi_bindings_common::{
  css_or_stylesheets,
  input::decode_images,
  options::{Draw, RenderOptions},
};
use takumi_core::Fonts;

use crate::{NodeSource, RenderInput, map_error, renderer::RendererState};

/// A render of a node tree, read on the JS thread and drawn on a worker.
pub(crate) struct RenderJob {
  input: Option<RenderInput>,
  state: Arc<RendererState>,
  /// The fonts as they were when the call was made, so a later change leaves this one alone.
  pub(crate) fonts: Arc<Fonts>,
  pub(crate) options: RenderOptions,
}

impl RenderJob {
  pub(crate) fn new(
    source: NodeSource,
    mut options: RenderOptions,
    state: Arc<RendererState>,
  ) -> Self {
    let input = RenderInput::new(
      source,
      css_or_stylesheets(options.css.take(), options.stylesheets.take()),
      options.keyframes.take().unwrap_or_default(),
    );

    Self {
      input: Some(input),
      fonts: state.fonts_for_render(options.font_families.as_deref()),
      state,
      options,
    }
  }

  /// The raster options of the render, its source parsed and its images decoded.
  pub(crate) fn raster(&mut self) -> Result<takumi_raster::RenderOptions<'_>> {
    let Some(input) = self.input.take() else {
      unreachable!()
    };
    let (node, stylesheet) = input.resolve(&self.state)?;
    let images = decode_images(
      &self.state.resource_cache,
      &self.options.images.take().unwrap_or_default(),
    )
    .map_err(map_error)?;
    let draw = Draw {
      fonts: &self.fonts,
      cache: &self.state.resource_cache,
      stylesheet,
      images,
    };

    self.options.raster(node, draw).map_err(map_error)
  }
}
