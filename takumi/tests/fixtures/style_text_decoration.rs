use takumi::layout::{
  node::{ContainerNode, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_text_decoration() {
  let text = TextNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .text_align(TextAlign::Center)
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(72.0)))
        .text_decoration(TextDecoration {
          line: [
            TextDecorationLine::Underline,
            TextDecorationLine::LineThrough,
            TextDecorationLine::Overline,
          ]
          .into(),
          style: None,
          color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
        })
        .build()
        .unwrap(),
    ),
    text: "Text Decoration with Underline, Line-Through, and Overline".to_string(),
  };

  run_fixture_test(text.into(), "style_text_decoration");
}

#[test]
fn text_decoration_skip_ink_parapsychologists() {
  let make_line = |label: &str, skip_ink: TextDecorationSkipInk| {
    TextNode {
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .width(Percentage(100.0))
          .text_align(TextAlign::Center)
          .font_size(Some(Px(96.0)))
          .text_decoration(TextDecoration {
            line: [TextDecorationLine::Underline].into(),
            style: None,
            color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
          })
          .text_decoration_skip_ink(skip_ink)
          .build()
          .unwrap(),
      ),
      text: format!("{label}: parapsychologists"),
    }
    .into()
  };

  let container = ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .display(Display::Flex)
        .flex_direction(FlexDirection::Column)
        .row_gap(Some(Px(28.0)))
        .padding_top(Some(Px(40.0)))
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        make_line("auto", TextDecorationSkipInk::Auto),
        make_line("none", TextDecorationSkipInk::None),
      ]
      .into(),
    ),
  };

  run_fixture_test(
    container.into(),
    "text_decoration_skip_ink_parapsychologists",
  );
}
