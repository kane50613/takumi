use takumi::layout::{
  node::Node,
  style::{
    AlignItems, Color, ColorInput, Display, JustifyContent,
    Length::{Percentage, Px},
    Style, StyleDeclaration,
  },
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_align_items() {
  let container = Node::container([
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(50.0)))
        .with(StyleDeclaration::height(Px(50.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 0, 0, 255]),
        ))),
    ),
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(50.0)))
        .with(StyleDeclaration::height(Px(50.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 255, 0, 255]),
        ))),
    ),
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(50.0)))
        .with(StyleDeclaration::height(Px(50.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 0, 255]),
        ))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_align_items");
}

#[test]
fn test_style_justify_content() {
  let container = Node::container([
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(50.0)))
        .with(StyleDeclaration::height(Px(50.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 0, 0, 255]),
        ))),
    ),
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(50.0)))
        .with(StyleDeclaration::height(Px(50.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 255, 0, 255]),
        ))),
    ),
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(50.0)))
        .with(StyleDeclaration::height(Px(50.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 0, 255]),
        ))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_justify_content");
}
