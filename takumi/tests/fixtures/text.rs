use parley::FontVariation;
use swash::tag_from_bytes;
use takumi::layout::{
  node::Node,
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

// Basic text render with defaults
#[test]
fn text_basic() {
  let text = Node::text("The quick brown fox jumps over the lazy dog 12345".to_string())
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        ))),
    );

  run_fixture_test(text, "text_basic");
}

#[test]
fn text_typography_regular_24px() {
  let text = Node::text("Regular 24px".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::font_size(Px(24.0).into())),
  );

  run_fixture_test(text, "text_typography_regular_24px");
}

#[test]
fn text_typography_variable_width() {
  const WIDTHS: &[f32] = &[60.0, 100.0, 130.0];

  let nodes: Vec<Node> = WIDTHS
    .iter()
    .map(|width| {
      Node::text(format!(
        "Hello world, this is a test of the variable width font: {}%",
        width
      ))
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::font_variation_settings(Box::new([
            FontVariation {
              tag: tag_from_bytes(b"wdth"),
              value: *width,
            },
          ]))),
      )
    })
    .collect::<Vec<_>>();

  let Ok(family) = FontFamily::from_str("Archivo") else {
    unreachable!()
  };

  let container = Node::container(nodes.into_boxed_slice()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::font_family(family))
      .with(StyleDeclaration::font_size(Px(48.0).into()))
      .with(StyleDeclaration::flex_wrap(FlexWrap::Wrap))
      .with(StyleDeclaration::row_gap(Px(48.0)))
      .with(StyleDeclaration::width(Percentage(100.0))),
  );

  run_fixture_test(container, "text_typography_variable_width");
}

#[test]
fn text_typography_variable_weight() {
  let nodes: Vec<Node> = (400..=900)
    .step_by(50)
    .map(|weight| {
      Node::text(weight.to_string()).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::font_size(Px(48.0).into()))
          .with(StyleDeclaration::font_weight(FontWeight::from(
            weight as f32,
          ))),
      )
    })
    .collect();

  let container = Node::container(nodes.into_boxed_slice()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::font_size(Px(24.0).into()))
      .with_gap(SpacePair::from_reversed_pair(Px(0.0), Px(24.0)))
      .with(StyleDeclaration::flex_wrap(FlexWrap::Wrap)),
  );

  run_fixture_test(container, "text_typography_variable_weight");
}

#[test]
fn text_typography_medium_weight_500() {
  let text = Node::text("Medium 24px".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::font_size(Px(24.0).into()))
      .with(StyleDeclaration::font_weight(FontWeight::from(500.0))),
  );

  run_fixture_test(text, "text_typography_medium_weight_500");
}

#[test]
fn text_typography_line_height_40px() {
  let text = Node::text("Line height 40px".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::font_size(Px(24.0).into()))
      .with(StyleDeclaration::line_height(LineHeight::Length(Px(40.0)))),
  );

  run_fixture_test(text, "text_typography_line_height_40px");
}

#[test]
fn text_typography_letter_spacing_2px() {
  let text = Node::text("Letter spacing 2px".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::font_size(Px(24.0).into()))
      .with(StyleDeclaration::letter_spacing(Px(2.0))),
  );

  run_fixture_test(text, "text_typography_letter_spacing_2px");
}

#[test]
fn text_align_start() {
  let text = Node::text("Start aligned".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::font_size(Px(24.0).into()))
      .with(StyleDeclaration::text_align(TextAlign::Start)),
  );

  run_fixture_test(text, "text_align_start");
}

#[test]
fn text_align_center() {
  let text = Node::text("Center aligned".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::font_size(Px(24.0).into()))
      .with(StyleDeclaration::text_align(TextAlign::Center)),
  );

  run_fixture_test(text, "text_align_center");
}

#[test]
fn text_align_center_in_block_container() {
  let paragraph = Node::text("This line should align center.".to_string())
    .with_tag_name("p")
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::font_size(Px(48.0).into())),
    );

  let container = Node::container([paragraph])
    .with_tag_name("div")
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::text_align(TextAlign::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        ))),
    );

  run_fixture_test(container, "text_align_center_chinese_in_block_container");
}

#[test]
fn text_align_right() {
  let text = Node::text("Right aligned".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::font_size(Px(24.0).into()))
      .with(StyleDeclaration::text_align(TextAlign::Right)),
  );

  run_fixture_test(text, "text_align_right");
}

