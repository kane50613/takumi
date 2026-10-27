//! The source a wasm binding renders: a node tree, the `{ json }` text of one, or HTML.

use js_sys::{Error, Reflect};
use serde_wasm_bindgen::from_value;
use takumi_core::layout::node::Node;
use takumi_html::{FromHtmlOptions, parse_html};
use wasm_bindgen::JsValue;

use crate::{
  input::node_from_json,
  parse_emoji,
  source_cache::{ParsedSource, SourceCache, SourceKind},
};

/// Splits each emoji in `node` out as an image from the CDN `emoji` names; `from-font` or none
/// leaves them as text.
pub fn split_emojis(node: &mut Node, emoji: Option<&str>) -> Result<(), Error> {
  if let Some(provider) = parse_emoji(emoji).map_err(error)? {
    node.extract_emojis(provider);
  }

  Ok(())
}

/// The node tree `source` describes, and the CSS of its `<style>` elements when it is HTML. HTML
/// and JSON come from `sources` when they are the text it parsed last.
///
/// HTML parses with `takumi_html`, which recovers from less malformed markup than a browser: it
/// does not reconstruct misnested formatting elements or move stray table content out of a table.
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
