use takumi::layout::{
  node::ImageNode,
  style::{
    BackgroundPosition, Length::Percentage, ObjectFit, PositionComponent, PositionKeywordX,
    PositionKeywordY, SpacePair, Style, StyleDeclaration,
  },
};

use crate::test_utils::run_fixture_test;

fn image_with_style(style: Style) -> ImageNode {
  ImageNode::default()
    .with_src("assets/images/yeecord.png")
    .with_style(style)
}

#[test]
fn test_style_object_position_contain_center() {
  let image = image_with_style(
    Style::default()
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::object_fit(ObjectFit::Contain))
      .with(StyleDeclaration::object_position(BackgroundPosition(
        SpacePair::from_single(PositionComponent::KeywordX(PositionKeywordX::Center)),
      ))),
  );

  run_fixture_test(image.into(), "style_object_position_contain_center");
}

#[test]
fn test_style_object_position_contain_top_left() {
  let image = image_with_style(
    Style::default()
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::object_fit(ObjectFit::Contain))
      .with(StyleDeclaration::object_position(BackgroundPosition(
        SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Left),
          PositionComponent::KeywordY(PositionKeywordY::Top),
        ),
      ))),
  );

  run_fixture_test(image.into(), "style_object_position_contain_top_left");
}

#[test]
fn test_style_object_position_contain_bottom_right() {
  let image = image_with_style(
    Style::default()
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::object_fit(ObjectFit::Contain))
      .with(StyleDeclaration::object_position(BackgroundPosition(
        SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Right),
          PositionComponent::KeywordY(PositionKeywordY::Bottom),
        ),
      ))),
  );

  run_fixture_test(image.into(), "style_object_position_contain_bottom_right");
}

#[test]
fn test_style_object_position_cover_center() {
  let image = image_with_style(
    Style::default()
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::object_fit(ObjectFit::Cover))
      .with(StyleDeclaration::object_position(BackgroundPosition(
        SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Center),
          PositionComponent::KeywordY(PositionKeywordY::Center),
        ),
      ))),
  );

  run_fixture_test(image.into(), "style_object_position_cover_center");
}

#[test]
fn test_style_object_position_cover_top_left() {
  let image = image_with_style(
    Style::default()
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::object_fit(ObjectFit::Cover))
      .with(StyleDeclaration::object_position(BackgroundPosition(
        SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Left),
          PositionComponent::KeywordY(PositionKeywordY::Top),
        ),
      ))),
  );

  run_fixture_test(image.into(), "style_object_position_cover_top_left");
}

#[test]
fn test_style_object_position_none_center() {
  let image = image_with_style(
    Style::default()
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::object_fit(ObjectFit::None))
      .with(StyleDeclaration::object_position(BackgroundPosition(
        SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Center),
          PositionComponent::KeywordY(PositionKeywordY::Center),
        ),
      ))),
  );

  run_fixture_test(image.into(), "style_object_position_none_center");
}

#[test]
fn test_style_object_position_none_top_left() {
  let image = image_with_style(
    Style::default()
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::object_fit(ObjectFit::None))
      .with(StyleDeclaration::object_position(BackgroundPosition(
        SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Left),
          PositionComponent::KeywordY(PositionKeywordY::Top),
        ),
      ))),
  );

  run_fixture_test(image.into(), "style_object_position_none_top_left");
}

#[test]
fn test_style_object_position_percentage_25_75() {
  let image = image_with_style(
    Style::default()
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::object_fit(ObjectFit::Contain))
      .with(StyleDeclaration::object_position(BackgroundPosition(
        SpacePair::from_pair(Percentage(25.0).into(), Percentage(75.0).into()),
      ))),
  );

  run_fixture_test(image.into(), "style_object_position_percentage_25_75");
}
