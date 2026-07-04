use takumi::prelude::{Length::*, *};

use crate::test_utils::run_fixture_test;

fn create_overflow_fixture(overflows: SpacePair<Overflow>) -> Node {
  Node::container([
    Node::container([Node::image("assets/images/yeecord.png").with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(300.0)))
        .with(StyleDeclaration::height(Px(300.0)))
        .with_border_width(Sides([Px(4.0).into(); 4]))
        .with_border_style(Sides([BorderStyle::Solid; 4]))
        .with_border_color(Sides([ColorInput::Value(Color([0, 255, 0, 255])); 4])),
    )])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::width(Px(200.0)))
        .with(StyleDeclaration::height(Px(200.0)))
        .with_border_width(Sides([Px(4.0).into(); 4]))
        .with_border_style(Sides([BorderStyle::Solid; 4]))
        .with_border_color(Sides([Color([255, 0, 0, 255]).into(); 4]))
        .with_overflow(overflows),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center)),
  )
}

fn create_text_overflow_fixture(overflows: SpacePair<Overflow>) -> Node {
  Node::container([
    Node::container([
        Node::text("This is a very long text that should overflow the container and demonstrate text overflow behavior with a large font size of 4rem.".to_string())
          .with_style(
            Style::default().with(StyleDeclaration::display(Display::Flex))
              .with(StyleDeclaration::font_size(Rem(4.0).into()))
              .with(StyleDeclaration::color(ColorInput::Value(Color([0, 0, 0, 255]))))
              .with_border_width(Sides([Px(2.0).into(); 4]))
              .with_border_style(Sides([BorderStyle::Solid; 4]))
              .with_border_color(Sides([Color([255, 0, 0, 255]).into(); 4])),
          ),
      ])
      .with_style(
        Style::default().with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::display(Display::Block))
          .with(StyleDeclaration::width(Px(400.0)))
          .with(StyleDeclaration::height(Px(200.0)))
          .with_border_width(Sides([Px(4.0).into(); 4]))
          .with_border_style(Sides([BorderStyle::Solid; 4]))
          .with_border_color(Sides([Color([0, 0, 0, 255]).into(); 4]))
          .with_overflow(overflows),
      ),
  ])
  .with_style(Style::default().with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(Color::white())))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center)),)
}

#[derive(Clone, Copy)]
struct SplitPillSideStyle {
  color: Color,
  overflow: Overflow,
  rounded_full: bool,
}

fn create_split_pill_side(style: SplitPillSideStyle) -> Node {
  let side_style = Style::default()
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::width(Percentage(50.0)))
    .with(StyleDeclaration::height(Percentage(100.0)))
    .with_overflow(SpacePair::from_single(style.overflow));

  let side_style = if style.rounded_full {
    side_style.with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(9999.0)); 4])))
  } else {
    side_style
  };

  Node::container([Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        style.color,
      ))),
  )])
  .with_style(side_style)
}

fn create_split_pill_fixture(left: SplitPillSideStyle, right: SplitPillSideStyle) -> Node {
  Node::container([create_split_pill_side(left), create_split_pill_side(right)]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0))),
  )
}

fn create_split_pill_case(
  label: &str,
  left: SplitPillSideStyle,
  right: SplitPillSideStyle,
) -> Node {
  Node::container([
    Node::text(label.to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Px(16.0).into()))
        .with(StyleDeclaration::color(ColorInput::Value(Color([
          20, 20, 20, 255,
        ])))),
    ),
    Node::container([create_split_pill_fixture(left, right)]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::height(Px(220.0)))
        .with_border_width(Sides([Px(2.0).into(); 4]))
        .with_border_style(Sides([BorderStyle::Solid; 4]))
        .with_border_color(Sides([ColorInput::Value(Color([220, 220, 220, 255])); 4])),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with_padding(Sides([Px(12.0); 4]))
      .with_gap(SpacePair::from_single(Px(8.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([248, 250, 252, 255]),
      ))),
  )
}

