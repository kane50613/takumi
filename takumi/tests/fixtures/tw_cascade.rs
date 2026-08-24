use std::{str::FromStr, sync::Arc};

use takumi::prelude::*;

use crate::test_utils::{CONTEXT, create_test_viewport, run_fixture_test_with_css};

const CSS: &str =
  ".card { background-color: #1447e6 } .card.shout { background-color: #00a63e !important }";

#[test]
fn test_tw_loses_to_a_stylesheet_rule() {
  let stylesheet = StyleSheet::parse_list([CSS]).expect("stylesheet should parse");

  let card = |class_name: &str, tw: &str| {
    Node::container([])
      .with_class_name(class_name.to_owned())
      .with_tw(TailwindValues::from_str(tw).expect("tailwind values should parse"))
  };

  let node = Node::container([
    card("card", "bg-red-500 w-full h-32"),
    card("card", "bg-red-500! w-full h-32"),
    card("card shout", "bg-red-500! w-full h-32"),
    card("card", "bg-red-500 bg-brand-500! w-full h-32"),
  ])
  .with_tw(
    TailwindValues::from_str("flex flex-col gap-4 p-4 bg-white w-full h-full")
      .expect("tailwind values should parse"),
  );

  let options = RenderOptions::builder()
    .viewport(create_test_viewport())
    .node(node)
    .fonts(&CONTEXT)
    .stylesheet(Arc::new(stylesheet))
    .build();

  run_fixture_test_with_css(options, CSS, "tw_cascade");
}
