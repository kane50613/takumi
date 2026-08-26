//! Data models and types for the WebAssembly bindings.

use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Deserializer, de::Error as DeError};
use serde_bytes::ByteBuf;
use takumi_core::{
  keyframes::deserialize_optional_keyframes,
  layout::node::Node,
  resources::image::ImageCacheMode,
  style::{FontStyle as CssFontStyle, FromCssStr, KeyframesRule},
};
use takumi_raster::DitheringAlgorithm;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
  /// JavaScript object representing a layout node.
  #[wasm_bindgen(typescript_type = "Node")]
  #[derive(Debug)]
  pub type NodeType;

  /// JavaScript object representing renderer construction options.
  #[wasm_bindgen(typescript_type = "RendererOptions")]
  pub type RendererOptionsType;

  /// JavaScript object representing render options.
  #[wasm_bindgen(typescript_type = "RenderOptions")]
  pub type RenderOptionsType;

  /// JavaScript object representing SVG render options.
  #[wasm_bindgen(typescript_type = "SvgRenderOptions")]
  pub type SvgRenderOptionsType;

  /// JavaScript object representing animation render options.
  #[wasm_bindgen(typescript_type = "RenderAnimationOptions")]
  pub type RenderAnimationOptionsType;

  /// JavaScript type for font input (FontDetails or ByteBuf).
  #[wasm_bindgen(typescript_type = "Font")]
  pub type FontType;

  /// JavaScript type for the families produced by `registerFont`.
  #[wasm_bindgen(typescript_type = "RegisteredFamily[]")]
  pub type RegisteredFamiliesType;

  /// JavaScript object representing a measured node tree.
  #[wasm_bindgen(typescript_type = "MeasuredNode")]
  pub type MeasuredNodeType;
}

/// Options for constructing a `Renderer`.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RendererOptions {
  /// Byte budget shared by every cached resource — decoded images, SVG
  /// rasters, parsed stylesheets. `0` disables caching. Defaults to 16 MiB.
  pub cache_max_bytes: Option<u64>,
}

/// Opt-in flags for behavior that becomes the default in a future major.
#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct FutureFlags {
  /// Parse `className` tokens as Tailwind utilities, the way `tw` does.
  pub class_name_utilities: Option<bool>,
}

impl FutureFlags {
  pub(crate) fn class_name_utilities(this: Option<Self>) -> bool {
    this
      .and_then(|flags| flags.class_name_utilities)
      .unwrap_or_default()
  }
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
  /// Pre-fetched images to use during rendering.
  pub images: Option<Vec<ImageSource>>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// CSS custom properties for `:root`, which utilities and `var()` both read.
  pub css_variables: Option<HashMap<String, String>>,
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
  /// Default BCP-47 language applied to the root, inherited by nodes without their own lang.
  pub lang: Option<String>,
  /// Opt-in future behavior flags.
  pub future: Option<FutureFlags>,
}

/// Options for rendering a node tree to an SVG document. SVG is a vector
/// format, so the raster-only knobs (`format`, `quality`, `dithering`,
/// `drawDebugBorder`, `devicePixelRatio`) do not apply.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SvgRenderOptions {
  /// The width of the viewport in pixels.
  pub width: Option<u32>,
  /// The height of the viewport in pixels.
  pub height: Option<u32>,
  /// Pre-fetched images to use during rendering.
  pub images: Option<Vec<ImageSource>>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// CSS custom properties for `:root`, which utilities and `var()` both read.
  pub css_variables: Option<HashMap<String, String>>,
  /// Structured keyframes to register alongside stylesheets.
  #[serde(default, deserialize_with = "deserialize_optional_keyframes")]
  pub(crate) keyframes: Option<Vec<KeyframesRule>>,
  /// The animation timeline time in milliseconds.
  pub time_ms: Option<i64>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  /// Defaults to all registered families in registration order.
  pub font_families: Option<Vec<String>>,
  /// Default BCP-47 language applied to the root, inherited by nodes without their own lang.
  pub lang: Option<String>,
  /// Opt-in future behavior flags.
  pub future: Option<FutureFlags>,
}

/// Options for rendering an animated image.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderAnimationOptions {
  /// The scenes to render sequentially.
  pub scenes: Vec<AnimationScene>,
  /// The width of each frame in pixels.
  pub width: u32,
  /// The height of each frame in pixels.
  pub height: u32,
  /// The output animation format (WebP, APNG, or GIF).
  pub format: Option<AnimationOutputFormat>,
  /// Pre-fetched images to use during rendering.
  pub images: Option<Vec<ImageSource>>,
  /// Whether to draw debug borders around layout elements.
  pub draw_debug_border: Option<bool>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// CSS custom properties for `:root`, which utilities and `var()` both read.
  pub css_variables: Option<HashMap<String, String>>,
  /// Structured keyframes to register alongside stylesheets.
  #[serde(default, deserialize_with = "deserialize_optional_keyframes")]
  pub(crate) keyframes: Option<Vec<KeyframesRule>>,
  /// The device pixel ratio for scaling.
  pub device_pixel_ratio: Option<f32>,
  /// Frames per second for timeline sampling.
  pub fps: u32,
  /// Per-render font stack: ordered family names used as the fallback chain.
  pub font_families: Option<Vec<String>>,
  /// Default BCP-47 language applied to the root, inherited by nodes without their own lang.
  pub lang: Option<String>,
  /// Opt-in future behavior flags.
  pub future: Option<FutureFlags>,
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
  /// Logical family this font is a coverage subset of; expands at render time.
  pub subset_of: Option<String>,
  /// Where this subset sits in its group's fallback order; lowest is tried first.
  pub subset_rank: Option<u32>,
  /// CSS generic family keyword (e.g. `monospace`) this font resolves for.
  pub generic: Option<String>,
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
pub struct FontStyle(pub CssFontStyle);

impl<'de> Deserialize<'de> for FontStyle {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let value = String::deserialize(deserializer)?;
    Ok(Self(
      CssFontStyle::from_css_str(&value).map_err(D::Error::custom)?,
    ))
  }
}

impl From<FontStyle> for CssFontStyle {
  fn from(style: FontStyle) -> Self {
    style.0
  }
}

/// A single scene in a sequential animation timeline.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationScene {
  /// The node tree to render for this scene.
  pub node: Node,
  /// The duration of this scene in milliseconds.
  pub duration_ms: u32,
}
