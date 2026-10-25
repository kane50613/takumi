//! Reading JS values in the WebAssembly bindings.

use serde::de::DeserializeOwned;
use serde_wasm_bindgen::from_value;
use takumi_core::style::InvalidDeclarations;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
  #[wasm_bindgen(js_namespace = console, js_name = warn)]
  fn console_warn(message: &str);
}

/// Deserializes `value`, reporting each declaration its style objects drop through `console.warn`, as a browser console reports one.
pub fn from_js<T: DeserializeOwned>(value: JsValue) -> Result<T, js_sys::Error> {
  let (value, invalid) = InvalidDeclarations::collect(|| from_value(value));

  for message in invalid.messages() {
    console_warn(&format!("takumi: ignoring {message}"));
  }

  value.map_err(|error| js_sys::Error::new(&error.to_string()))
}
