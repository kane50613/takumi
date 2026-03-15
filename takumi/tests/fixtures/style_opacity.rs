use takumi::layout::{
  node::Node,
  style::{PercentageNumber, *},
};

use crate::test_utils::run_fixture_test;

fn create_test_container(opacity: f32) -> Node {
  Node::container([Node::text(opacity.to_string())]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Length::Percentage(8.0)))
      .with(StyleDeclaration::height(Length::Percentage(6.0)))
      .with_border_radius(Box::new(BorderRadius(Sides(
        [SpacePair::from_single(Length::Rem(1.0)); 4],
      ))))
      .with(StyleDeclaration::opacity(PercentageNumber(opacity)))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 255]),
      ))),
  )
}

#[test]
fn test_style_opacity() {
  let container = Node::container([
    create_test_container(0.1),
    create_test_container(0.3),
    create_test_container(0.5),
    create_test_container(1.0),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Length::Percentage(100.0)))
      .with(StyleDeclaration::height(Length::Percentage(100.0)))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      )))
      .with_gap(SpacePair::from_single(Length::Rem(4.0))),
  );

  run_fixture_test(container, "style_opacity");
}

#[test]
fn test_style_opacity_image_with_text() {
  let container = Node::container([
    Node::container([Node::image("assets/images/yeecord.png").with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Length::Percentage(100.0)))
        .with(StyleDeclaration::height(Length::Percentage(100.0))),
    )])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Length::Rem(20.0)))
        .with(StyleDeclaration::height(Length::Rem(20.0)))
        .with(StyleDeclaration::opacity(PercentageNumber(0.5))),
    ),
    Node::text("0.5".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::font_size(Length::Rem(3.0).into()))
        .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
        .with(StyleDeclaration::color(ColorInput::Value(Color([
          60, 60, 60, 255,
        ]))))
        .with(StyleDeclaration::opacity(PercentageNumber(0.5))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Length::Percentage(100.0)))
      .with(StyleDeclaration::height(Length::Percentage(100.0)))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with_gap(SpacePair::from_single(Length::Rem(2.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      ))),
  );

  run_fixture_test(container, "style_opacity_image_with_text");
}

#[test]
fn test_style_opacity_flex_text_node_vs_nested_container() {
  let left: Node = Node::text("A".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Length::Px(300.0)))
      .with(StyleDeclaration::height(Length::Px(220.0)))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::font_size(Length::Px(120.0).into()))
      .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
      .with(StyleDeclaration::color(ColorInput::Value(Color::black())))
      .with(StyleDeclaration::opacity(PercentageNumber(0.5)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      ))),
  );

  let right: Node = Node::container([Node::text("A".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::font_size(Length::Px(120.0).into()))
      .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
      .with(StyleDeclaration::color(ColorInput::Value(Color::black()))),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Length::Px(300.0)))
      .with(StyleDeclaration::height(Length::Px(220.0)))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::opacity(PercentageNumber(0.5)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      ))),
  );

  let root = Node::container([left, right]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Length::Percentage(100.0)))
      .with(StyleDeclaration::height(Length::Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with_gap(SpacePair::from_single(Length::Px(48.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(root, "style_opacity_flex_text_node_vs_nested_container");
}
