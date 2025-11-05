use takumi::layout::{
  node::ContainerNode,
  style::{
    Color, ColorInput,
    LengthUnit::{Percentage, Px},
    Sides, StyleBuilder,
  },
};

mod test_utils;
use test_utils::run_style_width_test;

#[test]
fn test_style_margin() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([0, 0, 255, 255]))) // Blue background to show margin
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
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
      .into(),
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/style_margin.webp");
}

#[test]
fn test_style_padding() {
  let container = ContainerNode {
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
    children: Some(vec![
      ContainerNode {
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
      .into(),
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/style_padding.webp");
}
