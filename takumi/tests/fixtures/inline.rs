use serde_json::{from_value, json};
use takumi::layout::{
  node::{ContainerNode, ImageNode, TextNode},
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

  let children = Box::from_iter(texts.iter().map(|(text, style)| {
    TextNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(style.clone()),
      text: text.to_string(),
    }
    .into()
  }));

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
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
    ),
    children: Some(children),
  };

  run_fixture_test(container.into(), "text_inline");
}

#[test]
fn inline_image() {
  // Inline image should behave as inline-level box content
  let children = [
    TextNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(Style::default().with(StyleDeclaration::display(Display::Inline))),
      text: "Before ".to_string(),
    }
    .into(),
    ImageNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(
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
      src: "assets/images/yeecord.png".into(),
      width: Some(64.0),
      height: Some(64.0),
    }
    .into(),
    TextNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(Style::default().with(StyleDeclaration::display(Display::Inline))),
      text: " After".to_string(),
    }
    .into(),
  ];

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with_white_space(WhiteSpace::pre()),
    ),
    children: Some(
      [ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with_border_width(Sides([Px(2.0); 4]))
            .with(StyleDeclaration::border_style(BorderStyle::Solid))
            .with(StyleDeclaration::display(Display::Block))
            .with(StyleDeclaration::font_size(Px(48.0).into())),
        ),
        children: Some(children.into()),
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "inline_image");
}

#[test]
fn inline_block_in_inline() {
  // A block-level container inside inline content: should create anonymous block formatting context
  let children = vec![
    TextNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(Style::default().with(StyleDeclaration::display(Display::Inline))),
      text: "Start ".to_string(),
    }
    .into(),
    ContainerNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(
        Style::default()
          .with(StyleDeclaration::display(Display::Block))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([200, 200, 255, 255]),
          )))
          .with(StyleDeclaration::width(Percentage(80.0)))
          .with(StyleDeclaration::font_size(Px(18.0).into())),
      ),
      children: Some(
        [TextNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(Style::default().with(StyleDeclaration::display(Display::Block))),
          text: "Block inside inline".to_string(),
        }
        .into()]
        .into(),
      ),
    }
    .into(),
    TextNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(Style::default().with(StyleDeclaration::display(Display::Inline))),
      text: " End".to_string(),
    }
    .into(),
  ];

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::font_size(Px(24.0).into()))
        .with_white_space(WhiteSpace::pre()),
    ),
    children: Some(children.into_boxed_slice()),
  };

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

  let children = Box::from_iter(texts.iter().map(|(text, style)| {
    TextNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(style.clone()),
      text: text.to_string(),
    }
    .into()
  }));

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
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
    ),
    children: Some(children),
  };

  run_fixture_test(container.into(), "inline_span_background_color");
}

