//! Helper functions and utilities for the WebAssembly bindings.

use crate::model::NodeType;
use serde_wasm_bindgen::from_value;
use std::fmt::Display;
use takumi::{layout::node::Node, resources::task::FetchTaskCollection};
use wasm_bindgen::prelude::*;

/// Maps any error to a JavaScript Error object.
pub fn map_error<E: Display>(err: E) -> js_sys::Error {
  js_sys::Error::new(&err.to_string())
}

/// Type alias for JavaScript result.
pub type JsResult<T> = Result<T, js_sys::Error>;

/// Collects the fetch task urls from the node.
#[wasm_bindgen(js_name = extractResourceUrls)]
pub fn extract_resource_urls(node: NodeType) -> JsResult<Vec<String>> {
  let node: Node = from_value(node.into()).map_err(map_error)?;

  let mut collection = FetchTaskCollection::default();

  node.collect_fetch_tasks(&mut collection);
  node.collect_style_fetch_tasks(&mut collection);

  Ok(
    collection
      .into_inner()
      .iter()
      .map(|task| task.to_string())
      .collect(),
  )
}
