use std::sync::Arc;

use napi::bindgen_prelude::*;
use takumi_bindings_common::{
  css_or_stylesheets,
  input::decode_images,
  options::{Draw, SvgRenderOptions},
};
use takumi_core::Fonts;

use crate::{NodeSource, RenderInput, map_error, renderer::RendererState};

pub struct SvgRenderTask {
  input: Option<RenderInput>,
  state: Arc<RendererState>,
  /// The fonts as they were when the call was made, so a later change leaves this one alone.
  fonts: Arc<Fonts>,
  options: SvgRenderOptions,
}

impl SvgRenderTask {
  pub(crate) fn new(
    source: NodeSource,
    mut options: SvgRenderOptions,
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
}

impl Task for SvgRenderTask {
  type Output = String;
  type JsValue = String;

  fn compute(&mut self) -> Result<Self::Output> {
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

    takumi_svg::render(self.options.svg(node, draw).map_err(map_error)?).map_err(map_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}
