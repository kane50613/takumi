use takumi::layout::{
  node::{ContainerNode, NodeKind, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

/// Creates a single card with backdrop-filter for testing.
fn create_backdrop_card(filter: &str, label_font_size_px: f32) -> NodeKind {
  ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::backdrop_filter(
          Filters::from_str(filter).unwrap(),
        ))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 255, 60]),
        )))
        .with(StyleDeclaration::font_size(Px(label_font_size_px).into()))
        .with(StyleDeclaration::color(ColorInput::Value(Color::black())))
        .with_padding(Sides([Px(8.0); 4])),
    ),
    children: Some(
      [TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: None,
        text: filter.to_string(),
      }
      .into()]
      .into(),
    ),
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
          GridTemplateComponents::from_str("repeat(4, 1fr)").ok(),
        ))
        .with(StyleDeclaration::background_image(Some(
          BackgroundImages::from_str(
            "linear-gradient(135deg, #667eea 0%, #764ba2 25%, #f857a6 50%, #ff5858 75%, #ffb199 100%)",
          )
          .unwrap(),
        )))
        .with(StyleDeclaration::background_position(
          BackgroundPositions::from_str("center center").unwrap(),
        )),
    ),
    children: Some(
      filter_effects
        .iter()
        .map(|filter| create_backdrop_card(filter, 14.0))
        .collect(),
    ),
  }
  .into();

  run_fixture_test(container, "style_backdrop_filter");
}

#[test]
fn test_style_backdrop_filter_frosted_glass() {
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
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::background_image(Some(
          BackgroundImages::from_str("url(assets/images/yeecord.png)").unwrap(),
        )))
        .with(StyleDeclaration::background_position(
          BackgroundPositions::from_str("center center").unwrap(),
        ))
        .with(StyleDeclaration::background_size(
          BackgroundSizes::from_str("cover").unwrap(),
        )),
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
            .with(StyleDeclaration::display(Display::Flex))
            .with(StyleDeclaration::flex_direction(FlexDirection::Column))
            .with(StyleDeclaration::align_items(AlignItems::Center))
            .with(StyleDeclaration::justify_content(JustifyContent::Center))
            .with(StyleDeclaration::backdrop_filter(
              Filters::from_str("blur(16px)").unwrap(),
            ))
            .with(StyleDeclaration::background_color(ColorInput::Value(
              Color([255, 255, 255, 80]),
            )))
            .with_border_radius(Box::new(BorderRadius::from_str("24px").unwrap()))
            .with_padding(Sides([Px(48.0); 4]))
            .with_gap(SpacePair::from_pair(Px(16.0), Px(16.0))),
        ),
        children: Some(
          [
            TextNode {
              class_name: None,
              id: None,
              tag_name: None,
              preset: None,
              tw: None,
              style: Some(
                Style::default()
                  .with(StyleDeclaration::font_size(Px(48.0).into()))
                  .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
                  .with(StyleDeclaration::color(ColorInput::Value(Color([
                    0, 0, 0, 200,
                  ])))),
              ),
              text: "Frosted Glass".to_string(),
            }
            .into(),
            TextNode {
              class_name: None,
              id: None,
              tag_name: None,
              preset: None,
              tw: None,
              style: Some(
                Style::default()
                  .with(StyleDeclaration::font_size(Px(24.0).into()))
                  .with(StyleDeclaration::color(ColorInput::Value(Color([
                    0, 0, 0, 150,
                  ])))),
              ),
              text: "backdrop-filter: blur(16px)".to_string(),
            }
            .into(),
          ]
          .into(),
        ),
      }
      .into()]
      .into(),
    ),
  }
  .into();

  run_fixture_test(container, "style_backdrop_filter_frosted_glass");
}
