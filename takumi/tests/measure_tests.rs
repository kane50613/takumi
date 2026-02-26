mod test_utils;

use takumi::{
  layout::{
    node::{ContainerNode, ImageNode, NodeKind, TextNode},
    style::{Affine, Color, ColorInput, Display, Length::*, Position, Sides, StyleBuilder},
  },
  rendering::{MeasuredNode, MeasuredTextRun, RenderOptionsBuilder, measure_layout},
};
use test_utils::{CONTEXT, create_test_viewport};

#[test]
fn test_measure_simple_container() {
  let node: NodeKind = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(100.0))
        .height(Px(100.0))
        .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
        .build()
        .unwrap(),
    ),
    children: None,
  }
  .into();

  let result = measure_layout(
    RenderOptionsBuilder::default()
      .viewport(create_test_viewport())
      .node(node)
      .global(&CONTEXT)
      .build()
      .unwrap(),
  )
  .unwrap();

  assert_eq!(
    result,
    MeasuredNode {
      width: 100.0,
      height: 100.0,
      transform: Affine::IDENTITY.to_cols_array(),
      children: Vec::new(),
      runs: Vec::new(),
    }
  );
}

#[test]
fn test_measure_text_node() {
  let node: NodeKind = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(300.0))
        .font_size(Some(Px(20.0)))
        .build()
        .unwrap(),
    ),
    text: "Hello World".to_string(),
  }
  .into();

  let result = measure_layout(
    RenderOptionsBuilder::default()
      .viewport(create_test_viewport())
      .node(node)
      .global(&CONTEXT)
      .build()
      .unwrap(),
  )
  .unwrap();

  assert_eq!(
    result,
    MeasuredNode {
      width: 300.0,
      height: 26.0,
      transform: Affine::IDENTITY.to_cols_array(),
      children: Vec::new(),
      runs: Vec::new(), // it's a block node, so no runs!
    }
  )
}

#[test]
fn test_measure_inline_layout() {
  let node: NodeKind = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(400.0))
        .height(Px(300.0))
        .font_size(Some(Px(20.0)))
        .display(Display::Block)
        .build()
        .unwrap(),
    ),
    children: Some(
      vec![
        TextNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .display(Display::Inline)
              .build()
              .unwrap(),
          ),
          text: "Hello World".to_string(),
        }
        .into(),
        ImageNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .display(Display::Inline)
              .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
              .build()
              .unwrap(),
          ),
          width: None,
          height: None,
          src: "assets/images/yeecord.png".into(),
        }
        .into(),
        TextNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .display(Display::Inline)
              .build()
              .unwrap(),
          ),
          text: "This is Takumi Speaking".to_string(),
        }
        .into(),
      ]
      .into_boxed_slice(),
    ),
  }
  .into();

  let result = measure_layout(
    RenderOptionsBuilder::default()
      .viewport(create_test_viewport())
      .node(node)
      .global(&CONTEXT)
      .build()
      .unwrap(),
  )
  .unwrap();

  assert_eq!(
    result,
    MeasuredNode {
      width: 400.0,
      height: 300.0,
      transform: Affine::IDENTITY.to_cols_array(),
      runs: vec![
        MeasuredTextRun {
          text: "Hello World".to_string(),
          x: 0.0,
          y: 104.9, // we have the image 128px height on the same line, so the text is centered vertically
          width: 105.46001,
          height: 26.0,
        },
        MeasuredTextRun {
          text: "This is Takumi ".to_string(),
          x: 233.46,
          y: 104.9,
          width: 132.79999,
          height: 26.0,
        },
        MeasuredTextRun {
          text: "Speaking".to_string(),
          x: 0.0,
          y: 127.9,
          width: 85.71999,
          height: 26.0,
        },
      ],
      children: vec![MeasuredNode {
        width: 128.0,
        height: 128.0,
        transform: [1.0, 0.0, 0.0, 1.0, 105.46001, -3.0],
        children: Vec::new(),
        runs: Vec::new(),
      }],
    }
  )
}

#[test]
fn test_measure_svg_attr_size_in_absolute_flex_container() {
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

  let result = measure_layout(
    RenderOptionsBuilder::default()
      .viewport(create_test_viewport())
      .node(node)
      .global(&CONTEXT)
      .build()
      .unwrap(),
  )
  .unwrap();

  assert_eq!(result.children.len(), 1);

  let absolute_container = &result.children[0];
  assert_eq!(absolute_container.width, 100.0);
  assert_eq!(absolute_container.height, 100.0);
  assert_eq!(
    absolute_container.transform,
    [1.0, 0.0, 0.0, 1.0, 1060.0, 490.0]
  );
  assert_eq!(absolute_container.children.len(), 1);

  let svg_child = &absolute_container.children[0];
  assert_eq!(svg_child.width, 100.0);
  assert_eq!(svg_child.height, 100.0);
  assert_eq!(svg_child.transform, [1.0, 0.0, 0.0, 1.0, 1060.0, 490.0]);
}
