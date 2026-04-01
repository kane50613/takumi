use takumi::layout::{
  node::Node,
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;
use std::sync::Arc;

/// Creates a single card with solid blocks and mix-blend-mode for testing.
fn create_blend_card(mode: BlendMode) -> Node {
  let foreground = Node::container([
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(120.0)))
        .with(StyleDeclaration::height(Px(200.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([173, 107, 96, 255]),
        ))),
    ),
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(120.0)))
        .with(StyleDeclaration::height(Px(200.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([102, 156, 116, 255]),
        ))),
    ),
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(120.0)))
        .with(StyleDeclaration::height(Px(200.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([98, 122, 176, 255]),
        ))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(200.0)))
      .with(StyleDeclaration::mix_blend_mode(mode))
      .with(StyleDeclaration::flex_direction(FlexDirection::Row)),
  );

  Node::container([foreground]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(200.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([118, 128, 138, 255]),
      ))),
  )
}

#[test]
fn test_style_mix_blend_mode() {
  let blend_modes = [
    (BlendMode::Normal, "normal"),
    (BlendMode::Multiply, "multiply"),
    (BlendMode::Screen, "screen"),
    (BlendMode::Overlay, "overlay"),
    (BlendMode::Darken, "darken"),
    (BlendMode::Lighten, "lighten"),
    (BlendMode::ColorDodge, "color_dodge"),
    (BlendMode::ColorBurn, "color_burn"),
    (BlendMode::HardLight, "hard_light"),
    (BlendMode::SoftLight, "soft_light"),
    (BlendMode::Difference, "difference"),
    (BlendMode::Exclusion, "exclusion"),
    (BlendMode::Hue, "hue"),
    (BlendMode::Saturation, "saturation"),
    (BlendMode::Color, "color"),
    (BlendMode::Luminosity, "luminosity"),
    (BlendMode::PlusLighter, "plus_lighter"),
    (BlendMode::PlusDarker, "plus_darker"),
  ];

  for (mode, fixture_suffix) in blend_modes {
    let container = Node::container([create_blend_card(mode)]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::background_color(
          Color::from_str("sandybrown")
            .map(ColorInput::Value)
            .unwrap(),
        )),
    );

    run_fixture_test(container, &format!("style_mix_blend_mode_{fixture_suffix}"));
  }
}

#[test]
fn test_style_mlx_blend_mode_isolation() {
  let container = Node::container([
    Node::container([
      Node::image(Arc::from("assets/images/yeecord.png")).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::mix_blend_mode(BlendMode::Multiply)),
      ),
    ])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::isolation(Isolation::Auto))
        .with(StyleDeclaration::width(Px(128.0)))
        .with(StyleDeclaration::height(Px(128.0))),
    ),
    Node::container([
      Node::image(Arc::from("assets/images/yeecord.png")).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::mix_blend_mode(BlendMode::Multiply)),
      ),
    ])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::isolation(Isolation::Isolate))
        .with(StyleDeclaration::width(Px(128.0)))
        .with(StyleDeclaration::height(Px(128.0))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(
        Color::from_str("deepskyblue")
          .map(ColorInput::Value)
          .unwrap(),
      )),
  );

  run_fixture_test(container, "style_mix_blend_mode_isolation");
}
