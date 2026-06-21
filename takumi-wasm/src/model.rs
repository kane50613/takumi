//! Data models and types for the WebAssembly bindings.

use parley::FontStyle as ParleyFontStyle;
use serde::{Deserialize, Deserializer};
use serde_bytes::ByteBuf;
use std::sync::Arc;
use takumi_core::{
  keyframes::deserialize_optional_keyframes,
  layout::{node::Node, style::KeyframesRule},
  resources::image::ImageCacheMode,
};
use takumi_raster::DitheringAlgorithm;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
  /// JavaScript object representing a layout node.
  #[wasm_bindgen(typescript_type = "Node")]
  #[derive(Debug)]
  pub type NodeType;

  /// JavaScript object representing render options.
  #[wasm_bindgen(typescript_type = "RenderOptions")]
  pub type RenderOptionsType;

  /// JavaScript object representing animation render options.
  #[wasm_bindgen(typescript_type = "RenderAnimationOptions")]
  pub type RenderAnimationOptionsType;

  /// JavaScript object representing frame encoding options.
  #[wasm_bindgen(typescript_type = "EncodeFramesOptions")]
  pub type EncodeFramesOptionsType;

  /// JavaScript type for font input (FontDetails or ByteBuf).
  #[wasm_bindgen(typescript_type = "Font")]
  pub type FontType;

  /// JavaScript type for the families produced by `registerFont`.
  #[wasm_bindgen(typescript_type = "RegisteredFamily[]")]
  pub type RegisteredFamiliesType;

  /// JavaScript object representing an image source.
  #[wasm_bindgen(typescript_type = "ImageSource")]
  pub type ImageSourceType;

  /// JavaScript object representing a measured node tree.
  #[wasm_bindgen(typescript_type = "MeasuredNode")]
  pub type MeasuredNodeType;

  /// JavaScript object representing an animation frame source.
  #[wasm_bindgen(typescript_type = "AnimationFrameSource")]
  pub type AnimationFrameSourceType;

  /// JavaScript object representing an animation scene source.
  #[wasm_bindgen(typescript_type = "AnimationSceneSource")]
  pub type AnimationSceneSourceType;
}

/// Options for rendering an image.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RenderOptions {
  /// The width of the image in pixels.
  pub width: Option<u32>,
  /// The height of the image in pixels.
  pub height: Option<u32>,
  /// The output image format (PNG, JPEG, WebP, or ICO).
  pub format: Option<OutputFormat>,
  /// The JPEG quality (0-100), if applicable.
  pub quality: Option<u8>,
  /// Pre-fetched image resources to use during rendering.
  pub images: Option<Vec<ImageSource>>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// Structured keyframes to register alongside stylesheets.
  #[serde(default, deserialize_with = "deserialize_optional_keyframes")]
  pub(crate) keyframes: Option<Vec<KeyframesRule>>,
  /// Whether to draw debug borders around layout elements.
  pub draw_debug_border: Option<bool>,
  /// The device pixel ratio for scaling.
  pub device_pixel_ratio: Option<f32>,
  /// The animation timeline time in milliseconds.
  pub time_ms: Option<i64>,
  /// The output dithering algorithm.
  pub dithering: Option<DitheringAlgorithm>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  /// Defaults to all registered families in registration order.
  pub font_families: Option<Vec<String>>,
}

/// Options for rendering an animated image.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderAnimationOptions {
  /// The scenes to render sequentially.
  pub scenes: Vec<AnimationSceneSource>,
  /// The width of each frame in pixels.
  pub width: u32,
  /// The height of each frame in pixels.
  pub height: u32,
  /// The output animation format (WebP, APNG, or GIF).
  pub format: Option<AnimationOutputFormat>,
  /// Pre-fetched image resources to use during rendering.
  pub images: Option<Vec<ImageSource>>,
  /// Whether to draw debug borders around layout elements.
  pub draw_debug_border: Option<bool>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// The device pixel ratio for scaling.
  pub device_pixel_ratio: Option<f32>,
  /// Frames per second for timeline sampling.
  pub fps: u32,
  /// Per-render font stack: ordered family names used as the fallback chain.
  pub font_families: Option<Vec<String>>,
}

