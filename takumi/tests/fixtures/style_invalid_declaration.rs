use takumi::prelude::*;

use crate::test_utils::run_fixture_test;

#[test]
fn style_invalid_declaration_keeps_the_valid_ones() {
  let style: Style = serde_json::from_value(serde_json::json!({
    "height": "tall",
    "padding": 16,
    "backgroundColor": "#ff0000",
  }))
  .expect("a style object with an invalid value still deserializes");

  run_fixture_test(
    Node::container([Node::text("Kept".to_string())]).with_style(style),
    "style_invalid_declaration_keeps_the_valid_ones",
  );
}