#[test]
fn inline_atomic_containers() {
  let atomic = |display, color, label: &str| {
    ContainerNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(
        Style::default()
          .with(StyleDeclaration::display(display))
          .with_padding(Sides([Px(8.0); 4]))
          .with(StyleDeclaration::background_color(ColorInput::Value(color)))
          .with_border_width(Sides([Px(2.0); 4]))
          .with(StyleDeclaration::border_style(BorderStyle::Solid)),
      ),
      children: Some(
        [TextNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: None,
          text: label.to_string(),
        }
        .into()]
        .into(),
      ),
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
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with_white_space(WhiteSpace::pre()),
    ),
    children: Some(
      [ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with(StyleDeclaration::display(Display::Block))
            .with(StyleDeclaration::font_size(Px(24.0).into()))
            .with_border_width(Sides([Px(2.0); 4]))
            .with(StyleDeclaration::border_style(BorderStyle::Solid)),
        ),
        children: Some(
          [
            TextNode {
              class_name: None,
              id: None,
              tag_name: None,
              preset: None,
              tw: None,
              style: Some(Style::default().with(StyleDeclaration::display(Display::Inline))),
              text: "before ".to_string(),
            }
            .into(),
            atomic(
              Display::InlineBlock,
              Color([255, 0, 0, 100]),
              "inline-block",
            ),
            TextNode {
              class_name: None,
              id: None,
              tag_name: None,
              preset: None,
              tw: None,
              style: Some(Style::default().with(StyleDeclaration::display(Display::Inline))),
              text: " mid ".to_string(),
            }
            .into(),
            atomic(Display::InlineFlex, Color([0, 255, 0, 100]), "inline-flex"),
            TextNode {
              class_name: None,
              id: None,
              tag_name: None,
              preset: None,
              tw: None,
              style: Some(Style::default().with(StyleDeclaration::display(Display::Inline))),
              text: " end ".to_string(),
            }
            .into(),
            atomic(Display::InlineGrid, Color([0, 0, 255, 100]), "inline-grid"),
          ]
          .into(),
        ),
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "inline_atomic_containers");
}
#[test]
fn inline_nested_flex_block() {
  let children = [
    TextNode {
        class_name: None,
        id: None,
        tag_name: None,
      preset: None,
      tw: None,
      style: Some(
        Style::default()
          .with(StyleDeclaration::display(Display::Inline)),
      ),
      text: "This is some preceding text that is long enough to wrap eventually. ".to_string(),
    }
    .into(),
    ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
      preset: None,
      tw: None,
      style: Some(
        Style::default()
          .with(StyleDeclaration::display(Display::InlineFlex))
          .with(StyleDeclaration::background_color(ColorInput::Value(Color([200, 255, 200, 255]))))
          .with_padding(Sides([Px(5.0); 4]))
          .with(StyleDeclaration::align_items(AlignItems::Center))
          .with(StyleDeclaration::vertical_align(VerticalAlign::Keyword(VerticalAlignKeyword::Middle))),
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
              Style::default()
                .with(StyleDeclaration::display(Display::Inline)),
            ),
            text: "Flex Start ".to_string(),
          }
          .into(),
          ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
            preset: None,
            tw: None,
            style: Some(
              Style::default()
                .with(StyleDeclaration::display(Display::InlineBlock))
                .with_padding(Sides([Px(4.0); 4]))
                .with_margin(Sides([Px(0.0), Px(10.0), Px(0.0), Px(10.0)]))
                .with(StyleDeclaration::background_color(ColorInput::Value(Color([255, 200, 200, 255])))),
            ),
            children: Some(
              [TextNode {
        class_name: None,
        id: None,
        tag_name: None,
                preset: None,
                tw: None,
                style: None,
                text: "Inner".to_string(),
              }
              .into()]
              .into(),
            ),
          }
          .into(),
          TextNode {
        class_name: None,
        id: None,
        tag_name: None,
            preset: None,
            tw: None,
            style: Some(
              Style::default()
                .with(StyleDeclaration::display(Display::Inline)),
            ),
            text: " Flex End".to_string(),
          }
          .into(),
        ]
        .into(),
      ),
    }
    .into(),
    TextNode {
        class_name: None,
        id: None,
        tag_name: None,
      preset: None,
      tw: None,
      style: Some(
        Style::default()
          .with(StyleDeclaration::display(Display::Inline)),
      ),
      text: " followed by more text that should definitely wrap and show how the inline-flex container behaves when it is part of a wrapped line. We want to make sure the nested boxes are drawn in the correct positions even after wrapping.".to_string(),
    }
    .into(),
  ];

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Px(800.0)))
        .with(StyleDeclaration::display(Display::Block))
        .with_padding(Sides([Px(20.0); 4]))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with(StyleDeclaration::font_size(Px(20.0).into()))
        .with(StyleDeclaration::line_height(LineHeight::Length(Px(40.0)))),
    ),
    children: Some(children.into()),
  };

  run_fixture_test(container.into(), "inline_nested_flex_block");
}

