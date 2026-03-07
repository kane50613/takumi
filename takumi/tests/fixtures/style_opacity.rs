use takumi::layout::{
  node::{ContainerNode, ImageNode, NodeKind, TextNode},
  style::{PercentageNumber, *},
};

use crate::test_utils::run_fixture_test;

fn create_test_container(opacity: f32) -> NodeKind {
  ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
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
    ),
    children: Some(
      [TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: None,
        text: opacity.to_string(),
      }
      .into()]
      .into(),
    ),
  }
  .into()
}

#[test]
fn test_style_opacity() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Length::Percentage(100.0)))
        .with(StyleDeclaration::height(Length::Percentage(100.0)))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 255, 255]),
        )))
        .with_gap(SpacePair::from_single(Length::Rem(4.0))),
    ),
    children: Some(
      [
        create_test_container(0.1),
        create_test_container(0.3),
        create_test_container(0.5),
        create_test_container(1.0),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_opacity");
}

#[test]
fn test_style_opacity_image_with_text() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Length::Percentage(100.0)))
        .with(StyleDeclaration::height(Length::Percentage(100.0)))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with_gap(SpacePair::from_single(Length::Rem(2.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        ))),
    ),
    children: Some(
      [
        ContainerNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            Style::default()
              .with(StyleDeclaration::width(Length::Rem(20.0)))
              .with(StyleDeclaration::height(Length::Rem(20.0)))
              .with(StyleDeclaration::opacity(PercentageNumber(0.5))),
          ),
          children: Some(
            [ImageNode {
              class_name: None,
              id: None,
              tag_name: None,
              preset: None,
              tw: None,
              style: Some(
                Style::default()
                  .with(StyleDeclaration::width(Length::Percentage(100.0)))
                  .with(StyleDeclaration::height(Length::Percentage(100.0))),
              ),
              src: "assets/images/yeecord.png".into(),
              width: None,
              height: None,
            }
            .into()]
            .into(),
          ),
        }
        .into(),
        TextNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            Style::default()
              .with(StyleDeclaration::display(Display::Block))
              .with(StyleDeclaration::font_size(Length::Rem(3.0).into()))
              .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
              .with(StyleDeclaration::color(ColorInput::Value(Color([
                60, 60, 60, 255,
              ]))))
              .with(StyleDeclaration::opacity(PercentageNumber(0.5))),
          ),
          text: "0.5".to_string(),
        }
        .into(),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_opacity_image_with_text");
}

#[test]
fn test_style_opacity_flex_text_node_vs_nested_container() {
  let left: NodeKind = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
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
    ),
    text: "A".to_string(),
  }
  .into();

  let right: NodeKind = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Length::Px(300.0)))
        .with(StyleDeclaration::height(Length::Px(220.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::opacity(PercentageNumber(0.5)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        ))),
    ),
    children: Some(
      [TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with(StyleDeclaration::display(Display::Block))
            .with(StyleDeclaration::font_size(Length::Px(120.0).into()))
            .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
            .with(StyleDeclaration::color(ColorInput::Value(Color::black()))),
        ),
        text: "A".to_string(),
      }
      .into()]
      .into(),
    ),
  }
  .into();

  let root = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Length::Percentage(100.0)))
        .with(StyleDeclaration::height(Length::Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with_gap(SpacePair::from_single(Length::Px(48.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        ))),
    ),
    children: Some([left, right].into()),
  };

  run_fixture_test(
    root.into(),
    "style_opacity_flex_text_node_vs_nested_container",
  );
}
