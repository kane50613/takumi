use takumi::layout::{
  node::Node,
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn text_inline() {
  let texts = &[
    (
      "The quick brown fox jumps over the lazy dog.",
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    (
      "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ",
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::text_transform(TextTransform::Uppercase))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    (
      "Nothing beats a jet2 holiday! ",
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::color(ColorInput::Value(Color([
          255, 0, 0, 255,
        ]))))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    (
      "I'm making a browser at this point. ",
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_weight(FontWeight::from(600.0)))
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::color(ColorInput::Value(Color([
          0, 0, 255, 255,
        ]))))
        .with(StyleDeclaration::font_style(FontStyle::italic())),
    ),
  ];

  let children: Vec<Node> = texts
    .iter()
    .map(|(text, style)| Node::text(text.to_string()).with_style(style.clone()))
    .collect();

  let container = Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::line_clamp(Some(3.into())))
      .with(StyleDeclaration::text_overflow(TextOverflow::Ellipsis))
      .with(StyleDeclaration::font_size(Px(48.0).into()))
      .with_white_space(WhiteSpace::pre_wrap()),
  );

  run_fixture_test(container, "text_inline");
}

#[test]
fn inline_image() {
  // Inline image should behave as inline-level box content
  let children: Vec<Node> = vec![
    Node::text("Before ".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    Node::image(("assets/images/yeecord.png", 64.0, 64.0)).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Inline))
        .with_border_width(Sides([Px(12.0); 4]))
        .with(StyleDeclaration::border_style(BorderStyle::Solid))
        .with(StyleDeclaration::border_color(ColorInput::Value(
          Color::transparent(),
        )))
        .with(StyleDeclaration::background_image(
          BackgroundImages::from_str("linear-gradient(to right, red, blue)").ok(),
        ))
        .with(StyleDeclaration::background_clip(
          BackgroundClip::BorderArea,
        )),
    ),
    Node::text(" After".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
  ];

  let container = Node::container([Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with_border_width(Sides([Px(2.0); 4]))
      .with(StyleDeclaration::border_style(BorderStyle::Solid))
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::font_size(Px(48.0).into())),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with_white_space(WhiteSpace::pre()),
  );

  run_fixture_test(container, "inline_image");
}

#[test]
fn inline_block_in_inline() {
  // A block-level container inside inline content: should create anonymous block formatting context
  let children: Vec<Node> = vec![
    Node::text("Start ".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    Node::container([Node::text("Block inside inline".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Block)))])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([200, 200, 255, 255]),
        )))
        .with(StyleDeclaration::width(Percentage(80.0)))
        .with(StyleDeclaration::font_size(Px(18.0).into())),
    ),
    Node::text(" End".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
  ];

  let container = Node::container(children.into_boxed_slice()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::font_size(Px(24.0).into()))
      .with_white_space(WhiteSpace::pre()),
  );

  run_fixture_test(container, "inline_block_in_inline");
}

#[test]
fn inline_span_background_color() {
  let texts = &[
    (
      "Hello ",
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 200, 200, 255]),
        )))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    (
      "world ",
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([200, 255, 200, 255]),
        )))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    (
      "from ",
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([200, 200, 255, 255]),
        )))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    (
      "Takumi!",
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 200, 255]),
        )))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
  ];

  let children: Vec<Node> = texts
    .iter()
    .map(|(text, style)| Node::text(text.to_string()).with_style(style.clone()))
    .collect();

  let container = Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with_white_space(WhiteSpace::pre())
      .with(StyleDeclaration::font_size(Px(48.0).into())),
  );

  run_fixture_test(container, "inline_span_background_color");
}

#[test]
fn inline_outline_span_boundaries() {
  let children: Vec<Node> = vec![
    Node::text("STEAM ".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      ,
    Node::text(
        "education can become accessible through a sequence of free and high-quality teaching examples"
          .to_string(),
      )
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Inline))
          .with(StyleDeclaration::outline_width(Px(3.0)))
          .with(StyleDeclaration::outline_style(BorderStyle::Solid))
          .with(StyleDeclaration::outline_color(ColorInput::Value(Color([
            255, 0, 0, 255,
          ])))),
      )
      ,
    Node::text(" for everyone.".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      ,
  ];

  let container = Node::container([Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Px(320.0)))
      .with_padding(Sides([Px(24.0); 4]))
      .with_border_width(Sides([Px(2.0); 4]))
      .with(StyleDeclaration::border_style(BorderStyle::Solid))
      .with(StyleDeclaration::font_size(Px(28.0).into()))
      .with(StyleDeclaration::line_height(Px(34.0).into()))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      ))),
  );

  run_fixture_test(container, "inline_outline_span_boundaries");
}

