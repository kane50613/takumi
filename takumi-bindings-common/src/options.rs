//! The render options every image binding reads from JavaScript.

use std::{collections::HashMap, sync::Arc};

use serde::Deserialize;
use takumi_core::{
  Error, Fonts,
  keyframes::deserialize_optional_keyframes,
  layout::node::Node,
  resources::image::{ImageSource as DecodedImage, ResourceCache},
  style::{CssSource, FontFamily, KeyframesRule, StyleSheet},
  viewport::Viewport,
};
use takumi_raster::{
  AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFormat, DitheringAlgorithm,
  OutputFormat as RasterOutputFormat, Quality, RenderOptions as RasterRenderOptions,
};
use takumi_svg::SvgOptions;

use crate::{device_pixel_ratio, input::ImageSource, parse_lang, time_ms};

/// Options for constructing a renderer.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RendererOptions {
  /// Byte budget shared by every cached resource. `0` disables caching.
  pub cache_max_bytes: Option<f64>,
  /// Bytes of decoded fonts that renders' `fonts` lists may keep registered.
  pub font_cache_max_bytes: Option<f64>,
}

impl RendererOptions {
  /// [`Self::cache_max_bytes`] as a byte count.
  pub fn cache_bytes(&self) -> Option<u64> {
    self.cache_max_bytes.map(bytes)
  }

  /// [`Self::font_cache_max_bytes`] as a byte count.
  pub fn font_cache_bytes(&self) -> Option<u64> {
    self.font_cache_max_bytes.map(bytes)
  }
}

/// A JavaScript byte count, negative ones read as zero.
fn bytes(value: f64) -> u64 {
  value.max(0.0) as u64
}

/// Options for rendering an image or an SVG document.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RenderOptions {
  /// The width of the image in pixels.
  pub width: Option<u32>,
  /// The height of the image in pixels.
  pub height: Option<u32>,
  /// The output image format.
  pub format: Option<OutputFormat>,
  /// The quality of lossy formats, 0 to 100.
  pub quality: Option<u8>,
  /// Whether WebP encodes losslessly.
  pub lossless: Option<bool>,
  /// Whether to draw debug borders.
  pub draw_debug_border: Option<bool>,
  /// The device pixel ratio.
  pub device_pixel_ratio: Option<f32>,
  /// Dithering for gradient fills.
  pub dithering: Option<DitheringAlgorithm>,
  /// The animation timeline time in milliseconds.
  pub time_ms: Option<i64>,
  /// The images the render looks up by `src`.
  pub images: Option<Vec<ImageSource>>,
  /// CSS to apply before rendering.
  pub css: Option<Vec<CssSource>>,
  /// Deprecated alias of `css`.
  pub stylesheets: Option<Vec<String>>,
  /// Deprecated top-level keyframes.
  #[serde(default, deserialize_with = "deserialize_optional_keyframes")]
  pub keyframes: Option<Vec<KeyframesRule>>,
  /// The families the render falls back through, in order.
  pub font_families: Option<Vec<String>>,
  /// The `@font-face` files the render's `prepare` failed to download.
  pub failed_font_files: Option<Vec<String>>,
  /// The BCP-47 language of the root.
  pub lang: Option<String>,
}

impl RenderOptions {
  /// The viewport the width, height, and device pixel ratio describe.
  pub fn viewport(&self) -> Viewport {
    Viewport::new((self.width, self.height))
      .with_device_pixel_ratio(device_pixel_ratio(self.device_pixel_ratio))
  }

  /// The raster options for drawing `node` under these options.
  pub fn raster<'f>(&self, node: Node, draw: Draw<'f>) -> Result<RasterRenderOptions<'f>, Error> {
    Ok(
      RasterRenderOptions::builder()
        .viewport(self.viewport())
        .draw_debug_border(self.draw_debug_border.unwrap_or_default())
        .dithering(self.dithering.unwrap_or_default())
        .time_ms(time_ms(self.time_ms))
        .font_families(self.font_families.clone().map(FontFamily::from_names))
        .lang(parse_lang(self.lang.as_deref())?)
        .images(draw.images)
        .resource_cache(draw.cache.clone())
        .stylesheet(draw.stylesheet)
        .fonts(draw.fonts)
        .node(node)
        .build(),
    )
  }

  /// The SVG options for drawing `node` under these options, which draw at a device pixel ratio
  /// of 1.
  pub fn svg<'f>(&self, node: Node, draw: Draw<'f>) -> Result<SvgOptions<'f>, Error> {
    Ok(
      SvgOptions::builder()
        .viewport(Viewport::new((self.width, self.height)))
        .time_ms(time_ms(self.time_ms))
        .font_families(self.font_families.clone().map(FontFamily::from_names))
        .lang(parse_lang(self.lang.as_deref())?)
        .images(draw.images)
        .stylesheet(draw.stylesheet)
        .fonts(draw.fonts)
        .node(node)
        .build(),
    )
  }
}

/// What a render draws with besides its options: the fonts, the stylesheet, and the images.
pub struct Draw<'f> {
  pub fonts: &'f Fonts,
  pub cache: &'f ResourceCache,
  pub stylesheet: Arc<StyleSheet>,
  pub images: HashMap<Arc<str>, DecodedImage>,
}

