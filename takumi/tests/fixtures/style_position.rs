use takumi::layout::{
  node::Node,
  style::{
    Color, ColorInput, Display,
    Length::{Percentage, Px},
    Position, Sides, Style, StyleDeclaration, ZIndex,
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

#[test]
fn test_style_stacking_context_z_index_siblings() {
  let negative = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::z_index(ZIndex::Integer(-1)))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(360.0)))
      .with(StyleDeclaration::top(Px(120.0)))
      .with(StyleDeclaration::left(Px(120.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 255]),
      ))),
  );

  let positive = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::z_index(ZIndex::Integer(2)))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(360.0)))
      .with(StyleDeclaration::top(Px(180.0)))
      .with(StyleDeclaration::left(Px(180.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 255, 0, 255]),
      ))),
  );

  let auto = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(360.0)))
      .with(StyleDeclaration::top(Px(240.0)))
      .with(StyleDeclaration::left(Px(240.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  let container = Node::container([negative, positive, auto]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([245, 245, 245, 255]),
      ))),
  );

  run_fixture_test(container, "style_stacking_context_z_index_siblings");
}

#[test]
fn test_style_stacking_context_nested_context_atomicity() {
  let nested_high = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::z_index(ZIndex::Integer(9999)))
      .with(StyleDeclaration::width(Px(220.0)))
      .with(StyleDeclaration::height(Px(220.0)))
      .with(StyleDeclaration::top(Px(30.0)))
      .with(StyleDeclaration::left(Px(260.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 0, 255]),
      ))),
  );

  let context_low = Node::container([nested_high]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::z_index(ZIndex::Integer(1)))
      .with(StyleDeclaration::width(Px(460.0)))
      .with(StyleDeclaration::height(Px(300.0)))
      .with(StyleDeclaration::top(Px(130.0)))
      .with(StyleDeclaration::left(Px(160.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 128, 128, 255]),
      ))),
  );

  let context_high = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::z_index(ZIndex::Integer(2)))
      .with(StyleDeclaration::width(Px(340.0)))
      .with(StyleDeclaration::height(Px(340.0)))
      .with(StyleDeclaration::top(Px(200.0)))
      .with(StyleDeclaration::left(Px(200.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([128, 128, 255, 255]),
      ))),
  );

  let container = Node::container([context_low, context_high]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_stacking_context_nested_context_atomicity");
}

#[test]
fn test_style_stacking_context_flex_item_z_index() {
  let flex_a = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::z_index(ZIndex::Integer(2)))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(260.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 200]),
      ))),
  );

  let flex_b = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::z_index(ZIndex::Integer(1)))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(260.0)))
      .with(StyleDeclaration::margin_left(Px(-120.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 200]),
      ))),
  );

  let container = Node::container([flex_a, flex_b]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([245, 245, 245, 255]),
      ))),
  );

  run_fixture_test(container, "style_stacking_context_flex_item_z_index");
}