#[test]
fn inline_complex_nested_fixture() {
  let json_data = json!({
    "type": "container",
    "style": {
      "display": "block",
      "fontFamily": "Inter, sans-serif",
      "fontSize": "16px",
      "lineHeight": "1.5",
      "color": "#333",
      "backgroundColor": "white",
      "width": "600px",
      "padding": "20px"
    },
    "children": [
      {
        "type": "text",
        "text": "Start with some basic inline text. ",
        "style": { "display": "inline" }
      },
      {
        "type": "container",
        "style": {
          "display": "inline-flex",
          "verticalAlign": "middle",
          "backgroundColor": "#f0f4f8",
          "borderWidth": "1px",
          "borderStyle": "solid",
          "borderColor": "#d9e2ec",
          "borderRadius": "4px",
          "padding": "8px 12px",
          "margin": "0 8px"
        },
        "children": [
          {
            "type": "text",
            "text": "Metadata: ",
            "style": {
              "display": "inline",
              "fontWeight": "bold",
              "color": "#102a43",
              "textTransform": "uppercase",
              "fontSize": "12px"
            }
          },
          {
            "type": "container",
            "style": {
              "display": "inline-flex",
              "alignItems": "center",
              "gap": "4px",
              "backgroundColor": "#bcccdc",
              "borderRadius": "999px",
              "padding": "2px 8px",
              "verticalAlign": "baseline"
            },
            "children": [
              {
                "type": "text",
                "text": "Tag",
                "style": { "display": "inline", "color": "white", "fontSize": "10px", "fontWeight": "600" }
              }
            ]
          }
        ]
      },
      {
        "type": "text",
        "text": "Followed by a longer sentence that demonstrates how text wraps around inline-block elements. ",
        "style": { "display": "inline" }
      },
      {
        "type": "container",
        "style": {
          "display": "inline-block",
          "verticalAlign": "bottom",
          "width": "120px",
          "backgroundColor": "#ffeedb",
          "borderWidth": "1px",
          "borderStyle": "solid",
          "borderColor": "#ff9c38",
          "padding": "10px",
          "margin": "0 5px"
        },
        "children": [
           {
             "type": "text",
             "text": "A fixed-width block that sits on the bottom of the line box.",
             "style": { "display": "block", "fontSize": "12px", "lineHeight": "1.2" }
           }
        ]
      },
      {
        "type": "text",
        "text": " And finally some more text to close things out.",
        "style": { "display": "inline"}
      }
    ]
  });

  let node = from_value(json_data).expect("Failed to parse JSON fixture");
  run_fixture_test(node, "inline_complex_nested_fixture");
}

#[test]
fn inline_text_decorations() {
  let json_data = json!({
    "type": "container",
    "style": {
      "display": "block",
      "width": "100%",
      "height": "100%",
      "backgroundColor": "white",
      "padding": "40px",
      "fontSize": "48px",
    },
    "children": [
      {
        "type": "text",
        "text": "Hello World",
        "style": {
          "display": "inline",
          "textDecoration": "4px underline line-through blue",
        }
      },
      {
        "type": "text",
        "text": "Woah",
        "style": {
          "display": "inline-block",
          "backgroundColor": "rgb(255 0 0 / 0.5)",
          "verticalAlign": "text-bottom",
        }
      },
      {
        "type": "container",
        "style": {
          "display": "inline-block",
          "backgroundColor": "rgb(0 0 255 / 0.5)",
          "fontStyle": "italic",
          "verticalAlign": "middle",
          "padding": "10px",
        },
        "children": [
          {
            "type": "text",
            "text": "It works right",
            "style": {
              "display": "inline-block",
              "background": "yellow",
            }
          },
          {
            "type": "text",
            "text": "A flexbox!",
            "style": {
              "display": "inline-flex",
              "background": "green",
            }
          }
        ]
      },
      {
        "type": "text",
        "text": " Red Underline",
        "style": {
          "display": "inline",
          "color": "red",
          "textDecoration": "underline",
        }
      },
    ]
  });

  let node = from_value(json_data).expect("Failed to parse JSON fixture");
  run_fixture_test(node, "inline_text_decorations");
}
