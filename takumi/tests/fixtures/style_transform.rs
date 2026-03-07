use takumi::layout::{
  node::{ContainerNode, ImageNode, TextNode},
  style::{
    Length::{Percentage, Px, Rem},
    *,
  },
};

use crate::test_utils::run_fixture_test;

const ROTATED_ANGLES: &[f32] = &[0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0];

#[test]
fn test_rotate_image() {
  let image = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center)),
    ),
    tw: None,
    children: Some(
      [ImageNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        style: Some(Style::default().with(StyleDeclaration::rotate(Some(Angle::new(90.0))))),
        tw: None,
        src: "assets/images/yeecord.png".into(),
        width: None,
        height: None,
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(image.into(), "style_rotate_image");
}

#[test]
fn test_rotate() {
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
          Color::white(),
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
        style: Some(
          Style::default()
            .with(StyleDeclaration::width(Rem(16.0)))
            .with(StyleDeclaration::height(Rem(16.0)))
            .with(StyleDeclaration::background_color(ColorInput::Value(
              Color::black(),
            )))
            .with(StyleDeclaration::rotate(Some(Angle::new(45.0)))),
        ),
        children: None,
        tw: None,
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_rotate");
}

#[test]
fn test_style_transform_origin_center() {
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
          Color::white(),
        ))),
    ),
    children: Some(Box::from_iter(ROTATED_ANGLES.iter().map(|angle| {
      create_rotated_container(*angle, TransformOrigin::default()).into()
    }))),
  };

  run_fixture_test(container.into(), "style_transform_origin_center");
}

#[test]
fn test_style_transform_origin_top_left() {
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
          Color::white(),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Px(24.0).into())),
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

  run_fixture_test(container.into(), "style_transform_origin_top_left");
}

fn create_rotated_container(angle: f32, transform_origin: TransformOrigin) -> ImageNode {
  ImageNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::translate(SpacePair::from_single(
          Percentage(-50.0),
        )))
        .with(StyleDeclaration::rotate(Some(Angle::new(angle))))
        .with(StyleDeclaration::position(Position::Absolute))
        .with(StyleDeclaration::top(Percentage(50.0)))
        .with(StyleDeclaration::left(Percentage(50.0)))
        .with(StyleDeclaration::transform_origin(transform_origin))
        .with(StyleDeclaration::width(Px(200.0)))
        .with(StyleDeclaration::height(Px(200.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 0, 0, 30]),
        )))
        .with_border_width(Sides([Px(1.0); 4]))
        .with(StyleDeclaration::border_style(BorderStyle::Solid))
        .with_border_radius(Box::new(BorderRadius(Sides(
          [SpacePair::from_single(Px(12.0)); 4],
        )))),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  }
}

#[test]
fn test_style_transform_translate_and_scale() {
  let mut container = ContainerNode {
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
          Color::white(),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Px(24.0).into())),
    ),
    children: None,
  };

  let position = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Px(200.0)))
        .with(StyleDeclaration::height(Px(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 0, 0, 255]),
        ))),
    ),
    children: Some(
      [TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        text: "200px x 100px".to_string(),
        tw: None,
        style: None,
      }
      .into()]
      .into(),
    ),
  };

  let translated = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Px(300.0)))
        .with(StyleDeclaration::height(Px(300.0)))
        .with_border_width(Sides([Px(1.0); 4]))
        .with(StyleDeclaration::border_style(BorderStyle::Solid))
        .with(StyleDeclaration::translate(SpacePair::from_single(Px(
          300.0,
        ))))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 128, 255, 255]),
        ))),
    ),
    children: Some(
      [ImageNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        src: "assets/images/yeecord.png".into(),
        style: Some(
          Style::default()
            .with(StyleDeclaration::width(Percentage(100.0)))
            .with(StyleDeclaration::height(Percentage(100.0))),
        ),
        width: None,
        height: None,
      }
      .into()]
      .into(),
    ),
  };

  let scaled = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::scale(SpacePair::from_single(
          PercentageNumber(2.0),
        )))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 255, 0, 255]),
        )))
        .with(StyleDeclaration::width(Px(100.0)))
        .with(StyleDeclaration::height(Px(100.0)))
        .with_border_width(Sides([Px(1.0); 4]))
        .with(StyleDeclaration::border_style(BorderStyle::Solid))
        .with(StyleDeclaration::font_size(Px(12.0).into())),
    ),
    children: Some(
      [TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        text: "100px x 100px, scale(2.0, 2.0)".to_string(),
        tw: None,
        style: None,
      }
      .into()]
      .into(),
    ),
  };

  let rotated = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::rotate(Some(Angle::new(45.0))))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 0, 255, 255]),
        )))
        .with(StyleDeclaration::width(Px(200.0)))
        .with(StyleDeclaration::height(Px(200.0)))
        .with_border_width(Sides([Px(1.0); 4]))
        .with(StyleDeclaration::border_style(BorderStyle::Solid))
        .with(StyleDeclaration::color(ColorInput::Value(Color::white())))
        .with(StyleDeclaration::border_color(ColorInput::Value(
          Color::black(),
        ))),
    ),
    children: Some(
      [TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        text: "200px x 200px, rotate(45deg)".to_string(),
        tw: None,
        style: None,
      }
      .into()]
      .into(),
    ),
  };

  container.children = Some(
    [
      position.into(),
      translated.into(),
      scaled.into(),
      rotated.into(),
    ]
    .into(),
  );

  run_fixture_test(container.into(), "style_transform_translate_and_scale");
}
