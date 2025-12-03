use takumi::layout::{
  node::{ContainerNode, NodeKind, TextNode},
  style::{LengthUnit::*, *},
};

mod test_utils;
use test_utils::run_style_width_test;

fn create_container_with_background_clip(
  background_clip: BackgroundClip,
  background_color: Color,
  padding: f32,
  border_width: f32,
) -> ContainerNode<NodeKind> {
  ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([200, 200, 200, 255])))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Rem(16.0))
            .height(Rem(10.0))
            .background_color(ColorInput::Value(background_color))
            .background_clip(background_clip)
            .padding(Sides([Px(padding); 4]))
            .border_width(Some(Sides([Px(border_width); 4])))
            .border_color(Some(ColorInput::Value(Color([0, 0, 0, 255]))))
            .border_radius(BorderRadius(Sides([SpacePair::from_single(Px(8.0)); 4])))
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into(),
    ]),
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

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_clip_border_box.webp",
  );
}

#[test]
fn test_style_background_clip_padding_box() {
  let container = create_container_with_background_clip(
    BackgroundClip::PaddingBox,
    Color([0, 128, 255, 255]),
    20.0,
    10.0,
  );

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_clip_padding_box.webp",
  );
}

#[test]
fn test_style_background_clip_content_box() {
  let container = create_container_with_background_clip(
    BackgroundClip::ContentBox,
    Color([34, 197, 94, 255]),
    20.0,
    10.0,
  );

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_clip_content_box.webp",
  );
}

#[test]
fn test_style_background_clip_text_gradient() {
  let gradient_images = BackgroundImages::from_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .font_size(Some(Px(72.0)))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .background_image(Some(gradient_images))
            .background_size(Some(BackgroundSizes::from_str("100% 100%").unwrap()))
            .background_position(Some(BackgroundPositions::from_str("0 0").unwrap()))
            .background_repeat(Some(BackgroundRepeats::from_str("no-repeat").unwrap()))
            .background_clip(BackgroundClip::Text)
            .color(ColorInput::Value(Color::transparent()))
            .build()
            .unwrap(),
        ),
        text: "Gradient Text".to_string(),
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_clip_text_gradient.webp",
  );
}

#[test]
fn test_style_background_clip_text_radial_gradient() {
  let gradient_images =
    BackgroundImages::from_str("radial-gradient(circle, #ff0080, #7928ca, #0070f3)").unwrap();

  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([255, 255, 255, 255])))
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .font_size(Some(Px(64.0)))
        .font_weight(FontWeight::from(700.0))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .background_image(Some(gradient_images))
            .background_size(Some(BackgroundSizes::from_str("100% 100%").unwrap()))
            .background_clip(BackgroundClip::Text)
            .color(ColorInput::Value(Color::transparent()))
            .build()
            .unwrap(),
        ),
        text: "Radial Gradient".to_string(),
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_clip_text_radial.webp",
  );
}

#[test]
fn test_style_background_clip_border_area() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([200, 200, 200, 255])))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Rem(16.0))
            .height(Rem(10.0))
            .background_color(ColorInput::Value(Color([255, 165, 0, 255])))
            .background_clip(BackgroundClip::BorderArea)
            .padding(Sides([Px(20.0); 4]))
            .border_width(Some(Sides([Px(10.0); 4])))
            .border_color(Some(ColorInput::Value(Color([0, 0, 0, 128]))))
            .border_radius(BorderRadius(Sides([SpacePair::from_single(Px(8.0)); 4])))
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
    "tests/fixtures/style_background_clip_border_area.webp",
  );
}

