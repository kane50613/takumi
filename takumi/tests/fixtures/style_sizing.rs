use takumi::layout::{
  node::ContainerNode,
  style::{
    Color, ColorInput,
    Length::{Percentage, Px},
    Style, StyleDeclaration,
  },
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_width() {
  let container = ContainerNode::default().with_style(
    Style::default()
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(container.into(), "style_width");
}

#[test]
fn test_style_height() {
  let container = ContainerNode::default().with_style(
    Style::default()
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(container.into(), "style_height");
}

#[test]
fn test_style_min_width() {
  let container = ContainerNode::default().with_style(
    Style::default()
      .with(StyleDeclaration::min_width(Px(50.0)))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(container.into(), "style_min_width");
}

#[test]
fn test_style_min_height() {
  let container = ContainerNode::default().with_style(
    Style::default()
      .with(StyleDeclaration::min_height(Px(50.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(container.into(), "style_min_height");
}

#[test]
fn test_style_max_width() {
  let container = ContainerNode::default().with_style(
    Style::default()
      .with(StyleDeclaration::max_width(Px(100.0)))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(container.into(), "style_max_width");
}

#[test]
fn test_style_max_height() {
  let container = ContainerNode::default().with_style(
    Style::default()
      .with(StyleDeclaration::max_height(Px(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(container.into(), "style_max_height");
}
