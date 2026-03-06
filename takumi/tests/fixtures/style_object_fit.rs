use takumi::layout::{
  node::ImageNode,
  style::{Length::Percentage, ObjectFit, Style, StyleDeclaration},
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_object_fit_contain() {
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
        .with(StyleDeclaration::object_fit(ObjectFit::Contain)),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_fixture_test(image.into(), "style_object_fit_contain");
}

#[test]
fn test_style_object_fit_cover() {
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
        .with(StyleDeclaration::object_fit(ObjectFit::Cover)),
    ),
    width: None,
    height: None,
    src: "assets/images/yeecord.png".into(),
  };

  run_fixture_test(image.into(), "style_object_fit_cover");
}

#[test]
fn test_style_object_fit_fill() {
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
        .with(StyleDeclaration::object_fit(ObjectFit::Fill)),
    ),
    src: "assets/images/yeecord.png".into(),
    width: None,
    height: None,
  };

  run_fixture_test(image.into(), "style_object_fit_fill");
}

#[test]
fn test_style_object_fit_none() {
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
        .with(StyleDeclaration::object_fit(ObjectFit::None)),
    ),
    src: "assets/images/yeecord.png".into(),
    width: None,
    height: None,
  };

  run_fixture_test(image.into(), "style_object_fit_none");
}

#[test]
fn test_style_object_fit_scale_down() {
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
        .with(StyleDeclaration::object_fit(ObjectFit::ScaleDown)),
    ),
    src: "assets/images/yeecord.png".into(),
    width: None,
    height: None,
  };

  run_fixture_test(image.into(), "style_object_fit_scale_down");
}
