use std::{collections::HashMap, mem::take, sync::Arc};

use napi::bindgen_prelude::*;
use takumi_core::{
  layout::node::Node,
  style::{FontFamily, KeyframesRule, Lang},
  viewport::{DEFAULT_DEVICE_PIXEL_RATIO, Viewport},
};
use takumi_raster::{
  AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFormat, RenderOptions,
  SequentialScene, write_animation,
};

use crate::{
  buffer_from_object, deserialize_with_tracing, map_error, parse_stylesheet,
  renderer::{
    AnimationOutputFormat, ImageCacheMode, ImageSource, RenderAnimationOptions, RendererState,
    decode_images, deserialize_keyframes, webp_lossless,
  },
};

pub struct RenderAnimationTask {
  pub(crate) scenes: Option<Vec<(Node, u32)>>,
  pub(crate) state: Arc<RendererState>,
  pub(crate) viewport: Viewport,
  pub(crate) format: AnimationOutputFormat,
  pub(crate) quality: Option<u8>,
  pub(crate) lossless: Option<bool>,
  pub(crate) draw_debug_border: bool,
  pub(crate) stylesheets: Option<Vec<String>>,
  pub(crate) keyframes: Vec<KeyframesRule>,
  pub(crate) images: HashMap<Arc<str>, (Buffer, ImageCacheMode)>,
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
      stylesheets,
      keyframes,
      device_pixel_ratio,
      font_families,
      lang,
    } = options;
    let scenes = scenes
      .into_iter()
      .map(|scene| Ok((deserialize_with_tracing(scene.node)?, scene.duration_ms)))
      .collect::<Result<Vec<(Node, u32)>>>()?;

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
      state,
      viewport: Viewport::new((width, height)).with_device_pixel_ratio(
        device_pixel_ratio
          .map(|ratio| ratio as f32)
          .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
      ),
      format: format.unwrap_or(AnimationOutputFormat::WebP),
      quality,
      lossless,
      draw_debug_border: draw_debug_border.unwrap_or_default(),
      stylesheets,
      keyframes: deserialize_keyframes(keyframes)?,
      images: images
        .unwrap_or_default()
        .into_iter()
        .map(|image: ImageSource<'_>| {
          Ok((
            Arc::from(image.src),
            (
              buffer_from_object(env, image.data)?,
              image.cache.unwrap_or_default(),
            ),
          ))
        })
        .collect::<Result<_>>()?,
      font_families: font_families.map(FontFamily::from_names),
      lang: lang
        .as_deref()
        .map(Lang::parse)
        .transpose()
        .map_err(map_error)?,
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
      let fonts = self.state.fonts.load();
      let initialized_images = decode_images(&self.state.image_cache, take(&mut self.images))?;
      let stylesheet = parse_stylesheet(take(&mut self.stylesheets), take(&mut self.keyframes))?;
      let scene_options = scenes
        .into_iter()
        .map(|(node, duration_ms)| {
          SequentialScene::builder()
            .duration_ms(duration_ms)
            .options(
              RenderOptions::builder()
                .viewport(self.viewport)
                .images(initialized_images.clone())
                .stylesheet(stylesheet.clone())
                .node(node)
                .fonts(&fonts)
                .font_families(self.font_families.clone())
                .lang(self.lang.clone())
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
