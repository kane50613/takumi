use smallvec::smallvec;
use takumi::layout::{
  node::{ContainerNode, NodeKind, TextNode},
  style::{LengthUnit::*, *},
};

mod test_utils;
use test_utils::run_style_width_test;

// Basic text render with defaults
#[test]
fn fixtures_text_basic() {
  let text = TextNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .build()
        .unwrap(),
    ),
    text: "The quick brown fox jumps over the lazy dog 12345".to_string(),
  };

  run_style_width_test(NodeKind::Text(text), "tests/fixtures/text_basic.webp");
}

#[test]
fn fixtures_text_typography_regular_24px() {
  let text = TextNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(24.0)))
        .build()
        .unwrap(),
    ),
    text: "Regular 24px".to_string(),
  };

  run_style_width_test(
    text.into(),
    "tests/fixtures/text_typography_regular_24px.webp",
  );
}

#[test]
fn fixtures_text_typography_variable_weight() {
  let nodes = (400..=900)
    .step_by(50)
    .map(|weight| {
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .font_size(Some(Px(48.0)))
            .font_weight(FontWeight::from(weight as f32))
            .build()
            .unwrap(),
        ),
        text: weight.to_string(),
      }
      .into()
    })
    .collect::<Vec<_>>();

  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(24.0)))
        .gap(SpacePair::from_pair(Px(0.0), Px(24.0)))
        .flex_wrap(FlexWrap::Wrap)
        .build()
        .unwrap(),
    ),
    children: Some(nodes),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/text_typography_variable_weight.webp",
  );
}

#[test]
fn fixtures_text_typography_medium_weight_500() {
  let text = TextNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(24.0)))
        .font_weight(FontWeight::from(500.0))
        .build()
        .unwrap(),
    ),
    text: "Medium 24px".to_string(),
  };

  run_style_width_test(
    text.into(),
    "tests/fixtures/text_typography_medium_weight_500.webp",
  );
}

#[test]
fn fixtures_text_typography_line_height_40px() {
  let text = TextNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(24.0)))
        .line_height(LineHeight(Px(40.0)))
        .build()
        .unwrap(),
    ),
    text: "Line height 40px".to_string(),
  };

  run_style_width_test(
    text.into(),
    "tests/fixtures/text_typography_line_height_40px.webp",
  );
}

#[test]
fn fixtures_text_typography_letter_spacing_2px() {
  let text = TextNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(24.0)))
        .letter_spacing(Some(Px(2.0)))
        .build()
        .unwrap(),
    ),
    text: "Letter spacing 2px".to_string(),
  };

  run_style_width_test(
    text.into(),
    "tests/fixtures/text_typography_letter_spacing_2px.webp",
  );
}

#[test]
fn fixtures_text_align_start() {
  let text = TextNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .width(Percentage(100.0))
        .font_size(Some(Px(24.0)))
        .text_align(TextAlign::Start)
        .build()
        .unwrap(),
    ),
    text: "Start aligned".to_string(),
  };

  run_style_width_test(text.into(), "tests/fixtures/text_align_start.webp");
}

#[test]
fn fixtures_text_align_center() {
  let text = TextNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .width(Percentage(100.0))
        .font_size(Some(Px(24.0)))
        .text_align(TextAlign::Center)
        .build()
        .unwrap(),
    ),
    text: "Center aligned".to_string(),
  };

  run_style_width_test(text.into(), "tests/fixtures/text_align_center.webp");
}

#[test]
fn fixtures_text_align_right() {
  let text = TextNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .width(Percentage(100.0))
        .font_size(Some(Px(24.0)))
        .text_align(TextAlign::Right)
        .build()
        .unwrap(),
    ),
    text: "Right aligned".to_string(),
  };

  run_style_width_test(text.into(), "tests/fixtures/text_align_right.webp");
}

#[test]
fn fixtures_text_ellipsis_line_clamp_2() {
  let long_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.";

  let text = TextNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(48.0)))
        .text_overflow(TextOverflow::Ellipsis)
        .line_clamp(Some(2.into()))
        .build()
        .unwrap(),
    ),
    text: long_text.to_string(),
  };

  run_style_width_test(
    text.into(),
    "tests/fixtures/text_ellipsis_line_clamp_2.webp",
  );
}

#[test]
fn fixtures_text_transform_all() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Percentage(100.0))
            .font_size(Some(Px(28.0)))
            .text_transform(TextTransform::None)
            .build()
            .unwrap(),
        ),
        text: "None: The quick Brown Fox".to_string(),
      }
      .into(),
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Percentage(100.0))
            .font_size(Some(Px(28.0)))
            .text_transform(TextTransform::Uppercase)
            .build()
            .unwrap(),
        ),
        text: "Uppercase: The quick Brown Fox".to_string(),
      }
      .into(),
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Percentage(100.0))
            .font_size(Some(Px(28.0)))
            .text_transform(TextTransform::Lowercase)
            .build()
            .unwrap(),
        ),
        text: "Lowercase: The QUICK Brown FOX".to_string(),
      }
      .into(),
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Percentage(100.0))
            .font_size(Some(Px(28.0)))
            .text_transform(TextTransform::Capitalize)
            .build()
            .unwrap(),
        ),
        text: "Capitalize: the quick brown fox".to_string(),
      }
      .into(),
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/text_transform_all.webp");
}

