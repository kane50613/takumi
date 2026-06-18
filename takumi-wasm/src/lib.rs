//! WebAssembly bindings for Takumi.

#![deny(clippy::unwrap_used, clippy::expect_used)]
#![deny(missing_docs)]

mod helper;
mod model;
mod renderer;

pub use helper::*;
pub use model::*;
pub use renderer::*;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = include_str!("./dts-header.d.ts");

/// Options for `Renderer.configureImageCache`.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageCacheOptions {
  pub(crate) max_bytes: Option<f64>,
  pub(crate) max_size: Option<f64>,
}
