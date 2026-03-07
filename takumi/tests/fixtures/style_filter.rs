use takumi::layout::{
  node::{ContainerNode, ImageNode, NodeKind, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

/// Helper function to create a filter test container with labeled images.
/// All sizes are in pixels for simplicity.
fn create_filter_test_container(
  filter_values: &[&str],
  gap_px: f32,
  image_size_px: f32,
  label_font_size_px: f32,
) -> NodeKind {
  ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Grid))
        .with(StyleDeclaration::grid_template_columns(
          GridTemplateComponents::from_str("repeat(5, 1fr)").ok(),
        ))
        .with_gap(SpacePair::from_single(Px(gap_px)))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        ))),
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
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with_gap(SpacePair::from_single(Px(16.0)))
        .with(StyleDeclaration::font_size(Px(label_font_size_px).into())),
    ),
    children: Some(
      [
        ImageNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          src: "assets/images/yeecord.png".into(),
          style: Some(
            Style::default()
              .with(StyleDeclaration::width(Px(image_size_px)))
              .with(StyleDeclaration::height(Px(image_size_px)))
              .with(StyleDeclaration::filter(Filters::from_str(filter).unwrap())),
          ),
          width: None,
          height: None,
        }
        .into(),
        TextNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(Style::default().with(StyleDeclaration::display(Display::Block))),
          text: filter.to_string(),
        }
        .into(),
      ]
      .into(),
    ),
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
    "sepia(100%)",
  ];

  let container = create_filter_test_container(&effects, 16.0, 128.0, 24.0);
  run_fixture_test(container, "style_filter");
}

#[test]
fn test_style_filter_blur() {
  let blur_values = ["blur(0px)", "blur(2px)", "blur(5px)", "blur(10px)"];

  let container = create_filter_test_container(&blur_values, 16.0, 150.0, 24.0);
  run_fixture_test(container, "style_filter_blur");
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
  run_fixture_test(container, "style_filter_drop_shadow");
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
  run_fixture_test(container, "style_filter_combined");
}

#[test]
fn test_style_filter_sepia() {
  let sepia_values = ["sepia(0%)", "sepia(50%)", "sepia(75%)", "sepia(100%)"];

  let container = create_filter_test_container(&sepia_values, 16.0, 150.0, 24.0);
  run_fixture_test(container, "style_filter_sepia");
}
