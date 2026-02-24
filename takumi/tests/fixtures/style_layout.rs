use takumi::layout::{
  node::ContainerNode,
  style::{
    Length::{Percentage, Px},
    *,
  },
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_flex_basis() {
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
        .flex_direction(FlexDirection::Row)
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
              .flex_basis(Some(Px(100.0)))
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
              .flex_basis(Some(Px(100.0)))
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
              .flex_basis(Some(Px(100.0)))
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

  run_fixture_test(container.into(), "style_flex_basis");
}

#[test]
fn test_style_flex_direction() {
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
        .flex_direction(FlexDirection::Column)
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

  run_fixture_test(container.into(), "style_flex_direction");
}

#[test]
fn test_style_gap() {
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
        .gap(SpacePair::from_pair(Px(20.0), Px(40.0)))
        .background_color(ColorInput::Value(Color([0, 0, 255, 255])))
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        // First child
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
        // Second child
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
        // Third child
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

  run_fixture_test(container.into(), "style_gap");
}

#[test]
fn test_style_grid_template_columns() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(200.0))
        .height(Px(200.0))
        .display(Display::Grid)
        .grid_template_columns(Some(vec![
          GridTemplateComponent::Single(GridTrackSize::Fixed(GridLength::Unit(Px(50.0)))),
          GridTemplateComponent::Single(GridTrackSize::Fixed(GridLength::Unit(Px(100.0)))),
        ]))
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
              .background_color(ColorInput::Value(Color([0, 255, 0, 255])))
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

  run_fixture_test(container.into(), "style_grid_template_columns");
}

#[test]
fn test_style_grid_template_rows() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(200.0))
        .height(Px(200.0))
        .display(Display::Grid)
        .grid_template_rows(Some(vec![
          GridTemplateComponent::Single(GridTrackSize::Fixed(GridLength::Unit(Px(50.0)))),
          GridTemplateComponent::Single(GridTrackSize::Fixed(GridLength::Unit(Px(100.0)))),
        ]))
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
              .background_color(ColorInput::Value(Color([0, 255, 0, 255])))
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

  run_fixture_test(container.into(), "style_grid_template_rows");
}
