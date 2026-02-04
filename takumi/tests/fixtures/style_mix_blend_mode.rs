use takumi::layout::{
  node::{ContainerNode, ImageNode, NodeKind, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;
use std::sync::Arc;

/// Creates a single card with an image and mix-blend-mode for testing.
fn create_blend_card(mode: BlendMode, label_font_size_px: f32) -> NodeKind {
  ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .display(Display::Flex)
        .flex_direction(FlexDirection::Column)
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .padding(Sides([Px(8.0); 4]))
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        ImageNode {
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .width(Px(80.0))
              .height(Px(80.0))
              .mix_blend_mode(mode)
              .build()
              .unwrap(),
          ),
          src: Arc::from("assets/images/yeecord.png"),
          width: None,
          height: None,
        }
        .into(),
        TextNode {
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .font_size(Some(Px(label_font_size_px)))
              .margin_top(Px(4.0))
              .color(ColorInput::Value(Color::black()))
              .build()
              .unwrap(),
          ),
          text: format!("{:?}", mode),
        }
        .into(),
      ]
      .into(),
    ),
  }
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
  ];

  let container = ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .display(Display::Grid)
        .grid_template_columns(GridTemplateComponents::from_str("repeat(4, 1fr)").ok())
        .background_color(Color::from_str("sandybrown").map(ColorInput::Value).ok())
        .build()
        .unwrap(),
    ),
    children: Some(
      blend_modes
        .iter()
        .map(|&mode| create_blend_card(mode, 12.0))
        .collect(),
    ),
  }
  .into();

  run_fixture_test(container, "style_mix_blend_mode");
}

#[test]
fn test_style_mlx_blend_mode_isolation() {
  let container = ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .background_color(Color::from_str("deepskyblue").map(ColorInput::Value).ok())
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        ContainerNode {
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .isolation(Isolation::Auto)
              .width(Px(128.0))
              .height(Px(128.0))
              .build()
              .unwrap(),
          ),
          children: Some(
            [ImageNode {
              preset: None,
              tw: None,
              style: Some(
                StyleBuilder::default()
                  .mix_blend_mode(BlendMode::Multiply)
                  .build()
                  .unwrap(),
              ),
              src: Arc::from("assets/images/yeecord.png"),
              width: None,
              height: None,
            }
            .into()]
            .into(),
          ),
        }
        .into(),
        ContainerNode {
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .isolation(Isolation::Isolate)
              .width(Px(128.0))
              .height(Px(128.0))
              .build()
              .unwrap(),
          ),
          children: Some(
            [ImageNode {
              preset: None,
              tw: None,
              style: Some(
                StyleBuilder::default()
                  .mix_blend_mode(BlendMode::Multiply)
                  .build()
                  .unwrap(),
              ),
              src: Arc::from("assets/images/yeecord.png"),
              width: None,
              height: None,
            }
            .into()]
            .into(),
          ),
        }
        .into(),
      ]
      .into(),
    ),
  }
  .into();

  run_fixture_test(container, "style_mix_blend_mode_isolation");
}
