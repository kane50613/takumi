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
  let text = TextNode::default()
    .with_style(
      Style::default().with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      ))),
    )
    .with_text("The quick brown fox jumps over the lazy dog 12345".to_string());

  run_fixture_test(text.into(), "text_basic");
}

#[test]
fn text_typography_regular_24px() {
  let text = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::font_size(Px(24.0).into())),
    )
    .with_text("Regular 24px".to_string());

  run_fixture_test(text.into(), "text_typography_regular_24px");
}

#[test]
fn text_typography_variable_width() {
  const WIDTHS: &[f32] = &[60.0, 100.0, 130.0];

  let nodes: Vec<NodeKind> = WIDTHS
    .iter()
    .map(|width| {
      TextNode::default()
        .with_style(
          Style::default().with(StyleDeclaration::font_variation_settings(Box::new([
            FontVariation {
              tag: tag_from_bytes(b"wdth"),
              value: *width,
            },
          ]))),
        )
        .with_text(format!(
          "Hello world, this is a test of the variable width font: {}%",
          width
        ))
        .into()
    })
    .collect::<Vec<_>>();

  let Ok(family) = FontFamily::from_str("Archivo") else {
    unreachable!()
  };

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::font_family(family))
        .with(StyleDeclaration::font_size(Px(48.0).into()))
        .with(StyleDeclaration::flex_wrap(FlexWrap::Wrap))
        .with(StyleDeclaration::row_gap(Px(48.0)))
        .with(StyleDeclaration::width(Percentage(100.0))),
    )
    .with_children(nodes.into_boxed_slice());

  run_fixture_test(container.into(), "text_typography_variable_width");
}

#[test]
fn text_typography_variable_weight() {
  let nodes: Vec<NodeKind> = (400..=900)
    .step_by(50)
    .map(|weight| {
      TextNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::font_size(Px(48.0).into()))
            .with(StyleDeclaration::font_weight(FontWeight::from(
              weight as f32,
            ))),
        )
        .with_text(weight.to_string())
        .into()
    })
    .collect();

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::font_size(Px(24.0).into()))
        .with_gap(SpacePair::from_reversed_pair(Px(0.0), Px(24.0)))
        .with(StyleDeclaration::flex_wrap(FlexWrap::Wrap)),
    )
    .with_children(nodes.into_boxed_slice());

  run_fixture_test(container.into(), "text_typography_variable_weight");
}

#[test]
fn text_typography_medium_weight_500() {
  let text = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::font_size(Px(24.0).into()))
        .with(StyleDeclaration::font_weight(FontWeight::from(500.0))),
    )
    .with_text("Medium 24px".to_string());

  run_fixture_test(text.into(), "text_typography_medium_weight_500");
}

#[test]
fn text_typography_line_height_40px() {
  let text = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::font_size(Px(24.0).into()))
        .with(StyleDeclaration::line_height(LineHeight::Length(Px(40.0)))),
    )
    .with_text("Line height 40px".to_string());

  run_fixture_test(text.into(), "text_typography_line_height_40px");
}

#[test]
fn text_typography_letter_spacing_2px() {
  let text = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::font_size(Px(24.0).into()))
        .with(StyleDeclaration::letter_spacing(Px(2.0))),
    )
    .with_text("Letter spacing 2px".to_string());

  run_fixture_test(text.into(), "text_typography_letter_spacing_2px");
}

#[test]
fn text_align_start() {
  let text = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::font_size(Px(24.0).into()))
        .with(StyleDeclaration::text_align(TextAlign::Start)),
    )
    .with_text("Start aligned".to_string());

  run_fixture_test(text.into(), "text_align_start");
}

#[test]
fn text_align_center() {
  let text = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::font_size(Px(24.0).into()))
        .with(StyleDeclaration::text_align(TextAlign::Center)),
    )
    .with_text("Center aligned".to_string());

  run_fixture_test(text.into(), "text_align_center");
}

#[test]
fn text_align_right() {
  let text = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::font_size(Px(24.0).into()))
        .with(StyleDeclaration::text_align(TextAlign::Right)),
    )
    .with_text("Right aligned".to_string());

  run_fixture_test(text.into(), "text_align_right");
}

#[test]
fn text_ellipsis_line_clamp_2() {
  let long_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.";

  let text = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::font_size(Px(48.0).into()))
        .with(StyleDeclaration::text_overflow(TextOverflow::Ellipsis))
        .with(StyleDeclaration::line_clamp(Some(2.into()))),
    )
    .with_text(long_text.to_string());

  run_fixture_test(text.into(), "text_ellipsis_line_clamp_2");
}

