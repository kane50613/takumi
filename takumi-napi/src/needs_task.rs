use std::{collections::HashMap, mem::take, sync::Arc};

use napi::bindgen_prelude::*;
use takumi_bindings_common::{
  device_pixel_ratio,
  input::{ListedFont, ListedFonts},
  parse_lang, time_ms,
};
use takumi_core::{
  Fonts,
  style::{FontFamily, Lang},
  viewport::Viewport,
};
use takumi_raster::needs;

use crate::{
  JsBytes, NodeSource, RenderInput, map_error,
  render_animation_task::AnimationInput,
  renderer::{
    ImageCacheMode, RenderAnimationOptions, RenderNeeds, RenderOptions, RendererState,
    collect_images, decode_images, retain_svg,
  },
};

/// `fonts` with each font `listed` still loading registered as a pending face.
fn with_listed(fonts: Arc<Fonts>, listed: Option<&ListedFonts>) -> Arc<Fonts> {
  match listed {
    Some(listed) => Arc::new(listed.fonts(&fonts)),
    None => fonts,
  }
}

pub struct NeedsTask {
  pub(crate) input: Option<RenderInput>,
  pub(crate) state: Arc<RendererState>,
  /// The fonts as they were when the call was made, so a later change leaves this one alone.
  pub(crate) fonts: Arc<Fonts>,
  pub(crate) viewport: Viewport,
  pub(crate) time_ms: u64,
  pub(crate) font_families: Option<FontFamily>,
  pub(crate) lang: Option<Lang>,
  pub(crate) images: HashMap<Arc<str>, (JsBytes, ImageCacheMode)>,
  pub(crate) listed: Option<ListedFonts>,
}

impl NeedsTask {
  pub(crate) fn from_options(
    env: Env,
    source: NodeSource,
    mut options: RenderOptions,
    fonts: Option<Vec<ListedFont>>,
    state: Arc<RendererState>,
  ) -> Result<Self> {
    let listed = state.listed(&mut options.font_families, fonts);
    let fonts = state.fonts_for_render(options.font_families.as_deref());
    let mut images = collect_images(env, options.images)?;

    retain_svg(&mut images);

    Ok(NeedsTask {
      input: Some(RenderInput::new(
        source,
        options.css,
        options.stylesheets,
        options.keyframes,
      )?),
      fonts: with_listed(fonts, listed.as_ref()),
      state,
      viewport: Viewport::new((options.width, options.height)).with_device_pixel_ratio(
        device_pixel_ratio(options.device_pixel_ratio.map(|ratio| ratio as f32)),
      ),
      time_ms: time_ms(options.time_ms),
      font_families: options.font_families.map(FontFamily::from_names),
      lang: parse_lang(options.lang.as_deref()).map_err(map_error)?,
      images,
      listed,
    })
  }
}

impl Task for NeedsTask {
  type Output = takumi_raster::RenderNeeds;
  type JsValue = RenderNeeds;

  fn compute(&mut self) -> Result<Self::Output> {
    let Some(input) = self.input.take() else {
      unreachable!()
    };
    let (node, stylesheet) = input.resolve(&self.state)?;
    let images = decode_images(&self.state.resource_cache, take(&mut self.images))?;

    let options = takumi_raster::RenderOptions::builder()
      .viewport(self.viewport)
      .images(images)
      .resource_cache(self.state.resource_cache.clone())
      .stylesheet(stylesheet)
      .time_ms(self.time_ms)
      .node(node)
      .fonts(&self.fonts)
      .font_families(take(&mut self.font_families))
      .lang(take(&mut self.lang))
      .build();

    Ok(needs(options))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(RenderNeeds::new(output, self.listed.as_ref()))
  }
}

/// Reads what every scene of an animation needs fetched before it renders.
pub struct AnimationNeedsTask {
  pub(crate) input: AnimationInput,
  pub(crate) listed: Option<ListedFonts>,
}

impl AnimationNeedsTask {
  pub(crate) fn from_options(
    env: Env,
    mut options: RenderAnimationOptions,
    fonts: Option<Vec<ListedFont>>,
    state: Arc<RendererState>,
  ) -> Result<Self> {
    let listed = state.listed(&mut options.font_families, fonts);
    let mut input = AnimationInput::from_options(env, options, state)?;

    input.fonts = with_listed(input.fonts, listed.as_ref());
    retain_svg(&mut input.images);

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
