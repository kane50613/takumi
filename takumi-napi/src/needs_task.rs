use std::sync::Arc;

use napi::bindgen_prelude::*;
use takumi_bindings_common::{
  input::{ListedFont, ListedFonts, retain_svg},
  options::RenderOptions,
};
use takumi_core::Fonts;
use takumi_raster::needs;

use crate::{
  NodeSource,
  render_animation_task::AnimationInput,
  render_job::RenderJob,
  renderer::{RenderNeeds, RendererState},
};

/// `fonts` with each font `listed` still loading registered as a pending face.
fn with_listed(fonts: Arc<Fonts>, listed: Option<&ListedFonts>) -> Arc<Fonts> {
  match listed {
    Some(listed) => Arc::new(listed.fonts(&fonts)),
    None => fonts,
  }
}

pub struct NeedsTask {
  job: RenderJob,
  listed: Option<ListedFonts>,
}

impl NeedsTask {
  pub(crate) fn new(
    source: NodeSource,
    mut options: RenderOptions,
    fonts: Option<Vec<ListedFont>>,
    state: Arc<RendererState>,
  ) -> Self {
    let listed = state.listed(&mut options.font_families, fonts);

    retain_svg(&mut options.images);

    let mut job = RenderJob::new(source, options, state);

    job.fonts = with_listed(job.fonts, listed.as_ref());

    Self { job, listed }
  }
}

impl Task for NeedsTask {
  type Output = takumi_raster::RenderNeeds;
  type JsValue = RenderNeeds;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(needs(self.job.raster()?))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(RenderNeeds::new(output, self.listed.as_ref()))
  }
}

/// Reads what every scene of an animation needs fetched before it renders.
pub struct AnimationNeedsTask {
  input: AnimationInput,
  listed: Option<ListedFonts>,
}

impl AnimationNeedsTask {
  pub(crate) fn read(
    options: Object,
    fonts: Option<Vec<ListedFont>>,
    state: Arc<RendererState>,
  ) -> Result<Self> {
    let mut listed = None;
    let mut input = AnimationInput::read(
      options,
      |families| listed = state.listed(families, fonts),
      Arc::clone(&state),
    )?;

    input.fonts = with_listed(input.fonts, listed.as_ref());
    retain_svg(&mut input.options.images);

    Ok(Self { input, listed })
  }
}

impl Task for AnimationNeedsTask {
  type Output = takumi_raster::RenderNeeds;
  type JsValue = RenderNeeds;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(
      self
        .input
        .scenes()?
        .into_iter()
        .map(|(options, _)| needs(options))
        .collect(),
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(RenderNeeds::new(output, self.listed.as_ref()))
  }
}
