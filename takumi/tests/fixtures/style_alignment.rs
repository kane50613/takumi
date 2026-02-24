use takumi::layout::{
  node::ContainerNode,
  style::{
    AlignItems, Color, ColorInput, Display, JustifyContent,
    Length::{Percentage, Px},
    StyleBuilder,
  },
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_align_items() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .display(Display::Flex)
        .align_items(AlignItems::Center)
        .background_color(ColorInput::Value(Color([0, 0, 255, 255])))
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
              .width(Px(50.0))
              .height(Px(50.0))
              .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
              .build()
              .unwrap(),
          ),
          children: None,
        }
        .into(),
        ContainerNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .width(Px(50.0))
              .height(Px(50.0))
              .background_color(ColorInput::Value(Color([0, 255, 0, 255])))
              .build()
              .unwrap(),
          ),
          children: None,
        }
        .into(),
        ContainerNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .width(Px(50.0))
              .height(Px(50.0))
              .background_color(ColorInput::Value(Color([255, 255, 0, 255])))
              .build()
              .unwrap(),
          ),
          children: None,
        }
        .into(),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_align_items");
}

#[test]
fn test_style_justify_content() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .display(Display::Flex)
        .justify_content(JustifyContent::Center)
        .background_color(ColorInput::Value(Color([0, 0, 255, 255])))
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
              .width(Px(50.0))
              .height(Px(50.0))
              .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
              .build()
              .unwrap(),
          ),
          children: None,
        }
        .into(),
        ContainerNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .width(Px(50.0))
              .height(Px(50.0))
              .background_color(ColorInput::Value(Color([0, 255, 0, 255])))
              .build()
              .unwrap(),
          ),
          children: None,
        }
        .into(),
        ContainerNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .width(Px(50.0))
              .height(Px(50.0))
              .background_color(ColorInput::Value(Color([255, 255, 0, 255])))
              .build()
              .unwrap(),
          ),
          children: None,
        }
        .into(),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_justify_content");
}
