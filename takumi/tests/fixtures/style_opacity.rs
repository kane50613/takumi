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
      StyleBuilder::default()
        .width(Length::Percentage(8.0))
        .height(Length::Percentage(6.0))
        .border_radius(Box::new(BorderRadius(Sides(
          [SpacePair::from_single(Length::Rem(1.0)); 4],
        ))))
        .opacity(PercentageNumber(opacity))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
        .build()
        .unwrap(),
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
      StyleBuilder::default()
        .width(Length::Percentage(100.0))
        .height(Length::Percentage(100.0))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .background_color(ColorInput::Value(Color([255, 255, 255, 255])))
        .gap(SpacePair::from_single(Length::Rem(4.0)))
        .build()
        .unwrap(),
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
      StyleBuilder::default()
        .width(Length::Percentage(100.0))
        .height(Length::Percentage(100.0))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .flex_direction(FlexDirection::Column)
        .gap(SpacePair::from_single(Length::Rem(2.0)))
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .build()
        .unwrap(),
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
            StyleBuilder::default()
              .width(Length::Rem(20.0))
              .height(Length::Rem(20.0))
              .opacity(PercentageNumber(0.5))
              .build()
              .unwrap(),
          ),
          children: Some(
            [ImageNode {
              class_name: None,
              id: None,
              tag_name: None,
              preset: None,
              tw: None,
              style: Some(
                StyleBuilder::default()
                  .width(Length::Percentage(100.0))
                  .height(Length::Percentage(100.0))
                  .build()
                  .unwrap(),
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
            StyleBuilder::default()
              .font_size(Some(Length::Rem(3.0)))
              .font_weight(FontWeight::from(700.0))
              .color(ColorInput::Value(Color([60, 60, 60, 255])))
              .opacity(PercentageNumber(0.5))
              .build()
              .unwrap(),
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
