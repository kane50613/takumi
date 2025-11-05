use takumi::layout::{
  node::{ContainerNode, ImageNode, NodeKind},
  style::{LengthUnit::*, *},
};

mod test_utils;
use test_utils::run_style_width_test;

fn create_luma_logo_container() -> ContainerNode<NodeKind> {
  ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_image(CssOption::some(
          BackgroundImages::from_str("linear-gradient(135deg, #2d3748 0%, #1a202c 100%)").unwrap(),
        ))
        .display(Display::Flex)
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .build()
        .unwrap(),
    ),
    children: Some(vec![NodeKind::Image(ImageNode {
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
    })]),
  }
}

#[test]
fn test_svg_luma_logo_gradient_background() {
  let container = create_luma_logo_container();

  run_style_width_test(
    NodeKind::Container(container),
    "tests/fixtures/svg_luma_logo_gradient_background.webp",
  );
}
