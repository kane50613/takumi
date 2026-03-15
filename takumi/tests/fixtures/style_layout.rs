use takumi::layout::{
  node::Node,
  style::{
    Length::{Percentage, Px},
    *,
  },
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_flex_basis() {
  let container = Node::container([
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_basis(Some(Px(100.0))))
        .with(StyleDeclaration::height(Px(50.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 0, 0, 255]),
        ))),
    ),
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_basis(Some(Px(100.0))))
        .with(StyleDeclaration::height(Px(50.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 255, 0, 255]),
        ))),
    ),
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_basis(Some(Px(100.0))))
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
      .with(StyleDeclaration::flex_direction(FlexDirection::Row))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_flex_basis");
}

#[test]
fn test_style_flex_direction() {
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
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_flex_direction");
}

#[test]
fn test_style_gap() {
  let container = Node::container([
    // First child
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(50.0)))
        .with(StyleDeclaration::height(Px(50.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 0, 0, 255]),
        ))),
    ),
    // Second child
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(50.0)))
        .with(StyleDeclaration::height(Px(50.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 255, 0, 255]),
        ))),
    ),
    // Third child
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
      .with_gap(SpacePair::from_reversed_pair(Px(0.0), Px(20.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_gap");
}

#[test]
fn test_style_grid_template_columns() {
  let container = Node::container([
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 0, 0, 255]),
        ))),
    ),
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 255, 0, 255]),
        ))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(200.0)))
      .with(StyleDeclaration::height(Px(200.0)))
      .with(StyleDeclaration::display(Display::Grid))
      .with(StyleDeclaration::grid_template_columns(Some(vec![
        GridTemplateComponent::Single(GridTrackSize::Fixed(GridLength::Unit(Px(50.0)))),
        GridTemplateComponent::Single(GridTrackSize::Fixed(GridLength::Unit(Px(100.0)))),
      ])))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_grid_template_columns");
}

#[test]
fn test_style_grid_template_rows() {
  let container = Node::container([
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 0, 0, 255]),
        ))),
    ),
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 255, 0, 255]),
        ))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(200.0)))
      .with(StyleDeclaration::height(Px(200.0)))
      .with(StyleDeclaration::display(Display::Grid))
      .with(StyleDeclaration::grid_template_rows(Some(vec![
        GridTemplateComponent::Single(GridTrackSize::Fixed(GridLength::Unit(Px(50.0)))),
        GridTemplateComponent::Single(GridTrackSize::Fixed(GridLength::Unit(Px(100.0)))),
      ])))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_grid_template_rows");
}
