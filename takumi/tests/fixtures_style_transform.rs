use takumi::layout::{
  node::{ContainerNode, ImageNode, TextNode},
  style::{
    LengthUnit::{Percentage, Px, Rem},
    *,
  },
};

mod test_utils;
use test_utils::run_style_width_test;

const ROTATED_ANGLES: &[f32] = &[0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0];

#[test]
fn test_rotate_image() {
  let image = ContainerNode {
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .build()
        .unwrap(),
    ),
    tw: None,
    children: Some(vec![
      ImageNode {
        style: Some(
          StyleBuilder::default()
            .rotate(Some(Angle::new(90.0)))
            .build()
            .unwrap(),
        ),
        tw: None,
        src: "assets/images/yeecord.png".into(),
        width: None,
        height: None,
      }
      .into(),
    ]),
  };

  run_style_width_test(image.into(), "tests/fixtures/style_rotate_image.webp");
}

#[test]
fn test_rotate() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
        style: Some(
          StyleBuilder::default()
            .width(Rem(16.0))
            .height(Rem(16.0))
            .background_color(ColorInput::Value(Color::black()))
            .rotate(Some(Angle::new(45.0)))
            .build()
            .unwrap(),
        ),
        children: None,
        tw: None,
      }
      .into(),
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/style_rotate.webp");
}

#[test]
fn test_style_transform_origin_center() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .build()
        .unwrap(),
    ),
    children: Some(
      ROTATED_ANGLES
        .iter()
        .map(|angle| create_rotated_container(*angle, BackgroundPosition::default()).into())
        .collect(),
    ),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_transform_origin_center.webp",
  );
}

#[test]
fn test_style_transform_origin_top_left() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .display(Display::Flex)
        .font_size(Some(Px(24.0)))
        .build()
        .unwrap(),
    ),
    children: Some(
      ROTATED_ANGLES
        .iter()
        .map(|angle| {
          create_rotated_container(
            *angle,
            BackgroundPosition(SpacePair::from_pair(
              PositionComponent::KeywordX(PositionKeywordX::Left),
              PositionComponent::KeywordY(PositionKeywordY::Top),
            )),
          )
          .into()
        })
        .collect(),
    ),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_transform_origin_top_left.webp",
  );
}

fn create_rotated_container(angle: f32, transform_origin: BackgroundPosition) -> ImageNode {
  ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .translate(Some(SpacePair::from_single(Percentage(-50.0))))
        .rotate(Some(Angle::new(angle)))
        .position(Position::Absolute)
        .top(Some(Percentage(50.0)))
        .left(Some(Percentage(50.0)))
        .transform_origin(Some(transform_origin))
        .width(Px(200.0))
        .height(Px(200.0))
        .background_color(ColorInput::Value(Color([255, 0, 0, 30])))
        .border_width(Some(Sides([Px(1.0); 4])))
        .border_radius(BorderRadius(Sides([SpacePair::from_single(Px(12.0)); 4])))
        .build()
        .unwrap(),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  }
}

#[test]
fn test_style_transform_translate_and_scale() {
  let mut container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .display(Display::Flex)
        .font_size(Some(Px(24.0)))
        .build()
        .unwrap(),
    ),
    children: None,
  };

  let position = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(200.0))
        .height(Px(100.0))
        .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      TextNode {
        text: "200px x 100px".to_string(),
        tw: None,
        style: None,
      }
      .into(),
    ]),
  };

  let translated = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(300.0))
        .height(Px(300.0))
        .border_width(Some(Sides([Px(1.0); 4])))
        .translate(Some(SpacePair::from_single(Px(300.0))))
        .background_color(ColorInput::Value(Color([0, 128, 255, 255])))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ImageNode {
        tw: None,
        src: "assets/images/yeecord.png".into(),
        style: Some(
          StyleBuilder::default()
            .width(Percentage(100.0))
            .height(Percentage(100.0))
            .build()
            .unwrap(),
        ),
        width: None,
        height: None,
      }
      .into(),
    ]),
  };

  let scaled = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .scale(Some(SpacePair::from_single(PercentageNumber(2.0))))
        .background_color(ColorInput::Value(Color([0, 255, 0, 255])))
        .width(Px(100.0))
        .height(Px(100.0))
        .border_width(Some(Sides([Px(1.0); 4])))
        .font_size(Some(Px(12.0)))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      TextNode {
        text: "100px x 100px, scale(2.0, 2.0)".to_string(),
        tw: None,
        style: None,
      }
      .into(),
    ]),
  };

  let rotated = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .rotate(Some(Angle::new(45.0)))
        .background_color(ColorInput::Value(Color([0, 0, 255, 255])))
        .width(Px(200.0))
        .height(Px(200.0))
        .border_width(Some(Sides([Px(1.0); 4])))
        .color(ColorInput::Value(Color::white()))
        .border_color(Some(ColorInput::Value(Color::black())))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      TextNode {
        text: "200px x 200px, rotate(45deg)".to_string(),
        tw: None,
        style: None,
      }
      .into(),
    ]),
  };

  container.children = Some(vec![
    position.into(),
    translated.into(),
    scaled.into(),
    rotated.into(),
  ]);

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_transform_translate_and_scale.webp",
  );
}