/// Options for rendering an animation whose scenes each binding reads as `S`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderAnimationOptions<S> {
  /// The scenes to render in order.
  pub scenes: Vec<S>,
  /// The width of each frame in pixels.
  pub width: u32,
  /// The height of each frame in pixels.
  pub height: u32,
  /// The output animation format.
  pub format: Option<AnimationOutputFormat>,
  /// The quality of lossy WebP, 0 to 100.
  pub quality: Option<u8>,
  /// Whether WebP encodes losslessly.
  pub lossless: Option<bool>,
  /// Frames per second for timeline sampling.
  pub fps: u32,
  /// Whether to draw debug borders.
  pub draw_debug_border: Option<bool>,
  /// The device pixel ratio.
  pub device_pixel_ratio: Option<f32>,
  /// The images the render looks up by `src`.
  pub images: Option<Vec<ImageSource>>,
  /// CSS to apply before rendering.
  pub css: Option<Vec<CssSource>>,
  /// Deprecated alias of `css`.
  pub stylesheets: Option<Vec<String>>,
  /// Deprecated top-level keyframes.
  #[serde(default, deserialize_with = "deserialize_optional_keyframes")]
  pub keyframes: Option<Vec<KeyframesRule>>,
  /// The families the render falls back through, in order.
  pub font_families: Option<Vec<String>>,
  /// The `@font-face` files the render's `prepare` failed to download.
  pub failed_font_files: Option<Vec<String>>,
  /// The BCP-47 language of the root.
  pub lang: Option<String>,
}

impl<S> RenderAnimationOptions<S> {
  /// The raster options of each scene's node and how long it shows, every frame drawing with
  /// `draw`.
  pub fn frames<'f>(
    &self,
    scenes: Vec<(Node, u32)>,
    draw: Draw<'f>,
  ) -> Result<Vec<(RasterRenderOptions<'f>, u32)>, Error> {
    let viewport = Viewport::new((self.width, self.height))
      .with_device_pixel_ratio(device_pixel_ratio(self.device_pixel_ratio));
    let font_families = self.font_families.clone().map(FontFamily::from_names);
    let lang = parse_lang(self.lang.as_deref())?;

    Ok(
      scenes
        .into_iter()
        .map(|(node, duration_ms)| {
          let options = RasterRenderOptions::builder()
            .viewport(viewport)
            .draw_debug_border(self.draw_debug_border.unwrap_or_default())
            .font_families(font_families.clone())
            .lang(lang)
            .images(draw.images.clone())
            .resource_cache(draw.cache.clone())
            .stylesheet(Arc::clone(&draw.stylesheet))
            .fonts(draw.fonts)
            .node(node)
            .build();

          (options, duration_ms)
        })
        .collect(),
    )
  }
}

/// Output format for static images.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
  /// WebP format.
  WebP,
  /// PNG format.
  #[default]
  Png,
  /// JPEG format.
  Jpeg,
  /// ICO format.
  Ico,
  /// Raw pixels.
  Raw,
}

impl OutputFormat {
  /// The raster format, `None` for raw pixels. WebP is lossless unless a `quality` is given
  /// without `lossless`, and always lossless on wasm.
  pub fn raster(self, quality: Option<u8>, lossless: Option<bool>) -> Option<RasterOutputFormat> {
    let quality_or_default = || quality.map_or_else(Quality::default, Quality::new);

    Some(match self {
      Self::WebP if webp_lossless(quality, lossless) => RasterOutputFormat::WebPLossless,
      #[cfg(not(target_arch = "wasm32"))]
      Self::WebP => RasterOutputFormat::WebP {
        quality: quality_or_default(),
      },
      #[cfg(target_arch = "wasm32")]
      Self::WebP => RasterOutputFormat::WebPLossless,
      Self::Png => RasterOutputFormat::Png,
      Self::Jpeg => RasterOutputFormat::Jpeg {
        quality: quality_or_default(),
      },
      Self::Ico => RasterOutputFormat::Ico,
      Self::Raw => return None,
    })
  }
}

/// Whether WebP encodes losslessly: when asked to, or when no `quality` is given.
pub fn webp_lossless(quality: Option<u8>, lossless: Option<bool>) -> bool {
  lossless.unwrap_or(quality.is_none())
}

/// Output format for animated images.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AnimationOutputFormat {
  /// Animated WebP.
  #[default]
  WebP,
  /// Animated PNG.
  APng,
  /// Animated GIF.
  Gif,
}

impl AnimationOutputFormat {
  /// The raster animation format. WebP is lossless unless a `quality` is given without
  /// `lossless`, and always lossless on wasm.
  pub fn raster(self, quality: Option<u8>, lossless: Option<bool>) -> AnimationFormat {
    match self {
      #[cfg(not(target_arch = "wasm32"))]
      Self::WebP => AnimationFormat::WebP(
        AnimatedWebpOptions::builder()
          .lossless(webp_lossless(quality, lossless))
          .quality(quality.unwrap_or(75))
          .build(),
      ),
      #[cfg(target_arch = "wasm32")]
      Self::WebP => {
        let _ = (quality, lossless);

        AnimationFormat::WebP(AnimatedWebpOptions::default())
      }
      Self::APng => AnimationFormat::Apng(AnimatedPngOptions::default()),
      Self::Gif => AnimationFormat::Gif(AnimatedGifOptions::default()),
    }
  }
}

impl<S> RenderAnimationOptions<S> {
  /// Why the animation cannot render, if it cannot.
  pub fn invalid(&self) -> Option<&'static str> {
    if self.scenes.is_empty() {
      return Some("Expected at least one animation scene");
    }

    (self.fps == 0).then_some("Expected fps to be greater than 0")
  }
}
