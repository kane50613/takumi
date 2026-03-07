use takumi::layout::{
  node::{ContainerNode, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_text_decoration() {
  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::text_align(TextAlign::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::font_size(Px(72.0).into()))
        .with_text_decoration(TextDecoration {
          line: TextDecorationLines::all(),
          style: None,
          color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
          thickness: None,
        }),
    ),
    text: "Text Decoration with Underline, Line-Through, and Overline".to_string(),
  };

  run_fixture_test(text.into(), "style_text_decoration");
}

#[test]
fn text_decoration_skip_ink_parapsychologists() {
  let make_line = |label: &str, skip_ink: TextDecorationSkipInk| {
    TextNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(
        Style::default()
          .with(StyleDeclaration::width(Percentage(100.0)))
          .with(StyleDeclaration::display(Display::Block))
          .with(StyleDeclaration::text_align(TextAlign::Center))
          .with(StyleDeclaration::font_size(Px(96.0).into()))
          .with_text_decoration(TextDecoration {
            line: TextDecorationLines::UNDERLINE,
            style: None,
            color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
            thickness: None,
          })
          .with(StyleDeclaration::text_decoration_skip_ink(skip_ink)),
      ),
      text: format!("{label}: parapsychologists"),
    }
    .into()
  };

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::row_gap(Px(28.0)))
        .with(StyleDeclaration::padding_top(Px(40.0))),
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
