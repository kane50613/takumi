use takumi::layout::{
  node::{ContainerNode, ImageNode, NodeKind, TextNode},
  style::{
    Length::{Percentage, Px, Rem},
    *,
  },
};

use crate::test_utils::run_fixture_test;

const ROTATED_ANGLES: &[f32] = &[0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0];

#[test]
fn test_rotate_image() {
  let image = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center)),
    )
    .with_children([ImageNode::default()
      .with_style(Style::default().with(StyleDeclaration::rotate(Some(Angle::new(90.0)))))
      .with_src("assets/images/yeecord.png")]);

  run_fixture_test(image.into(), "style_rotate_image");
}

#[test]
fn test_rotate() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center)),
    )
    .with_children([ContainerNode::default().with_style(
      Style::default()
        .with(StyleDeclaration::width(Rem(16.0)))
        .with(StyleDeclaration::height(Rem(16.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::black(),
        )))
        .with(StyleDeclaration::rotate(Some(Angle::new(45.0)))),
    )]);

  run_fixture_test(container.into(), "style_rotate");
}

#[test]
fn test_style_transform_origin_center() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        ))),
    )
    .with_children(
      ROTATED_ANGLES
        .iter()
        .map(|angle| NodeKind::from(create_rotated_container(*angle, TransformOrigin::default())))
        .collect::<Vec<_>>(),
    );

  run_fixture_test(container.into(), "style_transform_origin_center");
}

#[test]
fn test_style_transform_origin_top_left() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Px(24.0).into())),
    )
    .with_children(
      ROTATED_ANGLES
        .iter()
        .map(|angle| {
          NodeKind::from(create_rotated_container(
            *angle,
            BackgroundPosition(SpacePair::from_pair(
              PositionComponent::KeywordX(PositionKeywordX::Left),
              PositionComponent::KeywordY(PositionKeywordY::Top),
            )),
          ))
        })
        .collect::<Vec<_>>(),
    );

  run_fixture_test(container.into(), "style_transform_origin_top_left");
}

fn create_rotated_container(angle: f32, transform_origin: TransformOrigin) -> ImageNode {
  ImageNode::default()
    .with_src("assets/images/yeecord.png")
    .with_style(
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
    )
}

#[test]
fn test_style_transform_translate_and_scale() {
  let position = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Px(200.0)))
        .with(StyleDeclaration::height(Px(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 0, 0, 255]),
        ))),
    )
    .with_children([TextNode::default().with_text("200px x 100px".to_string())]);

  let translated = ContainerNode::default()
    .with_style(
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
    )
    .with_children([ImageNode::default()
      .with_style(
        Style::default()
          .with(StyleDeclaration::width(Percentage(100.0)))
          .with(StyleDeclaration::height(Percentage(100.0))),
      )
      .with_src("assets/images/yeecord.png")]);

  let scaled = ContainerNode::default()
    .with_style(
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
    )
    .with_children([TextNode::default().with_text("100px x 100px, scale(2.0, 2.0)".to_string())]);

  let rotated = ContainerNode::default()
    .with_style(
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
    )
    .with_children([TextNode::default().with_text("200px x 200px, rotate(45deg)".to_string())]);

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Px(24.0).into())),
    )
    .with_children([position, translated, scaled, rotated]);

  run_fixture_test(container.into(), "style_transform_translate_and_scale");
}
