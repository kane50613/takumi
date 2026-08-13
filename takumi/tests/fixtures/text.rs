use serde_json::{from_value, json};
use takumi::{
  prelude::{Length::*, *},
  render,
};

use crate::test_utils::{CONTEXT, run_fixture_test};

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
            FontVariation::new(Tag::new(b"wdth"), *width),
          ]))),
      )
    })
    .collect::<Vec<_>>();

  let Ok(family) = FontFamily::from_css_str("Archivo") else {
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
      .with(StyleDeclaration::row_gap(Px(48.0).into()))
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
      .with_gap(SpacePair::from_pair(Px(0.0).into(), Px(24.0).into()))
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
fn text_typography_line_height_variants() {
  let sample = "Sphinx of black quartz,\njudge my vow.\nPack my box.";
  let variant = |label: &str, line_height: LineHeight, panel: Color| {
    Node::container([
      Node::text(label.to_string()).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Block))
          .with(StyleDeclaration::font_size(Px(18.0).into()))
          .with(StyleDeclaration::font_weight(FontWeight::from(600.0)))
          .with(StyleDeclaration::margin_bottom(Px(10.0))),
      ),
      Node::text(sample.to_string()).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Block))
          .with(StyleDeclaration::font_size(Px(24.0).into()))
          .with(StyleDeclaration::line_height(line_height))
          .with_white_space(WhiteSpace::pre_line()),
      ),
    ])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(panel)))
        .with_padding(Sides([Px(18.0); 4]))
        .with_border_width(Sides([Px(1.0).into(); 4]))
        .with_border_style(Sides([BorderStyle::Solid; 4]))
        .with_border_color(Sides([ColorInput::Value(Color([205, 214, 228, 255])); 4])),
    )
  };

  let text = Node::container([
    Node::text("Line Height Variants".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::font_size(Px(34.0).into()))
        .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
        .with(StyleDeclaration::margin_bottom(Px(18.0))),
    ),
    Node::container([
      variant(
        "Unitless 0.9",
        LineHeight::Unitless(0.9),
        Color([248, 250, 252, 255]),
      ),
      variant(
        "Length 32px",
        LineHeight::Length(Px(32.0)),
        Color([241, 245, 249, 255]),
      ),
      variant(
        "Length 40px",
        LineHeight::Length(Px(40.0)),
        Color([236, 242, 255, 255]),
      ),
      variant(
        "Length 56px",
        LineHeight::Length(Px(56.0)),
        Color([250, 245, 255, 255]),
      ),
    ])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with_gap(SpacePair::from_single(Px(16.0).into()))
        .with(StyleDeclaration::align_items(AlignItems::Stretch)),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([232, 236, 241, 255]),
      )))
      .with_padding(Sides([Px(28.0); 4]))
      .with(StyleDeclaration::color(ColorInput::Value(Color([
        15, 23, 42, 255,
      ])))),
  );

  run_fixture_test(text, "text_typography_line_height_variants");
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

const TEXT_FIT_CARD_WIDTH: Length = Px(258.0);
const TEXT_FIT_CARD_CONTENT_WIDTH: Length = Px(226.0);
const TEXT_FIT_CARD_MIN_HEIGHT: Length = Px(68.0);

fn text_fit_card_container(label: &str, content: Node) -> Node {
  Node::container([
    Node::text(label.to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::font_size(Px(14.0).into()))
        .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
        .with(StyleDeclaration::margin_bottom(Px(5.0))),
    ),
    content,
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(TEXT_FIT_CARD_WIDTH))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([229, 234, 240, 255]),
      )))
      .with_padding(Sides([Px(10.0); 4])),
  )
}

