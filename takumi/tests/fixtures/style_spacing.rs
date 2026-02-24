use takumi::layout::{
  node::ContainerNode,
  style::{
    Color, ColorInput,
    Length::{Percentage, Px},
    Sides, StyleBuilder,
  },
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_margin() {
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
        .background_color(ColorInput::Value(Color([0, 0, 255, 255]))) // Blue background to show margin
        .build()
        .unwrap(),
    ),
    children: Some(
      [ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .margin(Sides([Px(20.0); 4])) // Uniform margin of 20px
            .width(Px(100.0)) // Fixed width
            .height(Px(100.0)) // Fixed height
            .background_color(ColorInput::Value(Color([255, 0, 0, 255]))) // Red child to show margin effect
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_margin");
}

#[test]
fn test_style_padding() {
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
        .background_color(ColorInput::Value(Color([0, 0, 255, 255]))) // Blue background to show padding
        .padding(Sides([Px(20.0); 4])) // Uniform padding of 20px
        .build()
        .unwrap(),
    ),
    children: Some(
      [ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Percentage(100.0))
            .height(Percentage(100.0))
            .background_color(ColorInput::Value(Color([255, 0, 0, 255]))) // Red child to show padding effect
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_padding");
}