#[test]
fn text_ellipsis_line_clamp_2() {
  let long_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.";

  let text = Node::text(long_text.to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::font_size(Px(48.0).into()))
      .with(StyleDeclaration::text_overflow(TextOverflow::Ellipsis))
      .with(StyleDeclaration::line_clamp(Some(2.into()))),
  );

  run_fixture_test(text, "text_ellipsis_line_clamp_2");
}

#[test]
fn text_transform_all() {
  let container = Node::container([
    Node::text("None: The quick Brown Fox".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::font_size(Px(28.0).into()))
        .with(StyleDeclaration::text_transform(TextTransform::None)),
    ),
    Node::text("Uppercase: The quick Brown Fox".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::font_size(Px(28.0).into()))
        .with(StyleDeclaration::text_transform(TextTransform::Uppercase)),
    ),
    Node::text("Lowercase: The QUICK Brown FOX".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::font_size(Px(28.0).into()))
        .with(StyleDeclaration::text_transform(TextTransform::Lowercase)),
    ),
    Node::text("Capitalize: the quick brown fox".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::font_size(Px(28.0).into()))
        .with(StyleDeclaration::text_transform(TextTransform::Capitalize)),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      ))),
  );

  run_fixture_test(container, "text_transform_all");
}

#[test]
fn text_mask_image_gradient_and_emoji() {
  let gradient_images = BackgroundImages::from_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let container = Node::container([Node::text("Gradient Mask Emoji: 🪓 🦊 💩".to_string())
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
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
    )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::font_size(Px(72.0).into()))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center)),
  );

  run_fixture_test(container, "text_mask_image_gradient_emoji");
}

#[test]
fn text_stroke_black_red() {
  let text = Node::text("Red Stroke".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
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
  );

  run_fixture_test(text, "text_stroke_black_red");
}

#[test]
fn text_stroke_background_clip() {
  let gradient_images = BackgroundImages::from_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let text = Node::text("Gradient Stroke".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
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
  );

  let container = Node::container([text]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center)),
  );

  run_fixture_test(container, "text_stroke_background_clip");
}

// Text shadow fixture
#[test]
fn text_shadow() {
  // #ffcc00 1px 0 10px
  let shadows = [TextShadow::builder()
    .offset_x(Px(1.0))
    .blur_radius(Px(10.0))
    .color(ColorInput::Value(Color([255, 204, 0, 255])))
    .build()];

  let text = Node::text("Shadowed Text".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::font_size(Px(48.0).into()))
      .with(StyleDeclaration::text_shadow(Some(shadows.into()))),
  );

  run_fixture_test(text, "text_shadow");
}

#[test]
fn text_shadow_no_blur_radius() {
  // 5px 5px #558abb
  let shadows = [TextShadow::builder()
    .offset_x(Px(5.0))
    .offset_y(Px(5.0))
    .color(ColorInput::Value(Color([85, 138, 187, 255])))
    .build()];

  let text = Node::text("Shadowed Text".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::font_size(Px(72.0).into()))
      .with(StyleDeclaration::text_shadow(Some(shadows.into()))),
  );

  run_fixture_test(text, "text_shadow_no_blur_radius");
}

#[test]
fn text_wrap_nowrap() {
  let long_text = "This is a very long piece of text that should demonstrate text wrapping behavior when it exceeds the container width. The quick brown fox jumps over the lazy dog.";

  let container = Node::container([
    // Wrap text
    Node::text(format!("wrap: {}", long_text)).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::text_wrap_mode(TextWrapMode::Wrap)),
    ),
    Node::text(format!("nowrap: {}", long_text)).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap)),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
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
  );

  run_fixture_test(container, "text_wrap_nowrap");
}

#[test]
fn text_whitespace_collapse() {
  let container = Node::container([
    Node::text("collapse: Multiple    spaces   and\ttabs\t\tare    collapsed".to_string())
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::white_space_collapse(
            WhiteSpaceCollapse::Collapse,
          )),
      ),
    Node::text("preserve: Multiple    spaces   and\ttabs\t\tare    preserved".to_string())
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::white_space_collapse(
            WhiteSpaceCollapse::Preserve,
          )),
      ),
    Node::text("preserve-spaces: Multiple    spaces   preserved\nbut\nbreaks\nremoved".to_string())
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::white_space_collapse(
            WhiteSpaceCollapse::PreserveSpaces,
          )),
      ),
    Node::text("preserve-breaks: Spaces    collapsed\n but\nline\nbreaks\npreserved".to_string())
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::white_space_collapse(
            WhiteSpaceCollapse::PreserveBreaks,
          )),
      ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
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
  );

  run_fixture_test(container, "text_whitespace_collapse");
}