#[test]
fn inline_atomic_containers() {
  let atomic = |display, color, label: &str| -> Node {
    Node::container([Node::text(label.to_string())]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(display))
        .with_padding(Sides([Px(8.0); 4]))
        .with(StyleDeclaration::background_color(ColorInput::Value(color)))
        .with_border_width(Sides([Px(2.0); 4]))
        .with(StyleDeclaration::border_style(BorderStyle::Solid)),
    )
  };

  let container = Node::container([Node::container([
    Node::text("before ".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    atomic(
      Display::InlineBlock,
      Color([255, 0, 0, 100]),
      "inline-block",
    ),
    Node::text(" mid ".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    atomic(Display::InlineFlex, Color([0, 255, 0, 100]), "inline-flex"),
    Node::text(" end ".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    atomic(Display::InlineGrid, Color([0, 0, 255, 100]), "inline-grid"),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::font_size(Px(24.0).into()))
      .with_border_width(Sides([Px(2.0); 4]))
      .with(StyleDeclaration::border_style(BorderStyle::Solid)),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with_white_space(WhiteSpace::pre()),
  );

  run_fixture_test(container, "inline_atomic_containers");
}
#[test]
fn inline_nested_flex_block() {
  let inline_flex_children: Vec<Node> = vec![
    Node::text("Flex Start ".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    Node::container([Node::text("Inner".to_string())]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::InlineBlock))
        .with_padding(Sides([Px(4.0); 4]))
        .with_margin(Sides([Px(0.0), Px(10.0), Px(0.0), Px(10.0)]))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 200, 200, 255]),
        ))),
    ),
    Node::text(" Flex End".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
  ];

  let children: Vec<Node> = vec![
    Node::text(
        "This is some preceding text that is long enough to wrap eventually. ".to_string(),
      )
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      ,
    Node::container(inline_flex_children)
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::InlineFlex))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([200, 255, 200, 255]),
          )))
          .with_padding(Sides([Px(5.0); 4]))
          .with(StyleDeclaration::align_items(AlignItems::Center))
          .with(StyleDeclaration::vertical_align(VerticalAlign::Keyword(
            VerticalAlignKeyword::Middle,
          ))),
      )
      ,
    Node::text(
        " followed by more text that should definitely wrap and show how the inline-flex container behaves when it is part of a wrapped line. We want to make sure the nested boxes are drawn in the correct positions even after wrapping.".to_string(),
      )
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      ,
  ];

  let container = Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(800.0)))
      .with(StyleDeclaration::display(Display::Block))
      .with_padding(Sides([Px(20.0); 4]))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with(StyleDeclaration::font_size(Px(20.0).into()))
      .with(StyleDeclaration::line_height(LineHeight::Length(Px(40.0)))),
  );

  run_fixture_test(container, "inline_nested_flex_block");
}

