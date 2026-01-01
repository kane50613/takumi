use takumi::layout::{
  node::{ContainerNode, NodeKind, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_style_width_test;

/// Creates a single card with backdrop-filter for testing.
fn create_backdrop_card(filter: &str, label_font_size_px: f32) -> NodeKind {
  ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .display(Display::Flex)
        .flex_direction(FlexDirection::Column)
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .backdrop_filter(Filters::from_str(filter).unwrap())
        .background_color(ColorInput::Value(Color([255, 255, 255, 60])))
        .font_size(Some(Px(label_font_size_px)))
        .color(ColorInput::Value(Color::black()))
        .padding(Sides([Px(8.0); 4]))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
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
fn test_style_backdrop_filter() {
  let filter_effects = [
    // Row 1: Blur effects
    "blur(0px)",
    "blur(5px)",
    "blur(10px)",
    "blur(20px)",
    // Row 2: Color effects
    "grayscale(100%)",
    "sepia(100%)",
    "invert(100%)",
    "hue-rotate(180deg)",
    // Row 3: Adjustment effects
    "brightness(50%)",
    "brightness(150%)",
    "contrast(50%)",
    "contrast(200%)",
    // Row 4: Saturation and combined
    "saturate(0%)",
    "saturate(200%)",
    "opacity(50%)",
    "blur(5px) grayscale(50%)",
  ];

  let container = ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .display(Display::Grid)
        .grid_template_columns(GridTemplateComponents::from_str("repeat(4, 1fr)").ok())
        .background_image(Some(
          BackgroundImages::from_str(
            "linear-gradient(135deg, #667eea 0%, #764ba2 25%, #f857a6 50%, #ff5858 75%, #ffb199 100%)",
          )
          .unwrap(),
        ))
        .build()
        .unwrap(),
    ),
    children: Some(
      filter_effects
        .iter()
        .map(|filter| create_backdrop_card(filter, 14.0))
        .collect(),
    ),
  }
  .into();

  run_style_width_test(container, "style_backdrop_filter.png");
}

#[test]
fn test_style_backdrop_filter_frosted_glass() {
  let container = ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .display(Display::Flex)
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .background_image(Some(
          BackgroundImages::from_str("url(assets/images/yeecord.png)").unwrap(),
        ))
        .background_size(Some(BackgroundSizes::from_str("cover").unwrap()))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .display(Display::Flex)
            .flex_direction(FlexDirection::Column)
            .align_items(AlignItems::Center)
            .justify_content(JustifyContent::Center)
            .backdrop_filter(Filters::from_str("blur(16px)").unwrap())
            .background_color(ColorInput::Value(Color([255, 255, 255, 80])))
            .border_radius(BorderRadius::from_str("24px").unwrap())
            .padding(Sides([Px(48.0); 4]))
            .gap(SpacePair::from_pair(Px(16.0), Px(16.0)))
            .build()
            .unwrap(),
        ),
        children: Some(vec![
          TextNode {
            preset: None,
            tw: None,
            style: Some(
              StyleBuilder::default()
                .font_size(Some(Px(48.0)))
                .font_weight(FontWeight::from(700.0))
                .color(ColorInput::Value(Color([0, 0, 0, 200])))
                .build()
                .unwrap(),
            ),
            text: "Frosted Glass".to_string(),
          }
          .into(),
          TextNode {
            preset: None,
            tw: None,
            style: Some(
              StyleBuilder::default()
                .font_size(Some(Px(24.0)))
                .color(ColorInput::Value(Color([0, 0, 0, 150])))
                .build()
                .unwrap(),
            ),
            text: "backdrop-filter: blur(16px)".to_string(),
          }
          .into(),
        ]),
      }
      .into(),
    ]),
  }
  .into();

  run_style_width_test(container, "style_backdrop_filter_frosted_glass.png");
}
