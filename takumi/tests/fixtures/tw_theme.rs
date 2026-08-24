use std::{str::FromStr, sync::Arc};

use takumi::prelude::*;

use crate::test_utils::{CONTEXT, create_test_viewport, run_fixture_test_with_options};

#[test]
fn test_tw_theme_tokens() {
  let mut stylesheet = StyleSheet::default();

  stylesheet.set_theme(Theme::from_unordered([
    ("--color-brand-500".to_owned(), "#5b21b6".to_owned()),
    ("--color-red-500".to_owned(), "#00a63e".to_owned()),
    ("--spacing-gutter".to_owned(), "2.5rem".to_owned()),
  ]));

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

  run_fixture_test_with_options(options, "tw_theme");
}
