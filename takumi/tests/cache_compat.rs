//! The `resource_cache` builder setter still takes the render's cache.

#![allow(deprecated)]

use takumi::prelude::*;

use crate::test_utils::{CONTEXT, create_test_viewport};

#[test]
fn the_resource_cache_setter_still_renders() {
  let cache = Cache::default();
  let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4"/></svg>"#;
  let source = cache.decode_image(svg, ImageCacheMode::Auto).unwrap();
  let options = RenderOptions::builder()
    .viewport(create_test_viewport())
    .node(Node::image(source))
    .fonts(&CONTEXT)
    .resource_cache(cache)
    .build();

  takumi::render(options).unwrap();
}