fn create_overflow_issue_630_grid_fixture() -> Node {
  Node::container([
    create_split_pill_case(
      "#630 target: both halves hidden+rounded render",
      SplitPillSideStyle {
        color: Color([59, 130, 246, 255]),
        overflow: Overflow::Hidden,
        rounded_full: true,
      },
      SplitPillSideStyle {
        color: Color([239, 68, 68, 255]),
        overflow: Overflow::Hidden,
        rounded_full: true,
      },
    ),
    create_split_pill_case(
      "right-only target: right hidden+rounded renders",
      SplitPillSideStyle {
        color: Color([241, 245, 249, 255]),
        overflow: Overflow::Visible,
        rounded_full: false,
      },
      SplitPillSideStyle {
        color: Color([239, 68, 68, 255]),
        overflow: Overflow::Hidden,
        rounded_full: true,
      },
    ),
    create_split_pill_case(
      "left-only target: left hidden+rounded renders",
      SplitPillSideStyle {
        color: Color([59, 130, 246, 255]),
        overflow: Overflow::Hidden,
        rounded_full: true,
      },
      SplitPillSideStyle {
        color: Color([241, 245, 249, 255]),
        overflow: Overflow::Visible,
        rounded_full: false,
      },
    ),
    create_split_pill_case(
      "control: rounded without overflow-hidden",
      SplitPillSideStyle {
        color: Color([59, 130, 246, 255]),
        overflow: Overflow::Visible,
        rounded_full: true,
      },
      SplitPillSideStyle {
        color: Color([239, 68, 68, 255]),
        overflow: Overflow::Visible,
        rounded_full: true,
      },
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Grid))
      .with(StyleDeclaration::grid_template_columns(
        GridTemplateComponents::from_css_str("repeat(2, 1fr)").ok(),
      ))
      .with(StyleDeclaration::grid_template_rows(
        GridTemplateComponents::from_css_str("repeat(2, 1fr)").ok(),
      ))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  )
}

#[test]
fn test_style_overflow_visible() {
  let container = create_overflow_fixture(SpacePair::from_single(Overflow::Visible));

  run_fixture_test(container, "style_overflow_visible_image");
}

#[test]
fn test_overflow_hidden() {
  let container = create_overflow_fixture(SpacePair::from_single(Overflow::Hidden));

  run_fixture_test(container, "style_overflow_hidden_image");
}

#[test]
fn test_overflow_clip() {
  let container = create_overflow_fixture(SpacePair::from_single(Overflow::Clip));

  run_fixture_test(container, "style_overflow_clip_image");
}

#[test]
fn test_overflow_mixed_axes() {
  let container = create_overflow_fixture(SpacePair::from_pair(Overflow::Clip, Overflow::Visible));

  run_fixture_test(container, "style_overflow_clip_visible_image");
}

#[test]
fn test_text_overflow_visible() {
  let container = create_text_overflow_fixture(SpacePair::from_single(Overflow::Visible));

  run_fixture_test(container, "style_overflow_visible_text");
}

#[test]
fn test_text_overflow_hidden() {
  let container = create_text_overflow_fixture(SpacePair::from_single(Overflow::Hidden));

  run_fixture_test(container, "style_overflow_hidden_text");
}

#[test]
fn test_text_overflow_clip() {
  let container = create_text_overflow_fixture(SpacePair::from_single(Overflow::Clip));

  run_fixture_test(container, "style_overflow_clip_text");
}

#[test]
fn test_text_overflow_mixed_axes() {
  let container =
    create_text_overflow_fixture(SpacePair::from_pair(Overflow::Hidden, Overflow::Visible));

  run_fixture_test(container, "style_overflow_hidden_visible_text");
}

#[test]
// Regression coverage for issue #630:
// https://github.com/kane50613/takumi/issues/630
fn test_overflow_issue_630_grid() {
  let container = create_overflow_issue_630_grid_fixture();
  run_fixture_test(container, "style_overflow_issue_630_grid");
}
