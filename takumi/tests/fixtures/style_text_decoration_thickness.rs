use takumi::layout::{
  node::{ContainerNode, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_text_decoration_thickness() {
  let make_line = |label: &str, thickness: TextDecorationThickness| {
    TextNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .width(Percentage(100.0))
          .display(Display::Block)
          .text_align(TextAlign::Center)
          .font_size(Some(Px(48.0)))
          .text_decoration(TextDecoration {
            line: TextDecorationLines::UNDERLINE,
            style: None,
            color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
            thickness: Some(thickness),
          })
          .build()
          .unwrap(),
      ),
      text: format!("{label}: thickness parapsychologists"),
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
      StyleBuilder::default()
        .width(Percentage(100.0))
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .display(Display::Flex)
        .flex_direction(FlexDirection::Column)
        .row_gap(Some(Px(20.0)))
        .padding_top(Some(Px(40.0)))
        .padding_bottom(Some(Px(40.0)))
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        make_line("auto (48/18=2.66px)", TextDecorationThickness::Length(Auto)),
        make_line("from-font", TextDecorationThickness::FromFont),
        make_line("2px", TextDecorationThickness::Length(Px(2.0))),
        make_line("5px", TextDecorationThickness::Length(Px(5.0))),
        make_line("10px", TextDecorationThickness::Length(Px(10.0))),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_text_decoration_thickness");
}
