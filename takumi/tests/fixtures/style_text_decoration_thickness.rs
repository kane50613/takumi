use takumi::layout::{
  node::{ContainerNode, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_text_decoration_thickness() {
  let make_line = |label: &str, thickness: TextDecorationThickness| -> TextNode {
    TextNode::default()
      .with_style(
        Style::default()
          .with(StyleDeclaration::width(Percentage(100.0)))
          .with(StyleDeclaration::display(Display::Block))
          .with(StyleDeclaration::text_align(TextAlign::Center))
          .with(StyleDeclaration::font_size(Px(48.0).into()))
          .with_text_decoration(TextDecoration {
            line: TextDecorationLines::UNDERLINE,
            style: None,
            color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
            thickness: Some(thickness),
          }),
      )
      .with_text(format!("{label}: thickness parapsychologists"))
  };

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::row_gap(Px(20.0)))
        .with(StyleDeclaration::padding_top(Px(40.0)))
        .with(StyleDeclaration::padding_bottom(Px(40.0))),
    )
    .with_children([
      make_line("auto (48/18=2.66px)", TextDecorationThickness::Length(Auto)),
      make_line("from-font", TextDecorationThickness::FromFont),
      make_line("2px", TextDecorationThickness::Length(Px(2.0))),
      make_line("5px", TextDecorationThickness::Length(Px(5.0))),
      make_line("10px", TextDecorationThickness::Length(Px(10.0))),
    ]);

  run_fixture_test(container.into(), "style_text_decoration_thickness");
}
