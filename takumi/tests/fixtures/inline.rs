use takumi::layout::{
  node::{ContainerNode, ImageNode, NodeKind, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn text_inline() {
  let texts = &[
    (
      "The quick brown fox jumps over the lazy dog.",
      Style::default().with(StyleDeclaration::display(Display::Inline)),
    ),
    (
      "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ",
      Style::default()
        .with(StyleDeclaration::text_transform(TextTransform::Uppercase))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    (
      "Nothing beats a jet2 holiday! ",
      Style::default()
        .with(StyleDeclaration::color(ColorInput::Value(Color([
          255, 0, 0, 255,
        ]))))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    (
      "I'm making a browser at this point. ",
      Style::default()
        .with(StyleDeclaration::font_weight(FontWeight::from(600.0)))
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::color(ColorInput::Value(Color([
          0, 0, 255, 255,
        ]))))
        .with(StyleDeclaration::font_style(FontStyle::italic())),
    ),
  ];

  let children: Vec<NodeKind> = texts
    .iter()
    .map(|(text, style)| {
      TextNode::default()
        .with_style(style.clone())
        .with_text(text.to_string())
        .into()
    })
    .collect();

  let container = ContainerNode::default()
    .with_style(
      Style::default()
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
    )
    .with_children(children);

  run_fixture_test(container.into(), "text_inline");
}

#[test]
fn inline_image() {
  // Inline image should behave as inline-level box content
  let children: Vec<NodeKind> = vec![
    TextNode::default()
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      .with_text("Before ".to_string())
      .into(),
    ImageNode::default()
      .with_style(
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
      )
      .with_src("assets/images/yeecord.png")
      .with_width(64.0)
      .with_height(64.0)
      .into(),
    TextNode::default()
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      .with_text(" After".to_string())
      .into(),
  ];

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with_white_space(WhiteSpace::pre()),
    )
    .with_children([ContainerNode::default()
      .with_style(
        Style::default()
          .with_border_width(Sides([Px(2.0); 4]))
          .with(StyleDeclaration::border_style(BorderStyle::Solid))
          .with(StyleDeclaration::display(Display::Block))
          .with(StyleDeclaration::font_size(Px(48.0).into())),
      )
      .with_children(children)]);

  run_fixture_test(container.into(), "inline_image");
}

#[test]
fn inline_block_in_inline() {
  // A block-level container inside inline content: should create anonymous block formatting context
  let children: Vec<NodeKind> = vec![
    TextNode::default()
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      .with_text("Start ".to_string())
      .into(),
    ContainerNode::default()
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Block))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([200, 200, 255, 255]),
          )))
          .with(StyleDeclaration::width(Percentage(80.0)))
          .with(StyleDeclaration::font_size(Px(18.0).into())),
      )
      .with_children([TextNode::default()
        .with_style(Style::default().with(StyleDeclaration::display(Display::Block)))
        .with_text("Block inside inline".to_string())])
      .into(),
    TextNode::default()
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      .with_text(" End".to_string())
      .into(),
  ];

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::font_size(Px(24.0).into()))
        .with_white_space(WhiteSpace::pre()),
    )
    .with_children(children.into_boxed_slice());

  run_fixture_test(container.into(), "inline_block_in_inline");
}

#[test]
fn inline_span_background_color() {
  let texts = &[
    (
      "Hello ",
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 200, 200, 255]),
        )))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    (
      "world ",
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([200, 255, 200, 255]),
        )))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    (
      "from ",
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([200, 200, 255, 255]),
        )))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    (
      "Takumi!",
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 200, 255]),
        )))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
  ];

  let children: Vec<NodeKind> = texts
    .iter()
    .map(|(text, style)| {
      TextNode::default()
        .with_style(style.clone())
        .with_text(text.to_string())
        .into()
    })
    .collect();

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with_white_space(WhiteSpace::pre())
        .with(StyleDeclaration::font_size(Px(48.0).into())),
    )
    .with_children(children);

  run_fixture_test(container.into(), "inline_span_background_color");
}