/// Options for encoding a precomputed frame sequence.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodeFramesOptions {
  /// The width of each frame in pixels.
  pub width: u32,
  /// The height of each frame in pixels.
  pub height: u32,
  /// The output animation format (WebP, APNG, or GIF).
  pub format: Option<AnimationOutputFormat>,
  /// Pre-fetched image resources to use during rendering.
  pub images: Option<Vec<ImageSource>>,
  /// Whether to draw debug borders around layout elements.
  pub draw_debug_border: Option<bool>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// The device pixel ratio for scaling.
  pub device_pixel_ratio: Option<f32>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  pub font_families: Option<Vec<String>>,
}

/// Details for loading a custom font.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontDetails {
  /// The name of the font family.
  pub name: Option<String>,
  /// The raw font data bytes.
  pub data: ByteBuf,
  /// The font weight (e.g., 400 for normal, 700 for bold).
  pub weight: Option<f64>,
  /// The font style (normal, italic, or oblique).
  pub style: Option<FontStyle>,
}

/// Font input, either as detailed object or raw buffer.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Font {
  /// Font loaded with detailed configuration.
  Object(FontDetails),
  /// Raw font buffer.
  Buffer(ByteBuf),
}

/// An image source with its URL and raw data.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSource {
  /// The source URL of the image.
  pub src: Arc<str>,
  /// The raw image data bytes.
  pub data: ByteBuf,
  /// Cache policy for the decoded image. Defaults to `"auto"`.
  pub cache: Option<ImageCacheMode>,
}

/// Output format for static images.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
  /// PNG format.
  Png,
  /// JPEG format.
  Jpeg,
  /// WebP format.
  WebP,
  /// ICO format.
  Ico,
  /// Raw pixels format.
  Raw,
}

impl OutputFormat {
  /// Maps to a raster [`OutputFormat`](takumi_raster::OutputFormat). JPEG folds
  /// `quality` (default 75); WebP is lossless-only on wasm, so `quality` is
  /// ignored for it.
  pub(crate) fn into_image_output_format(self, quality: Option<u8>) -> takumi_raster::OutputFormat {
    use takumi_raster::{OutputFormat as RasterOutputFormat, Quality};
    match self {
      OutputFormat::Png => RasterOutputFormat::Png,
      OutputFormat::Jpeg => RasterOutputFormat::Jpeg {
        quality: quality.map_or_else(Quality::default, Quality::new),
      },
      OutputFormat::WebP => RasterOutputFormat::WebPLossless,
      OutputFormat::Ico => RasterOutputFormat::Ico,
      OutputFormat::Raw => unreachable!("Raw format should be handled separately"),
    }
  }
}

/// Output format for animated images.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnimationOutputFormat {
  /// Animated PNG format.
  APng,
  /// Animated WebP format.
  WebP,
  /// Animated GIF format.
  Gif,
}

/// Font style input parsed from CSS-like font-style strings.
#[derive(Clone, Copy)]
pub struct FontStyle(pub ParleyFontStyle);

impl<'de> Deserialize<'de> for FontStyle {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let value = String::deserialize(deserializer)?;
    Ok(Self(ParleyFontStyle::parse_css(&value).unwrap_or_default()))
  }
}

impl From<FontStyle> for ParleyFontStyle {
  fn from(style: FontStyle) -> Self {
    style.0
  }
}

/// A single frame in an animation sequence.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationFrameSource {
  /// The node tree to render for this frame.
  pub node: Node,
  /// The duration of this frame in milliseconds.
  pub duration_ms: u32,
}

/// A single scene in a sequential animation timeline.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationSceneSource {
  /// The node tree to render for this scene.
  pub node: Node,
  /// The duration of this scene in milliseconds.
  pub duration_ms: u32,
}
