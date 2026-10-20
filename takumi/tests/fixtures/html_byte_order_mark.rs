use takumi::prelude::*;

use crate::test_utils::run_fixture_test;

/// A document saved with a byte order mark keeps its body, and the body's margin, as in a browser.
#[test]
fn html_byte_order_mark() {
  let node = Node::from_html(
    "\u{feff}<!doctype html><html><body><div style=\"width: 200px; height: 100px; background-color: rgb(59, 130, 246)\"></div></body></html>",
    FromHtmlOptions::default(),
  )
  .unwrap();

  run_fixture_test(node, "html_byte_order_mark");
}
