use takumi::layout::{
  node::{ContainerNode, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn inline_vertical_align_types() {
  let row = |label: &str, align: VerticalAlign, color: Color| {
    ContainerNode {
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::Block)
          .margin(Sides([Px(10.0); 4]))
          .line_height(LineHeight::Length(Px(60.0))) // Explicit line height
          .font_size(Some(Px(24.0))) // Explicit font size
          .background_color(ColorInput::Value(Color([240, 240, 240, 255]))) // Light gray background to see line box
          .border_width(Some(Sides([Px(1.0); 4])))
          .border_style(Some(BorderStyle::Solid))
          .border_color(ColorInput::Value(Color::black()))
          .text_decoration(TextDecoration {
            line: TextDecorationLines::UNDERLINE | TextDecorationLines::OVERLINE,
            ..Default::default()
          })
          .build()
          .unwrap(),
      ),
      children: Some(
        [
          TextNode {
            preset: None,
            tw: None,
            style: Some(
              StyleBuilder::default()
                .display(Display::Inline)
                .build()
                .unwrap(),
            ),
            text: format!("Ref {} ", label),
          }
          .into(),
          ContainerNode {
            preset: None,
            tw: None,
            style: Some(
              StyleBuilder::default()
                .display(Display::InlineBlock)
                .width(Px(40.0))
                .height(Px(40.0))
                .background_color(ColorInput::Value(color))
                .vertical_align(align)
                .build()
                .unwrap(),
            ),
            children: None,
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
            text: " Post".to_string(),
          }
          .into(),
        ]
        .into(),
      ),
    }
    .into()
  };

  let children = [
    row("baseline", VerticalAlign::Baseline, Color([255, 0, 0, 100])),
    row("top", VerticalAlign::Top, Color([0, 255, 0, 100])),
    row("middle", VerticalAlign::Middle, Color([0, 0, 255, 100])),
    row("bottom", VerticalAlign::Bottom, Color([255, 255, 0, 100])),
    row(
      "text-top",
      VerticalAlign::TextTop,
      Color([0, 255, 255, 100]),
    ),
    row(
      "text-bottom",
      VerticalAlign::TextBottom,
      Color([255, 0, 255, 100]),
    ),
    row("sub", VerticalAlign::Sub, Color([100, 100, 100, 100])),
    row("super", VerticalAlign::Super, Color([200, 200, 200, 100])),
  ];

  let container = ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .display(Display::Flex)
        .flex_direction(FlexDirection::Column)
        .padding(Sides([Px(20.0); 4]))
        .background_color(ColorInput::Value(Color::white()))
        .build()
        .unwrap(),
    ),
    children: Some(children.into()),
  };

  run_fixture_test(container.into(), "inline_vertical_align_types");
}

#[test]
fn inline_vertical_align_multiline() {
  let children = [
    TextNode {
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::Inline)
          .build()
          .unwrap(),
      ),
      // Long text to force line break
      text: "This is a long text that should definitely wrap to multiple lines, allowing us to test vertical alignment on the second line as well. ".to_string(),
    }
    .into(),
    ContainerNode {
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::InlineBlock)
          .width(Px(40.0))
          .height(Px(40.0))
          .background_color(ColorInput::Value(Color([0, 255, 0, 255])))
          .vertical_align(VerticalAlign::Top)
          .build()
          .unwrap(),
      ),
      children: None,
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
      text: " After Top. ".to_string(),
    }
    .into(),
    ContainerNode {
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::InlineBlock)
          .width(Px(40.0))
          .height(Px(40.0))
          .background_color(ColorInput::Value(Color([255, 255, 0, 255])))
          .vertical_align(VerticalAlign::Bottom)
          .build()
          .unwrap(),
      ),
      children: None,
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
      text: " After Bottom.".to_string(),
    }
    .into(),
  ];

  let container = ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(400.0))
        .display(Display::Block)
        .padding(Sides([Px(20.0); 4]))
        .background_color(ColorInput::Value(Color::white()))
        .font_size(Some(Px(24.0)))
        .line_height(LineHeight::Length(Px(60.0)))
        .text_decoration(TextDecoration {
          line: TextDecorationLines::UNDERLINE | TextDecorationLines::OVERLINE,
          ..Default::default()
        })
        .build()
        .unwrap(),
    ),
    children: Some(children.into()),
  };

  run_fixture_test(container.into(), "inline_vertical_align_multiline");
}
