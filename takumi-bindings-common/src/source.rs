//! The source a wasm binding renders: a node tree, the `{ json }` text of one, or HTML.

use js_sys::{Error, Reflect};
use serde_wasm_bindgen::from_value;
use takumi_core::html::{FromHtmlOptions, parse_html};
use wasm_bindgen::JsValue;

use crate::{
  input::node_from_json,
  source_cache::{ParsedSource, SourceCache, SourceKind},
};

/// The node tree `source` describes, and the CSS of its `<style>` elements when it is HTML.
/// Approximate: `takumi_core::html` does not rebuild misnested formatting elements or move stray table content.
pub fn read_source(source: JsValue, sources: &SourceCache) -> Result<ParsedSource, Error> {
  if let Some(html) = source.as_string() {
    return sources.parse(SourceKind::Html, &html, |html| {
      let parsed = parse_html(html, FromHtmlOptions::default()).map_err(error)?;

      Ok((parsed.node, parsed.css))
    });
  }

  if let Some(json) = Reflect::get(&source, &JsValue::from_str("json"))
    .ok()
    .and_then(|json| json.as_string())
  {
    // The object path's errors come from `serde_wasm_bindgen`, which starts them with `Error: `.
    return sources.parse(SourceKind::Json, &json, |json| {
      let node = node_from_json(json).map_err(|message| error(format!("Error: {message}")))?;

      Ok((node, Vec::new()))
    });
  }

  Ok((from_value(source).map_err(error)?, Vec::new()))
}

fn error(message: impl ToString) -> Error {
  Error::new(&message.to_string())
}
