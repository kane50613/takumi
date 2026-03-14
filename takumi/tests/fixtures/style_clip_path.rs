use takumi::layout::{
  node::{ContainerNode, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn clip_path_text_stroke_filled() {
  let text = "clip-path works in Takumi";

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([0, 0, 0, 255]),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::font_size(Px(84.0).into()))
        .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
        .with(StyleDeclaration::text_align(TextAlign::Center)),
    )
    .with_children([
      TextNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Block))
            .with(StyleDeclaration::position(Position::Absolute))
            .with(StyleDeclaration::top(Percentage(50.0)))
            .with(StyleDeclaration::left(Percentage(50.0)))
            .with(StyleDeclaration::translate(SpacePair::from_single(
              Percentage(-50.0),
            )))
            .with(StyleDeclaration::color(ColorInput::Value(Color::white())))
            .with(StyleDeclaration::clip_path(Some(
              BasicShape::from_str("inset(0 0 50% 0)").unwrap(),
            ))),
        )
        .with_text(text.to_string()),
      TextNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Block))
            .with(StyleDeclaration::position(Position::Absolute))
            .with(StyleDeclaration::top(Percentage(50.0)))
            .with(StyleDeclaration::left(Percentage(50.0)))
            .with(StyleDeclaration::translate(SpacePair::from_single(
              Percentage(-50.0),
            )))
            .with(StyleDeclaration::color(ColorInput::Value(
              Color::transparent(),
            )))
            .with(StyleDeclaration::webkit_text_stroke_width(Some(Px(2.0))))
            .with(StyleDeclaration::webkit_text_stroke_color(Some(
              ColorInput::Value(Color([128, 128, 128, 255])),
            )))
            .with(StyleDeclaration::clip_path(Some(
              BasicShape::from_str("inset(50% 0 0 0)").unwrap(),
            ))),
        )
        .with_text(text.to_string()),
    ]);

  run_fixture_test(container.into(), "clip_path_text_stroke_filled");
}

// Triangle clip-path similar to Vercel logo using polygon
#[test]
fn clip_path_triangle_vercel() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 255, 255]),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column)),
    )
    .with_children([
      // Triangle with clip-path
      ContainerNode::default().with_style(
        Style::default()
          .with(StyleDeclaration::width(Px(128.0)))
          .with(StyleDeclaration::height(Px(128.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color::black(),
          )))
          .with(StyleDeclaration::clip_path(Some(
            BasicShape::from_str("polygon(0% 100%, 100% 100%, 50% 12.25%)").unwrap(),
          ))),
      ),
    ]);

  run_fixture_test(container.into(), "clip_path_triangle_vercel");
}

// Alternative triangle with gradient background to show clipping more clearly
#[test]
fn clip_path_triangle_gradient() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 255, 255]),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column)),
    )
    .with_children([
      // Triangle with gradient background and clip-path
      ContainerNode::default().with_style(
        Style::default()
          .with(StyleDeclaration::width(Px(300.0)))
          .with(StyleDeclaration::height(Px(300.0)))
          .with(StyleDeclaration::background_image(Some(
            BackgroundImages::from_str(
              "linear-gradient(45deg, #ff3b30, #ff9500, #ffcc00, #34c759, #007aff, #5856d6)",
            )
            .unwrap(),
          )))
          .with(StyleDeclaration::clip_path(Some(
            BasicShape::from_str("polygon(0% 100%, 100% 100%, 50% 12.25%)").unwrap(),
          ))),
      ),
    ]);

  run_fixture_test(container.into(), "clip_path_triangle_gradient");
}

// Circle clip-path test
#[test]
fn clip_path_circle() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 255, 255]),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column)),
    )
    .with_children([
      // Circle with clip-path
      ContainerNode::default().with_style(
        Style::default()
          .with(StyleDeclaration::width(Px(200.0)))
          .with(StyleDeclaration::height(Px(200.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([255, 0, 100, 255]),
          )))
          .with(StyleDeclaration::clip_path(Some(
            BasicShape::from_str("circle(50%)").unwrap(),
          ))),
      ),
    ]);

  run_fixture_test(container.into(), "clip_path_circle");
}

// Inset with border radius clip-path test
#[test]
fn clip_path_inset_rounded() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 255, 255]),
        )))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column)),
    )
    .with_children([
      // Inset with border radius and clip-path
      ContainerNode::default().with_style(
        Style::default()
          .with(StyleDeclaration::width(Px(200.0)))
          .with(StyleDeclaration::height(Px(200.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([100, 200, 255, 255]),
          )))
          .with(StyleDeclaration::clip_path(Some(
            BasicShape::from_str("inset(50px 0 round 20px)").unwrap(),
          ))),
      ),
    ]);

  run_fixture_test(container.into(), "clip_path_inset_rounded");
}

// Test: clip-path on parent clips absolutely-positioned children
#[test]
fn clip_path_inset_round_clips_children() {
  // Outer wrapper (white background, defines canvas)
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        ))),
    )
    .with_children([
      // Inner container with clip-path: inset(0px round 50px)
      ContainerNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::position(Position::Absolute))
            .with(StyleDeclaration::top(Px(0.0)))
            .with(StyleDeclaration::left(Px(0.0)))
            .with(StyleDeclaration::width(Percentage(100.0)))
            .with(StyleDeclaration::height(Percentage(100.0)))
            .with(StyleDeclaration::clip_path(Some(
              BasicShape::from_str("inset(0px round 50px)").unwrap(),
            )))
            .with(StyleDeclaration::background_color(ColorInput::Value(
              Color([0, 0, 0, 255]),
            ))),
        )
        .with_children([
          // Full-bleed red child — should be clipped to rounded rect
          ContainerNode::default().with_style(
            Style::default()
              .with(StyleDeclaration::position(Position::Absolute))
              .with(StyleDeclaration::top(Px(0.0)))
              .with(StyleDeclaration::left(Px(0.0)))
              .with(StyleDeclaration::width(Percentage(100.0)))
              .with(StyleDeclaration::height(Percentage(100.0)))
              .with(StyleDeclaration::background_color(ColorInput::Value(
                Color([255, 0, 0, 255]),
              ))),
          ),
        ]),
    ]);

  run_fixture_test(container.into(), "clip_path_inset_round_clips_children");
}
