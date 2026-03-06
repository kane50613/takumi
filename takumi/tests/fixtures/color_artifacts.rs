use takumi::layout::{
  node::{ContainerNode, ImageNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

// zune-jpeg had some strange decoding issues with jpeg (https://github.com/kane50613/takumi/commit/058f87ab1d668c1316ff72319d242989f0adfa43).
// This test is to ensure that never happens again.
#[test]
fn test_color_artifacts() {
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
          Color([147, 197, 253, 255]),
        )))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with_padding(Sides([Rem(4.0); 4])),
    ),
    children: Some(
      [ImageNode {
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
            .with_border_radius(Box::new(BorderRadius::from_str("10px").unwrap())),
        ),
        src: "assets/images/luma-cover-0dfbf65d-0f58-4941-947c-d84a5b131dc0.jpeg".into(),
        width: None,
        height: None,
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "color_artifacts");
}