#[test]
fn inline_outline_span_boundaries() {
  let children: Vec<NodeKind> = vec![
    TextNode::default()
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      .with_text("STEAM ".to_string())
      .into(),
    TextNode::default()
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Inline))
          .with(StyleDeclaration::outline_width(Px(3.0)))
          .with(StyleDeclaration::outline_style(BorderStyle::Solid))
          .with(StyleDeclaration::outline_color(ColorInput::Value(Color([
            255, 0, 0, 255,
          ])))),
      )
      .with_text(
        "education can become accessible through a sequence of free and high-quality teaching examples"
          .to_string(),
      )
      .into(),
    TextNode::default()
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      .with_text(" for everyone.".to_string())
      .into(),
  ];

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        ))),
    )
    .with_children([ContainerNode::default()
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Block))
          .with(StyleDeclaration::width(Px(320.0)))
          .with_padding(Sides([Px(24.0); 4]))
          .with_border_width(Sides([2.0.into(); 4]))
          .with(StyleDeclaration::border_style(BorderStyle::Solid))
          .with(StyleDeclaration::font_size(Px(28.0).into()))
          .with(StyleDeclaration::line_height(Px(34.0).into()))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color::white(),
          ))),
      )
      .with_children(children)]);

  run_fixture_test(container.into(), "inline_outline_span_boundaries");
}

#[test]
fn inline_atomic_containers() {
  let atomic = |display, color, label: &str| -> NodeKind {
    ContainerNode::default()
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(display))
          .with_padding(Sides([Px(8.0); 4]))
          .with(StyleDeclaration::background_color(ColorInput::Value(color)))
          .with_border_width(Sides([Px(2.0); 4]))
          .with(StyleDeclaration::border_style(BorderStyle::Solid)),
      )
      .with_children([TextNode::default().with_text(label.to_string())])
      .into()
  };

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with_white_space(WhiteSpace::pre()),
    )
    .with_children([ContainerNode::default()
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Block))
          .with(StyleDeclaration::font_size(Px(24.0).into()))
          .with_border_width(Sides([Px(2.0); 4]))
          .with(StyleDeclaration::border_style(BorderStyle::Solid)),
      )
      .with_children([
        TextNode::default()
          .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
          .with_text("before ".to_string())
          .into(),
        atomic(
          Display::InlineBlock,
          Color([255, 0, 0, 100]),
          "inline-block",
        ),
        TextNode::default()
          .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
          .with_text(" mid ".to_string())
          .into(),
        atomic(Display::InlineFlex, Color([0, 255, 0, 100]), "inline-flex"),
        TextNode::default()
          .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
          .with_text(" end ".to_string())
          .into(),
        atomic(Display::InlineGrid, Color([0, 0, 255, 100]), "inline-grid"),
      ])]);

  run_fixture_test(container.into(), "inline_atomic_containers");
}
#[test]
fn inline_nested_flex_block() {
  let inline_flex_children: Vec<NodeKind> = vec![
    TextNode::default()
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      .with_text("Flex Start ".to_string())
      .into(),
    ContainerNode::default()
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::InlineBlock))
          .with_padding(Sides([Px(4.0); 4]))
          .with_margin(Sides([Px(0.0), Px(10.0), Px(0.0), Px(10.0)]))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([255, 200, 200, 255]),
          ))),
      )
      .with_children([TextNode::default().with_text("Inner".to_string())])
      .into(),
    TextNode::default()
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      .with_text(" Flex End".to_string())
      .into(),
  ];

  let children: Vec<NodeKind> = vec![
    TextNode::default()
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      .with_text(
        "This is some preceding text that is long enough to wrap eventually. ".to_string(),
      )
      .into(),
    ContainerNode::default()
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
      .with_children(inline_flex_children)
      .into(),
    TextNode::default()
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      .with_text(
        " followed by more text that should definitely wrap and show how the inline-flex container behaves when it is part of a wrapped line. We want to make sure the nested boxes are drawn in the correct positions even after wrapping.".to_string(),
      )
      .into(),
  ];

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Px(800.0)))
        .with(StyleDeclaration::display(Display::Block))
        .with_padding(Sides([Px(20.0); 4]))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with(StyleDeclaration::font_size(Px(20.0).into()))
        .with(StyleDeclaration::line_height(LineHeight::Length(Px(40.0)))),
    )
    .with_children(children);

  run_fixture_test(container.into(), "inline_nested_flex_block");
}

