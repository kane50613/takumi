//! Code written for `ResourceCache` still builds and renders.

#![allow(deprecated)]

use takumi::prelude::*;
use takumi_core::resources::image::ResourceCache;

use crate::test_utils::{CONTEXT, create_test_viewport};

#[test]
fn a_resource_cache_still_renders() {
  let cache = ResourceCache::new(1 << 20);
  let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4"/></svg>"#;
  let source = cache.get_or_decode(svg, ImageCacheMode::Auto).unwrap();
  let options = RenderOptions::builder()
    .viewport(create_test_viewport())
    .node(Node::image(source))
    .fonts(&CONTEXT)
    .stylesheet(cache.get_or_parse_stylesheet(Vec::new()))
    .resource_cache(cache)
    .build();

  takumi::render(options).unwrap();
}
