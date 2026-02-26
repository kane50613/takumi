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

#[test]
fn test_svg_attr_size_in_absolute_flex_container() {
  let svg = r##"<svg width="100" height="100" viewBox="0 0 40 40" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M20 0L24.4903 15.5097L40 20L24.4903 24.4903L20 40L15.5097 24.4903L0 20L15.5097 15.5097L20 0Z" fill="#E0FF25"/></svg>"##;

  let node: NodeKind = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([35, 35, 35, 255])))
        .build()
        .unwrap(),
    ),
    children: Some(
      [ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .position(Position::Absolute)
            .inset(Sides([Auto, Px(40.0), Px(40.0), Auto]))
            .display(Display::Flex)
            .build()
            .unwrap(),
        ),
        children: Some(
          [ImageNode {
            class_name: None,
            id: None,
            tag_name: Some("svg".into()),
            preset: None,
            tw: None,
            style: None,
            src: svg.into(),
            width: None,
            height: None,
          }
          .into()]
          .into(),
        ),
      }
      .into()]
      .into(),
    ),
  }
  .into();

  run_fixture_test(node, "svg_attr_size_in_absolute_flex_container");
}
