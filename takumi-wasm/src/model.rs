//! Data models and types for the WebAssembly bindings.

use serde::Deserialize;
pub use takumi_bindings_common::input::{Font, ImageSource};
use takumi_core::{
  keyframes::deserialize_optional_keyframes,
  layout::node::Node,
  style::{CssSource, KeyframesRule},
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
  /// rasters, parsed stylesheets. `0` disables caching. Defaults to 64 MiB.
  pub cache_max_bytes: Option<u64>,
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
  /// CSS to apply before rendering: stylesheet text, or a rule written as an
  /// object.
  pub css: Option<Vec<CssSource>>,
  /// @deprecated Use `css` instead. Will be removed in v3.
  pub stylesheets: Option<Vec<String>>,
  /// @deprecated Use a `{ keyframes, steps }` entry in `css` instead. Will be removed in v3.
  #[serde(default, deserialize_with = "deserialize_optional_keyframes")]
  pub(crate) keyframes: Option<Vec<KeyframesRule>>,
  /// Whether to draw debug borders around layout elements.
  pub draw_debug_border: Option<bool>,
  /// The device pixel ratio for scaling.
  pub device_pixel_ratio: Option<f32>,
  /// The animation timeline time in milliseconds.
  pub time_ms: Option<i64>,
  /// Dithers gradient fills before they quantize to 8-bit.
  pub dithering: Option<DitheringAlgorithm>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  /// Defaults to all registered families in registration order.
  pub font_families: Option<Vec<String>>,
  /// Default BCP-47 language applied to the root, inherited by nodes without their own lang.
  pub lang: Option<String>,
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
  /// CSS to apply before rendering: stylesheet text, or a rule written as an
  /// object.
  pub css: Option<Vec<CssSource>>,
  /// @deprecated Use `css` instead. Will be removed in v3.
  pub stylesheets: Option<Vec<String>>,
  /// @deprecated Use a `{ keyframes, steps }` entry in `css` instead. Will be removed in v3.
  #[serde(default, deserialize_with = "deserialize_optional_keyframes")]
  pub(crate) keyframes: Option<Vec<KeyframesRule>>,
  /// The animation timeline time in milliseconds.
  pub time_ms: Option<i64>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  /// Defaults to all registered families in registration order.
  pub font_families: Option<Vec<String>>,
  /// Default BCP-47 language applied to the root, inherited by nodes without their own lang.
  pub lang: Option<String>,
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
  /// CSS to apply before rendering: stylesheet text, or a rule written as an
  /// object.
  pub css: Option<Vec<CssSource>>,
  /// @deprecated Use `css` instead. Will be removed in v3.
  pub stylesheets: Option<Vec<String>>,
  /// @deprecated Use a `{ keyframes, steps }` entry in `css` instead. Will be removed in v3.
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

/// A single scene in a sequential animation timeline.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationScene {
  /// The node tree to render for this scene.
  pub node: Node,
  /// The duration of this scene in milliseconds.
  pub duration_ms: u32,
}
