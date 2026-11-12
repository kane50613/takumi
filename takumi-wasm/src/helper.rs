//! Helper functions and utilities for the WebAssembly bindings.

use std::fmt::Display;

use wasm_bindgen::prelude::wasm_bindgen;

/// Maps any error to a JavaScript Error object.
pub fn map_error<E: Display>(err: E) -> js_sys::Error {
  js_sys::Error::new(&err.to_string())
}

/// Sets the byte budget of the resolved-glyph and glyph-mask cache of each
/// `Renderer` created after this call; `0` stops caching. Defaults to 8 MiB.
///
/// Raise it for scripts with large glyph sets: a CJK outline runs a few
/// kilobytes, so the default holds around a thousand of them and a page of
/// Chinese re-rasterizes glyphs it just evicted.
#[wasm_bindgen(js_name = setGlyphCacheMaxBytes)]
pub fn set_glyph_cache_max_bytes(bytes: f64) {
  takumi_bindings_common::renderer::set_glyph_cache_max_bytes(bytes);
}
