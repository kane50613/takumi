use parley::FontVariation;
use swash::tag_from_bytes;
use takumi::layout::{
  node::{ContainerNode, NodeKind, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

// Basic text render with defaults
#[test]
fn text_basic() {
  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .build()
        .unwrap(),
    ),
    text: "The quick brown fox jumps over the lazy dog 12345".to_string(),
  };

  run_fixture_test(text.into(), "text_basic");
}

#[test]
fn text_typography_regular_24px() {
  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
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

  run_fixture_test(text.into(), "text_typography_regular_24px");
}

#[test]
fn text_typography_variable_width() {
  const WIDTHS: &[f32] = &[60.0, 100.0, 130.0];

  let nodes = WIDTHS
    .iter()
    .map(|width| {
      TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .font_variation_settings(Some(
              [FontVariation {
                tag: tag_from_bytes(b"wdth"),
                value: *width,
              }]
              .into(),
            ))
            .build()
            .unwrap(),
        ),
        text: format!(
          "Hello world, this is a test of the variable width font: {}%",
          width
        ),
      }
      .into()
    })
    .collect::<Vec<_>>();

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_family(FontFamily::from_str("Archivo").ok())
        .font_size(Some(Px(48.0)))
        .flex_wrap(FlexWrap::Wrap)
        .row_gap(Some(Px(48.0)))
        .width(Percentage(100.0))
        .build()
        .unwrap(),
    ),
    children: Some(nodes.into_boxed_slice()),
  };

  run_fixture_test(container.into(), "text_typography_variable_width");
}

#[test]
fn text_typography_variable_weight() {
  let nodes = (400..=900)
    .step_by(50)
    .map(|weight| {
      TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
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
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
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
    children: Some(nodes.into_boxed_slice()),
  };

  run_fixture_test(container.into(), "text_typography_variable_weight");
}

#[test]
fn text_typography_medium_weight_500() {
  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
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

  run_fixture_test(text.into(), "text_typography_medium_weight_500");
}

#[test]
fn text_typography_line_height_40px() {
  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(24.0)))
        .line_height(LineHeight::Length(Px(40.0)))
        .build()
        .unwrap(),
    ),
    text: "Line height 40px".to_string(),
  };

  run_fixture_test(text.into(), "text_typography_line_height_40px");
}

#[test]
fn text_typography_letter_spacing_2px() {
  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
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

  run_fixture_test(text.into(), "text_typography_letter_spacing_2px");
}

#[test]
fn text_align_start() {
  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .width(Percentage(100.0))
        .display(Display::Block)
        .font_size(Some(Px(24.0)))
        .text_align(TextAlign::Start)
        .build()
        .unwrap(),
    ),
    text: "Start aligned".to_string(),
  };

  run_fixture_test(text.into(), "text_align_start");
}

#[test]
fn text_align_center() {
  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .width(Percentage(100.0))
        .display(Display::Block)
        .font_size(Some(Px(24.0)))
        .text_align(TextAlign::Center)
        .build()
        .unwrap(),
    ),
    text: "Center aligned".to_string(),
  };

  run_fixture_test(text.into(), "text_align_center");
}

#[test]
fn text_align_right() {
  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .width(Percentage(100.0))
        .display(Display::Block)
        .font_size(Some(Px(24.0)))
        .text_align(TextAlign::Right)
        .build()
        .unwrap(),
    ),
    text: "Right aligned".to_string(),
  };

  run_fixture_test(text.into(), "text_align_right");
}

#[test]
fn text_ellipsis_line_clamp_2() {
  let long_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.";

  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
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

  run_fixture_test(text.into(), "text_ellipsis_line_clamp_2");
}

#[test]
fn text_transform_all() {
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
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
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
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
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
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
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
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
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
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "text_transform_all");
}

