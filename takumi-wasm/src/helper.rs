//! Helper functions and utilities for the WebAssembly bindings.

use std::fmt::Display;

use wasm_bindgen::prelude::wasm_bindgen;

/// Maps any error to a JavaScript Error object.
pub fn map_error<E: Display>(err: E) -> js_sys::Error {
  js_sys::Error::new(&err.to_string())
}

/// Gives each `Renderer` created after this call a glyph budget, the most bytes its resolved
/// glyphs and glyph masks may hold. `0` stops caching glyphs.
///
/// @deprecated Use `new Renderer({ cache: { budgets: { glyphs } } })`.
#[wasm_bindgen(js_name = setGlyphCacheMaxBytes)]
pub fn set_glyph_cache_max_bytes(bytes: f64) {
  takumi_bindings_common::renderer::set_glyph_cache_max_bytes(bytes);
}
