use takumi::layout::{
  node::ContainerNode,
  style::{
    LengthUnit::{Percentage, Px},
    *,
  },
};

mod test_utils;
use test_utils::run_style_width_test;

#[test]
fn test_style_flex_basis() {
  let container = ContainerNode {
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
    children: Some(vec![
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .flex_basis(CssOption::some(Px(100.0)))
            .height(Px(50.0))
            .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into(),
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .flex_basis(CssOption::some(Px(100.0)))
            .height(Px(50.0))
            .background_color(ColorInput::Value(Color([0, 255, 0, 255])))
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into(),
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .flex_basis(CssOption::some(Px(100.0)))
            .height(Px(50.0))
            .background_color(ColorInput::Value(Color([255, 255, 0, 255])))
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into(),
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/style_flex_basis.webp");
}

#[test]
fn test_style_flex_direction() {
  let container = ContainerNode {
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
    children: Some(vec![
      ContainerNode {
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
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/style_flex_direction.webp");
}

#[test]
fn test_style_gap() {
  let container = ContainerNode {
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
    children: Some(vec![
      // First child
      ContainerNode {
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
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/style_gap.webp");
}

#[test]
fn test_style_grid_template_columns() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(200.0))
        .height(Px(200.0))
        .display(Display::Grid)
        .grid_template_columns(CssOption::some(GridTemplateComponents(vec![
          GridTemplateComponent::Single(GridTrackSize::Fixed(GridLengthUnit::Unit(Px(50.0)))),
          GridTemplateComponent::Single(GridTrackSize::Fixed(GridLengthUnit::Unit(Px(100.0)))),
        ])))
        .background_color(ColorInput::Value(Color([0, 0, 255, 255])))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
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
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_grid_template_columns.webp",
  );
}

#[test]
fn test_style_grid_template_rows() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(200.0))
        .height(Px(200.0))
        .display(Display::Grid)
        .grid_template_rows(CssOption::some(GridTemplateComponents(vec![
          GridTemplateComponent::Single(GridTrackSize::Fixed(GridLengthUnit::Unit(Px(50.0)))),
          GridTemplateComponent::Single(GridTrackSize::Fixed(GridLengthUnit::Unit(Px(100.0)))),
        ])))
        .background_color(ColorInput::Value(Color([0, 0, 255, 255])))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
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
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_grid_template_rows.webp",
  );
}