#[test]
fn test_style_background_clip_with_gradient_background() {
  let gradient_images =
    BackgroundImages::from_str("linear-gradient(135deg, #667eea 0%, #764ba2 100%)").unwrap();

  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([200, 200, 200, 255])))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Rem(16.0))
            .height(Rem(10.0))
            .background_image(Some(gradient_images))
            .background_clip(BackgroundClip::PaddingBox)
            .padding(Sides([Px(30.0); 4]))
            .border_width(Some(Sides([Px(15.0); 4])))
            .border_color(Some(ColorInput::Value(Color([255, 255, 255, 255]))))
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
    "tests/fixtures/style_background_clip_gradient_padding.webp",
  );
}

#[test]
fn test_style_background_clip_text_multiline() {
  let gradient_images =
    BackgroundImages::from_str("linear-gradient(45deg, #12c2e9, #c471ed, #f64f59)").unwrap();

  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([255, 255, 255, 255])))
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .font_size(Some(Px(48.0)))
        .font_weight(FontWeight::from(800.0))
        .padding(Sides([Px(40.0); 4]))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .background_image(Some(gradient_images))
            .background_size(Some(BackgroundSizes::from_str("100% 100%").unwrap()))
            .background_clip(BackgroundClip::Text)
            .color(ColorInput::Value(Color::transparent()))
            .width(Percentage(100.0))
            .build()
            .unwrap(),
        ),
        text: "This is a multiline text with a beautiful gradient background clipped to the text shape. It demonstrates how background-clip: text works with longer content.".to_string(),
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_clip_text_multiline.webp",
  );
}

#[test]
fn test_style_background_clip_comparison() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .display(Display::Flex)
        .flex_direction(FlexDirection::Column)
        .gap(SpacePair::from_single(Px(20.0)))
        .padding(Sides([Px(20.0); 4]))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      // Border Box
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Percentage(100.0))
            .height(Px(80.0))
            .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
            .background_clip(BackgroundClip::BorderBox)
            .padding(Sides([Px(15.0); 4]))
            .border_width(Some(Sides([Px(8.0); 4])))
            .border_color(Some(ColorInput::Value(Color([0, 0, 0, 128]))))
            .build()
            .unwrap(),
        ),
        children: Some(vec![
          TextNode {
            tw: None,
            style: Some(
              StyleBuilder::default()
                .font_size(Some(Px(20.0)))
                .color(ColorInput::Value(Color::white()))
                .build()
                .unwrap(),
            ),
            text: "border-box".to_string(),
          }
          .into(),
        ]),
      }
      .into(),
      // Padding Box
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Percentage(100.0))
            .height(Px(80.0))
            .background_color(ColorInput::Value(Color([0, 128, 255, 255])))
            .background_clip(BackgroundClip::PaddingBox)
            .padding(Sides([Px(15.0); 4]))
            .border_width(Some(Sides([Px(8.0); 4])))
            .border_color(Some(ColorInput::Value(Color([0, 0, 0, 128]))))
            .build()
            .unwrap(),
        ),
        children: Some(vec![
          TextNode {
            tw: None,
            style: Some(
              StyleBuilder::default()
                .font_size(Some(Px(20.0)))
                .color(ColorInput::Value(Color::white()))
                .build()
                .unwrap(),
            ),
            text: "padding-box".to_string(),
          }
          .into(),
        ]),
      }
      .into(),
      // Content Box
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Percentage(100.0))
            .height(Px(80.0))
            .background_color(ColorInput::Value(Color([34, 197, 94, 255])))
            .background_clip(BackgroundClip::ContentBox)
            .padding(Sides([Px(15.0); 4]))
            .border_width(Some(Sides([Px(8.0); 4])))
            .border_color(Some(ColorInput::Value(Color([0, 0, 0, 128]))))
            .build()
            .unwrap(),
        ),
        children: Some(vec![
          TextNode {
            tw: None,
            style: Some(
              StyleBuilder::default()
                .font_size(Some(Px(20.0)))
                .color(ColorInput::Value(Color::white()))
                .build()
                .unwrap(),
            ),
            text: "content-box".to_string(),
          }
          .into(),
        ]),
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_clip_comparison.webp",
  );
}
