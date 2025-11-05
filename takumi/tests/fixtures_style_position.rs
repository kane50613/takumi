use takumi::layout::{
  node::ContainerNode,
  style::{
    Color, ColorInput,
    LengthUnit::{Percentage, Px},
    Position, Sides, StyleBuilder,
  },
};

mod test_utils;
use test_utils::run_style_width_test;

#[test]
fn test_style_position() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([0, 0, 255, 255]))) // Blue background to serve as container
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Px(100.0))
            .height(Px(100.0))
            .position(Position::Absolute) // Test the position property
            .inset(Sides([Px(20.0); 4])) // Position with inset properties
            .background_color(ColorInput::Value(Color([255, 0, 0, 255]))) // Red child to make it visible
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into(),
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/style_position.webp");
}
