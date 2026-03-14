use takumi::layout::{
  node::ImageNode,
  style::{Length::Percentage, ObjectFit, Style, StyleDeclaration},
};

use crate::test_utils::run_fixture_test;

fn image_with_object_fit(object_fit: ObjectFit) -> ImageNode {
  ImageNode::default()
    .with_src("assets/images/yeecord.png")
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::object_fit(object_fit)),
    )
}

#[test]
fn test_style_object_fit_contain() {
  run_fixture_test(
    image_with_object_fit(ObjectFit::Contain).into(),
    "style_object_fit_contain",
  );
}

#[test]
fn test_style_object_fit_cover() {
  run_fixture_test(
    image_with_object_fit(ObjectFit::Cover).into(),
    "style_object_fit_cover",
  );
}

#[test]
fn test_style_object_fit_fill() {
  run_fixture_test(
    image_with_object_fit(ObjectFit::Fill).into(),
    "style_object_fit_fill",
  );
}

#[test]
fn test_style_object_fit_none() {
  run_fixture_test(
    image_with_object_fit(ObjectFit::None).into(),
    "style_object_fit_none",
  );
}

#[test]
fn test_style_object_fit_scale_down() {
  run_fixture_test(
    image_with_object_fit(ObjectFit::ScaleDown).into(),
    "style_object_fit_scale_down",
  );
}