#[test]
fn text_transform_all() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        ))),
    )
    .with_children([
      TextNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::width(Percentage(100.0)))
            .with(StyleDeclaration::font_size(Px(28.0).into()))
            .with(StyleDeclaration::text_transform(TextTransform::None)),
        )
        .with_text("None: The quick Brown Fox".to_string()),
      TextNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::width(Percentage(100.0)))
            .with(StyleDeclaration::font_size(Px(28.0).into()))
            .with(StyleDeclaration::text_transform(TextTransform::Uppercase)),
        )
        .with_text("Uppercase: The quick Brown Fox".to_string()),
      TextNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::width(Percentage(100.0)))
            .with(StyleDeclaration::font_size(Px(28.0).into()))
            .with(StyleDeclaration::text_transform(TextTransform::Lowercase)),
        )
        .with_text("Lowercase: The QUICK Brown FOX".to_string()),
      TextNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::width(Percentage(100.0)))
            .with(StyleDeclaration::font_size(Px(28.0).into()))
            .with(StyleDeclaration::text_transform(TextTransform::Capitalize)),
        )
        .with_text("Capitalize: the quick brown fox".to_string()),
    ]);

  run_fixture_test(container.into(), "text_transform_all");
}

#[test]
fn text_mask_image_gradient_and_emoji() {
  let gradient_images = BackgroundImages::from_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::font_size(Px(72.0).into()))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center)),
    )
    .with_children([TextNode::default()
      .with_style(
        Style::default()
          .with(StyleDeclaration::background_image(Some(gradient_images)))
          .with(StyleDeclaration::background_size(
            BackgroundSizes::from_str("100% 100%").unwrap(),
          ))
          .with(StyleDeclaration::background_position(
            BackgroundPositions::from_str("0 0").unwrap(),
          ))
          .with(StyleDeclaration::background_repeat(
            BackgroundRepeats::from_str("no-repeat").unwrap(),
          ))
          .with(StyleDeclaration::background_clip(BackgroundClip::Text))
          .with(StyleDeclaration::color(ColorInput::Value(
            Color::transparent(),
          ))),
      )
      .with_text("Gradient Mask Emoji: 🪓 🦊 💩".to_string())]);

  run_fixture_test(container.into(), "text_mask_image_gradient_emoji");
}

#[test]
fn text_stroke_black_red() {
  let text = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::color(ColorInput::Value(Color([
          0, 0, 0, 255,
        ]))))
        .with(StyleDeclaration::font_size(Px(96.0).into()))
        .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
        .with_padding(Sides([Px(24.0); 4]))
        .with(StyleDeclaration::webkit_text_stroke_width(Some(Px(4.0))))
        .with(StyleDeclaration::webkit_text_stroke_color(Some(
          ColorInput::Value(Color([255, 0, 0, 255])),
        ))),
    )
    .with_text("Red Stroke".to_string());

  run_fixture_test(text.into(), "text_stroke_black_red");
}

#[test]
fn text_stroke_background_clip() {
  let gradient_images = BackgroundImages::from_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let text = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_image(Some(gradient_images)))
        .with(StyleDeclaration::background_position(
          BackgroundPositions::from_str("center center").unwrap(),
        ))
        .with(StyleDeclaration::background_clip(BackgroundClip::Text))
        .with(StyleDeclaration::color(ColorInput::Value(Color::white())))
        .with(StyleDeclaration::font_size(Px(96.0).into()))
        .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
        .with(StyleDeclaration::webkit_text_stroke_width(Some(Px(4.0))))
        .with(StyleDeclaration::webkit_text_stroke_color(Some(
          ColorInput::Value(Color::transparent()),
        ))),
    )
    .with_text("Gradient Stroke".to_string());

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center)),
    )
    .with_children([text]);

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

  let text = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::font_size(Px(48.0).into()))
        .with(StyleDeclaration::text_shadow(Some(shadows.into()))),
    )
    .with_text("Shadowed Text".to_string());

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

  let text = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::font_size(Px(72.0).into()))
        .with(StyleDeclaration::text_shadow(Some(shadows.into()))),
    )
    .with_text("Shadowed Text".to_string());

  run_fixture_test(text.into(), "text_shadow_no_blur_radius");
}

