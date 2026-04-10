use takumi::layout::{
  node::Node,
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

fn create_test_nodes() -> Vec<Node> {
  (1..5)
    .map(|i| {
      Node::text(format!("Node {i}")).with_style(
        Style::default()
          .with_border_width(Sides([Px(1.0); 4]))
          .with_padding(Sides([Px(16.0); 4]))
          .with_border_style(Sides([BorderStyle::Solid; 4]))
          .with(StyleDeclaration::flex_grow(Some(FlexGrow(1.0))))
          .with(StyleDeclaration::font_size(FontSize::Length(Px(24.0))))
          .with(StyleDeclaration::font_family(
            FontFamily::from_str("monospace").unwrap(),
          )),
      )
    })
    .collect()
}

#[test]
fn test_direction_flex_row() {
  let children = create_test_nodes();

  let container = Node::container([
    Node::container(children.clone()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::direction(Direction::Ltr))
        .with_gap(SpacePair::from_pair(Px(16.0), Px(16.0)))
        .with(StyleDeclaration::width(Percentage(100.0))),
    ),
    Node::container(children).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::direction(Direction::Rtl))
        .with_gap(SpacePair::from_pair(Px(16.0), Px(16.0)))
        .with(StyleDeclaration::width(Percentage(100.0))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with_padding(Sides([Px(16.0); 4]))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with_gap(SpacePair::from_pair(Px(16.0), Px(16.0))),
  );

  run_fixture_test(container, "direction_flex_row");
}

#[test]
fn test_direction_grid() {
  let children = create_test_nodes();

  let container = Node::container([
    Node::container(children.clone()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Grid))
        .with(StyleDeclaration::grid_template_columns(Some(
          GridTemplateComponents::from_str("repeat(4, 1fr)").unwrap(),
        )))
        .with(StyleDeclaration::direction(Direction::Ltr))
        .with_gap(SpacePair::from_pair(Px(16.0), Px(16.0)))
        .with(StyleDeclaration::width(Percentage(100.0))),
    ),
    Node::container(children).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Grid))
        .with(StyleDeclaration::grid_template_columns(Some(
          GridTemplateComponents::from_str("repeat(4, 1fr)").unwrap(),
        )))
        .with(StyleDeclaration::direction(Direction::Rtl))
        .with_gap(SpacePair::from_pair(Px(16.0), Px(16.0)))
        .with(StyleDeclaration::width(Percentage(100.0))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with_padding(Sides([Px(16.0); 4]))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with_gap(SpacePair::from_pair(Px(16.0), Px(16.0))),
  );

  run_fixture_test(container, "direction_grid");
}
