use takumi::layout::{
  node::{ContainerNode, ImageNode, NodeKind, TextNode},
  style::{LengthUnit::*, *},
};

mod test_utils;
use test_utils::run_style_width_test;

/// Helper function to create a filter test container with labeled images.
/// All sizes are in pixels for simplicity.
fn create_filter_test_container(
  filter_values: &[&str],
  gap_px: f32,
  image_size_px: f32,
  label_font_size_px: f32,
) -> NodeKind {
  ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .display(Display::Grid)
        .grid_template_columns(GridTemplateComponents::from_str("repeat(5, 1fr)").ok())
        .gap(SpacePair::from_single(Px(gap_px)))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .background_color(ColorInput::Value(Color::white()))
        .build()
        .unwrap(),
    ),
    children: Some(
      filter_values
        .iter()
        .map(|filter| create_filter_card(filter, image_size_px, label_font_size_px))
        .collect(),
    ),
  }
  .into()
}

/// Creates a single card with an image and label for filter testing.
fn create_filter_card(filter: &str, image_size_px: f32, label_font_size_px: f32) -> NodeKind {
  ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .flex_direction(FlexDirection::Column)
        .align_items(AlignItems::Center)
        .gap(SpacePair::from_single(Px(16.0)))
        .font_size(Some(Px(label_font_size_px)))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ImageNode {
        preset: None,
        tw: None,
        src: "assets/images/yeecord.png".into(),
        style: Some(
          StyleBuilder::default()
            .width(Px(image_size_px))
            .height(Px(image_size_px))
            .filter(Filters::from_str(filter).unwrap())
            .build()
            .unwrap(),
        ),
        width: None,
        height: None,
      }
      .into(),
      TextNode {
        preset: None,
        tw: None,
        style: None,
        text: filter.to_string(),
      }
      .into(),
    ]),
  }
  .into()
}

#[test]
fn test_style_filter_on_image_node() {
  let effects = [
    "blur(5px)",
    "grayscale(75%)",
    "opacity(30%)",
    "contrast(150%)",
    "brightness(150%)",
    "invert(50%)",
    "hue-rotate(90deg)",
    "saturate(0.3)",
  ];

  let container = create_filter_test_container(&effects, 16.0, 128.0, 24.0);
  run_style_width_test(container, "tests/fixtures/style_filter.webp");
}

#[test]
fn test_style_filter_blur() {
  let blur_values = ["blur(0px)", "blur(2px)", "blur(5px)", "blur(10px)"];

  let container = create_filter_test_container(&blur_values, 16.0, 150.0, 24.0);
  run_style_width_test(container, "tests/fixtures/style_filter_blur.webp");
}

#[test]
fn test_style_filter_drop_shadow() {
  let shadow_values = [
    "drop-shadow(5px 5px 5px black)",
    "drop-shadow(10px 10px 10px rgba(0,0,0,0.5))",
    "drop-shadow(-5px -5px 8px red)",
    "drop-shadow(0px 10px 15px blue)",
  ];

  let container = create_filter_test_container(&shadow_values, 16.0, 120.0, 16.0);
  run_style_width_test(container, "tests/fixtures/style_filter_drop_shadow.webp");
}

#[test]
fn test_style_filter_combined() {
  let combined_filters = [
    "blur(3px) grayscale(50%)",
    "drop-shadow(5px 5px 10px black) brightness(120%)",
    "blur(2px) drop-shadow(3px 3px 5px red)",
    "saturate(150%) blur(1px)",
  ];

  let container = create_filter_test_container(&combined_filters, 16.0, 140.0, 16.0);
  run_style_width_test(container, "tests/fixtures/style_filter_combined.webp");
}
