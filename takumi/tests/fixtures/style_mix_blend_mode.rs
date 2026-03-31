use takumi::layout::{
  node::Node,
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;
use std::sync::Arc;

/// Creates a single card with an image and mix-blend-mode for testing.
fn create_blend_card(mode: BlendMode, label_font_size_px: f32) -> Node {
  Node::container([
    Node::image(Arc::from("assets/images/yeecord.png")).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(164.0)))
        .with(StyleDeclaration::height(Px(164.0)))
        .with(StyleDeclaration::mix_blend_mode(mode)),
    ),
    Node::text(format!("{:?}", mode)).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Px(label_font_size_px).into()))
        .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
        .with(StyleDeclaration::margin_top(Px(10.0)))
        .with(StyleDeclaration::color(ColorInput::Value(Color::black()))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with_padding(Sides([Px(20.0); 4])),
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
    let container = Node::container([create_blend_card(mode, 22.0)]).with_style(
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
