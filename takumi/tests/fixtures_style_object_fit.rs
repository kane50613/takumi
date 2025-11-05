use takumi::layout::{
  node::ImageNode,
  style::{LengthUnit::Percentage, ObjectFit, StyleBuilder},
};

mod test_utils;
use test_utils::run_style_width_test;

#[test]
fn test_style_object_fit_contain() {
  let image = ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .object_fit(ObjectFit::Contain)
        .build()
        .unwrap(),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_style_width_test(image.into(), "tests/fixtures/style_object_fit_contain.webp");
}

#[test]
fn test_style_object_fit_cover() {
  let image = ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .object_fit(ObjectFit::Cover)
        .build()
        .unwrap(),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_style_width_test(image.into(), "tests/fixtures/style_object_fit_cover.webp");
}

#[test]
fn test_style_object_fit_fill() {
  let image = ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .object_fit(ObjectFit::Fill)
        .build()
        .unwrap(),
    ),
    src: "assets/images/yeecord.png".into(),
    width: None,
    height: None,
  };

  run_style_width_test(image.into(), "tests/fixtures/style_object_fit_fill.webp");
}

#[test]
fn test_style_object_fit_none() {
  let image = ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .object_fit(ObjectFit::None)
        .build()
        .unwrap(),
    ),
    src: "assets/images/yeecord.png".into(),
    width: None,
    height: None,
  };

  run_style_width_test(image.into(), "tests/fixtures/style_object_fit_none.webp");
}

#[test]
fn test_style_object_fit_scale_down() {
  let image = ImageNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .object_fit(ObjectFit::ScaleDown)
        .build()
        .unwrap(),
    ),
    src: "assets/images/yeecord.png".into(),
    width: None,
    height: None,
  };

  run_style_width_test(
    image.into(),
    "tests/fixtures/style_object_fit_scale_down.webp",
  );
}