#[test]
fn text_mask_image_gradient_and_emoji() {
  let gradient_images = BackgroundImages::from_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
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
    children: Some(
      [TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
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
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "text_mask_image_gradient_emoji");
}

#[test]
fn text_stroke_black_red() {
  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .color(ColorInput::Value(Color([0, 0, 0, 255]))) // Black text
        .font_size(Some(Px(96.0)))
        .font_weight(FontWeight::from(700.0))
        .padding(Sides([Px(24.0); 4]))
        .webkit_text_stroke_width(Some(Px(4.0)))
        .webkit_text_stroke_color(Some(ColorInput::Value(Color([255, 0, 0, 255])))) // Red stroke
        .build()
        .unwrap(),
    ),
    text: "Red Stroke".to_string(),
  };

  run_fixture_test(text.into(), "text_stroke_black_red");
}

#[test]
fn text_stroke_background_clip() {
  let gradient_images = BackgroundImages::from_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_image(Some(gradient_images))
        .background_clip(BackgroundClip::Text)
        .color(ColorInput::Value(Color::white()))
        .font_size(Some(Px(96.0)))
        .font_weight(FontWeight::from(700.0))
        .webkit_text_stroke_width(Some(Px(4.0)))
        .webkit_text_stroke_color(Some(ColorInput::Value(Color::transparent())))
        .build()
        .unwrap(),
    ),
    text: "Gradient Stroke".to_string(),
  };

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color::white()))
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .build()
        .unwrap(),
    ),
    children: Some([text.into()].into()),
  };

  run_fixture_test(container.into(), "text_stroke_background_clip");
}

// Text shadow fixture
#[test]
fn text_shadow() {
  // #ffcc00 1px 0 10px
  let shadows = [TextShadow {
    offset_x: Px(1.0),
    offset_y: Px(0.0),
    blur_radius: Px(10.0),
    color: ColorInput::Value(Color([255, 204, 0, 255])),
  }];

  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(48.0)))
        .text_shadow(Some(shadows.into()))
        .build()
        .unwrap(),
    ),
    text: "Shadowed Text".to_string(),
  };

  run_fixture_test(text.into(), "text_shadow");
}

#[test]
fn text_shadow_no_blur_radius() {
  // 5px 5px #558abb
  let shadows = [TextShadow {
    offset_x: Px(5.0),
    offset_y: Px(5.0),
    blur_radius: Px(0.0),
    color: ColorInput::Value(Color([85, 138, 187, 255])),
  }];

  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(72.0)))
        .text_shadow(Some(shadows.into()))
        .build()
        .unwrap(),
    ),
    text: "Shadowed Text".to_string(),
  };

  run_fixture_test(text.into(), "text_shadow_no_blur_radius");
}

#[test]
fn text_wrap_nowrap() {
  let long_text = "This is a very long piece of text that should demonstrate text wrapping behavior when it exceeds the container width. The quick brown fox jumps over the lazy dog.";

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
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
    children: Some(
      [
        // Wrap text
        TextNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .text_wrap_mode(Some(TextWrapMode::Wrap))
              .build()
              .unwrap(),
          ),
          text: format!("wrap: {}", long_text),
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
              .text_wrap_mode(Some(TextWrapMode::NoWrap))
              .build()
              .unwrap(),
          ),
          text: format!("nowrap: {}", long_text),
        }
        .into(),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "text_wrap_nowrap");
}

#[test]
fn text_whitespace_collapse() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
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
              .white_space_collapse(Some(WhiteSpaceCollapse::Collapse))
              .build()
              .unwrap(),
          ),
          text: "collapse: Multiple    spaces   and\ttabs\t\tare    collapsed".to_string(),
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
              .white_space_collapse(Some(WhiteSpaceCollapse::Preserve))
              .build()
              .unwrap(),
          ),
          text: "preserve: Multiple    spaces   and\ttabs\t\tare    preserved".to_string(),
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
              .white_space_collapse(Some(WhiteSpaceCollapse::PreserveSpaces))
              .build()
              .unwrap(),
          ),
          text: "preserve-spaces: Multiple    spaces   preserved\nbut\nbreaks\nremoved".to_string(),
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
              .white_space_collapse(Some(WhiteSpaceCollapse::PreserveBreaks))
              .build()
              .unwrap(),
          ),
          text: "preserve-breaks: Spaces    collapsed\n but\nline\nbreaks\npreserved".to_string(),
        }
        .into(),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "text_whitespace_collapse");
}

