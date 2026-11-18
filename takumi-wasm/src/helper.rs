//! Helper functions and utilities for the WebAssembly bindings.

use std::fmt::Display;

use takumi_core::resources::glyph_cache;
use wasm_bindgen::prelude::wasm_bindgen;

/// Maps any error to a JavaScript Error object.
pub fn map_error<E: Display>(err: E) -> js_sys::Error {
  js_sys::Error::new(&err.to_string())
}

/// Sets the byte budget shared by the resolved-glyph and glyph-mask caches;
/// `0` stops caching. Defaults to 8 MiB.
///
/// These caches live in the module, not in a `Renderer`, so this budget covers
/// every renderer sharing the module instance. The value is read when a cache
/// is first used, so call this before the first render.
///
/// Raise it for scripts with large glyph sets: a CJK outline runs a few
/// kilobytes, so the default holds around a thousand of them and a page of
/// Chinese re-rasterizes glyphs it just evicted.
#[wasm_bindgen(js_name = setGlyphCacheMaxBytes)]
pub fn set_glyph_cache_max_bytes(bytes: f64) {
  glyph_cache::set_glyph_cache_max_bytes(bytes.max(0.0) as usize);
}
