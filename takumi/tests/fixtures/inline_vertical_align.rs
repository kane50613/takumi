use takumi::layout::{
  node::{ContainerNode, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn inline_vertical_align_types() {
  let row = |label: &str, align: VerticalAlign, color: Color| {
    ContainerNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::Block)
          .width(Percentage(48.0))
          .margin(Sides([Px(4.0); 4]))
          .padding(Sides([Px(4.0), Px(8.0), Px(4.0), Px(8.0)]))
          .line_height(LineHeight::Length(Px(72.0)))
          .font_size(Some(Px(32.0)))
          .background_color(ColorInput::Value(Color([248, 248, 248, 255])))
          .border_width(Some(Sides([Px(1.0); 4])))
          .border_style(Some(BorderStyle::Solid))
          .border_color(ColorInput::Value(Color([180, 180, 180, 255])))
          .build()
          .unwrap(),
      ),
      children: Some(
        [
          TextNode {
            class_name: None,
            id: None,
            tag_name: None,
            preset: None,
            tw: None,
            style: Some(
              StyleBuilder::default()
                .display(Display::Inline)
                .text_decoration(TextDecoration {
                  line: TextDecorationLines::UNDERLINE,
                  style: None,
                  color: Some(ColorInput::Value(Color([220, 38, 38, 255]))),
                  thickness: Some(TextDecorationThickness::Length(Px(3.0))),
                })
                .text_decoration_skip_ink(TextDecorationSkipInk::None)
                .build()
                .unwrap(),
            ),
            text: format!("Baseline guide {label} "),
          }
          .into(),
          ContainerNode {
            class_name: None,
            id: None,
            tag_name: None,
            preset: None,
            tw: None,
            style: Some(
              StyleBuilder::default()
                .display(Display::InlineBlock)
                .width(Px(44.0))
                .height(Px(44.0))
                .background_color(ColorInput::Value(color))
                .vertical_align(align)
                .border_width(Some(Sides([Px(2.0); 4])))
                .border_style(Some(BorderStyle::Solid))
                .border_color(ColorInput::Value(Color([30, 30, 30, 255])))
                .build()
                .unwrap(),
            ),
            children: None,
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
                .text_decoration(TextDecoration {
                  line: TextDecorationLines::UNDERLINE,
                  style: None,
                  color: Some(ColorInput::Value(Color([220, 38, 38, 255]))),
                  thickness: Some(TextDecorationThickness::Length(Px(3.0))),
                })
                .text_decoration_skip_ink(TextDecorationSkipInk::None)
                .build()
                .unwrap(),
            ),
            text: " marker".to_string(),
          }
          .into(),
        ]
        .into(),
      ),
    }
    .into()
  };

  let children = [
    row(
      "baseline",
      VerticalAlign::Keyword(VerticalAlignKeyword::Baseline),
      Color([239, 68, 68, 160]),
    ),
    row(
      "top",
      VerticalAlign::Keyword(VerticalAlignKeyword::Top),
      Color([59, 130, 246, 160]),
    ),
    row(
      "middle",
      VerticalAlign::Keyword(VerticalAlignKeyword::Middle),
      Color([16, 185, 129, 160]),
    ),
    row(
      "bottom",
      VerticalAlign::Keyword(VerticalAlignKeyword::Bottom),
      Color([245, 158, 11, 160]),
    ),
    row(
      "text-top",
      VerticalAlign::Keyword(VerticalAlignKeyword::TextTop),
      Color([14, 165, 233, 160]),
    ),
    row(
      "text-bottom",
      VerticalAlign::Keyword(VerticalAlignKeyword::TextBottom),
      Color([168, 85, 247, 160]),
    ),
    row(
      "sub",
      VerticalAlign::Keyword(VerticalAlignKeyword::Sub),
      Color([107, 114, 128, 160]),
    ),
    row(
      "super",
      VerticalAlign::Keyword(VerticalAlignKeyword::Super),
      Color([75, 85, 99, 160]),
    ),
    row(
      "10px",
      VerticalAlign::Length(Px(10.0)),
      Color([236, 72, 153, 160]),
    ),
    row(
      "-8px",
      VerticalAlign::Length(Px(-8.0)),
      Color([244, 63, 94, 160]),
    ),
    row(
      "0.5em",
      VerticalAlign::Length(Em(0.5)),
      Color([34, 197, 94, 160]),
    ),
    row(
      "50%",
      VerticalAlign::Length(Percentage(50.0)),
      Color([251, 146, 60, 160]),
    ),
  ];

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .display(Display::Flex)
        .flex_direction(FlexDirection::Row)
        .flex_wrap(FlexWrap::Wrap)
        .padding(Sides([Px(8.0); 4]))
        .background_color(ColorInput::Value(Color::white()))
        .build()
        .unwrap(),
    ),
    children: Some(children.into()),
  };

  run_fixture_test(container.into(), "inline_vertical_align_types");
}
