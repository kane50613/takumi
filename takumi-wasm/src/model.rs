//! Data models and types for the WebAssembly bindings.

use serde::Deserialize;
pub use takumi_bindings_common::{
  input::{Font, ImageSource},
  options::{AnimationOutputFormat, OutputFormat, RenderOptions, RendererOptions},
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
  /// A layout node, or HTML markup to parse in its place.
  #[wasm_bindgen(typescript_type = "Node | string | { json: string }")]
  #[derive(Debug)]
  pub type SourceType;

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

  /// JavaScript object listing what a node tree needs fetched.
  #[wasm_bindgen(typescript_type = "RenderNeeds")]
  pub type RenderNeedsType;
  /// JavaScript list of a render's fonts, described before their bytes arrive.
  #[wasm_bindgen(typescript_type = "ListedFont[]")]
  pub type ListedFontsType;
}

/// A single scene in a sequential animation timeline.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationScene {
  /// The node tree to render for this scene, the `{ json }` text of one, or HTML markup.
  #[serde(with = "serde_wasm_bindgen::preserve")]
  pub node: JsValue,
  /// The duration of this scene in milliseconds.
  pub duration_ms: u32,
}

/// Options for rendering an animated image.
pub type RenderAnimationOptions =
  takumi_bindings_common::options::RenderAnimationOptions<AnimationScene>;
