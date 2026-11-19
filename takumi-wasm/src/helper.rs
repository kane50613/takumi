//! Helper functions and utilities for the WebAssembly bindings.

use std::fmt::Display;

use takumi_bindings_common::renderer::set_default_glyph_mode;
use wasm_bindgen::prelude::wasm_bindgen;

/// Maps any error to a JavaScript Error object.
pub fn map_error<E: Display>(err: E) -> js_sys::Error {
  js_sys::Error::new(&err.to_string())
}

/// Keeps no glyphs in each `Renderer` created after this call when `bytes` is `0`, and glyphs again
/// otherwise. Glyphs share the renderer's `cache.maxBytes` with every other kind of data.
///
/// @deprecated Use `new Renderer({ cache: { glyphs: "none" } })`.
#[wasm_bindgen(js_name = setGlyphCacheMaxBytes)]
pub fn set_glyph_cache_max_bytes(bytes: f64) {
  set_default_glyph_mode(bytes);
}
