use takumi::core::layout::{
  node::Node,
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn inline_top_bottom_box_line_box_height() {
  // A `top`/`bottom` box taller than line-height grows the line box to the box
  // height, attaching to the line edge without adding text leading on the far
  // side. https://www.w3.org/TR/CSS22/visudet.html#line-height
  let tall = |align: VerticalAlign, color: Color| {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::InlineBlock))
        .with(StyleDeclaration::vertical_align(align))
        .with(StyleDeclaration::width(Px(24.0)))
        .with(StyleDeclaration::height(Px(80.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(color))),
    )
  };
  let line = |align: VerticalAlign, color: Color| {
    Node::container([
      Node::text("Hxgp ".to_string()),
      tall(align, color),
      Node::text(" Hxgp".to_string()),
    ])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::font_size(Px(28.0).into()))
        .with(StyleDeclaration::line_height(LineHeight::Length(Px(32.0))))
        .with_margin(Sides([Px(0.0), Px(0.0), Px(10.0), Px(0.0)]))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([241, 245, 249, 255]),
        ))),
    )
  };

  let container = Node::container([
    line(
      VerticalAlign::Keyword(VerticalAlignKeyword::Top),
      Color([253, 186, 116, 255]),
    ),
    line(
      VerticalAlign::Keyword(VerticalAlignKeyword::Bottom),
      Color([147, 197, 253, 255]),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with_padding(Sides([Px(20.0); 4]))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(container, "inline_top_bottom_box_line_box_height");
}

#[test]
fn inline_empty_atomic_baseline() {
  // An empty atomic inline box (no in-flow content) aligns its bottom margin
  // edge to the baseline instead of hanging below it; a box WITH content keeps
  // its own content baseline. https://www.w3.org/TR/CSS22/visudet.html#leading
  let empty = |display: Display, color: Color| {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(display))
        .with(StyleDeclaration::width(Px(40.0)))
        .with(StyleDeclaration::height(Px(56.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(color)))
        .with_border_width(Sides([Px(2.0); 4]))
        .with_border_style(Sides([BorderStyle::Solid; 4]))
        .with_border_color(Sides([ColorInput::Value(Color([30, 41, 59, 255])); 4])),
    )
  };

  // Guard against over-correction: a content-bearing pill must align by its
  // content baseline, not be pulled down to its bottom edge.
  let filled = Node::container([Node::text("Hi".to_string())]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::InlineBlock))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([253, 224, 71, 255]),
      )))
      .with_padding(Sides([Px(6.0), Px(14.0), Px(6.0), Px(14.0)]))
      .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(9999.0)); 4]))),
  );

  let space = || Node::text(" ".to_string());
  let line = Node::container([
    Node::text("Ag ".to_string()),
    empty(Display::InlineBlock, Color([252, 165, 165, 255])),
    space(),
    empty(Display::InlineFlex, Color([134, 239, 172, 255])),
    space(),
    empty(Display::InlineGrid, Color([147, 197, 253, 255])),
    space(),
    filled,
    Node::text(" Ag".to_string()),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::font_size(Px(40.0).into()))
      .with(StyleDeclaration::line_height(LineHeight::Length(Px(84.0)))),
  );

  let container = Node::container([line]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with_padding(Sides([Px(24.0); 4]))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(container, "inline_empty_atomic_baseline");
}

#[test]
fn inline_vertical_align_types() {
  let row = |label: &str, align: VerticalAlign, color: Color| {
    Node::container([
      Node::text(format!("Baseline guide {label} ")).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::display(Display::Inline))
          .with_text_decoration(
            TextDecoration::builder()
              .line(TextDecorationLines::UNDERLINE)
              .color(ColorInput::Value(Color([220, 38, 38, 255])))
              .thickness(TextDecorationThickness::Length(Px(3.0)))
              .build(),
          )
          .with(StyleDeclaration::text_decoration_skip_ink(
            TextDecorationSkipInk::None,
          )),
      ),
      Node::container([]).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::display(Display::InlineBlock))
          .with(StyleDeclaration::width(Px(44.0)))
          .with(StyleDeclaration::height(Px(44.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(color)))
          .with(StyleDeclaration::vertical_align(align))
          .with_border_width(Sides([Px(2.0); 4]))
          .with_border_style(Sides([BorderStyle::Solid; 4]))
          .with_border_color(Sides([ColorInput::Value(Color([30, 30, 30, 255])); 4])),
      ),
      Node::text(" marker".to_string()).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::display(Display::Inline))
          .with_text_decoration(
            TextDecoration::builder()
              .line(TextDecorationLines::UNDERLINE)
              .color(ColorInput::Value(Color([220, 38, 38, 255])))
              .thickness(TextDecorationThickness::Length(Px(3.0)))
              .build(),
          )
          .with(StyleDeclaration::text_decoration_skip_ink(
            TextDecorationSkipInk::None,
          )),
      ),
    ])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::width(Percentage(48.0)))
        .with_margin(Sides([Px(4.0); 4]))
        .with_padding(Sides([Px(4.0), Px(8.0), Px(4.0), Px(8.0)]))
        .with(StyleDeclaration::line_height(LineHeight::Length(Px(72.0))))
        .with(StyleDeclaration::font_size(Px(32.0).into()))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([248, 248, 248, 255]),
        )))
        .with_border_width(Sides([Px(1.0); 4]))
        .with_border_style(Sides([BorderStyle::Solid; 4]))
        .with_border_color(Sides([ColorInput::Value(Color([180, 180, 180, 255])); 4])),
    )
  };

  let children: Vec<Node> = vec![
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

  let container = Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Row))
      .with(StyleDeclaration::flex_wrap(FlexWrap::Wrap))
      .with_padding(Sides([Px(8.0); 4]))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(container, "inline_vertical_align_types");
}
