use takumi::layout::{
  node::Node,
  style::{Display, Length::Percentage, ObjectFit, Style, StyleDeclaration},
};

use crate::test_utils::run_fixture_test;

fn image_with_object_fit(object_fit: ObjectFit) -> Node {
  Node::image("assets/images/yeecord.png").with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::object_fit(object_fit)),
  )
}

#[test]
fn test_style_object_fit_contain() {
  run_fixture_test(
    image_with_object_fit(ObjectFit::Contain),
    "style_object_fit_contain",
  );
}

#[test]
fn test_style_object_fit_cover() {
  run_fixture_test(
    image_with_object_fit(ObjectFit::Cover),
    "style_object_fit_cover",
  );
}

#[test]
fn test_style_object_fit_fill() {
  run_fixture_test(
    image_with_object_fit(ObjectFit::Fill),
    "style_object_fit_fill",
  );
}

#[test]
fn test_style_object_fit_none() {
  run_fixture_test(
    image_with_object_fit(ObjectFit::None),
    "style_object_fit_none",
  );
}

#[test]
fn test_style_object_fit_scale_down() {
  run_fixture_test(
    image_with_object_fit(ObjectFit::ScaleDown),
    "style_object_fit_scale_down",
  );
}