/// Handles special case where nowrap + ellipsis is used.
#[test]
fn text_ellipsis_text_nowrap() {
  let container = Node::container([
      Node::text("This is a very long piece of text that should demonstrate text wrapping behavior when it exceeds the container width. The quick brown fox jumps over the lazy dog.".to_string())
  .with_style(Style::default().with(StyleDeclaration::display(Display::Flex))
            .with(StyleDeclaration::text_overflow(TextOverflow::Ellipsis))
            .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap))
            .with_border_width(Sides([Px(1.0); 4]))
            .with(StyleDeclaration::border_style(BorderStyle::Solid))
            .with(StyleDeclaration::border_color(ColorInput::Value(Color([255, 0, 0, 255]))))
            .with(StyleDeclaration::word_break(WordBreak::BreakAll))
            .with(StyleDeclaration::width(Percentage(100.0))),)

    ])
  .with_style(Style::default().with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::background_color(ColorInput::Value(Color([240, 240, 240, 255]))))
        .with(StyleDeclaration::font_size(Px(48.0).into()))
        .with_padding(Sides([Px(20.0); 4]))
        .with_overflow(SpacePair::from_single(Overflow::Hidden))
        .with(StyleDeclaration::width(Percentage(100.0))),);

  run_fixture_test(container, "text_ellipsis_text_nowrap");
}

#[test]
fn text_wrap_style_all() {
  let children: Vec<Node> = vec![
    Node::text("Auto: The quick brown fox jumps over the lazy dog.".to_string())
      .with_style(Style::default().with(StyleDeclaration::text_wrap_style(TextWrapStyle::Auto))),
    Node::text("Balance: The quick brown fox jumps over the lazy dog.".to_string())
      .with_style(Style::default().with(StyleDeclaration::text_wrap_style(TextWrapStyle::Balance))),
    Node::text("Pretty: The quick brown fox jumps over the lazy dog and catches it.".to_string())
      .with_style(Style::default().with(StyleDeclaration::text_wrap_style(TextWrapStyle::Pretty))),
  ];

  let container = Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
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
  );

  run_fixture_test(container, "text_wrap_style_all");
}

#[test]
fn text_super_bold_stroke_background_clip() {
  let gradient_images = BackgroundImages::from_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let text = Node::text("Super Bold".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
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
  );

  let container = Node::container([text]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center)),
  );

  run_fixture_test(container, "text_super_bold_stroke_background_clip");
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

  let nodes: Vec<Node> = stretches
    .iter()
    .map(|(label, stretch)| {
      Node::text(format!("font-stretch: {}", label)).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::font_size(Px(36.0).into()))
          .with(StyleDeclaration::font_stretch(*stretch)),
      )
    })
    .collect::<Vec<_>>();

  let Ok(family) = FontFamily::from_str("Archivo") else {
    unreachable!()
  };

  let container = Node::container(nodes.into_boxed_slice()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::font_family(family))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with_padding(Sides([Px(20.0); 4]))
      .with_gap(SpacePair::from_single(Px(12.0))),
  );

  run_fixture_test(container, "text_font_stretch");
}

#[test]
fn text_flex_centered_text_node_vs_nested_container() {
  let first_box_text: Node = Node::text("centered...?".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
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
  );

  let second_box_nested_text: Node = Node::container([Node::text("centered".to_string())])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(300.0)))
        .with(StyleDeclaration::height(Px(200.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::from_str("#ab82f6").unwrap(),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::font_size(Px(30.0).into())),
    );

  let root = Node::container([Node::container([first_box_text, second_box_nested_text])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::color(ColorInput::Value(Color::white()))),
    )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::black(),
      )))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center)),
  );

  run_fixture_test(root, "text_flex_centered_text_node_vs_nested_container");
}

#[test]
fn text_font_synthesis_weight_auto_none() {
  let Ok(family) = FontFamily::from_str("Scheherazade New Test") else {
    unreachable!()
  };

  let nodes: Vec<Node> = [("auto", FontSynthesic::Auto), ("none", FontSynthesic::None)]
    .iter()
    .map(|(label, synthesis_weight)| {
      Node::text(format!("font-synthesis-weight: {} - السلام عليكم", label)).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::font_size(Px(72.0).into()))
          .with(StyleDeclaration::font_family(family.clone()))
          .with(StyleDeclaration::font_weight(FontWeight::from(900.0)))
          .with(StyleDeclaration::font_synthesis_weight(*synthesis_weight)),
      )
    })
    .collect::<Vec<_>>();

  let container = Node::container(nodes.into_boxed_slice()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with_padding(Sides([Px(20.0); 4]))
      .with_gap(SpacePair::from_single(Px(12.0))),
  );

  run_fixture_test(container, "text_font_synthesis_weight_auto_none");
}

