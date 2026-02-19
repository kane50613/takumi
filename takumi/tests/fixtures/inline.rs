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
      StyleBuilder::default()
        .display(Display::Inline)
        .build()
        .unwrap(),
    ),
    (
      "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ",
      StyleBuilder::default()
        .text_transform(TextTransform::Uppercase)
        .display(Display::Inline)
        .build()
        .unwrap(),
    ),
    (
      "Nothing beats a jet2 holiday! ",
      StyleBuilder::default()
        .color(ColorInput::Value(Color([255, 0, 0, 255])))
        .display(Display::Inline)
        .build()
        .unwrap(),
    ),
    (
      "I'm making a browser at this point. ",
      StyleBuilder::default()
        .font_weight(FontWeight::from(600.0))
        .display(Display::Inline)
        .build()
        .unwrap(),
    ),
  ];

  let children = Box::from_iter(texts.iter().map(|(text, style)| {
    TextNode {
      preset: None,
      tw: None,
      style: Some(style.clone()),
      text: text.to_string(),
    }
    .into()
  }));

  let container = ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color::white()))
        .width(Percentage(100.0))
        .display(Display::Block)
        .justify_content(JustifyContent::Center)
        .line_clamp(Some(3.into()))
        .text_overflow(TextOverflow::Ellipsis)
        .font_size(Some(Px(48.0)))
        .white_space(WhiteSpace::pre_wrap())
        .build()
        .unwrap(),
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
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::Inline)
          .build()
          .unwrap(),
      ),
      text: "Before ".to_string(),
    }
    .into(),
    ImageNode {
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::Inline)
          .border_width(Sides([Px(12.0); 4]))
          .border_style(BorderStyle::Solid)
          .border_color(ColorInput::Value(Color::transparent()))
          .background_image(BackgroundImages::from_str("linear-gradient(to right, red, blue)").ok())
          .background_clip(BackgroundClip::BorderArea)
          .build()
          .unwrap(),
      ),
      src: "assets/images/yeecord.png".into(),
      width: Some(64.0),
      height: Some(64.0),
    }
    .into(),
    TextNode {
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::Inline)
          .build()
          .unwrap(),
      ),
      text: " After".to_string(),
    }
    .into(),
  ];

  let container = ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .background_color(ColorInput::Value(Color::white()))
        .white_space(WhiteSpace::pre())
        .build()
        .unwrap(),
    ),
    children: Some(
      [ContainerNode {
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .border_width(Some(Sides([Px(2.0); 4])))
            .border_style(Some(BorderStyle::Solid))
            .display(Display::Block)
            .font_size(Some(Px(48.0)))
            .build()
            .unwrap(),
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
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::Inline)
          .build()
          .unwrap(),
      ),
      text: "Start ".to_string(),
    }
    .into(),
    ContainerNode {
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::Block)
          .background_color(ColorInput::Value(Color([200, 200, 255, 255])))
          .width(Percentage(80.0))
          .font_size(Some(Px(18.0)))
          .build()
          .unwrap(),
      ),
      children: Some(
        [TextNode {
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .display(Display::Block)
              .build()
              .unwrap(),
          ),
          text: "Block inside inline".to_string(),
        }
        .into()]
        .into(),
      ),
    }
    .into(),
    TextNode {
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::Inline)
          .build()
          .unwrap(),
      ),
      text: " End".to_string(),
    }
    .into(),
  ];

  let container = ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color::white()))
        .width(Percentage(100.0))
        .display(Display::Block)
        .font_size(Some(Px(24.0)))
        .white_space(WhiteSpace::pre())
        .build()
        .unwrap(),
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
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([255, 200, 200, 255])))
        .display(Display::Inline)
        .build()
        .unwrap(),
    ),
    (
      "world ",
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([200, 255, 200, 255])))
        .display(Display::Inline)
        .build()
        .unwrap(),
    ),
    (
      "from ",
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([200, 200, 255, 255])))
        .display(Display::Inline)
        .build()
        .unwrap(),
    ),
    (
      "Takumi!",
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([255, 255, 200, 255])))
        .display(Display::Inline)
        .build()
        .unwrap(),
    ),
  ];

  let children = Box::from_iter(texts.iter().map(|(text, style)| {
    TextNode {
      preset: None,
      tw: None,
      style: Some(style.clone()),
      text: text.to_string(),
    }
    .into()
  }));

  let container = ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .background_color(ColorInput::Value(Color::white()))
        .white_space(WhiteSpace::pre())
        .font_size(Some(Px(48.0)))
        .build()
        .unwrap(),
    ),
    children: Some(children),
  };

  run_fixture_test(container.into(), "inline_span_background_color");
}