/// Handles special case where nowrap + ellipsis is used.
#[test]
fn text_ellipsis_text_nowrap() {
  let container = ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
    preset: None,
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
    children: Some([
      TextNode {
        class_name: None,
        id: None,
        tag_name: None,
    preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .text_overflow(TextOverflow::Ellipsis)
            .text_wrap_mode(Some(TextWrapMode::NoWrap))
            .border_width(Some(Sides([Px(1.0); 4])))
            .border_style(Some(BorderStyle::Solid))
            .border_color(Some(ColorInput::Value(Color([255, 0, 0, 255]))))
            .word_break(WordBreak::BreakAll)
            .width(Percentage(100.0))
            .build()
            .unwrap(),
        ),
        text: "This is a very long piece of text that should demonstrate text wrapping behavior when it exceeds the container width. The quick brown fox jumps over the lazy dog.".to_string(),
      }
      .into(),
    ].into()),
  };

  run_fixture_test(container.into(), "text_ellipsis_text_nowrap");
}

#[test]
fn text_wrap_style_all() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([255, 255, 255, 255])))
        .font_size(Some(Px(48.0)))
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .display(Display::Flex)
        .flex_direction(FlexDirection::Column)
        .gap(SpacePair::from_single(Px(40.0)))
        .padding(Sides([Px(20.0); 4]))
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        // Auto (default) - standard line breaking
        TextNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .text_wrap_style(Some(TextWrapStyle::Auto))
              .build()
              .unwrap(),
          ),
          text: "Auto: The quick brown fox jumps over the lazy dog.".to_string(),
        }
        .into(),
        // Balance - evenly distributes text across lines
        TextNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .text_wrap_style(Some(TextWrapStyle::Balance))
              .build()
              .unwrap(),
          ),
          text: "Balance: The quick brown fox jumps over the lazy dog.".to_string(),
        }
        .into(),
        // Pretty - avoids orphans on the last line (text ends with short word "it")
        TextNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .text_wrap_style(Some(TextWrapStyle::Pretty))
              .build()
              .unwrap(),
          ),
          text: "Pretty: The quick brown fox jumps over the lazy dog and catches it.".to_string(),
        }
        .into(),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "text_wrap_style_all");
}

#[test]
fn text_super_bold_stroke_background_clip() {
  let gradient_images = BackgroundImages::from_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let text = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_image(Some(gradient_images))
        .background_clip(BackgroundClip::Text)
        .display(Display::Block)
        .color(ColorInput::Value(Color::white()))
        .font_size(Some(Px(120.0)))
        .font_weight(FontWeight::from(900.0))
        .webkit_text_stroke_width(Some(Px(20.0)))
        .webkit_text_stroke_color(Some(ColorInput::Value(Color::transparent())))
        .padding(Sides([Px(60.0); 4]))
        .build()
        .unwrap(),
    ),
    text: "Super Bold".to_string(),
  };

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color::white()))
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .build()
        .unwrap(),
    ),
    children: Some([text.into()].into()),
  };

  run_fixture_test(container.into(), "text_super_bold_stroke_background_clip");
}

#[test]
fn text_font_stretch() {
  let stretches = [
    (
      "ultra-condensed",
      FontStretch::from_str("ultra-condensed").unwrap(),
    ),
    ("condensed", FontStretch::from_str("condensed").unwrap()),
    (
      "semi-condensed",
      FontStretch::from_str("semi-condensed").unwrap(),
    ),
    ("normal", FontStretch::from_str("normal").unwrap()),
    (
      "semi-expanded",
      FontStretch::from_str("semi-expanded").unwrap(),
    ),
    ("expanded", FontStretch::from_str("expanded").unwrap()),
    (
      "ultra-expanded",
      FontStretch::from_str("ultra-expanded").unwrap(),
    ),
  ];

  let nodes = stretches
    .iter()
    .map(|(label, stretch)| {
      TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .font_size(Some(Px(36.0)))
            .font_stretch(*stretch)
            .build()
            .unwrap(),
        ),
        text: format!("font-stretch: {}", label),
      }
      .into()
    })
    .collect::<Vec<_>>();

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_family(FontFamily::from_str("Archivo").ok())
        .width(Percentage(100.0))
        .flex_direction(FlexDirection::Column)
        .padding(Sides([Px(20.0); 4]))
        .gap(SpacePair::from_single(Px(12.0)))
        .build()
        .unwrap(),
    ),
    children: Some(nodes.into_boxed_slice()),
  };

  run_fixture_test(container.into(), "text_font_stretch");
}

