use takumi::layout::{
  node::{ContainerNode, ImageNode, TextNode},
  style::{
    AlignItems, Color, ColorInput, CssOption, Display, FontWeight, JustifyContent,
    LengthUnit::{Percentage, Px},
    Sides, StyleBuilder, TextOverflow, TextTransform, WhiteSpace,
  },
};

mod test_utils;
use test_utils::run_style_width_test;

#[test]
fn fixtures_text_inline() {
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

  let children = texts
    .iter()
    .map(|(text, style)| {
      TextNode {
        tw: None,
        style: Some(style.clone()),
        text: text.to_string(),
      }
      .into()
    })
    .collect::<Vec<_>>();

  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color::white()))
        .width(Percentage(100.0))
        .display(Display::Block)
        .justify_content(JustifyContent::Center)
        .line_clamp(CssOption::some(3.into()))
        .text_overflow(TextOverflow::Ellipsis)
        .font_size(CssOption::some(Px(48.0)))
        .white_space(WhiteSpace::pre_wrap())
        .build()
        .unwrap(),
    ),
    children: Some(children),
  };

  run_style_width_test(container.into(), "tests/fixtures/text_inline.webp");
}

#[test]
fn fixtures_inline_image() {
  // Inline image should behave as inline-level box content
  let children = vec![
    TextNode {
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
    children: Some(vec![
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .border_width(CssOption::some(Sides([Px(1.0); 4])))
            .display(Display::Block)
            .font_size(CssOption::some(Px(48.0)))
            .build()
            .unwrap(),
        ),
        children: Some(children),
      }
      .into(),
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/inline_image.webp");
}

#[test]
fn fixtures_inline_block_in_inline() {
  // A block-level container inside inline content: should create anonymous block formatting context
  let children = vec![
    TextNode {
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
      tw: None,
      style: Some(
        StyleBuilder::default()
          .display(Display::Block)
          .background_color(ColorInput::Value(Color([200, 200, 255, 255])))
          .width(Percentage(80.0))
          .font_size(CssOption::some(Px(18.0)))
          .build()
          .unwrap(),
      ),
      children: Some(vec![
        TextNode {
          tw: None,
          style: Some(
            StyleBuilder::default()
              .display(Display::Block)
              .build()
              .unwrap(),
          ),
          text: "Block inside inline".to_string(),
        }
        .into(),
      ]),
    }
    .into(),
    TextNode {
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
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color::white()))
        .width(Percentage(100.0))
        .display(Display::Block)
        .font_size(CssOption::some(Px(24.0)))
        .white_space(WhiteSpace::pre())
        .build()
        .unwrap(),
    ),
    children: Some(children),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/inline_block_in_inline.webp",
  );
}
