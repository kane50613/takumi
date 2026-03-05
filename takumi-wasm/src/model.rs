//! Data models and types for the WebAssembly bindings.

use serde::Deserialize;
use serde_bytes::ByteBuf;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = r#"
export type AnyNode = { type: string; [key: string]: any };

export type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

export type RenderOptions = {
  /**
   * The width of the image. If not provided, the width will be automatically calculated based on the content.
   */
  width?: number,
  /**
   * The height of the image. If not provided, the height will be automatically calculated based on the content.
   */
  height?: number,
  /**
   * The format of the image.
   * @default "png"
   */
  format?: "png" | "jpeg" | "webp" | "raw",
  /**
   * The quality of JPEG format (0-100).
   */
  quality?: number,
  /**
   * The resources fetched externally. You should collect the fetch tasks first using `extractResourceUrls` and then pass the resources here.
   */
  fetchedResources?: ImageSource[],
  /**
   * CSS stylesheets to apply before rendering.
   */
  stylesheets?: string[],
  /**
   * Whether to draw debug borders.
   */
  drawDebugBorder?: boolean,
  /**
   * Defines the ratio resolution of the image to the physical pixels.
   * @default 1.0
   */
  devicePixelRatio?: number,
};

export type RenderAnimationOptions = {
  width: number,
  height: number,
  format?: "webp" | "apng" | "gif",
  /**
   * The quality of WebP format (0-100). Ignored for APNG and GIF.
   */
  quality?: number,
  drawDebugBorder?: boolean,
};

export type FontDetails = {
  name?: string,
  data: ByteBuf,
  weight?: number,
  style?: "normal" | "italic" | "oblique",
};

export type ImageSource = {
  src: string,
  data: ByteBuf,
};

export type Font = FontDetails | ByteBuf;

export type ConstructRendererOptions = {
  /**
   * The images that needs to be preloaded into the renderer.
   */
  persistentImages?: ImageSource[],
  /**
   * The fonts being used.
   */
  fonts?: Font[],
};

export type MeasuredTextRun = {
  text: string,
  x: number,
  y: number,
  width: number,
  height: number,
};

export type MeasuredNode = {
  width: number,
  height: number,
  transform: [number, number, number, number, number, number],
  children: MeasuredNode[],
  runs: MeasuredTextRun[],
};

export type AnimationFrameSource = {
  node: AnyNode,
  durationMs: number,
};
"#;

#[wasm_bindgen]
extern "C" {
  /// JavaScript object representing a layout node.
  #[wasm_bindgen(typescript_type = "AnyNode")]
  #[derive(Debug)]
  pub type AnyNode;

  /// JavaScript object representing render options.
  #[wasm_bindgen(typescript_type = "RenderOptions")]
  pub type RenderOptionsType;

  /// JavaScript object representing animation render options.
  #[wasm_bindgen(typescript_type = "RenderAnimationOptions")]
  pub type RenderAnimationOptionsType;

  /// JavaScript object representing font details.
  #[wasm_bindgen(typescript_type = "FontDetails")]
  pub type FontDetailsType;

  /// JavaScript type for font input (FontDetails or ByteBuf).
  #[wasm_bindgen(typescript_type = "Font")]
  pub type FontType;

  /// JavaScript object representing renderer construction options.
  #[wasm_bindgen(typescript_type = "ConstructRendererOptions")]
  pub type ConstructRendererOptionsType;

  /// JavaScript object representing an image source.
  #[wasm_bindgen(typescript_type = "ImageSource")]
  pub type ImageSourceType;

  /// JavaScript object representing a measured node tree.
  #[wasm_bindgen(typescript_type = "MeasuredNode")]
  pub type MeasuredNodeType;

  /// JavaScript object representing an animation frame source.
  #[wasm_bindgen(typescript_type = "AnimationFrameSource")]
  pub type AnimationFrameSourceType;
}

/// Options for rendering an image.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RenderOptions {
  /// The width of the image in pixels.
  pub width: Option<u32>,
  /// The height of the image in pixels.
  pub height: Option<u32>,
  /// The output image format (PNG, JPEG, or WebP).
  pub format: Option<OutputFormat>,
  /// The JPEG quality (0-100), if applicable.
  pub quality: Option<u8>,
  /// Pre-fetched image resources to use during rendering.
  pub fetched_resources: Option<Vec<ImageSource>>,
  /// CSS stylesheets to apply before rendering.
  pub stylesheets: Option<Vec<String>>,
  /// Whether to draw debug borders around layout elements.
  pub draw_debug_border: Option<bool>,
  /// The device pixel ratio for scaling.
  pub device_pixel_ratio: Option<f32>,
}

/// Options for rendering an animated image.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderAnimationOptions {
  /// The width of each frame in pixels.
  pub width: u32,
  /// The height of each frame in pixels.
  pub height: u32,
  /// The output animation format (WebP, APNG, or GIF).
  pub format: Option<AnimationOutputFormat>,
  /// The WebP quality (0-100). Ignored for APNG and GIF.
  pub quality: Option<u8>,
  /// Whether to draw debug borders around layout elements.
  pub draw_debug_border: Option<bool>,
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

/// Options for constructing a Renderer instance.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConstructRendererOptions {
  /// The images that needs to be preloaded into the renderer.
  pub persistent_images: Option<Vec<ImageSource>>,
  /// The fonts being used.
  pub fonts: Option<Vec<Font>>,
}

/// An image source with its URL and raw data.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSource {
  /// The source URL of the image.
  pub src: Arc<str>,
  /// The raw image data bytes.
  pub data: ByteBuf,
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
  /// Raw pixels format.
  Raw,
}

impl From<OutputFormat> for takumi::rendering::ImageOutputFormat {
  fn from(format: OutputFormat) -> Self {
    match format {
      OutputFormat::Png => takumi::rendering::ImageOutputFormat::Png,
      OutputFormat::Jpeg => takumi::rendering::ImageOutputFormat::Jpeg,
      OutputFormat::WebP => takumi::rendering::ImageOutputFormat::WebP,
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

/// Font style variants.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum FontStyle {
  /// Normal font style.
  Normal,
  /// Italic font style.
  Italic,
  /// Oblique font style.
  Oblique,
}

impl From<FontStyle> for takumi::parley::FontStyle {
  fn from(style: FontStyle) -> Self {
    match style {
      FontStyle::Italic => takumi::parley::FontStyle::Italic,
      FontStyle::Oblique => takumi::parley::FontStyle::Oblique(None),
      FontStyle::Normal => takumi::parley::FontStyle::Normal,
    }
  }
}

/// A single frame in an animation sequence.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationFrameSource {
  /// The node tree to render for this frame.
  pub node: takumi::layout::node::NodeKind,
  /// The duration of this frame in milliseconds.
  pub duration_ms: u32,
}

/// Key for caching images in the renderer.
#[derive(PartialEq, Eq, Hash)]
pub struct ImageCacheKey {
  pub(crate) src: Box<str>,
  pub(crate) data_hash: u64,
}
