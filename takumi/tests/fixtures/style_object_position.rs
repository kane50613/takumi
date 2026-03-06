use takumi::layout::{
  node::ImageNode,
  style::{
    BackgroundPosition, Length::Percentage, ObjectFit, PositionComponent, PositionKeywordX,
    PositionKeywordY, SpacePair, Style, StyleDeclaration,
  },
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_object_position_contain_center() {
  let image = ImageNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::object_fit(ObjectFit::Contain))
        .with(StyleDeclaration::object_position(BackgroundPosition(
          SpacePair::from_single(PositionComponent::KeywordX(PositionKeywordX::Center)),
        ))),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_fixture_test(image.into(), "style_object_position_contain_center");
}

#[test]
fn test_style_object_position_contain_top_left() {
  let image = ImageNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
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
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_fixture_test(image.into(), "style_object_position_contain_top_left");
}

#[test]
fn test_style_object_position_contain_bottom_right() {
  let image = ImageNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
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
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_fixture_test(image.into(), "style_object_position_contain_bottom_right");
}

#[test]
fn test_style_object_position_cover_center() {
  let image = ImageNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
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
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_fixture_test(image.into(), "style_object_position_cover_center");
}

#[test]
fn test_style_object_position_cover_top_left() {
  let image = ImageNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
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
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_fixture_test(image.into(), "style_object_position_cover_top_left");
}

#[test]
fn test_style_object_position_none_center() {
  let image = ImageNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
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
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_fixture_test(image.into(), "style_object_position_none_center");
}

#[test]
fn test_style_object_position_none_top_left() {
  let image = ImageNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
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
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_fixture_test(image.into(), "style_object_position_none_top_left");
}

#[test]
fn test_style_object_position_percentage_25_75() {
  let image = ImageNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::object_fit(ObjectFit::Contain))
        .with(StyleDeclaration::object_position(BackgroundPosition(
          SpacePair::from_pair(Percentage(25.0).into(), Percentage(75.0).into()),
        ))),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_fixture_test(image.into(), "style_object_position_percentage_25_75");
}
