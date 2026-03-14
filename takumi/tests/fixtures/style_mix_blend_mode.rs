use takumi::layout::{
  node::{ContainerNode, ImageNode, NodeKind, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;
use std::sync::Arc;

/// Creates a single card with an image and mix-blend-mode for testing.
fn create_blend_card(mode: BlendMode, label_font_size_px: f32) -> NodeKind {
  ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with_padding(Sides([Px(8.0); 4])),
    )
    .with_child(
      ImageNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::width(Px(80.0)))
            .with(StyleDeclaration::height(Px(80.0)))
            .with(StyleDeclaration::mix_blend_mode(mode)),
        )
        .with_src(Arc::from("assets/images/yeecord.png")),
    )
    .with_child(
      TextNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::font_size(Px(label_font_size_px).into()))
            .with(StyleDeclaration::margin_top(Px(4.0)))
            .with(StyleDeclaration::color(ColorInput::Value(Color::black()))),
        )
        .with_text(format!("{:?}", mode)),
    )
    .into()
}

#[test]
fn test_style_mix_blend_mode() {
  let blend_modes = [
    BlendMode::Normal,
    BlendMode::Multiply,
    BlendMode::Screen,
    BlendMode::Overlay,
    BlendMode::Darken,
    BlendMode::Lighten,
    BlendMode::ColorDodge,
    BlendMode::ColorBurn,
    BlendMode::HardLight,
    BlendMode::SoftLight,
    BlendMode::Difference,
    BlendMode::Exclusion,
    BlendMode::Hue,
    BlendMode::Saturation,
    BlendMode::Color,
    BlendMode::Luminosity,
    BlendMode::PlusLighter,
    BlendMode::PlusDarker,
  ];

  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Grid))
        .with(StyleDeclaration::grid_template_columns(
          GridTemplateComponents::from_str("repeat(4, 1fr)").ok(),
        ))
        .with(StyleDeclaration::background_color(
          Color::from_str("sandybrown")
            .map(ColorInput::Value)
            .unwrap(),
        )),
    )
    .with_children(
      blend_modes
        .iter()
        .map(|&mode| create_blend_card(mode, 12.0))
        .collect::<Vec<_>>(),
    )
    .into();

  run_fixture_test(container, "style_mix_blend_mode");
}

#[test]
fn test_style_mlx_blend_mode_isolation() {
  let container = ContainerNode::default()
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::background_color(
          Color::from_str("deepskyblue")
            .map(ColorInput::Value)
            .unwrap(),
        )),
    )
    .with_children([
      ContainerNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::isolation(Isolation::Auto))
            .with(StyleDeclaration::width(Px(128.0)))
            .with(StyleDeclaration::height(Px(128.0))),
        )
        .with_child(
          ImageNode::default()
            .with_style(
              Style::default().with(StyleDeclaration::mix_blend_mode(BlendMode::Multiply)),
            )
            .with_src(Arc::from("assets/images/yeecord.png")),
        ),
      ContainerNode::default()
        .with_style(
          Style::default()
            .with(StyleDeclaration::isolation(Isolation::Isolate))
            .with(StyleDeclaration::width(Px(128.0)))
            .with(StyleDeclaration::height(Px(128.0))),
        )
        .with_child(
          ImageNode::default()
            .with_style(
              Style::default().with(StyleDeclaration::mix_blend_mode(BlendMode::Multiply)),
            )
            .with_src(Arc::from("assets/images/yeecord.png")),
        ),
    ])
    .into();

  run_fixture_test(container, "style_mix_blend_mode_isolation");
}
