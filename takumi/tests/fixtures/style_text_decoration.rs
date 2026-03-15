use takumi::layout::{
  node::Node,
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_text_decoration() {
  let text = Node::text("Text Decoration with Underline, Line-Through, and Overline".to_string())
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::text_align(TextAlign::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::font_size(Px(72.0).into()))
        .with_text_decoration(
          TextDecoration::builder()
            .line(TextDecorationLines::all())
            .color(ColorInput::Value(Color([255, 0, 0, 255])))
            .build(),
        ),
    );

  run_fixture_test(text, "style_text_decoration");
}

#[test]
fn text_decoration_skip_ink_parapsychologists() {
  let make_line = |label: &str, skip_ink: TextDecorationSkipInk| -> Node {
    Node::text(format!("{label}: parapsychologists")).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::text_align(TextAlign::Center))
        .with(StyleDeclaration::font_size(Px(96.0).into()))
        .with_text_decoration(
          TextDecoration::builder()
            .line(TextDecorationLines::UNDERLINE)
            .color(ColorInput::Value(Color([255, 0, 0, 255])))
            .build(),
        )
        .with(StyleDeclaration::text_decoration_skip_ink(skip_ink)),
    )
  };

  let container = Node::container([
    make_line("auto", TextDecorationSkipInk::Auto),
    make_line("none", TextDecorationSkipInk::None),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::row_gap(Px(28.0)))
      .with(StyleDeclaration::padding_top(Px(40.0))),
  );

  run_fixture_test(container, "text_decoration_skip_ink_parapsychologists");
}
