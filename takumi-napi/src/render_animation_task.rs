use std::sync::Arc;

use napi::bindgen_prelude::*;
use serde::de::IgnoredAny;
use takumi_bindings_common::{
  Scenes, css_or_stylesheets,
  input::decode_images,
  options::{Draw, RenderAnimationOptions},
  stylesheet,
};
use takumi_core::Fonts;
use takumi_raster::{RenderOptions, SequentialScene, write_animation};

use crate::{
  NodeSource, deserialize_with_tracing, map_error,
  renderer::{AnimationScene, RendererState},
};

/// An animation's scenes and what they lay out with, read on the JS thread.
pub(crate) struct AnimationInput {
  scenes: Option<Vec<(NodeSource, u32)>>,
  state: Arc<RendererState>,
  /// The fonts as they were when the call was made, so a later change leaves this one alone.
  pub(crate) fonts: Arc<Fonts>,
  /// The options, their scenes read into [`Self::scenes`].
  pub(crate) options: RenderAnimationOptions<IgnoredAny>,
}

impl AnimationInput {
  /// Reads `options`, its scenes through napi so a scene's node stays a JS object until parsed.
  pub(crate) fn read(
    options: Object,
    listed_families: impl FnOnce(&mut Option<Vec<String>>),
    state: Arc<RendererState>,
  ) -> Result<Self> {
    let scenes = options
      .get::<Vec<AnimationScene>>("scenes")?
      .unwrap_or_default()
      .into_iter()
      .map(|scene| Ok((NodeSource::from_js(scene.node)?, scene.duration_ms)))
      .collect::<Result<Vec<_>>>()?;
    let mut options: RenderAnimationOptions<IgnoredAny> = deserialize_with_tracing(options)?;

    if let Some(reason) = options.invalid() {
      return Err(Error::new(Status::InvalidArg, reason.to_owned()));
    }

    listed_families(&mut options.font_families);

    Ok(Self {
      scenes: Some(scenes),
      fonts: state.fonts_for_render(
        options.font_families.as_deref(),
        options.failed_font_files.as_deref(),
      ),
      state,
      options,
    })
  }

  /// Each scene's render options and how long it shows, its HTML parsed.
  pub(crate) fn scenes(&mut self) -> Result<Vec<(RenderOptions<'_>, u32)>> {
    let Some(scenes) = self.scenes.take() else {
      unreachable!()
    };
    let options = &mut self.options;
    let images = decode_images(
      &self.state.resource_cache,
      &options.images.take().unwrap_or_default(),
    )
    .map_err(map_error)?;
    let Scenes { nodes, css } = Scenes::read(
      scenes,
      css_or_stylesheets(options.css.take(), options.stylesheets.take()),
      NodeSource::into_parts,
    )?;
    let stylesheet = stylesheet(
      &self.state.resource_cache,
      css,
      options.keyframes.take().unwrap_or_default(),
    )
    .map_err(map_error)?;
    let draw = Draw {
      fonts: &self.fonts,
      cache: &self.state.resource_cache,
      stylesheet,
      images,
    };

    self.options.frames(nodes, draw).map_err(map_error)
  }
}

pub struct RenderAnimationTask(pub(crate) AnimationInput);

impl Task for RenderAnimationTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    let options = &self.0.options;
    let format = options
      .format
      .unwrap_or_default()
      .raster(options.quality, options.lossless);
    let fps = options.fps;

    crate::pool::install(move || {
      let scenes = self
        .0
        .scenes()?
        .into_iter()
        .map(|(options, duration_ms)| {
          SequentialScene::builder()
            .duration_ms(duration_ms)
            .options(options)
            .build()
        })
        .collect::<Vec<_>>();
      let mut buffer = Vec::new();

      write_animation(&scenes, fps, format, &mut buffer).map_err(map_error)?;

      Ok(buffer)
    })
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
