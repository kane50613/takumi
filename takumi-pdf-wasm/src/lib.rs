//! WebAssembly bindings for takumi-pdf — proof of concept.

#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use js_sys::{Array, Uint8Array};
use serde_wasm_bindgen::from_value;
use takumi_core::{Fonts, layout::node::Node, resources::font::FontResource, viewport::Viewport};
use takumi_pdf::PdfOptions;
use wasm_bindgen::prelude::*;

fn map_error(error: impl core::fmt::Debug) -> js_sys::Error {
  js_sys::Error::new(&format!("{error:?}"))
}

/// Renders a node tree to a single-page PDF.
///
/// `fonts` is an array of font files (`Uint8Array`), registered in order; the
/// first family becomes the default.
#[wasm_bindgen(js_name = renderPdf)]
pub fn render_pdf(
  node: JsValue,
  width: u32,
  height: u32,
  fonts: Array,
) -> Result<Vec<u8>, js_sys::Error> {
  let node: Node = from_value(node).map_err(map_error)?;
  let mut registry = Fonts::default();

  for entry in fonts.iter() {
    let bytes = entry
      .dyn_into::<Uint8Array>()
      .map_err(|_| js_sys::Error::new("fonts entries must be Uint8Array"))?;

    registry
      .register(FontResource::new(bytes.to_vec()))
      .map_err(map_error)?;
  }

  takumi_pdf::render(
    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((width, height)))
      .fonts(&registry)
      .build(),
  )
  .map_err(map_error)
}