#[test]
fn text_wrap_nowrap() {
  let long_text = "This is a very long piece of text that should demonstrate text wrapping behavior when it exceeds the container width. The quick brown fox jumps over the lazy dog.";

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 255, 255]),
        )))
        .with(StyleDeclaration::font_size(Px(32.0).into()))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with_gap(SpacePair::from_single(Px(20.0)))
        .with_padding(Sides([Px(20.0); 4])),
    )
    .with_children([
      // Wrap text
      TextNode::default()
        .with_style(Style::default().with(StyleDeclaration::text_wrap_mode(TextWrapMode::Wrap)))
        .with_text(format!("wrap: {}", long_text)),
      TextNode::default()
        .with_style(Style::default().with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap)))
        .with_text(format!("nowrap: {}", long_text)),
    ]);

  run_fixture_test(container.into(), "text_wrap_nowrap");
}

#[test]
fn text_whitespace_collapse() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 255, 255]),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::font_size(Px(32.0).into()))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with_gap(SpacePair::from_single(Px(20.0)))
        .with_padding(Sides([Px(20.0); 4])),
    )
    .with_children([
      TextNode::default()
        .with_style(
          Style::default().with(StyleDeclaration::white_space_collapse(
            WhiteSpaceCollapse::Collapse,
          )),
        )
        .with_text("collapse: Multiple    spaces   and\ttabs\t\tare    collapsed".to_string()),
      TextNode::default()
        .with_style(
          Style::default().with(StyleDeclaration::white_space_collapse(
            WhiteSpaceCollapse::Preserve,
          )),
        )
        .with_text("preserve: Multiple    spaces   and\ttabs\t\tare    preserved".to_string()),
      TextNode::default()
        .with_style(
          Style::default().with(StyleDeclaration::white_space_collapse(
            WhiteSpaceCollapse::PreserveSpaces,
          )),
        )
        .with_text(
          "preserve-spaces: Multiple    spaces   preserved\nbut\nbreaks\nremoved".to_string(),
        ),
      TextNode::default()
        .with_style(
          Style::default().with(StyleDeclaration::white_space_collapse(
            WhiteSpaceCollapse::PreserveBreaks,
          )),
        )
        .with_text(
          "preserve-breaks: Spaces    collapsed\n but\nline\nbreaks\npreserved".to_string(),
        ),
    ]);

  run_fixture_test(container.into(), "text_whitespace_collapse");
}

/// Handles special case where nowrap + ellipsis is used.
#[test]
fn text_ellipsis_text_nowrap() {
  let container = ContainerNode::default()
  .with_style(Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(Color([240, 240, 240, 255]))))
        .with(StyleDeclaration::font_size(Px(48.0).into()))
        .with_padding(Sides([Px(20.0); 4]))
        .with_overflow(SpacePair::from_single(Overflow::Hidden))
        .with(StyleDeclaration::width(Percentage(100.0))),)
  .with_children([
      TextNode::default()
  .with_style(Style::default()
            .with(StyleDeclaration::text_overflow(TextOverflow::Ellipsis))
            .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap))
            .with_border_width(Sides([Px(1.0); 4]))
            .with(StyleDeclaration::border_style(BorderStyle::Solid))
            .with(StyleDeclaration::border_color(ColorInput::Value(Color([255, 0, 0, 255]))))
            .with(StyleDeclaration::word_break(WordBreak::BreakAll))
            .with(StyleDeclaration::width(Percentage(100.0))),)
  .with_text("This is a very long piece of text that should demonstrate text wrapping behavior when it exceeds the container width. The quick brown fox jumps over the lazy dog.".to_string())

    ]);

  run_fixture_test(container.into(), "text_ellipsis_text_nowrap");
}

#[test]
fn text_wrap_style_all() {
  let children: Vec<NodeKind> = vec![
    TextNode::default()
      .with_style(Style::default().with(StyleDeclaration::text_wrap_style(TextWrapStyle::Auto)))
      .with_text("Auto: The quick brown fox jumps over the lazy dog.".to_string())
      .into(),
    TextNode::default()
      .with_style(Style::default().with(StyleDeclaration::text_wrap_style(TextWrapStyle::Balance)))
      .with_text("Balance: The quick brown fox jumps over the lazy dog.".to_string())
      .into(),
    TextNode::default()
      .with_style(Style::default().with(StyleDeclaration::text_wrap_style(TextWrapStyle::Pretty)))
      .with_text("Pretty: The quick brown fox jumps over the lazy dog and catches it.".to_string())
      .into(),
  ];

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 255, 255]),
        )))
        .with(StyleDeclaration::font_size(Px(48.0).into()))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with_gap(SpacePair::from_single(Px(40.0)))
        .with_padding(Sides([Px(20.0); 4])),
    )
    .with_children(children);

  run_fixture_test(container.into(), "text_wrap_style_all");
}

