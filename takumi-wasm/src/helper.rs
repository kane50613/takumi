//! Helper functions and utilities for the WebAssembly bindings.

use std::fmt::Display;

/// Maps any error to a JavaScript Error object.
pub fn map_error<E: Display>(err: E) -> js_sys::Error {
  js_sys::Error::new(&err.to_string())
}
