use takumi::layout::{
  node::ContainerNode,
  style::{
    Length::{Percentage, Px},
    *,
  },
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_flex_basis() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Row))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 0, 255, 255]),
        ))),
    )
    .with_children([
      ContainerNode::default().with_style(
        Style::default()
          .with(StyleDeclaration::flex_basis(Some(Px(100.0))))
          .with(StyleDeclaration::height(Px(50.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([255, 0, 0, 255]),
          ))),
      ),
      ContainerNode::default().with_style(
        Style::default()
          .with(StyleDeclaration::flex_basis(Some(Px(100.0))))
          .with(StyleDeclaration::height(Px(50.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([0, 255, 0, 255]),
          ))),
      ),
      ContainerNode::default().with_style(
        Style::default()
          .with(StyleDeclaration::flex_basis(Some(Px(100.0))))
          .with(StyleDeclaration::height(Px(50.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([255, 255, 0, 255]),
          ))),
      ),
    ]);

  run_fixture_test(container.into(), "style_flex_basis");
}

#[test]
fn test_style_flex_direction() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 0, 255, 255]),
        ))),
    )
    .with_children([
      ContainerNode::default().with_style(
        Style::default()
          .with(StyleDeclaration::width(Px(50.0)))
          .with(StyleDeclaration::height(Px(50.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([255, 0, 0, 255]),
          ))),
      ),
      ContainerNode::default().with_style(
        Style::default()
          .with(StyleDeclaration::width(Px(50.0)))
          .with(StyleDeclaration::height(Px(50.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([0, 255, 0, 255]),
          ))),
      ),
      ContainerNode::default().with_style(
        Style::default()
          .with(StyleDeclaration::width(Px(50.0)))
          .with(StyleDeclaration::height(Px(50.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([255, 255, 0, 255]),
          ))),
      ),
    ]);

  run_fixture_test(container.into(), "style_flex_direction");
}

#[test]
fn test_style_gap() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with_gap(SpacePair::from_reversed_pair(Px(0.0), Px(20.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 0, 255, 255]),
        ))),
    )
    .with_children([
      // First child
      ContainerNode::default().with_style(
        Style::default()
          .with(StyleDeclaration::width(Px(50.0)))
          .with(StyleDeclaration::height(Px(50.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([255, 0, 0, 255]),
          ))),
      ),
      // Second child
      ContainerNode::default().with_style(
        Style::default()
          .with(StyleDeclaration::width(Px(50.0)))
          .with(StyleDeclaration::height(Px(50.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([0, 255, 0, 255]),
          ))),
      ),
      // Third child
      ContainerNode::default().with_style(
        Style::default()
          .with(StyleDeclaration::width(Px(50.0)))
          .with(StyleDeclaration::height(Px(50.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([255, 255, 0, 255]),
          ))),
      ),
    ]);

  run_fixture_test(container.into(), "style_gap");
}

#[test]
fn test_style_grid_template_columns() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
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
    )
    .with_children([
      ContainerNode::default().with_style(Style::default().with(
        StyleDeclaration::background_color(ColorInput::Value(Color([255, 0, 0, 255]))),
      )),
      ContainerNode::default().with_style(Style::default().with(
        StyleDeclaration::background_color(ColorInput::Value(Color([0, 255, 0, 255]))),
      )),
    ]);

  run_fixture_test(container.into(), "style_grid_template_columns");
}

#[test]
fn test_style_grid_template_rows() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
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
    )
    .with_children([
      ContainerNode::default().with_style(Style::default().with(
        StyleDeclaration::background_color(ColorInput::Value(Color([255, 0, 0, 255]))),
      )),
      ContainerNode::default().with_style(Style::default().with(
        StyleDeclaration::background_color(ColorInput::Value(Color([0, 255, 0, 255]))),
      )),
    ]);

  run_fixture_test(container.into(), "style_grid_template_rows");
}