#[test]
fn text_super_bold_stroke_background_clip() {
  let gradient_images = BackgroundImages::from_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let text = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_image(Some(gradient_images)))
        .with(StyleDeclaration::background_position(
          BackgroundPositions::from_str("center center").unwrap(),
        ))
        .with(StyleDeclaration::background_clip(BackgroundClip::Text))
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::color(ColorInput::Value(Color::white())))
        .with(StyleDeclaration::font_size(Px(120.0).into()))
        .with(StyleDeclaration::font_weight(FontWeight::from(900.0)))
        .with(StyleDeclaration::webkit_text_stroke_width(Some(Px(20.0))))
        .with(StyleDeclaration::webkit_text_stroke_color(Some(
          ColorInput::Value(Color::transparent()),
        )))
        .with_padding(Sides([Px(60.0); 4])),
    )
    .with_text("Super Bold".to_string());

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center)),
    )
    .with_children([text]);

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

  let nodes: Vec<NodeKind> = stretches
    .iter()
    .map(|(label, stretch)| {
      TextNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::font_size(Px(36.0).into()))
            .with(StyleDeclaration::font_stretch(*stretch)),
        )
        .with_text(format!("font-stretch: {}", label))
        .into()
    })
    .collect::<Vec<_>>();

  let Ok(family) = FontFamily::from_str("Archivo") else {
    unreachable!()
  };

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::font_family(family))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with_padding(Sides([Px(20.0); 4]))
        .with_gap(SpacePair::from_single(Px(12.0))),
    )
    .with_children(nodes.into_boxed_slice());

  run_fixture_test(container.into(), "text_font_stretch");
}

#[test]
fn text_flex_centered_text_node_vs_nested_container() {
  let first_box_text: NodeKind = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Px(300.0)))
        .with(StyleDeclaration::height(Px(200.0)))
        .with_margin(Sides([Px(0.0), Px(0.0), Px(30.0), Px(0.0)]))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::from_str("#3b82f6").unwrap(),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::font_size(Px(30.0).into())),
    )
    .with_text("centered...?".to_string())
    .into();

  let second_box_nested_text: NodeKind = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Px(300.0)))
        .with(StyleDeclaration::height(Px(200.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::from_str("#ab82f6").unwrap(),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::font_size(Px(30.0).into())),
    )
    .with_children([TextNode::default().with_text("centered".to_string())])
    .into();

  let root = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::black(),
        )))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center)),
    )
    .with_child(
      ContainerNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Flex))
            .with(StyleDeclaration::flex_direction(FlexDirection::Column))
            .with(StyleDeclaration::align_items(AlignItems::Center))
            .with(StyleDeclaration::justify_content(JustifyContent::Center))
            .with(StyleDeclaration::color(ColorInput::Value(Color::white()))),
        )
        .with_children([first_box_text, second_box_nested_text]),
    );

  run_fixture_test(
    root.into(),
    "text_flex_centered_text_node_vs_nested_container",
  );
}

#[test]
fn text_font_synthesis_weight_auto_none() {
  let Ok(family) = FontFamily::from_str("Scheherazade New Test") else {
    unreachable!()
  };

  let nodes: Vec<NodeKind> = [("auto", FontSynthesic::Auto), ("none", FontSynthesic::None)]
    .iter()
    .map(|(label, synthesis_weight)| {
      TextNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::font_size(Px(72.0).into()))
            .with(StyleDeclaration::font_family(family.clone()))
            .with(StyleDeclaration::font_weight(FontWeight::from(900.0)))
            .with(StyleDeclaration::font_synthesis_weight(*synthesis_weight)),
        )
        .with_text(format!("font-synthesis-weight: {} - السلام عليكم", label))
        .into()
    })
    .collect::<Vec<_>>();

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with_padding(Sides([Px(20.0); 4]))
        .with_gap(SpacePair::from_single(Px(12.0))),
    )
    .with_children(nodes.into_boxed_slice());

  run_fixture_test(container.into(), "text_font_synthesis_weight_auto_none");
}

