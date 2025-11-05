use takumi::layout::{
  node::ImageNode,
  style::{
    BackgroundPosition, LengthUnit::Percentage, ObjectFit, PositionComponent, PositionKeywordX,
    PositionKeywordY, SpacePair, StyleBuilder,
  },
};

mod test_utils;
use test_utils::run_style_width_test;

#[test]
fn test_style_object_position_contain_center() {
  let image = ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .object_fit(ObjectFit::Contain)
        .object_position(BackgroundPosition(SpacePair::from_single(
          PositionComponent::KeywordX(PositionKeywordX::Center),
        )))
        .build()
        .unwrap(),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_style_width_test(
    image.into(),
    "tests/fixtures/style_object_position_contain_center.webp",
  );
}

#[test]
fn test_style_object_position_contain_top_left() {
  let image = ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .object_fit(ObjectFit::Contain)
        .object_position(BackgroundPosition(SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Left),
          PositionComponent::KeywordY(PositionKeywordY::Top),
        )))
        .build()
        .unwrap(),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_style_width_test(
    image.into(),
    "tests/fixtures/style_object_position_contain_top_left.webp",
  );
}

#[test]
fn test_style_object_position_contain_bottom_right() {
  let image = ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .object_fit(ObjectFit::Contain)
        .object_position(BackgroundPosition(SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Right),
          PositionComponent::KeywordY(PositionKeywordY::Bottom),
        )))
        .build()
        .unwrap(),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_style_width_test(
    image.into(),
    "tests/fixtures/style_object_position_contain_bottom_right.webp",
  );
}

#[test]
fn test_style_object_position_cover_center() {
  let image = ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .object_fit(ObjectFit::Cover)
        .object_position(BackgroundPosition(SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Center),
          PositionComponent::KeywordY(PositionKeywordY::Center),
        )))
        .build()
        .unwrap(),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_style_width_test(
    image.into(),
    "tests/fixtures/style_object_position_cover_center.webp",
  );
}

#[test]
fn test_style_object_position_cover_top_left() {
  let image = ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .object_fit(ObjectFit::Cover)
        .object_position(BackgroundPosition(SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Left),
          PositionComponent::KeywordY(PositionKeywordY::Top),
        )))
        .build()
        .unwrap(),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_style_width_test(
    image.into(),
    "tests/fixtures/style_object_position_cover_top_left.webp",
  );
}

#[test]
fn test_style_object_position_none_center() {
  let image = ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .object_fit(ObjectFit::None)
        .object_position(BackgroundPosition(SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Center),
          PositionComponent::KeywordY(PositionKeywordY::Center),
        )))
        .build()
        .unwrap(),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_style_width_test(
    image.into(),
    "tests/fixtures/style_object_position_none_center.webp",
  );
}

#[test]
fn test_style_object_position_none_top_left() {
  let image = ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .object_fit(ObjectFit::None)
        .object_position(BackgroundPosition(SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Left),
          PositionComponent::KeywordY(PositionKeywordY::Top),
        )))
        .build()
        .unwrap(),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_style_width_test(
    image.into(),
    "tests/fixtures/style_object_position_none_top_left.webp",
  );
}

#[test]
fn test_style_object_position_percentage_25_75() {
  let image = ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .object_fit(ObjectFit::Contain)
        .object_position(BackgroundPosition(SpacePair::from_pair(
          PositionComponent::Length(Percentage(25.0)),
          PositionComponent::Length(Percentage(75.0)),
        )))
        .build()
        .unwrap(),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_style_width_test(
    image.into(),
    "tests/fixtures/style_object_position_percentage_25_75.webp",
  );
}