fn text_fit_text_style(text_fit: TextFit) -> Style {
  Style::default()
    .with(StyleDeclaration::display(Display::Block))
    .with(StyleDeclaration::width(TEXT_FIT_CARD_CONTENT_WIDTH))
    .with(StyleDeclaration::min_height(TEXT_FIT_CARD_MIN_HEIGHT))
    .with(StyleDeclaration::font_size(Px(26.0).into()))
    .with(StyleDeclaration::line_height(LineHeight::Unitless(1.0)))
    .with(StyleDeclaration::text_fit(text_fit))
    .with(StyleDeclaration::white_space_collapse(
      WhiteSpaceCollapse::PreserveBreaks,
    ))
    .with(StyleDeclaration::background_color(ColorInput::Value(
      Color([255, 255, 255, 255]),
    )))
    .with_padding(Sides([Px(8.0); 4]))
}

fn text_fit_text_card(label: &str, content: &str, text_fit: TextFit) -> Node {
  text_fit_card_container(
    label,
    Node::text(content.to_string()).with_style(text_fit_text_style(text_fit)),
  )
}

fn text_fit_text_card_with_style(label: &str, content: &str, style: Style) -> Node {
  text_fit_card_container(label, Node::text(content.to_string()).with_style(style))
}

fn text_fit(mode: TextFitMode, target: TextFitTarget, limit: Option<f32>) -> TextFit {
  TextFit::builder()
    .mode(mode)
    .target(target)
    .limit(limit)
    .build()
}

fn text_fit_overview_container(cards: impl Into<Box<[Node]>>) -> Node {
  Node::container(cards.into()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_wrap(FlexWrap::Wrap))
      .with_gap(SpacePair::from_single(Px(14.0).into()))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([246, 248, 251, 255]),
      )))
      .with_padding(Sides([Px(18.0); 4])),
  )
}

#[test]
fn text_fit_overview() {
  let image = || {
    Node::image(("assets/images/yeecord.png", 64.0, 64.0)).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::InlineBlock))
        .with(StyleDeclaration::width(Em(1.0)))
        .with(StyleDeclaration::height(Em(1.0)))
        .with(StyleDeclaration::vertical_align(VerticalAlign::Keyword(
          VerticalAlignKeyword::Middle,
        ))),
    )
  };
  let shadows = [TextShadow::builder()
    .offset_x(Px(1.0))
    .offset_y(Px(2.0))
    .blur_radius(Px(6.0))
    .color(ColorInput::Value(Color([69, 85, 110, 180])))
    .build()];
  let decorated_fit = text_fit(TextFitMode::Grow, TextFitTarget::Consistent, Some(2.25));

  let container = text_fit_overview_container([
    text_fit_text_card_with_style(
      "none",
      "No fit",
      text_fit_text_style(TextFit::default())
        .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap)),
    ),
    text_fit_text_card_with_style(
      "grow",
      "Quick note",
      text_fit_text_style(text_fit(TextFitMode::Grow, TextFitTarget::Consistent, None))
        .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap)),
    ),
    text_fit_text_card_with_style(
      "grow 150%",
      "Quick note",
      text_fit_text_style(text_fit(
        TextFitMode::Grow,
        TextFitTarget::Consistent,
        Some(1.5),
      ))
      .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap)),
    ),
    text_fit_text_card_with_style(
      "shrink",
      "This headline is intentionally long",
      text_fit_text_style(text_fit(
        TextFitMode::Shrink,
        TextFitTarget::Consistent,
        None,
      ))
      .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap)),
    ),
    text_fit_text_card_with_style(
      "shrink min 80%",
      "This headline is intentionally long",
      text_fit_text_style(text_fit(
        TextFitMode::Shrink,
        TextFitTarget::Consistent,
        Some(0.8),
      ))
      .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap)),
    ),
    text_fit_text_card(
      "per-line",
      "Short\nA much longer line",
      text_fit(TextFitMode::Grow, TextFitTarget::PerLine, Some(1.8)),
    ),
    text_fit_text_card(
      "per-line-all",
      "Short\nA much longer line",
      text_fit(TextFitMode::Grow, TextFitTarget::PerLineAll, Some(1.8)),
    ),
    text_fit_card_container(
      "mixed inline",
      Node::container([
        Node::text("Ship ".to_string())
          .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
        image(),
        Node::text(" now".to_string())
          .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
      ])
      .with_style(
        text_fit_text_style(decorated_fit)
          .with_white_space(WhiteSpace::pre())
          .with_text_decoration(
            TextDecoration::builder()
              .line(TextDecorationLines::UNDERLINE)
              .color(ColorInput::Value(Color([49, 130, 206, 255])))
              .build(),
          )
          .with(StyleDeclaration::text_shadow(Some(shadows.into()))),
      ),
    ),
    text_fit_text_card(
      "consistent multiline",
      "Tiny\nA much longer line\nEnd",
      text_fit(TextFitMode::Grow, TextFitTarget::Consistent, Some(1.6)),
    ),
    text_fit_text_card(
      "per-line wrapped",
      "Short\nA much longer line that wraps again",
      text_fit(TextFitMode::Grow, TextFitTarget::PerLine, Some(1.8)),
    ),
    text_fit_text_card(
      "per-line-all wrapped",
      "Short\nA much longer line that wraps again",
      text_fit(TextFitMode::Grow, TextFitTarget::PerLineAll, Some(1.8)),
    ),
    text_fit_text_card(
      "grow paragraph",
      "Tiny intro\nThis paragraph should grow without forcing a single line",
      text_fit(TextFitMode::Grow, TextFitTarget::Consistent, Some(1.4)),
    ),
  ]);

  run_fixture_test(container, "text_fit_overview");
}