#[test]
fn inline_complex_nested_fixture() {
  let metadata_children: Vec<NodeKind> = vec![
    TextNode::default()
      .with_text("Metadata: ".to_string())
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Inline))
          .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
          .with(StyleDeclaration::color(ColorInput::Value(Color([
            16, 42, 67, 255,
          ]))))
          .with(StyleDeclaration::text_transform(TextTransform::Uppercase))
          .with(StyleDeclaration::font_size(Px(12.0).into())),
      )
      .into(),
    ContainerNode::default()
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
      )
      .with_children(
        [TextNode::default().with_text("Tag".to_string()).with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Inline))
            .with(StyleDeclaration::color(ColorInput::Value(Color::white())))
            .with(StyleDeclaration::font_size(Px(10.0).into()))
            .with(StyleDeclaration::font_weight(FontWeight::from(600.0))),
        )],
      )
      .into(),
  ];

  let children: Vec<NodeKind> = vec![
    TextNode::default()
      .with_text("Start with some basic inline text. ".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      .into(),
    ContainerNode::default()
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
      .with_children(metadata_children)
      .into(),
    TextNode::default()
      .with_text(
        "Followed by a longer sentence that demonstrates how text wraps around inline-block elements. ".to_string(),
      )
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      .into(),
    ContainerNode::default()
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
      .with_children([TextNode::default()
        .with_text("A fixed-width block that sits on the bottom of the line box.".to_string())
        .with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Block))
            .with(StyleDeclaration::font_size(Px(12.0).into()))
            .with(StyleDeclaration::line_height(LineHeight::Length(Em(1.2)))),
        )])
      .into(),
    TextNode::default()
      .with_text(" And finally some more text to close things out.".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      .into(),
  ];

  let node = ContainerNode::default()
    .with_style(
      Style::default()
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
    )
    .with_children(children)
    .into();

  run_fixture_test(node, "inline_complex_nested_fixture");
}

#[test]
fn inline_text_decorations() {
  let decorated_children: Vec<NodeKind> = vec![
    TextNode::default()
      .with_text("Hello World".to_string())
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Inline))
          .with_text_decoration(TextDecoration {
            line: TextDecorationLines::UNDERLINE | TextDecorationLines::LINE_THROUGH,
            style: None,
            color: Some(ColorInput::Value(Color([0, 0, 255, 255]))),
            thickness: Some(TextDecorationThickness::Length(Px(4.0))),
          }),
      )
      .into(),
    TextNode::default()
      .with_text("Woah".to_string())
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::InlineBlock))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([255, 0, 0, 128]),
          )))
          .with(StyleDeclaration::vertical_align(VerticalAlign::Keyword(
            VerticalAlignKeyword::TextBottom,
          ))),
      )
      .into(),
    ContainerNode::default()
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
      )
      .with_children([
        TextNode::default()
          .with_text("It works right".to_string())
          .with_style(
            Style::default()
              .with(StyleDeclaration::display(Display::InlineBlock))
              .with(StyleDeclaration::background_color(ColorInput::Value(
                Color([255, 255, 0, 255]),
              ))),
          ),
        TextNode::default()
          .with_text("A flexbox!".to_string())
          .with_style(
            Style::default()
              .with(StyleDeclaration::display(Display::InlineFlex))
              .with(StyleDeclaration::background_color(ColorInput::Value(
                Color([0, 128, 0, 255]),
              ))),
          ),
      ])
      .into(),
    TextNode::default()
      .with_text(" Red Underline".to_string())
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Inline))
          .with(StyleDeclaration::color(ColorInput::Value(Color([
            255, 0, 0, 255,
          ]))))
          .with_text_decoration(TextDecoration {
            line: TextDecorationLines::UNDERLINE,
            style: None,
            color: None,
            thickness: None,
          }),
      )
      .into(),
  ];

  let node = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with_padding(Sides([Px(40.0); 4]))
        .with(StyleDeclaration::font_size(Px(48.0).into())),
    )
    .with_children(decorated_children)
    .into();

  run_fixture_test(node, "inline_text_decorations");
}
