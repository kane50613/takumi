use std::{collections::HashMap, mem::take, sync::Arc};

use napi::bindgen_prelude::*;
use takumi_bindings_common::{
  append_css, device_pixel_ratio, parse_lang, source_cache::SourceCache, stylesheet,
};
use takumi_core::{
  Fonts,
  layout::node::Node,
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

pub struct RenderAnimationTask {
  pub(crate) scenes: Option<Vec<(NodeSource, u32)>>,
  pub(crate) state: Arc<RendererState>,
  /// The fonts as they were when the call was made, so a later change leaves this one alone.
  pub(crate) fonts: Arc<Fonts>,
  pub(crate) viewport: Viewport,
  pub(crate) format: AnimationOutputFormat,
  pub(crate) quality: Option<u8>,
  pub(crate) lossless: Option<bool>,
  pub(crate) draw_debug_border: bool,
  pub(crate) css: Option<Vec<CssSource>>,
  pub(crate) keyframes: Vec<KeyframesRule>,
  pub(crate) images: HashMap<Arc<str>, (JsBytes, ImageCacheMode)>,
  pub(crate) font_families: Option<FontFamily>,
  pub(crate) lang: Option<Lang>,
  pub(crate) fps: u32,
}

impl RenderAnimationTask {
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
      format,
      quality,
      lossless,
      fps,
      images,
      css,
      stylesheets,
      keyframes,
      device_pixel_ratio: dpr,
      font_families,
      lang,
    } = options;
    let scenes = scenes
      .into_iter()
      .map(|scene| Ok((NodeSource::from_js(scene.node)?, scene.duration_ms)))
      .collect::<Result<Vec<_>>>()?;

    if scenes.is_empty() {
      return Err(Error::new(
        Status::InvalidArg,
        "Expected at least one animation scene".to_owned(),
      ));
    }

    if fps == 0 {
      return Err(Error::new(
        Status::InvalidArg,
        "Expected fps to be greater than 0".to_owned(),
      ));
    }

    Ok(Self {
      scenes: Some(scenes),
      fonts: state.fonts_for_render(font_families.as_deref())?,
      state,
      viewport: Viewport::new((width, height))
        .with_device_pixel_ratio(device_pixel_ratio(dpr.map(|ratio| ratio as f32))),
      format: format.unwrap_or(AnimationOutputFormat::WebP),
      quality,
      lossless,
      draw_debug_border: draw_debug_border.unwrap_or_default(),
      css: deserialize_css(css, stylesheets)?,
      keyframes: deserialize_keyframes(keyframes)?,
      images: collect_images(env, images)?,
      font_families: font_families.map(FontFamily::from_names),
      lang: parse_lang(lang.as_deref()).map_err(map_error)?,
      fps,
    })
  }
}

impl Task for RenderAnimationTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    crate::pool::install(move || {
      let Some(scenes) = self.scenes.take() else {
        unreachable!()
      };
      let initialized_images = decode_images(&self.state.resource_cache, take(&mut self.images))?;
      let mut document_css = Vec::new();
      let scenes = read_scenes(scenes, &mut document_css)?;
      let stylesheet = stylesheet(
        &self.state.resource_cache,
        append_css(take(&mut self.css), document_css),
        take(&mut self.keyframes),
      )
      .map_err(map_error)?;
      let scene_options = scenes
        .into_iter()
        .map(|(node, duration_ms)| {
          SequentialScene::builder()
            .duration_ms(duration_ms)
            .options(
              RenderOptions::builder()
                .viewport(self.viewport)
                .images(initialized_images.clone())
                .resource_cache(self.state.resource_cache.clone())
                .stylesheet(stylesheet.clone())
                .node(node)
                .fonts(&self.fonts)
                .font_families(self.font_families.clone())
                .lang(self.lang)
                .draw_debug_border(self.draw_debug_border)
                .build(),
            )
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
      write_animation(&scene_options, self.fps, format, &mut buffer)
        .map_err(|e| Error::from_reason(e.to_string()))?;

      Ok(buffer)
    })
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

/// Each scene's node tree, adding every HTML scene's `<style>` CSS to `document_css`.
fn read_scenes(
  scenes: Vec<(NodeSource, u32)>,
  document_css: &mut Vec<String>,
) -> Result<Vec<(Node, u32)>> {
  scenes
    .into_iter()
    .map(|(source, duration_ms)| {
      let (node, css) = source.into_parts(&SourceCache::default())?;

      document_css.extend(css);

      Ok((node, duration_ms))
    })
    .collect()
}
