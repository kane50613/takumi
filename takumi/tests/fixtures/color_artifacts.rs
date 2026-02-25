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
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([147, 197, 253, 255])))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .padding(Sides([Rem(4.0); 4]))
        .build()
        .unwrap(),
    ),
    children: Some(
      [ImageNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Percentage(100.0))
            .height(Percentage(100.0))
            .object_fit(ObjectFit::Contain)
            .border_radius(Box::new(BorderRadius::from_str("10px").unwrap()))
            .build()
            .unwrap(),
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
