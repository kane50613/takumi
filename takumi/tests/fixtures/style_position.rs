use takumi::layout::{
  node::Node,
  style::{
    Color, ColorInput, Display,
    Length::{Percentage, Px},
    Position, Sides, Style, StyleDeclaration,
  },
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_position() {
  let container = Node::container([Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(100.0)))
      .with(StyleDeclaration::height(Px(100.0)))
      .with(StyleDeclaration::position(Position::Absolute))
      .with_inset(Sides([Px(20.0); 4]))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 255]),
      ))),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_position");
}
