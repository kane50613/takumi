use takumi::layout::{
  node::ContainerNode,
  style::{
    Color, ColorInput,
    Length::{Percentage, Px},
    StyleBuilder,
  },
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_width() {
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
        .background_color(ColorInput::Value(Color::white()))
        .build()
        .unwrap(),
    ),
    children: None,
  };

  run_fixture_test(container.into(), "style_width");
}

#[test]
fn test_style_height() {
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
        .background_color(ColorInput::Value(Color::white()))
        .build()
        .unwrap(),
    ),
    children: None,
  };

  run_fixture_test(container.into(), "style_height");
}

#[test]
fn test_style_min_width() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .min_width(Px(50.0))
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .build()
        .unwrap(),
    ),
    children: None,
  };

  run_fixture_test(container.into(), "style_min_width");
}

#[test]
fn test_style_min_height() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .min_height(Px(50.0))
        .height(Percentage(100.0))
        .width(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .build()
        .unwrap(),
    ),
    children: None,
  };

  run_fixture_test(container.into(), "style_min_height");
}

#[test]
fn test_style_max_width() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .max_width(Px(100.0))
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .build()
        .unwrap(),
    ),
    children: None,
  };

  run_fixture_test(container.into(), "style_max_width");
}

#[test]
fn test_style_max_height() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .max_height(Px(100.0))
        .height(Percentage(100.0))
        .width(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .build()
        .unwrap(),
    ),
    children: None,
  };

  run_fixture_test(container.into(), "style_max_height");
}
