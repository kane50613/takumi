mod test_utils;

use takumi::{
  layout::{
    node::{ContainerNode, ImageNode, NodeKind, TextNode},
    style::{Affine, Color, ColorInput, Display, Length::*, StyleBuilder},
  },
  rendering::{MeasuredNode, MeasuredTextRun, RenderOptionsBuilder, measure_layout},
};
use test_utils::{CONTEXT, create_test_viewport};

#[test]
fn test_measure_simple_container() {
  let node: NodeKind = ContainerNode {
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
      height: 24.0,
      transform: Affine::IDENTITY.to_cols_array(),
      children: Vec::new(),
      runs: Vec::new(), // it's a block node, so no runs!
    }
  )
}

#[test]
fn test_measure_inline_layout() {
  let node: NodeKind = ContainerNode {
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
          y: 126.9,
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
