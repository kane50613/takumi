//! WebAssembly bindings for takumi's paint tree.
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::RwLock;

use serde::Deserialize;
use serde_wasm_bindgen::{from_value, to_value};
use takumi_bindings_common::{
  default_fonts,
  input::{Font, ImageSource, decode_images, register_font},
  stylesheet,
};
use takumi_core::{
  Fonts,
  layout::node::Node,
  paint_tree::{PaintTreeOptions, paint_tree},
  resources::image::ResourceCache,
  style::{CssSource, FontFamily, Lang},
  viewport::{DEFAULT_DEVICE_PIXEL_RATIO, Viewport},
};
use wasm_bindgen::prelude::*;

fn map_error(error: impl core::fmt::Display) -> js_sys::Error {
  js_sys::Error::new(&error.to_string())
}

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = include_str!("./dts-header.d.ts");

#[wasm_bindgen]
extern "C" {
  /// JavaScript object representing a layout node.
  #[wasm_bindgen(typescript_type = "Node")]
  pub type NodeType;
  /// JavaScript type for font input (details object or raw buffer).
  #[wasm_bindgen(typescript_type = "Font")]
  pub type FontType;
  /// JavaScript type for the families produced by `registerFont`.
  #[wasm_bindgen(typescript_type = "RegisteredFamily[]")]
  pub type RegisteredFamiliesType;
  /// JavaScript object representing paint options.
  #[wasm_bindgen(typescript_type = "PaintOptions")]
  pub type PaintOptionsType;
  /// JavaScript object representing a paint tree.
  #[wasm_bindgen(typescript_type = "PaintTree")]
  pub type PaintTreeType;
}

/// Options for [`Painter::paint`].
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PaintOptions {
  width: Option<u32>,
  height: Option<u32>,
  device_pixel_ratio: Option<f32>,
  images: Option<Vec<ImageSource>>,
  css: Option<Vec<CssSource>>,
  font_families: Option<Vec<String>>,
  lang: Option<String>,
}

/// A painter holding registered fonts and a decoded-resource cache.
///
/// State lives behind a lock and every method takes `&self`, mirroring the
/// other wasm bindings: a panic mid-call can't leave the wasm-bindgen borrow
/// flag permanently set.
#[wasm_bindgen]
pub struct Painter {
  state: RwLock<Fonts>,
  resource_cache: ResourceCache,
}

#[wasm_bindgen]
impl Painter {
  /// Creates a painter with the bundled last-resort fonts.
  #[wasm_bindgen(constructor)]
  pub fn new() -> Result<Painter, js_sys::Error> {
    Ok(Painter {
      state: RwLock::new(default_fonts().map_err(map_error)?),
      resource_cache: ResourceCache::default(),
    })
  }

  /// Registers a font (raw bytes or a details object), returning the families
  /// it produced.
  #[wasm_bindgen(js_name = registerFont)]
  pub fn register_font(&self, font: FontType) -> Result<RegisteredFamiliesType, js_sys::Error> {
    let font: Font = from_value(font.into()).map_err(map_error)?;
    let mut state = self
      .state
      .try_write()
      .map_err(|error| js_sys::Error::new(&format!("Painter state is locked: {error}")))?;
    let registered = register_font(&mut state, font).map_err(map_error)?;
    Ok(to_value(&registered).map_err(map_error)?.unchecked_into())
  }

  /// Lays out a node tree and returns what painting it would draw.
  pub fn paint(
    &self,
    node: NodeType,
    options: Option<PaintOptionsType>,
  ) -> Result<PaintTreeType, js_sys::Error> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let options: PaintOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();
    let images = decode_images(
      &self.resource_cache,
      options.images.as_deref().unwrap_or_default(),
    )
    .map_err(map_error)?;
    let lang = options
      .lang
      .as_deref()
      .map(Lang::parse)
      .transpose()
      .map_err(map_error)?;
    let state = self
      .state
      .try_read()
      .map_err(|error| js_sys::Error::new(&format!("Painter state is locked: {error}")))?;
    let tree = paint_tree(
      PaintTreeOptions::builder()
        .viewport(
          Viewport::new((options.width, options.height)).with_device_pixel_ratio(
            options
              .device_pixel_ratio
              .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
          ),
        )
        .fonts(&state)
        .node(node)
        .images(images)
        .stylesheet(stylesheet(&self.resource_cache, options.css, Vec::new()).map_err(map_error)?)
        .font_families(options.font_families.map(FontFamily::from_names))
        .lang(lang)
        .build(),
    )
    .map_err(map_error)?;
    Ok(to_value(&tree).map_err(map_error)?.unchecked_into())
  }
}
