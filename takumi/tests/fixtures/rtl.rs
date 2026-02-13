use takumi::layout::{
  node::{ContainerNode, NodeKind, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

fn create_test_nodes() -> Vec<NodeKind> {
  (1..5)
    .map(|i| {
      TextNode {
        style: Some(
          StyleBuilder::default()
            .border_width(Sides([Px(1.0); 4]))
            .flex_grow(Some(FlexGrow(1.0)))
            .padding(Sides([Px(16.0); 4]))
            .font_size(Some(Px(24.0)))
            .font_family(FontFamily::from_str("monospace").ok())
            .build()
            .unwrap(),
        ),
        text: format!("Node {i}"),
        preset: None,
        tw: None,
      }
      .into()
    })
    .collect::<Vec<NodeKind>>()
}

#[test]
fn test_direction_flex_row() {
  let children = create_test_nodes();

  let container = ContainerNode {
    style: Some(
      StyleBuilder::default()
        .flex_direction(FlexDirection::Column)
        .background_color(ColorInput::Value(Color::white()))
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .padding(Sides([Px(16.0); 4]))
        .justify_content(JustifyContent::Center)
        .gap(SpacePair::from_pair(Px(16.0), Px(16.0)))
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        ContainerNode {
          children: Some(children.clone().into_boxed_slice()),
          preset: None,
          style: Some(
            StyleBuilder::default()
              .direction(Direction::Ltr)
              .gap(SpacePair::from_pair(Px(16.0), Px(16.0)))
              .width(Percentage(100.0))
              .build()
              .unwrap(),
          ),
          tw: None,
        }
        .into(),
        ContainerNode {
          style: Some(
            StyleBuilder::default()
              .direction(Direction::Rtl)
              .gap(SpacePair::from_pair(Px(16.0), Px(16.0)))
              .width(Percentage(100.0))
              .build()
              .unwrap(),
          ),
          children: Some(children.into_boxed_slice()),
          preset: None,
          tw: None,
        }
        .into(),
      ]
      .into(),
    ),
    preset: None,
    tw: None,
  };

  run_fixture_test(container.into(), "direction_flex_row");
}

#[test]
fn test_direction_grid() {
  let children = create_test_nodes();

  let container = ContainerNode {
    style: Some(
      StyleBuilder::default()
        .flex_direction(FlexDirection::Column)
        .background_color(ColorInput::Value(Color::white()))
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .padding(Sides([Px(16.0); 4]))
        .justify_content(JustifyContent::Center)
        .gap(SpacePair::from_pair(Px(16.0), Px(16.0)))
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        ContainerNode {
          children: Some(children.clone().into_boxed_slice()),
          preset: None,
          style: Some(
            StyleBuilder::default()
              .display(Display::Grid)
              .grid_template_columns(GridTemplateComponents::from_str("repeat(4, 1fr)").ok())
              .direction(Direction::Ltr)
              .gap(SpacePair::from_pair(Px(16.0), Px(16.0)))
              .width(Percentage(100.0))
              .build()
              .unwrap(),
          ),
          tw: None,
        }
        .into(),
        ContainerNode {
          style: Some(
            StyleBuilder::default()
              .display(Display::Grid)
              .grid_template_columns(GridTemplateComponents::from_str("repeat(4, 1fr)").ok())
              .direction(Direction::Rtl)
              .gap(SpacePair::from_pair(Px(16.0), Px(16.0)))
              .width(Percentage(100.0))
              .build()
              .unwrap(),
          ),
          children: Some(children.into_boxed_slice()),
          preset: None,
          tw: None,
        }
        .into(),
      ]
      .into(),
    ),
    preset: None,
    tw: None,
  };

  run_fixture_test(container.into(), "direction_grid");
}
