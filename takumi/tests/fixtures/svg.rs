use takumi::layout::{
  node::{ContainerNode, ImageNode, NodeKind, TextNode},
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
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_image(Some(
          BackgroundImages::from_str("linear-gradient(135deg, #2d3748 0%, #1a202c 100%)").unwrap(),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center)),
    ),
    children: Some(
      [NodeKind::Image(ImageNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with(StyleDeclaration::width(Px(204.0)))
            .with(StyleDeclaration::height(Px(76.0)))
            .with(StyleDeclaration::object_fit(ObjectFit::Contain)),
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
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([35, 35, 35, 255]),
        ))),
    ),
    children: Some(
      [ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with(StyleDeclaration::position(Position::Absolute))
            .with_inset(Sides([Auto, Px(40.0), Px(40.0), Auto]))
            .with(StyleDeclaration::display(Display::Flex)),
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

#[test]
fn test_svg_current_color_fixture() {
  let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="120"><rect x="0" y="0" width="120" height="120" fill="currentColor"/></svg>"#;

  let swatch = |color: Color| {
    ContainerNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(
        Style::default()
          .with(StyleDeclaration::width(Px(160.0)))
          .with(StyleDeclaration::height(Px(160.0)))
          .with_padding(Sides([Px(20.0); 4]))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([240, 240, 240, 255]),
          )))
          .with(StyleDeclaration::color(ColorInput::Value(color)))
          .with(StyleDeclaration::flex_direction(FlexDirection::Column))
          .with(StyleDeclaration::align_items(AlignItems::Center)),
      ),
      children: Some(
        [
          ImageNode {
            class_name: None,
            id: None,
            tag_name: Some("svg".into()),
            preset: None,
            tw: None,
            style: Some(
              Style::default()
                .with(StyleDeclaration::width(Px(120.0)))
                .with(StyleDeclaration::height(Px(120.0))),
            ),
            src: svg.into(),
            width: None,
            height: None,
          }
          .into(),
          TextNode {
            class_name: None,
            id: None,
            tag_name: None,
            preset: None,
            tw: None,
            style: None,
            text: "Hello".into(),
          }
          .into(),
        ]
        .into(),
      ),
    }
    .into()
  };

  let node: NodeKind = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with_gap(SpacePair::from_single(Px(24.0)))
        .with_padding(Sides([Px(40.0); 4]))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([30, 30, 30, 255]),
        ))),
    ),
    children: Some(
      [
        swatch(Color([230, 40, 70, 255])),
        swatch(Color([60, 140, 255, 255])),
      ]
      .into(),
    ),
  }
  .into();

  run_fixture_test(node, "svg_current_color_fixture");
}
