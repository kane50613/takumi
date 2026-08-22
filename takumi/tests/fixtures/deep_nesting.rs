use takumi::{
  measure,
  prelude::{Length::*, *},
};

use crate::test_utils::{CONTEXT, create_test_viewport};

const STACK_OVERFLOW_DEPTH: usize = 200;

fn make_text_node(text: String) -> Node {
  Node::text(text).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::font_size(Px(20.0).into()))
      .with(StyleDeclaration::font_weight(FontWeight::from(600.0)))
      .with(StyleDeclaration::color(ColorInput::Value(Color([
        35, 35, 35, 255,
      ])))),
  )
}

fn iterative_nesting_node(depth: usize) -> Node {
  let mut current_node = make_text_node("Deep".to_string());

  for _ in 0..depth {
    current_node = Node::container([current_node]);
  }

  current_node
}

#[test]
fn deep_nesting_stack_overflow() {
  let options = RenderOptions::builder()
    .viewport(create_test_viewport())
    .node(iterative_nesting_node(STACK_OVERFLOW_DEPTH))
    .fonts(&CONTEXT)
    .build();

  let measured = measure(options).unwrap();

  assert!(measured.width > 0.0);
}