#[test]
fn inline_atomic_containers() {
  let atomic = |display, color, label: &str| {
    ContainerNode {
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(display)
          .padding(Sides([Px(8.0); 4]))
          .background_color(ColorInput::Value(color))
          .border_width(Some(Sides([Px(2.0); 4])))
          .border_style(Some(BorderStyle::Solid))
          .build()
          .unwrap(),
      ),
      children: Some(
        [TextNode {
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
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .background_color(ColorInput::Value(Color::white()))
        .white_space(WhiteSpace::pre())
        .build()
        .unwrap(),
    ),
    children: Some(
      [ContainerNode {
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .display(Display::Block)
            .font_size(Some(Px(24.0)))
            .border_width(Some(Sides([Px(2.0); 4])))
            .border_style(Some(BorderStyle::Solid))
            .build()
            .unwrap(),
        ),
        children: Some(
          [
            TextNode {
              preset: None,
              tw: None,
              style: Some(
                StyleBuilder::default()
                  .display(Display::Inline)
                  .build()
                  .unwrap(),
              ),
              text: "before ".to_string(),
            }
            .into(),
            atomic(
              Display::InlineBlock,
              Color([255, 0, 0, 100]),
              "inline-block",
            ),
            TextNode {
              preset: None,
              tw: None,
              style: Some(
                StyleBuilder::default()
                  .display(Display::Inline)
                  .build()
                  .unwrap(),
              ),
              text: " mid ".to_string(),
            }
            .into(),
            atomic(Display::InlineFlex, Color([0, 255, 0, 100]), "inline-flex"),
            TextNode {
              preset: None,
              tw: None,
              style: Some(
                StyleBuilder::default()
                  .display(Display::Inline)
                  .build()
                  .unwrap(),
              ),
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
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::Inline)
          .build()
          .unwrap(),
      ),
      text: "This is some preceding text that is long enough to wrap eventually. ".to_string(),
    }
    .into(),
    ContainerNode {
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::InlineFlex)
          .background_color(ColorInput::Value(Color([200, 255, 200, 255])))
          .padding(Sides([Px(5.0); 4]))
          .align_items(AlignItems::Center)
          .vertical_align(VerticalAlign::Middle)
          .build()
          .unwrap(),
      ),
      children: Some(
        [
          TextNode {
            preset: None,
            tw: None,
            style: Some(
              StyleBuilder::default()
                .display(Display::Inline)
                .build()
                .unwrap(),
            ),
            text: "Flex Start ".to_string(),
          }
          .into(),
          ContainerNode {
            preset: None,
            tw: None,
            style: Some(
              StyleBuilder::default()
                .display(Display::InlineBlock)
                .padding(Sides([Px(4.0); 4]))
                .margin(Sides([Px(0.0), Px(10.0), Px(0.0), Px(10.0)]))
                .background_color(ColorInput::Value(Color([255, 200, 200, 255])))
                .build()
                .unwrap(),
            ),
            children: Some(
              [TextNode {
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
            preset: None,
            tw: None,
            style: Some(
              StyleBuilder::default()
                .display(Display::Inline)
                .build()
                .unwrap(),
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
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::Inline)
          .build()
          .unwrap(),
      ),
      text: " followed by more text that should definitely wrap and show how the inline-flex container behaves when it is part of a wrapped line. We want to make sure the nested boxes are drawn in the correct positions even after wrapping.".to_string(),
    }
    .into(),
  ];

  let container = ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(800.0))
        .display(Display::Block)
        .padding(Sides([Px(20.0); 4]))
        .background_color(ColorInput::Value(Color::white()))
        .font_size(Some(Px(20.0)))
        .line_height(LineHeight::Length(Px(40.0)))
        .build()
        .unwrap(),
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
