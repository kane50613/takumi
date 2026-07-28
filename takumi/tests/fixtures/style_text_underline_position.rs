use takumi::prelude::{Length::*, *};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_text_underline_position() {
  let make_line = |label: &str, position: TextUnderlinePosition| -> Node {
    Node::text(format!("{label}: pgjy underline")).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::text_align(TextAlign::Center))
        .with(StyleDeclaration::font_size(Px(48.0).into()))
        .with(StyleDeclaration::text_underline_position(position))
        .with_text_decoration(
          TextDecoration::builder()
            .line(TextDecorationLines::UNDERLINE)
            .color(ColorInput::Value(Color([255, 0, 0, 255])))
            .build(),
        ),
    )
  };

  let container = Node::container([
    make_line("auto", TextUnderlinePosition::Auto),
    make_line("from-font", TextUnderlinePosition::FromFont),
    make_line("under", TextUnderlinePosition::Under),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::row_gap(Px(20.0).into()))
      .with(StyleDeclaration::padding_top(Px(40.0)))
      .with(StyleDeclaration::padding_bottom(Px(40.0))),
  );

  run_fixture_test(container, "style_text_underline_position");
}
