use takumi::layout::{
  node::{ContainerNode, ImageNode, TextNode},
  style::{
    AlignItems, Color, ColorInput, Display, FontWeight, JustifyContent,
    Length::{Percentage, Px},
    Sides, StyleBuilder, TextOverflow, TextTransform, WhiteSpace,
  },
};

use crate::test_utils::run_style_width_test;

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

  run_style_width_test(container.into(), "text_inline.png");
}

#[test]
fn inline_image() {
  // Inline image should behave as inline-level box content
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
      text: "Before ".to_string(),
    }
    .into(),
    ImageNode {
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::Inline)
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
            .border_width(Some(Sides([Px(1.0); 4])))
            .display(Display::Block)
            .font_size(Some(Px(48.0)))
            .build()
            .unwrap(),
        ),
        children: Some(children.into_boxed_slice()),
      }
      .into()]
      .into(),
    ),
  };

  run_style_width_test(container.into(), "inline_image.png");
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

  run_style_width_test(container.into(), "inline_block_in_inline.png");
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

  run_style_width_test(container.into(), "inline_span_background_color.png");
}
