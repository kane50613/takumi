//! The source a wasm binding renders: a node tree, the `{ json }` text of one, or HTML.

use std::cell::Cell;

use js_sys::{Error, Reflect};
use serde_wasm_bindgen::from_value;
use takumi_core::html::{FromHtmlOptions, parse_html};
use wasm_bindgen::JsValue;

use crate::{
  input::node_from_json,
  source_cache::{ParsedSource, SourceCache, SourceKind},
};

/// The source text and the node tree object a wasm renderer read last, so a render after `needs`
/// reads each once. Reading the last tree object again returns its earlier parse, so it must not change.
#[derive(Default)]
pub struct JsSourceCache {
  texts: SourceCache,
  tree: Cell<Option<(JsValue, ParsedSource)>>,
}

impl JsSourceCache {
  /// The tree `source` deserializes to, unless `source` is the object read last.
  fn read_tree(&self, source: JsValue) -> Result<ParsedSource, Error> {
    if let Some((held, parsed)) = self.tree.take()
      && held == source
    {
      self.tree.set(Some((held, parsed.clone())));

      return Ok(parsed);
    }

    let parsed: ParsedSource = (from_value(source.clone()).map_err(error)?, Vec::new());

    self.tree.set(Some((source, parsed.clone())));

    Ok(parsed)
  }
}

/// The node tree `source` describes, and the CSS of its `<style>` elements when it is HTML.
/// Approximate: `takumi_core::html` does not rebuild misnested formatting elements or move stray table content.
pub fn read_source(source: JsValue, sources: &JsSourceCache) -> Result<ParsedSource, Error> {
  if let Some(html) = source.as_string() {
    return sources.texts.parse(SourceKind::Html, &html, |html| {
      let parsed = parse_html(html, FromHtmlOptions::default()).map_err(error)?;

      Ok((parsed.node, parsed.css))
    });
  }

  if let Some(json) = Reflect::get(&source, &JsValue::from_str("json"))
    .ok()
    .and_then(|json| json.as_string())
  {
    // The object path's errors come from `serde_wasm_bindgen`, which starts them with `Error: `.
    return sources.texts.parse(SourceKind::Json, &json, |json| {
      let node = node_from_json(json).map_err(|message| error(format!("Error: {message}")))?;

      Ok((node, Vec::new()))
    });
  }

  sources.read_tree(source)
}

fn error(message: impl ToString) -> Error {
  Error::new(&message.to_string())
}