#[test]
fn text_font_synthesis_style_auto_none() {
  let Ok(family) = FontFamily::from_str("Scheherazade New Test") else {
    unreachable!()
  };

  let nodes: Vec<NodeKind> = [("auto", FontSynthesic::Auto), ("none", FontSynthesic::None)]
    .iter()
    .map(|(label, synthesis_style)| {
      TextNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::font_size(Px(72.0).into()))
            .with(StyleDeclaration::font_family(family.clone()))
            .with(StyleDeclaration::font_style(FontStyle::italic()))
            .with(StyleDeclaration::font_synthesis_style(*synthesis_style)),
        )
        .with_text(format!("font-synthesis-style: {} - السلام عليكم", label))
        .into()
    })
    .collect::<Vec<_>>();

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with_padding(Sides([Px(20.0); 4]))
        .with_gap(SpacePair::from_single(Px(12.0))),
    )
    .with_children(nodes.into_boxed_slice());

  run_fixture_test(container.into(), "text_font_synthesis_style_auto_none");
}

#[test]
fn text_font_synthesis_weight_emoji() {
  let Ok(family) = FontFamily::from_str("Scheherazade New Test") else {
    unreachable!()
  };

  let nodes: Vec<NodeKind> = [
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
    TextNode::default()
      .with_style(
        Style::default()
          .with(StyleDeclaration::font_size(Px(72.0).into()))
          .with(StyleDeclaration::font_family(family.clone()))
          .with(StyleDeclaration::font_weight(FontWeight::from(900.0)))
          .with(StyleDeclaration::font_style(FontStyle::italic()))
          .with_font_synthesis(*synthesis),
      )
      .with_text(format!("font-synthesis: {} - Takumi 😀 😺 🧪", label))
      .into()
  })
  .collect::<Vec<_>>();

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with_padding(Sides([Px(20.0); 4]))
        .with_gap(SpacePair::from_single(Px(12.0))),
    )
    .with_children(nodes.into_boxed_slice());

  run_fixture_test(container.into(), "text_font_synthesis_weight_emoji");
}

#[test]
fn text_chinese_ellipsis() {
  let text = "日本利用壓電磁磚將腳步轉化為電能。這些瓷磚捕捉來自你腳步的動能。當你行走時，你的重量和動作會對瓷磚產生壓力。磁磚會輕微彎曲，從而產生機械應力。磁磚內部的壓電材料將這種應力轉化為電能。每一步都會產生少量電荷，而數百萬步結合在一起就能產生足夠的電力來驅動 LED燈、數位顯示器和感測器。在像澀谷車站這樣繁忙的地方，每天大約有240萬個腳步為此系統作出貢獻。這些電能可以被儲存或立即使用，從而減少對傳統電賴，並支持永續的城市基礎設施。這種方法將日常運動轉化為實用的再生能源。";

  let Ok(family) = FontFamily::from_str("Noto Sans TC") else {
    unreachable!()
  };

  let node = TextNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::font_size(Px(64.0).into()))
        .with_padding(Sides::from(Px(24.0)))
        .with(StyleDeclaration::font_family(family))
        .with(StyleDeclaration::text_overflow(TextOverflow::Ellipsis)),
    )
    .with_text(text.to_string());

  run_fixture_test(node.into(), "text_chinese_ellipsis");
}

#[test]
fn text_devanagari_noto_sans() {
  fn create_node(weight: f32, font_family: &str) -> TextNode {
    let text = "नमस्ते दुनिया, यह देवनागरी लिपि का एक परीक्षण है।";

    let Ok(family) = FontFamily::from_str(font_family) else {
      unreachable!()
    };

    TextNode::default()
      .with_style(
        Style::default()
          .with(StyleDeclaration::width(Percentage(100.0)))
          .with(StyleDeclaration::height(Percentage(100.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([240, 240, 240, 255]),
          )))
          .with(StyleDeclaration::font_size(Px(48.0).into()))
          .with_padding(Sides::from(Px(24.0)))
          .with(StyleDeclaration::font_family(family))
          .with(StyleDeclaration::font_weight(FontWeight::from(weight))),
      )
      .with_text(text)
  }

  let nodes: Vec<NodeKind> = [
    (400.0, "Noto Sans Devanagari"),
    (700.0, "Noto Sans Devanagari"),
    (400.0, "Poppins"),
    (700.0, "Poppins Bold"),
  ]
  .iter()
  .map(|(weight, font_family)| create_node(*weight, font_family).into())
  .collect::<Vec<_>>();

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with_padding(Sides([Px(20.0); 4]))
        .with_gap(SpacePair::from_single(Px(12.0))),
    )
    .with_children(nodes.into_boxed_slice());

  run_fixture_test(container.into(), "text_devanagari_noto_sans");
}
