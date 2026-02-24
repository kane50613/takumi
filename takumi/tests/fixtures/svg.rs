use takumi::layout::{
  node::{ContainerNode, ImageNode, NodeKind},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

fn create_luma_logo_container() -> ContainerNode<NodeKind> {
  ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_image(Some(
          BackgroundImages::from_str("linear-gradient(135deg, #2d3748 0%, #1a202c 100%)").unwrap(),
        ))
        .display(Display::Flex)
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .build()
        .unwrap(),
    ),
    children: Some(
      [NodeKind::Image(ImageNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Px(204.0))
            .height(Px(76.0))
            .object_fit(ObjectFit::Contain)
            .build()
            .unwrap(),
        ),
        width: None,
        height: None,
        src: "assets/images/luma.svg".into(),
      })]
      .into(),
    ),
  }
}

#[test]
fn test_svg_luma_logo_gradient_background() {
  run_fixture_test(
    create_luma_logo_container().into(),
    "svg_luma_logo_gradient_background",
  );
}