#[test]
fn inline_complex_nested_fixture() {
  let metadata_children: Vec<Node> = vec![
    Node::text("Metadata: ".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
        .with(StyleDeclaration::color(ColorInput::Value(Color([
          16, 42, 67, 255,
        ]))))
        .with(StyleDeclaration::text_transform(TextTransform::Uppercase))
        .with(StyleDeclaration::font_size(Px(12.0).into())),
    ),
    Node::container([Node::text("Tag".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::color(ColorInput::Value(Color::white())))
        .with(StyleDeclaration::font_size(Px(10.0).into()))
        .with(StyleDeclaration::font_weight(FontWeight::from(600.0))),
    )])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::InlineFlex))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with_gap(SpacePair::from_single(Px(4.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([188, 204, 220, 255]),
        )))
        .with_border_radius(Box::new(BorderRadius(Sides(
          [SpacePair::from_single(Px(999.0)); 4],
        ))))
        .with_padding(Sides([Px(2.0), Px(8.0), Px(2.0), Px(8.0)]))
        .with(StyleDeclaration::vertical_align(VerticalAlign::Keyword(
          VerticalAlignKeyword::Baseline,
        ))),
    ),
  ];

  let children: Vec<Node> = vec![
    Node::text("Start with some basic inline text. ".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      ,
    Node::container(metadata_children)
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::InlineFlex))
          .with(StyleDeclaration::vertical_align(VerticalAlign::Keyword(
            VerticalAlignKeyword::Middle,
          )))
          .with(StyleDeclaration::background_color(ColorInput::Value(Color([
            240, 244, 248, 255,
          ]))))
          .with_border_width(Sides([Px(1.0); 4]))
          .with(StyleDeclaration::border_style(BorderStyle::Solid))
          .with(StyleDeclaration::border_color(ColorInput::Value(Color([
            217, 226, 236, 255,
          ]))))
          .with_border_radius(Box::new(BorderRadius(Sides(
            [SpacePair::from_single(Px(4.0)); 4],
          ))))
          .with_padding(Sides([Px(8.0), Px(12.0), Px(8.0), Px(12.0)]))
          .with_margin(Sides([Px(0.0), Px(8.0), Px(0.0), Px(8.0)])),
      )
      ,
    Node::text(
        "Followed by a longer sentence that demonstrates how text wraps around inline-block elements. ".to_string(),
      )
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      ,
    Node::container([Node::text("A fixed-width block that sits on the bottom of the line box.".to_string())
        .with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Block))
            .with(StyleDeclaration::font_size(Px(12.0).into()))
            .with(StyleDeclaration::line_height(LineHeight::Length(Em(1.2)))),
        )])
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::InlineBlock))
          .with(StyleDeclaration::vertical_align(VerticalAlign::Keyword(
            VerticalAlignKeyword::Bottom,
          )))
          .with(StyleDeclaration::width(Px(120.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(Color([
            255, 238, 219, 255,
          ]))))
          .with_border_width(Sides([Px(1.0); 4]))
          .with(StyleDeclaration::border_style(BorderStyle::Solid))
          .with(StyleDeclaration::border_color(ColorInput::Value(Color([
            255, 156, 56, 255,
          ]))))
          .with_padding(Sides([Px(10.0); 4]))
          .with_margin(Sides([Px(0.0), Px(5.0), Px(0.0), Px(5.0)])),
      )
      ,
    Node::text(" And finally some more text to close things out.".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      ,
  ];

  let node = Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::font_size(Px(16.0).into()))
      .with(StyleDeclaration::line_height(LineHeight::Length(Em(1.5))))
      .with(StyleDeclaration::color(ColorInput::Value(Color([
        51, 51, 51, 255,
      ]))))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with(StyleDeclaration::width(Px(600.0)))
      .with_padding(Sides([Px(20.0); 4])),
  );

  run_fixture_test(node, "inline_complex_nested_fixture");
}

#[test]
fn inline_text_decorations() {
  let decorated_children: Vec<Node> = vec![
    Node::text("Hello World".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Inline))
        .with_text_decoration(
          TextDecoration::builder()
            .line(TextDecorationLines::UNDERLINE | TextDecorationLines::LINE_THROUGH)
            .color(ColorInput::Value(Color([0, 0, 255, 255])))
            .thickness(TextDecorationThickness::Length(Px(4.0)))
            .build(),
        ),
    ),
    Node::text("Woah".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::InlineBlock))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 0, 0, 128]),
        )))
        .with(StyleDeclaration::vertical_align(VerticalAlign::Keyword(
          VerticalAlignKeyword::TextBottom,
        ))),
    ),
    Node::container([
      Node::text("It works right".to_string()).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::InlineBlock))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([255, 255, 0, 255]),
          ))),
      ),
      Node::text("A flexbox!".to_string()).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::InlineFlex))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([0, 128, 0, 255]),
          ))),
      ),
    ])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::InlineBlock))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 0, 255, 128]),
        )))
        .with(StyleDeclaration::font_style(FontStyle::italic()))
        .with(StyleDeclaration::vertical_align(VerticalAlign::Keyword(
          VerticalAlignKeyword::Middle,
        )))
        .with_padding(Sides([Px(10.0); 4])),
    ),
    Node::text(" Red Underline".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::color(ColorInput::Value(Color([
          255, 0, 0, 255,
        ]))))
        .with_text_decoration(
          TextDecoration::builder()
            .line(TextDecorationLines::UNDERLINE)
            .build(),
        ),
    ),
  ];

  let node = Node::container(decorated_children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with_padding(Sides([Px(40.0); 4]))
      .with(StyleDeclaration::font_size(Px(48.0).into())),
  );

  run_fixture_test(node, "inline_text_decorations");
}
