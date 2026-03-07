use takumi::layout::{
  node::{ContainerNode, NodeKind, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

fn create_container_with_background_clip(
  background_clip: BackgroundClip,
  background_color: Color,
  padding: f32,
  border_width: f32,
) -> ContainerNode<NodeKind> {
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
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([200, 200, 200, 255]),
        )))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center)),
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
            .with(StyleDeclaration::width(Rem(16.0)))
            .with(StyleDeclaration::height(Rem(10.0)))
            .with(StyleDeclaration::background_color(ColorInput::Value(
              background_color,
            )))
            .with(StyleDeclaration::background_clip(background_clip))
            .with_padding(Sides([Px(padding); 4]))
            .with_border_width(Sides([Px(border_width); 4]))
            .with(StyleDeclaration::border_style(BorderStyle::Solid))
            .with(StyleDeclaration::border_color(ColorInput::Value(Color([
              0, 0, 0, 255,
            ]))))
            .with_border_radius(Box::new(BorderRadius(Sides(
              [SpacePair::from_single(Px(8.0)); 4],
            )))),
        ),
        children: None,
      }
      .into()]
      .into(),
    ),
  }
}

#[test]
fn test_style_background_clip_border_box() {
  let container = create_container_with_background_clip(
    BackgroundClip::BorderBox,
    Color([255, 0, 0, 255]),
    20.0,
    10.0,
  );

  run_fixture_test(container.into(), "style_background_clip_border_box");
}

#[test]
fn test_style_background_clip_padding_box() {
  let container = create_container_with_background_clip(
    BackgroundClip::PaddingBox,
    Color([0, 128, 255, 255]),
    20.0,
    10.0,
  );

  run_fixture_test(container.into(), "style_background_clip_padding_box");
}

#[test]
fn test_style_background_clip_content_box() {
  let container = create_container_with_background_clip(
    BackgroundClip::ContentBox,
    Color([34, 197, 94, 255]),
    20.0,
    10.0,
  );

  run_fixture_test(container.into(), "style_background_clip_content_box");
}

#[test]
fn test_style_background_clip_text_gradient() {
  let gradient_images = BackgroundImages::from_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::font_size(Px(72.0).into()))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center)),
    ),
    children: Some(
      [TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with(StyleDeclaration::background_image(Some(gradient_images)))
            .with(StyleDeclaration::background_size(
              BackgroundSizes::from_str("100% 100%").unwrap(),
            ))
            .with(StyleDeclaration::background_position(
              BackgroundPositions::from_str("0 0").unwrap(),
            ))
            .with(StyleDeclaration::background_repeat(
              BackgroundRepeats::from_str("no-repeat").unwrap(),
            ))
            .with(StyleDeclaration::background_clip(BackgroundClip::Text))
            .with(StyleDeclaration::color(ColorInput::Value(
              Color::transparent(),
            ))),
        ),
        text: "Gradient Text".to_string(),
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_background_clip_text_gradient");
}

#[test]
fn test_style_background_clip_text_radial_gradient() {
  let gradient_images =
    BackgroundImages::from_str("radial-gradient(circle, #ff0080, #7928ca, #0070f3)").unwrap();

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 255, 255]),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::font_size(Px(64.0).into()))
        .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center)),
    ),
    children: Some(
      [TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with(StyleDeclaration::background_image(Some(gradient_images)))
            .with(StyleDeclaration::background_size(
              BackgroundSizes::from_str("100% 100%").unwrap(),
            ))
            .with(StyleDeclaration::background_position(
              BackgroundPositions::from_str("center center").unwrap(),
            ))
            .with(StyleDeclaration::background_clip(BackgroundClip::Text))
            .with(StyleDeclaration::color(ColorInput::Value(
              Color::transparent(),
            ))),
        ),
        text: "Radial Gradient".to_string(),
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_background_clip_text_radial");
}

#[test]
fn test_style_background_clip_border_area() {
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
          Color([200, 200, 200, 255]),
        )))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center)),
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
            .with(StyleDeclaration::width(Rem(16.0)))
            .with(StyleDeclaration::height(Rem(10.0)))
            .with(StyleDeclaration::background_color(ColorInput::Value(
              Color([255, 165, 0, 255]),
            )))
            .with(StyleDeclaration::background_clip(
              BackgroundClip::BorderArea,
            ))
            .with_padding(Sides([Px(20.0); 4]))
            .with_border_width(Sides([Px(10.0); 4]))
            .with(StyleDeclaration::border_style(BorderStyle::Solid))
            .with(StyleDeclaration::border_color(ColorInput::Value(Color([
              0, 0, 0, 128,
            ]))))
            .with_border_radius(Box::new(BorderRadius(Sides(
              [SpacePair::from_single(Px(8.0)); 4],
            )))),
        ),
        children: None,
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_background_clip_border_area");
}

#[test]
fn test_style_background_clip_with_gradient_background() {
  let gradient_images =
    BackgroundImages::from_str("linear-gradient(135deg, #667eea 0%, #764ba2 100%)").unwrap();

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
          Color([200, 200, 200, 255]),
        )))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center)),
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
            .with(StyleDeclaration::width(Rem(16.0)))
            .with(StyleDeclaration::height(Rem(10.0)))
            .with(StyleDeclaration::background_image(Some(gradient_images)))
            .with(StyleDeclaration::background_position(
              BackgroundPositions::from_str("center center").unwrap(),
            ))
            .with(StyleDeclaration::background_clip(
              BackgroundClip::PaddingBox,
            ))
            .with_padding(Sides([Px(30.0); 4]))
            .with_border_width(Sides([Px(15.0); 4]))
            .with(StyleDeclaration::border_style(BorderStyle::Solid))
            .with(StyleDeclaration::border_color(ColorInput::Value(Color([
              255, 255, 255, 255,
            ])))),
        ),
        children: None,
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_background_clip_gradient_padding");
}

