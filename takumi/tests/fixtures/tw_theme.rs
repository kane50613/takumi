use std::{str::FromStr, sync::Arc};

use takumi::prelude::*;

use crate::test_utils::{CONTEXT, create_test_viewport, run_fixture_test_with_css};

const CSS: &str =
  ":root { --color-brand-500: #5b21b6; --color-red-500: #00a63e; --spacing-gutter: 2.5rem }";

#[test]
fn test_tw_theme_tokens() {
  let stylesheet = StyleSheet::parse_list([CSS]).expect("stylesheet should parse");

  let node = Node::container([
    Node::container([]).with_tw(
      TailwindValues::from_str("bg-brand-500 w-full h-32").expect("tailwind values should parse"),
    ),
    Node::container([]).with_tw(
      TailwindValues::from_str("bg-red-500 w-full h-32").expect("tailwind values should parse"),
    ),
  ])
  .with_tw(
    TailwindValues::from_str("flex flex-col gap-gutter p-gutter bg-white w-full h-full")
      .expect("tailwind values should parse"),
  );

  let options = RenderOptions::builder()
    .viewport(create_test_viewport())
    .node(node)
    .fonts(&CONTEXT)
    .stylesheet(Arc::new(stylesheet))
    .build();

  run_fixture_test_with_css(options, CSS, "tw_theme");
}
