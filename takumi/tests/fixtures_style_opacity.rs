use takumi::layout::{
  node::{ContainerNode, NodeKind, TextNode},
  style::{PercentageNumber, *},
};

use crate::test_utils::run_style_width_test;

mod test_utils;

fn create_test_container(opacity: f32) -> NodeKind {
  ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(LengthUnit::Percentage(8.0))
        .height(LengthUnit::Percentage(6.0))
        .border_radius(Sides::from(LengthUnit::Rem(1.0)))
        .opacity(PercentageNumber(opacity))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      TextNode {
        tw: None,
        style: None,
        text: opacity.to_string(),
      }
      .into(),
    ]),
  }
  .into()
}

#[test]
fn test_style_opacity() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(LengthUnit::Percentage(100.0))
        .height(LengthUnit::Percentage(100.0))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .background_color(ColorInput::Value(Color([255, 255, 255, 255])))
        .gap(SpacePair::from_single(LengthUnit::Rem(4.0)))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      create_test_container(0.1),
      create_test_container(0.3),
      create_test_container(0.5),
      create_test_container(1.0),
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/style_opacity.webp");
}