#[test]
fn fixtures_text_mask_image_gradient_and_emoji() {
  let gradient_images = BackgroundImages::from_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .font_size(Some(Px(72.0)))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .background_image(Some(gradient_images))
            .background_size(Some(BackgroundSizes::from_str("100% 100%").unwrap()))
            .background_position(Some(BackgroundPositions::from_str("0 0").unwrap()))
            .background_repeat(Some(BackgroundRepeats::from_str("no-repeat").unwrap()))
            .background_clip(BackgroundClip::Text)
            .color(ColorInput::Value(Color::transparent()))
            .build()
            .unwrap(),
        ),
        text: "Gradient Mask Emoji: 🪓 🦊 💩".to_string(),
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/text_mask_image_gradient_emoji.webp",
  );
}

#[test]
fn fixtures_text_stroke_black_red() {
  let text = TextNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .color(ColorInput::Value(Color([0, 0, 0, 255]))) // Black text
        .font_size(Some(Px(72.0)))
        .webkit_text_stroke_width(Some(Px(2.0)))
        .webkit_text_stroke_color(Some(ColorInput::Value(Color([255, 0, 0, 255])))) // Red stroke
        .build()
        .unwrap(),
    ),
    text: "Red Stroke".to_string(),
  };

  run_style_width_test(text.into(), "tests/fixtures/text_stroke_black_red.webp");
}

// Text shadow fixture
#[test]
fn fixtures_text_shadow() {
  // #ffcc00 1px 0 10px
  let shadows = smallvec![TextShadow {
    offset_x: Px(1.0),
    offset_y: Px(0.0),
    blur_radius: Px(10.0),
    color: ColorInput::Value(Color([255, 204, 0, 255])),
  }];

  let text = TextNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(48.0)))
        .text_shadow(Some(shadows))
        .build()
        .unwrap(),
    ),
    text: "Shadowed Text".to_string(),
  };

  run_style_width_test(text.into(), "tests/fixtures/text_shadow.webp");
}

#[test]
fn fixtures_text_shadow_no_blur_radius() {
  // 5px 5px #558abb
  let shadows = smallvec![TextShadow {
    offset_x: Px(5.0),
    offset_y: Px(5.0),
    blur_radius: Px(0.0),
    color: ColorInput::Value(Color([85, 138, 187, 255])),
  }];

  let text = TextNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(72.0)))
        .text_shadow(Some(shadows))
        .build()
        .unwrap(),
    ),
    text: "Shadowed Text".to_string(),
  };

  run_style_width_test(
    text.into(),
    "tests/fixtures/text_shadow_no_blur_radius.webp",
  );
}

#[test]
fn fixtures_text_wrap_nowrap() {
  let long_text = "This is a very long piece of text that should demonstrate text wrapping behavior when it exceeds the container width. The quick brown fox jumps over the lazy dog.";

  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([255, 255, 255, 255])))
        .font_size(Some(Px(32.0)))
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .display(Display::Flex)
        .flex_direction(FlexDirection::Column)
        .gap(SpacePair::from_single(Px(20.0)))
        .padding(Sides([Px(20.0); 4]))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      // Wrap text
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .text_wrap(Some(TextWrapMode::Wrap))
            .build()
            .unwrap(),
        ),
        text: format!("wrap: {}", long_text),
      }
      .into(),
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .text_wrap(Some(TextWrapMode::NoWrap))
            .build()
            .unwrap(),
        ),
        text: format!("nowrap: {}", long_text),
      }
      .into(),
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/text_wrap_nowrap.webp");
}

#[test]
fn fixtures_text_whitespace_collapse() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([255, 255, 255, 255])))
        .display(Display::Flex)
        .flex_direction(FlexDirection::Column)
        .font_size(Some(Px(32.0)))
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .gap(SpacePair::from_single(Px(20.0)))
        .padding(Sides([Px(20.0); 4]))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .white_space_collapse(Some(WhiteSpaceCollapse::Collapse))
            .build()
            .unwrap(),
        ),
        text: "collapse: Multiple    spaces   and\ttabs\t\tare    collapsed".to_string(),
      }
      .into(),
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .white_space_collapse(Some(WhiteSpaceCollapse::Preserve))
            .build()
            .unwrap(),
        ),
        text: "preserve: Multiple    spaces   and\ttabs\t\tare    preserved".to_string(),
      }
      .into(),
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .white_space_collapse(Some(WhiteSpaceCollapse::PreserveSpaces))
            .build()
            .unwrap(),
        ),
        text: "preserve-spaces: Multiple    spaces   preserved\nbut\nbreaks\nremoved".to_string(),
      }
      .into(),
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .white_space_collapse(Some(WhiteSpaceCollapse::PreserveBreaks))
            .build()
            .unwrap(),
        ),
        text: "preserve-breaks: Spaces    collapsed\n but\nline\nbreaks\npreserved".to_string(),
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/text_whitespace_collapse.webp",
  );
}

/// Handles special case where nowrap + ellipsis is used.
#[test]
fn fixtures_text_ellipsis_text_nowrap() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(48.0)))
        .padding(Sides([Px(20.0); 4]))
        .overflow(SpacePair::from_single(Overflow::Hidden))
        .width(Percentage(100.0))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .text_overflow(TextOverflow::Ellipsis)
            .text_wrap(Some(TextWrapMode::NoWrap))
            .border_width(Some(Sides([Px(1.0); 4])))
            .border_color(Some(ColorInput::Value(Color([255, 0, 0, 255]))))
            .word_break(WordBreak::BreakAll)
            .width(Percentage(100.0))
            .build()
            .unwrap(),
        ),
        text: "This is a very long piece of text that should demonstrate text wrapping behavior when it exceeds the container width. The quick brown fox jumps over the lazy dog.".to_string(),
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/text_ellipsis_text_nowrap.webp",
  );
}
