use napi::bindgen_prelude::*;
use napi_derive::napi;
use takumi::layout::node::Node;

use crate::deserialize_with_tracing;

/// Collects the fetch task urls from the node.
#[napi(ts_args_type = "node: Node")]
pub fn extract_resource_urls(node: Object) -> Result<Vec<String>> {
  let node: Node = deserialize_with_tracing(node)?;
  Ok(node.resource_urls().map(str::to_owned).collect())
}