#[test]
fn text_font_synthesis_style_auto_none() {
  let Ok(family) = FontFamily::from_str("Scheherazade New Test") else {
    unreachable!()
  };

  let nodes: Vec<Node> = [("auto", FontSynthesic::Auto), ("none", FontSynthesic::None)]
    .iter()
    .map(|(label, synthesis_style)| {
      Node::text(format!("font-synthesis-style: {} - السلام عليكم", label)).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::font_size(Px(72.0).into()))
          .with(StyleDeclaration::font_family(family.clone()))
          .with(StyleDeclaration::font_style(FontStyle::italic()))
          .with(StyleDeclaration::font_synthesis_style(*synthesis_style)),
      )
    })
    .collect::<Vec<_>>();

  let container = Node::container(nodes.into_boxed_slice()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with_padding(Sides([Px(20.0); 4]))
      .with_gap(SpacePair::from_single(Px(12.0))),
  );

  run_fixture_test(container, "text_font_synthesis_style_auto_none");
}

#[test]
fn text_font_synthesis_weight_emoji() {
  let Ok(family) = FontFamily::from_str("Scheherazade New Test") else {
    unreachable!()
  };

  let nodes: Vec<Node> = [
    ("auto", FontSynthesis::default()),
    (
      "none",
      FontSynthesis::builder()
        .weight(FontSynthesic::None)
        .style(FontSynthesic::None)
        .build(),
    ),
  ]
  .iter()
  .map(|(label, synthesis)| {
    Node::text(format!("font-synthesis: {} - Takumi 😀 😺 🧪", label)).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Px(72.0).into()))
        .with(StyleDeclaration::font_family(family.clone()))
        .with(StyleDeclaration::font_weight(FontWeight::from(900.0)))
        .with(StyleDeclaration::font_style(FontStyle::italic()))
        .with_font_synthesis(*synthesis),
    )
  })
  .collect::<Vec<_>>();

  let container = Node::container(nodes.into_boxed_slice()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with_padding(Sides([Px(20.0); 4]))
      .with_gap(SpacePair::from_single(Px(12.0))),
  );

  run_fixture_test(container, "text_font_synthesis_weight_emoji");
}

#[test]
fn text_chinese_ellipsis() {
  let text = "日本利用壓電磁磚將腳步轉化為電能。這些瓷磚捕捉來自你腳步的動能。當你行走時，你的重量和動作會對瓷磚產生壓力。磁磚會輕微彎曲，從而產生機械應力。磁磚內部的壓電材料將這種應力轉化為電能。每一步都會產生少量電荷，而數百萬步結合在一起就能產生足夠的電力來驅動 LED燈、數位顯示器和感測器。在像澀谷車站這樣繁忙的地方，每天大約有240萬個腳步為此系統作出貢獻。這些電能可以被儲存或立即使用，從而減少對傳統電賴，並支持永續的城市基礎設施。這種方法將日常運動轉化為實用的再生能源。";

  let Ok(family) = FontFamily::from_str("Noto Sans TC") else {
    unreachable!()
  };

  let node = Node::text(text.to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::font_size(Px(64.0).into()))
      .with_padding(Sides::from(Px(24.0)))
      .with(StyleDeclaration::font_family(family))
      .with(StyleDeclaration::text_overflow(TextOverflow::Ellipsis)),
  );

  run_fixture_test(node, "text_chinese_ellipsis");
}

#[test]
fn text_devanagari_noto_sans() {
  fn create_node(weight: f32, font_family: &str) -> Node {
    let text = "नमस्ते दुनिया, यह देवनागरी लिपि का एक परीक्षण है।";

    let Ok(family) = FontFamily::from_str(font_family) else {
      unreachable!()
    };

    Node::text(text).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
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
  }

  let nodes: Vec<Node> = [
    (400.0, "Noto Sans Devanagari"),
    (700.0, "Noto Sans Devanagari"),
    (400.0, "Poppins"),
    (700.0, "Poppins Bold"),
  ]
  .iter()
  .map(|(weight, font_family)| create_node(*weight, font_family))
  .collect::<Vec<_>>();

  let container = Node::container(nodes.into_boxed_slice()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with_padding(Sides([Px(20.0); 4]))
      .with_gap(SpacePair::from_single(Px(12.0))),
  );

  run_fixture_test(container, "text_devanagari_noto_sans");
}
