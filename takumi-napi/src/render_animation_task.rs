use std::{collections::HashMap, mem::take, sync::Arc};

use napi::bindgen_prelude::*;
use takumi_bindings_common::{Scenes, device_pixel_ratio, parse_lang, stylesheet};
use takumi_core::{
  Fonts,
  style::{CssSource, FontFamily, KeyframesRule, Lang},
  viewport::Viewport,
};
use takumi_raster::{
  AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFormat, RenderOptions,
  SequentialScene, write_animation,
};

use crate::{
  JsBytes, NodeSource, map_error,
  renderer::{
    AnimationOutputFormat, ImageCacheMode, RenderAnimationOptions, RendererState, collect_images,
    decode_images, deserialize_css, deserialize_keyframes, webp_lossless,
  },
};

/// An animation's scenes and what they lay out with, read on the main thread.
pub(crate) struct AnimationInput {
  scenes: Option<Vec<(NodeSource, u32)>>,
  state: Arc<RendererState>,
  /// The fonts as they were when the call was made, so a later change leaves this one alone.
  pub(crate) fonts: Arc<Fonts>,
  viewport: Viewport,
  draw_debug_border: bool,
  css: Option<Vec<CssSource>>,
  keyframes: Vec<KeyframesRule>,
  pub(crate) images: HashMap<Arc<str>, (JsBytes, ImageCacheMode)>,
  font_families: Option<FontFamily>,
  lang: Option<Lang>,
}

impl AnimationInput {
  pub(crate) fn from_options(
    env: Env,
    options: RenderAnimationOptions,
    state: Arc<RendererState>,
  ) -> Result<Self> {
    let RenderAnimationOptions {
      scenes,
      draw_debug_border,
      width,
      height,
      images,
      css,
      stylesheets,
      keyframes,
      device_pixel_ratio: dpr,
      font_families,
      lang,
      ..
    } = options;
    let scenes = scenes
      .into_iter()
      .map(|scene| Ok((NodeSource::from_js(scene.node)?, scene.duration_ms)))
      .collect::<Result<Vec<_>>>()?;

    Ok(Self {
      scenes: Some(scenes),
      fonts: state.fonts_for_render(font_families.as_deref()),
      state,
      viewport: Viewport::new((width, height))
        .with_device_pixel_ratio(device_pixel_ratio(dpr.map(|ratio| ratio as f32))),
      draw_debug_border: draw_debug_border.unwrap_or_default(),
      css: deserialize_css(css, stylesheets)?,
      keyframes: deserialize_keyframes(keyframes)?,
      images: collect_images(env, images)?,
      font_families: font_families.map(FontFamily::from_names),
      lang: parse_lang(lang.as_deref()).map_err(map_error)?,
    })
  }

  /// Each scene's render options and how long it shows, its HTML parsed.
  pub(crate) fn scenes(&mut self) -> Result<Vec<(RenderOptions<'_>, u32)>> {
    let Some(scenes) = self.scenes.take() else {
      unreachable!()
    };
    let images = decode_images(&self.state.resource_cache, take(&mut self.images))?;
    let Scenes { nodes, css } = Scenes::read(scenes, take(&mut self.css), NodeSource::into_parts)?;
    let stylesheet =
      stylesheet(&self.state.resource_cache, css, take(&mut self.keyframes)).map_err(map_error)?;

    Ok(
      nodes
        .into_iter()
        .map(|(node, duration_ms)| {
          let options = RenderOptions::builder()
            .viewport(self.viewport)
            .images(images.clone())
            .resource_cache(self.state.resource_cache.clone())
            .stylesheet(stylesheet.clone())
            .node(node)
            .fonts(&self.fonts)
            .font_families(self.font_families.clone())
            .lang(self.lang)
            .draw_debug_border(self.draw_debug_border)
            .build();

          (options, duration_ms)
        })
        .collect(),
    )
  }
}

pub struct RenderAnimationTask {
  pub(crate) input: AnimationInput,
  pub(crate) format: AnimationOutputFormat,
  pub(crate) quality: Option<u8>,
  pub(crate) lossless: Option<bool>,
  pub(crate) fps: u32,
}

impl RenderAnimationTask {
  pub(crate) fn from_options(
    env: Env,
    options: RenderAnimationOptions,
    state: Arc<RendererState>,
  ) -> Result<Self> {
    if options.scenes.is_empty() {
      return Err(Error::new(
        Status::InvalidArg,
        "Expected at least one animation scene".to_owned(),
      ));
    }

    if options.fps == 0 {
      return Err(Error::new(
        Status::InvalidArg,
        "Expected fps to be greater than 0".to_owned(),
      ));
    }

    let format = options.format.unwrap_or(AnimationOutputFormat::WebP);
    let quality = options.quality;
    let lossless = options.lossless;
    let fps = options.fps;

    Ok(Self {
      input: AnimationInput::from_options(env, options, state)?,
      format,
      quality,
      lossless,
      fps,
    })
  }
}

impl Task for RenderAnimationTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    crate::pool::install(move || {
      let scenes = self
        .input
        .scenes()?
        .into_iter()
        .map(|(options, duration_ms)| {
          SequentialScene::builder()
            .duration_ms(duration_ms)
            .options(options)
            .build()
        })
        .collect::<Vec<_>>();
      let format = match self.format {
        AnimationOutputFormat::WebP => {
          let options = AnimatedWebpOptions::builder()
            .lossless(webp_lossless(self.quality, self.lossless))
            .quality(self.quality.unwrap_or(75))
            .build();
          AnimationFormat::WebP(options)
        }
        AnimationOutputFormat::Apng => AnimationFormat::Apng(AnimatedPngOptions::default()),
        AnimationOutputFormat::Gif => AnimationFormat::Gif(AnimatedGifOptions::default()),
      };

      let mut buffer = Vec::new();
      write_animation(&scenes, self.fps, format, &mut buffer)
        .map_err(|e| Error::from_reason(e.to_string()))?;

      Ok(buffer)
    })
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