#[test]
fn text_flex_centered_text_node_vs_nested_container() {
  let first_box_text: NodeKind = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(300.0))
        .height(Px(200.0))
        .margin(Sides([Px(0.0), Px(0.0), Px(30.0), Px(0.0)]))
        .background_color(ColorInput::Value(Color::from_str("#3b82f6").unwrap()))
        .display(Display::Flex)
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .font_size(Some(Px(30.0)))
        .build()
        .unwrap(),
    ),
    text: "centered...?".to_string(),
  }
  .into();

  let second_box_nested_text: NodeKind = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(300.0))
        .height(Px(200.0))
        .background_color(ColorInput::Value(Color::from_str("#ab82f6").unwrap()))
        .display(Display::Flex)
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .font_size(Some(Px(30.0)))
        .build()
        .unwrap(),
    ),
    children: Some(
      [TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: None,
        text: "centered".to_string(),
      }
      .into()]
      .into(),
    ),
  }
  .into();

  let root = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::black()))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .build()
        .unwrap(),
    ),
    children: Some(
      [ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .display(Display::Flex)
            .flex_direction(FlexDirection::Column)
            .align_items(AlignItems::Center)
            .justify_content(JustifyContent::Center)
            .color(ColorInput::Value(Color::white()))
            .build()
            .unwrap(),
        ),
        children: Some([first_box_text, second_box_nested_text].into()),
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(
    root.into(),
    "text_flex_centered_text_node_vs_nested_container",
  );
}

#[test]
fn text_font_synthesis_weight_auto_none() {
  let nodes = [("auto", FontSynthesic::Auto), ("none", FontSynthesic::None)]
    .iter()
    .map(|(label, synthesis_weight)| {
      TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .font_size(Some(Px(72.0)))
            .font_family(FontFamily::from_str("Scheherazade New Test").ok())
            .font_weight(FontWeight::from(900.0))
            .font_synthesis_weight(*synthesis_weight)
            .build()
            .unwrap(),
        ),
        text: format!("font-synthesis-weight: {} - السلام عليكم", label),
      }
      .into()
    })
    .collect::<Vec<_>>();

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .width(Percentage(100.0))
        .flex_direction(FlexDirection::Column)
        .padding(Sides([Px(20.0); 4]))
        .gap(SpacePair::from_single(Px(12.0)))
        .build()
        .unwrap(),
    ),
    children: Some(nodes.into_boxed_slice()),
  };

  run_fixture_test(container.into(), "text_font_synthesis_weight_auto_none");
}

#[test]
fn text_font_synthesis_style_auto_none() {
  let nodes = [("auto", FontSynthesic::Auto), ("none", FontSynthesic::None)]
    .iter()
    .map(|(label, synthesis_style)| {
      TextNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .font_size(Some(Px(72.0)))
            .font_family(FontFamily::from_str("Scheherazade New Test").ok())
            .font_style(FontStyle::italic())
            .font_synthesis_style(*synthesis_style)
            .build()
            .unwrap(),
        ),
        text: format!("font-synthesis-style: {} - السلام عليكم", label),
      }
      .into()
    })
    .collect::<Vec<_>>();

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .width(Percentage(100.0))
        .flex_direction(FlexDirection::Column)
        .padding(Sides([Px(20.0); 4]))
        .gap(SpacePair::from_single(Px(12.0)))
        .build()
        .unwrap(),
    ),
    children: Some(nodes.into_boxed_slice()),
  };

  run_fixture_test(container.into(), "text_font_synthesis_style_auto_none");
}

