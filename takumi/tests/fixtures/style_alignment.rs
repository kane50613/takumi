use takumi::layout::{
  node::ContainerNode,
  style::{
    AlignItems, Color, ColorInput, Display, JustifyContent,
    Length::{Percentage, Px},
    Style, StyleDeclaration,
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
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 0, 255, 255]),
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
              .with(StyleDeclaration::width(Px(50.0)))
              .with(StyleDeclaration::height(Px(50.0)))
              .with(StyleDeclaration::background_color(ColorInput::Value(
                Color([255, 0, 0, 255]),
              ))),
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
            Style::default()
              .with(StyleDeclaration::width(Px(50.0)))
              .with(StyleDeclaration::height(Px(50.0)))
              .with(StyleDeclaration::background_color(ColorInput::Value(
                Color([0, 255, 0, 255]),
              ))),
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
            Style::default()
              .with(StyleDeclaration::width(Px(50.0)))
              .with(StyleDeclaration::height(Px(50.0)))
              .with(StyleDeclaration::background_color(ColorInput::Value(
                Color([255, 255, 0, 255]),
              ))),
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
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 0, 255, 255]),
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
              .with(StyleDeclaration::width(Px(50.0)))
              .with(StyleDeclaration::height(Px(50.0)))
              .with(StyleDeclaration::background_color(ColorInput::Value(
                Color([255, 0, 0, 255]),
              ))),
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
            Style::default()
              .with(StyleDeclaration::width(Px(50.0)))
              .with(StyleDeclaration::height(Px(50.0)))
              .with(StyleDeclaration::background_color(ColorInput::Value(
                Color([0, 255, 0, 255]),
              ))),
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
            Style::default()
              .with(StyleDeclaration::width(Px(50.0)))
              .with(StyleDeclaration::height(Px(50.0)))
              .with(StyleDeclaration::background_color(ColorInput::Value(
                Color([255, 255, 0, 255]),
              ))),
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
