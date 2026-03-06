use takumi::layout::{
  node::ContainerNode,
  style::{
    Color, ColorInput,
    Length::{Percentage, Px},
    Sides, Style, StyleDeclaration,
  },
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_padding() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 0, 255, 255]),
        )))
        .with_padding(Sides([Px(20.0); 4])),
    ),
    children: Some(
      [ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with(StyleDeclaration::width(Percentage(100.0)))
            .with(StyleDeclaration::height(Percentage(100.0)))
            .with(StyleDeclaration::background_color(ColorInput::Value(
              Color([255, 0, 0, 255]),
            ))),
        ),
        children: None,
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_padding");
}