#[test]
fn test_style_background_clip_text_multiline() {
  let gradient_images =
    BackgroundImages::from_str("linear-gradient(45deg, #12c2e9, #c471ed, #f64f59)").unwrap();

  let container = ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(Color([255, 255, 255, 255]))))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::font_size(Px(48.0).into()))
        .with(StyleDeclaration::font_weight(FontWeight::from(800.0)))
        .with_padding(Sides([Px(40.0); 4])),
    ),
    children: Some([
      TextNode {
        class_name: None,
        id: None,
        tag_name: None,
    preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with(StyleDeclaration::background_image(Some(gradient_images)))
            .with(StyleDeclaration::background_size(BackgroundSizes::from_str("100% 100%").unwrap()))
            .with(StyleDeclaration::background_position(
              BackgroundPositions::from_str("center center").unwrap(),
            ))
            .with(StyleDeclaration::background_clip(BackgroundClip::Text))
            .with(StyleDeclaration::color(ColorInput::Value(Color::transparent())))
            .with(StyleDeclaration::width(Percentage(100.0))),
        ),
        text: "This is a multiline text with a beautiful gradient background clipped to the text shape. It demonstrates how background-clip: text works with longer content.".to_string(),
      }
      .into(),
    ].into()),
  };

  run_fixture_test(container.into(), "style_background_clip_text_multiline");
}

#[test]
fn test_style_background_clip_comparison() {
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
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with_gap(SpacePair::from_single(Px(20.0)))
        .with_padding(Sides([Px(20.0); 4])),
    ),
    children: Some(
      [
        // Border Box
        ContainerNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            Style::default()
              .with(StyleDeclaration::width(Percentage(100.0)))
              .with(StyleDeclaration::height(Px(80.0)))
              .with(StyleDeclaration::background_color(ColorInput::Value(
                Color([255, 0, 0, 255]),
              )))
              .with(StyleDeclaration::background_clip(BackgroundClip::BorderBox))
              .with_padding(Sides([Px(15.0); 4]))
              .with_border_width(Sides([Px(8.0); 4]))
              .with(StyleDeclaration::border_style(BorderStyle::Solid))
              .with(StyleDeclaration::border_color(ColorInput::Value(Color([
                0, 0, 0, 128,
              ])))),
          ),
          children: Some(
            [TextNode {
              class_name: None,
              id: None,
              tag_name: None,
              preset: None,
              tw: None,
              style: Some(
                Style::default()
                  .with(StyleDeclaration::font_size(Px(20.0).into()))
                  .with(StyleDeclaration::color(ColorInput::Value(Color::white()))),
              ),
              text: "border-box".to_string(),
            }
            .into()]
            .into(),
          ),
        }
        .into(),
        // Padding Box
        ContainerNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            Style::default()
              .with(StyleDeclaration::width(Percentage(100.0)))
              .with(StyleDeclaration::height(Px(80.0)))
              .with(StyleDeclaration::background_color(ColorInput::Value(
                Color([0, 128, 255, 255]),
              )))
              .with(StyleDeclaration::background_clip(
                BackgroundClip::PaddingBox,
              ))
              .with_padding(Sides([Px(15.0); 4]))
              .with_border_width(Sides([Px(8.0); 4]))
              .with(StyleDeclaration::border_style(BorderStyle::Solid))
              .with(StyleDeclaration::border_color(ColorInput::Value(Color([
                0, 0, 0, 128,
              ])))),
          ),
          children: Some(
            [TextNode {
              class_name: None,
              id: None,
              tag_name: None,
              preset: None,
              tw: None,
              style: Some(
                Style::default()
                  .with(StyleDeclaration::font_size(Px(20.0).into()))
                  .with(StyleDeclaration::color(ColorInput::Value(Color::white()))),
              ),
              text: "padding-box".to_string(),
            }
            .into()]
            .into(),
          ),
        }
        .into(),
        // Content Box
        ContainerNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            Style::default()
              .with(StyleDeclaration::width(Percentage(100.0)))
              .with(StyleDeclaration::height(Px(80.0)))
              .with(StyleDeclaration::background_color(ColorInput::Value(
                Color([34, 197, 94, 255]),
              )))
              .with(StyleDeclaration::background_clip(
                BackgroundClip::ContentBox,
              ))
              .with_padding(Sides([Px(15.0); 4]))
              .with_border_width(Sides([Px(8.0); 4]))
              .with(StyleDeclaration::border_style(BorderStyle::Solid))
              .with(StyleDeclaration::border_color(ColorInput::Value(Color([
                0, 0, 0, 128,
              ])))),
          ),
          children: Some(
            [TextNode {
              class_name: None,
              id: None,
              tag_name: None,
              preset: None,
              tw: None,
              style: Some(
                Style::default()
                  .with(StyleDeclaration::font_size(Px(20.0).into()))
                  .with(StyleDeclaration::color(ColorInput::Value(Color::white()))),
              ),
              text: "content-box".to_string(),
            }
            .into()]
            .into(),
          ),
        }
        .into(),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_background_clip_comparison");
}