#[test]
fn text_font_synthesis_weight_emoji() {
  let nodes = [
    (
      "auto",
      FontSynthesis {
        weight: FontSynthesic::Auto,
        style: FontSynthesic::Auto,
      },
    ),
    (
      "none",
      FontSynthesis {
        weight: FontSynthesic::None,
        style: FontSynthesic::None,
      },
    ),
  ]
  .iter()
  .map(|(label, synthesis)| {
    TextNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .font_size(Some(Px(72.0)))
          .font_family(FontFamily::from_str("Scheherazade New Test").ok())
          .font_weight(FontWeight::from(900.0))
          .font_style(FontStyle::italic())
          .font_synthesis(*synthesis)
          .build()
          .unwrap(),
      ),
      text: format!("font-synthesis: {} - Takumi 😀 😺 🧪", label),
    }
    .into()
  })
  .collect::<Vec<_>>();

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .width(Percentage(100.0))
        .flex_direction(FlexDirection::Column)
        .padding(Sides([Px(20.0); 4]))
        .gap(SpacePair::from_single(Px(12.0)))
        .build()
        .unwrap(),
    ),
    children: Some(nodes.into_boxed_slice()),
  };

  run_fixture_test(container.into(), "text_font_synthesis_weight_emoji");
}

#[test]
fn text_chinese_ellipsis() {
  let text = "日本利用壓電磁磚將腳步轉化為電能。這些瓷磚捕捉來自你腳步的動能。當你行走時，你的重量和動作會對瓷磚產生壓力。磁磚會輕微彎曲，從而產生機械應力。磁磚內部的壓電材料將這種應力轉化為電能。每一步都會產生少量電荷，而數百萬步結合在一起就能產生足夠的電力來驅動 LED燈、數位顯示器和感測器。在像澀谷車站這樣繁忙的地方，每天大約有240萬個腳步為此系統作出貢獻。這些電能可以被儲存或立即使用，從而減少對傳統電賴，並支持永續的城市基礎設施。這種方法將日常運動轉化為實用的再生能源。";

  let node = TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .font_size(Some(Px(64.0)))
        .padding(Sides::from(Px(24.0)))
        .font_family(FontFamily::from_str("Noto Sans TC").ok())
        .text_overflow(TextOverflow::Ellipsis)
        .build()
        .unwrap(),
    ),
    text: text.to_string(),
  };

  run_fixture_test(node.into(), "text_chinese_ellipsis");
}

#[test]
fn text_devanagari_noto_sans() {
  fn create_node(weight: f32, font_family: &str) -> TextNode {
    let text = "नमस्ते दुनिया, यह देवनागरी लिपि का एक परीक्षण है।";

    TextNode {
      class_name: None,
      id: None,
      tag_name: None,
      preset: None,
      tw: None,
      style: Some(
        StyleBuilder::default()
          .width(Percentage(100.0))
          .height(Percentage(100.0))
          .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
          .font_size(Some(Px(48.0)))
          .padding(Sides::from(Px(24.0)))
          .font_family(FontFamily::from_str(font_family).ok())
          .font_weight(FontWeight::from(weight))
          .build()
          .unwrap(),
      ),
      text: text.to_string(),
    }
  }

  let nodes = [
    (400.0, "Noto Sans Devanagari"),
    (700.0, "Noto Sans Devanagari"),
    (400.0, "Poppins"),
    (700.0, "Poppins Bold"),
  ]
  .iter()
  .map(|(weight, font_family)| create_node(*weight, font_family).into())
  .collect::<Vec<_>>();

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
        .width(Percentage(100.0))
        .flex_direction(FlexDirection::Column)
        .padding(Sides([Px(20.0); 4]))
        .gap(SpacePair::from_single(Px(12.0)))
        .build()
        .unwrap(),
    ),
    children: Some(nodes.into_boxed_slice()),
  };

  run_fixture_test(container.into(), "text_devanagari_noto_sans");
}
