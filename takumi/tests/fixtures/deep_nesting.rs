use takumi::rendering::measure_layout;
use takumi::{
  layout::{
    node::Node,
    style::{
      Color, ColorInput, Display, FlexDirection, FontWeight,
      Length::{Percentage, Px},
      Sides, Style, StyleDeclaration,
    },
  },
  rendering::RenderOptions,
};

use crate::test_utils::{CONTEXT, create_test_viewport, run_fixture_test};

const STACK_OVERFLOW_DEPTH: usize = 200;
const VISUAL_RECURSIVE_DEPTH: usize = 12;

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

fn wrap_in_plain_container(node: Node) -> Node {
  Node::container([node])
}

fn iterative_nesting_node(depth: usize) -> Node {
  let mut current_node = make_text_node("Deep".to_string());

  for _ in 0..depth {
    current_node = wrap_in_plain_container(current_node);
  }

  current_node
}

fn recursive_level_background(level: usize) -> Color {
  let shift = (level.min(VISUAL_RECURSIVE_DEPTH) as u8).saturating_mul(12);
  Color([
    255,
    245u8.saturating_sub(shift / 2),
    230u8.saturating_sub(shift),
    255,
  ])
}

fn recursive_visual_node(level: usize, max_depth: usize) -> Node {
  let label = if level == max_depth {
    "base case: return".to_string()
  } else {
    format!("recursive(level = {})", level)
  };

  let mut children = vec![make_text_node(label)];
  if level < max_depth {
    children.push(recursive_visual_node(level + 1, max_depth));
  }

  Node::container(children.into_boxed_slice()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with_padding(Sides([Px(10.0), Px(10.0), Px(10.0), Px(14.0)]))
      .with_margin(Sides([Px(0.0), Px(0.0), Px(0.0), Px(8.0)]))
      .with_border_width(Sides([Px(0.0), Px(0.0), Px(0.0), Px(3.0)]))
      .with(StyleDeclaration::border_color(ColorInput::Value(Color([
        215, 132, 55, 255,
      ]))))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        recursive_level_background(level),
      ))),
  )
}

fn recursive_visual_fixture_tree() -> Node {
  Node::container([recursive_visual_node(0, VISUAL_RECURSIVE_DEPTH)]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with_padding(Sides([Px(16.0); 4]))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([250, 248, 244, 255]),
      ))),
  )
}

#[test]
fn deep_nesting_stack_overflow() {
  let current_node = iterative_nesting_node(STACK_OVERFLOW_DEPTH);

  let viewport = create_test_viewport();
  let options = RenderOptions::builder()
    .viewport(viewport)
    .node(current_node)
    .global(&CONTEXT)
    .build();

  let measured = measure_layout(options).unwrap();
  assert!(measured.width > 0.0);

  run_fixture_test(
    recursive_visual_fixture_tree(),
    "deep_nesting_stack_overflow",
  );
}