#[test]
fn text_fit_line_height_behavior() {
  let sample = "Short\nA much longer line";
  let fit = text_fit(TextFitMode::Grow, TextFitTarget::PerLineAll, Some(1.8));
  let card = |label: &str, line_height: LineHeight, panel: Color| {
    text_fit_text_card_with_style(
      label,
      sample,
      text_fit_text_style(fit)
        .with(StyleDeclaration::line_height(line_height))
        .with(StyleDeclaration::background_color(ColorInput::Value(panel))),
    )
  };

  let container = text_fit_overview_container([
    card("normal", LineHeight::Normal, Color([255, 255, 255, 255])),
    card(
      "1.4",
      LineHeight::Unitless(1.4),
      Color([248, 250, 252, 255]),
    ),
    card(
      "1.5em (150%)",
      LineHeight::Length(Em(1.5)),
      Color([241, 245, 249, 255]),
    ),
    card(
      "40px",
      LineHeight::Length(Px(40.0)),
      Color([236, 242, 255, 255]),
    ),
  ]);

  run_fixture_test(container, "text_fit_line_height_behavior");
}

#[test]
fn text_fit_center_aligned_per_line_all() {
  let text = Node::text("Takumi 1.2 now support the latest.".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::font_size(Px(48.0).into()))
      .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
      .with(StyleDeclaration::text_align(TextAlign::Center))
      .with(StyleDeclaration::text_fit(text_fit(
        TextFitMode::Grow,
        TextFitTarget::PerLineAll,
        None,
      )))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      ))),
  );

  run_fixture_test(text, "text_fit_center_aligned_per_line_all");
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
fn text_indent_variants() {
  let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit.\nSed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";
  let variants = [
    ("first-line", TextIndent::new(Px(48.0))),
    ("each-line", TextIndent::new(Px(48.0)).with_each_line(true)),
    ("hanging", TextIndent::new(Px(48.0)).with_hanging(true)),
    (
      "hanging + each-line",
      TextIndent::new(Px(48.0))
        .with_each_line(true)
        .with_hanging(true),
    ),
  ];

  let nodes: Vec<Node> = variants
    .iter()
    .map(|(label, text_indent)| {
      Node::container([
        Node::text(label.to_string()).with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Flex))
            .with(StyleDeclaration::font_size(Px(24.0).into()))
            .with(StyleDeclaration::font_weight(FontWeight::from(700.0))),
        ),
        Node::text(text.to_string()).with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Flex))
            .with(StyleDeclaration::display(Display::Block))
            .with(StyleDeclaration::width(Px(380.0)))
            .with(StyleDeclaration::font_size(Px(28.0).into()))
            .with(StyleDeclaration::white_space_collapse(
              WhiteSpaceCollapse::PreserveBreaks,
            ))
            .with(StyleDeclaration::text_indent(*text_indent)),
        ),
      ])
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::flex_direction(FlexDirection::Column))
          .with(StyleDeclaration::width(Px(380.0)))
          .with_gap(SpacePair::from_single(Px(8.0).into())),
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
      .with(StyleDeclaration::flex_wrap(FlexWrap::Wrap))
      .with_padding(Sides([Px(20.0); 4]))
      .with_gap(SpacePair::from_single(Px(24.0).into())),
  );

  run_fixture_test(container, "text_indent_variants");
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
      .with_line_clamp(2u32.into()),
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
  let gradient_images = BackgroundImages::from_css_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let container = Node::container([Node::text("Gradient Mask Emoji: 🪓 🦊 💩".to_string())
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::background_image(Some(gradient_images)))
        .with(StyleDeclaration::background_size(
          BackgroundSizes::from_css_str("100% 100%").unwrap(),
        ))
        .with(StyleDeclaration::background_position(
          PositionValues::from_css_str("0 0").unwrap(),
        ))
        .with(StyleDeclaration::background_repeat(
          BackgroundRepeats::from_css_str("no-repeat").unwrap(),
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
  let gradient_images = BackgroundImages::from_css_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let text = Node::text("Gradient Stroke".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_image(Some(gradient_images)))
      .with(StyleDeclaration::background_position(
        PositionValues::from_css_str("center center").unwrap(),
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
      .with_gap(SpacePair::from_single(Px(20.0).into()))
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
      .with_gap(SpacePair::from_single(Px(20.0).into()))
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
            .with_border_width(Sides([Px(1.0).into(); 4]))
            .with_border_style(Sides([BorderStyle::Solid; 4]))
            .with_border_color(Sides([ColorInput::Value(Color([255, 0, 0, 255])); 4]))
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
      .with_gap(SpacePair::from_single(Px(40.0).into()))
      .with_padding(Sides([Px(20.0); 4])),
  );

  run_fixture_test(container, "text_wrap_style_all");
}

#[test]
fn text_super_bold_stroke_background_clip() {
  let gradient_images = BackgroundImages::from_css_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  let text = Node::text("Super Bold".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_image(Some(gradient_images)))
      .with(StyleDeclaration::background_position(
        PositionValues::from_css_str("center center").unwrap(),
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

/// Reproduces yeecord Quote.tsx layout where text fails to render with
/// `text-fit: grow` + `background-clip: text` + transparent webkit-text-stroke.
#[test]
fn text_fit_grow_bg_clip_transparent_stroke() {
  let root = from_value::<Node>(json!({
    "type": "container",
    "style": {
      "width": "100%",
      "height": "100%",
      "display": "flex",
      "justifyContent": "center",
      "alignItems": "center",
      "fontFamily": "Noto Sans TC",
      "backgroundColor": "rgb(186, 169, 207)"
    },
    "children": [
      {
        "type": "container",
        "style": {
          "display": "block",
          "textOverflow": "ellipsis",
          "textAlign": "center",
          "backgroundColor": "black",
          "backgroundClip": "text",
          "webkitTextStrokeWidth": "12px",
          "webkitTextStrokeColor": "transparent",
          "width": "100%",
          "textFit": "grow",
          "fontSize": "60px",
          "lineHeight": 1.5,
          "fontWeight": 700,
          "color": "white",
        },
        "children": [
          {
            "type": "text",
            "text": "Goo goo ga ga",
          }
        ]
      }
    ]
  }))
  .unwrap();

  run_fixture_test(root, "text_fit_grow_bg_clip_transparent_stroke");
}

#[test]
fn text_font_stretch() {
  let stretches = [
    (
      "ultra-condensed",
      FontStretch::from_css_str("ultra-condensed").unwrap(),
    ),
    ("condensed", FontStretch::from_css_str("condensed").unwrap()),
    (
      "semi-condensed",
      FontStretch::from_css_str("semi-condensed").unwrap(),
    ),
    ("normal", FontStretch::from_css_str("normal").unwrap()),
    (
      "semi-expanded",
      FontStretch::from_css_str("semi-expanded").unwrap(),
    ),
    ("expanded", FontStretch::from_css_str("expanded").unwrap()),
    (
      "ultra-expanded",
      FontStretch::from_css_str("ultra-expanded").unwrap(),
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

  let Ok(family) = FontFamily::from_css_str("Archivo") else {
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
      .with_gap(SpacePair::from_single(Px(12.0).into())),
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
        Color::from_css_str("#3b82f6").unwrap(),
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
          Color::from_css_str("#ab82f6").unwrap(),
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
  let Ok(family) = FontFamily::from_css_str("Scheherazade New Test") else {
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
      .with_gap(SpacePair::from_single(Px(12.0).into())),
  );

  run_fixture_test(container, "text_font_synthesis_weight_auto_none");
}

#[test]
fn text_font_synthesis_style_auto_none() {
  let Ok(family) = FontFamily::from_css_str("Scheherazade New Test") else {
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
      .with_gap(SpacePair::from_single(Px(12.0).into())),
  );

  run_fixture_test(container, "text_font_synthesis_style_auto_none");
}

#[test]
fn text_font_synthesis_weight_emoji() {
  let Ok(family) = FontFamily::from_css_str("Scheherazade New Test") else {
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
      .with_gap(SpacePair::from_single(Px(12.0).into())),
  );

  run_fixture_test(container, "text_font_synthesis_weight_emoji");
}

/// Variation selectors pick the font by presentation, not by stack order:
/// `U+FE0F` reaches the color font and `U+FE0E` the text font from either
/// order, while bare codepoints keep following the stack like browsers do.
#[test]
fn text_emoji_variation_selector() {
  let nodes: Vec<Node> = [
    "Noto Sans TC, Twemoji Mozilla",
    "Twemoji Mozilla, Noto Sans TC",
  ]
  .iter()
  .map(|stack| {
    let Ok(family) = FontFamily::from_css_str(stack) else {
      unreachable!()
    };

    Node::text("‼ ‼\u{FE0F} ‼\u{FE0E} 一").with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Px(96.0).into()))
        .with(StyleDeclaration::font_family(family)),
    )
  })
  .collect();

  let container = Node::container(nodes.into_boxed_slice()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with_padding(Sides([Px(20.0); 4]))
      .with_gap(SpacePair::from_single(Px(12.0).into())),
  );

  run_fixture_test(container, "text_emoji_variation_selector");
}

#[test]
fn text_chinese_ellipsis() {
  let text = "日本利用壓電磁磚將腳步轉化為電能。這些瓷磚捕捉來自你腳步的動能。當你行走時，你的重量和動作會對瓷磚產生壓力。磁磚會輕微彎曲，從而產生機械應力。磁磚內部的壓電材料將這種應力轉化為電能。每一步都會產生少量電荷，而數百萬步結合在一起就能產生足夠的電力來驅動 LED燈、數位顯示器和感測器。在像澀谷車站這樣繁忙的地方，每天大約有240萬個腳步為此系統作出貢獻。這些電能可以被儲存或立即使用，從而減少對傳統電賴，並支持永續的城市基礎設施。這種方法將日常運動轉化為實用的再生能源。";

  let Ok(family) = FontFamily::from_css_str("Noto Sans TC") else {
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

    let Ok(family) = FontFamily::from_css_str(font_family) else {
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
      .with_gap(SpacePair::from_single(Px(12.0).into())),
  );

  run_fixture_test(container, "text_devanagari_noto_sans");
}

#[test]
fn text_group_opacity_small_mono_regression() {
  let root = from_value::<Node>(json!({
    "type": "container",
    "style": {
      "width": "100%",
      "height": "100%",
      "display": "flex",
      "alignItems": "center",
      "justifyContent": "center",
      "backgroundColor": "rgb(2, 6, 23)"
    },
    "children": [
      {
        "type": "container",
        "style": {
          "display": "flex",
          "flexDirection": "column",
          "alignItems": "center"
        },
        "children": [
          {
            "type": "text",
            "text": "opacity + color opacity",
            "style": {
              "fontFamily": "Geist Mono",
              "fontSize": "28px",
              "fontWeight": 700,
              "color": "rgb(248, 250, 252)",
              "marginBottom": "18px"
            }
          },
          {
            "type": "container",
            "style": {
              "width": "340px",
              "display": "flex",
              "flexDirection": "column",
              "gap": "2px",
              "opacity": 0.3,
              "overflow": "hidden",
              "fontSize": "1.25rem",
              "lineHeight": 1.45,
              "color": "rgba(226, 232, 240, 0.95)",
              "fontFamily": "Geist Mono"
            },
            "children": [
              {
                "type": "container",
                "tagName": "div",
                "preset": { "display": "block" },
                "style": {
                  "display": "flex",
                  "alignItems": "center",
                  "paddingLeft": "0"
                },
                "children": [
                  {
                    "type": "text",
                    "text": "Functions",
                    "tagName": "span",
                    "style": {
                      "fontSize": "1.05rem"
                    }
                  }
                ]
              },
              {
                "type": "container",
                "tagName": "div",
                "preset": { "display": "block" },
                "style": {
                  "display": "flex",
                  "alignItems": "center",
                  "paddingLeft": "20px"
                },
                "children": [
                  {
                    "type": "text",
                    "text": "render",
                    "tagName": "span",
                    "style": {
                      "fontSize": "1.2rem"
                    }
                  }
                ]
              },
              {
                "type": "container",
                "tagName": "div",
                "preset": { "display": "block" },
                "style": {
                  "display": "flex",
                  "alignItems": "center",
                  "paddingLeft": "20px"
                },
                "children": [
                  {
                    "type": "text",
                    "text": "loadFont",
                    "tagName": "span",
                    "style": {
                      "fontSize": "1.2rem"
                    }
                  }
                ]
              },
              {
                "type": "container",
                "tagName": "div",
                "preset": { "display": "block" },
                "style": {
                  "display": "flex",
                  "alignItems": "center",
                  "paddingLeft": "0"
                },
                "children": [
                  {
                    "type": "text",
                    "text": "Classes",
                    "tagName": "span",
                    "style": {
                      "fontSize": "1.05rem"
                    }
                  }
                ]
              },
              {
                "type": "container",
                "tagName": "div",
                "preset": { "display": "block" },
                "style": {
                  "display": "flex",
                  "alignItems": "center",
                  "paddingLeft": "20px"
                },
                "children": [
                  {
                    "type": "text",
                    "text": "Renderer",
                    "tagName": "span",
                    "style": {
                      "fontSize": "1.2rem"
                    }
                  }
                ]
              }
            ]
          }
        ]
      }
    ]
  }))
  .unwrap();

  run_fixture_test(root, "text_group_opacity_small_mono_regression");
}

const ELLIPSIS_CANVAS_WIDTH: u32 = 480;
const ELLIPSIS_CANVAS_HEIGHT: u32 = 200;

fn rightmost_dark_column(image: &Bitmap) -> u32 {
  let width = image.width();

  image
    .as_raw()
    .chunks_exact(4)
    .enumerate()
    .filter_map(|(index, pixel)| {
      let dark = pixel[3] > 0 && pixel[0].min(pixel[1]).min(pixel[2]) < 160;
      dark.then(|| index as u32 % width)
    })
    .max()
    .unwrap_or(0)
}

fn unbreakable_ellipsis_root(text_overflow: TextOverflow) -> Node {
  let text = Node::text("gsijdsoifgdhaetlelwtyuxxxxxx".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::text_overflow(text_overflow))
      .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap))
      .with_overflow(SpacePair::from_single(Overflow::Hidden))
      .with(StyleDeclaration::width(Length::Px(360.0)))
      .with(StyleDeclaration::color(ColorInput::Value(Color::black()))),
  );

  Node::container([text]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::font_size(Length::Px(48.0).into()))
      .with(StyleDeclaration::width(Length::Px(
        ELLIPSIS_CANVAS_WIDTH as f32,
      )))
      .with(StyleDeclaration::height(Length::Px(
        ELLIPSIS_CANVAS_HEIGHT as f32,
      )))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  )
}

/// A single unbreakable token has no break opportunity, but browsers still
/// ellipsize it at a cluster boundary. The clipped variant runs ink to the
/// box edge; the ellipsis variant must stop short of it.
#[test]
fn test_nowrap_ellipsis_without_break_opportunity() {
  let clipped = render(
    RenderOptions::builder()
      .viewport(Viewport::new((
        ELLIPSIS_CANVAS_WIDTH,
        ELLIPSIS_CANVAS_HEIGHT,
      )))
      .node(unbreakable_ellipsis_root(TextOverflow::Clip))
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap();
  let ellipsized = render(
    RenderOptions::builder()
      .viewport(Viewport::new((
        ELLIPSIS_CANVAS_WIDTH,
        ELLIPSIS_CANVAS_HEIGHT,
      )))
      .node(unbreakable_ellipsis_root(TextOverflow::Ellipsis))
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap();

  run_fixture_test(
    unbreakable_ellipsis_root(TextOverflow::Ellipsis),
    "text_ellipsis_nowrap_unbreakable",
  );

  let clipped_right = rightmost_dark_column(&clipped);
  let ellipsized_right = rightmost_dark_column(&ellipsized);

  assert!(
    clipped_right >= 355,
    "expected the clip variant to run ink to the box edge, rightmost ink at x={clipped_right}",
  );
  assert!(
    ellipsized_right < 355,
    "expected the ellipsis variant to stop short of the box edge, rightmost ink at x={ellipsized_right}",
  );
  assert_ne!(clipped.as_raw(), ellipsized.as_raw());
}

fn kerning_row(label: &str, kerning: FontKerning, features: Box<[FontFeature]>) -> Node {
  Node::container([
    Node::text(label.to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Px(16.0).into()))
        .with(StyleDeclaration::width(Px(120.0))),
    ),
    Node::text("AVAVAWAY To Ta.".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_kerning(kerning))
        .with(StyleDeclaration::font_feature_settings(features)),
    ),
  ])
  .with_style(Style::default().with(StyleDeclaration::display(Display::Flex)))
}

#[test]
fn text_font_kerning() {
  let container = Node::container([
    kerning_row("auto", FontKerning::Auto, Box::new([])),
    kerning_row("none", FontKerning::None, Box::new([])),
    kerning_row(
      "none+fss",
      FontKerning::None,
      Box::new([FontFeature::new(Tag::new(b"kern"), 1)]),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::font_size(Px(40.0).into()))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      ))),
  );

  run_fixture_test(container, "text_font_kerning");
}

fn tab_size_block(label: &str, tab_size: Option<TabSize>) -> Node {
  let Ok(family) = FontFamily::from_css_str("Geist Mono") else {
    unreachable!()
  };

  let mut style = Style::default()
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::font_family(family))
    .with(StyleDeclaration::font_size(Px(20.0).into()))
    .with_white_space(WhiteSpace::pre());

  if let Some(tab_size) = tab_size {
    style = style.with(StyleDeclaration::tab_size(tab_size));
  }

  Node::container([
    Node::text(label.to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Px(16.0).into())),
    ),
    Node::text("fn main() {\n\tlet x = 1;\n\t\tnested\n}".to_string()).with_style(style),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column)),
  )
}

#[test]
fn text_tab_size_pre() {
  let container = Node::container([
    tab_size_block("default", None),
    tab_size_block("tab-size: 2", Some(TabSize::from(2.0))),
    tab_size_block("tab-size: 8", Some(TabSize::from(8.0))),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 240, 240, 255]),
      ))),
  );

  run_fixture_test(container, "text_tab_size_pre");
}
