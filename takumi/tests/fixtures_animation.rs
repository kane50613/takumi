use smallvec::smallvec;
use std::f32::consts::PI;
use takumi::layout::{
  node::{ContainerNode, NodeKind, TextNode},
  style::{
    AlignItems, Color, ColorInput, CssOption, FlexDirection, FontFamily, FontWeight,
    JustifyContent,
    LengthUnit::{Percentage, Px},
    StyleBuilder, Transform, Transforms,
  },
};

mod test_utils;
use test_utils::run_webp_animation_test;

use crate::test_utils::run_png_animation_test;

fn create_bouncing_text_nodes() -> Vec<(NodeKind, u32)> {
  const FPS: u32 = 30;
  const DURATION_MS: u32 = 1500;

  const TOTAL_FRAMES: u32 = DURATION_MS * FPS / 1000;

  (0..TOTAL_FRAMES)
    .map(|frame| {
      // compute bounce progress and transforms
      let frames = TOTAL_FRAMES;
      let t = frame as f32 / (frames - 1) as f32;
      let theta = t * 2.0 * PI; // full cycle over all frames
      let bounce = (theta.sin()).abs();
      let y_offset = -bounce * 140.0; // pixels up

      let node = ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
            .width(Percentage(100.0))
            .height(Percentage(100.0))
            .flex_direction(FlexDirection::Column)
            .align_items(AlignItems::Center)
            .justify_content(JustifyContent::Center)
            .build()
            .unwrap(),
        ),
        children: Some(vec![
          ContainerNode {
            tw: None,
            style: Some(
              StyleBuilder::default()
                .transform(CssOption::some(Transforms(smallvec![
                  Transform::Translate(Px(0.0), Px(y_offset)),
                ])))
                .build()
                .unwrap(),
            ),
            children: Some(vec![
              TextNode {
                tw: None,
                style: Some(
                  StyleBuilder::default()
                    .font_size(CssOption::some(Px(56.0)))
                    .font_family(CssOption::some(FontFamily::from("monospace")))
                    .font_weight(FontWeight::from(700.0))
                    .color(ColorInput::Value(Color([10, 10, 10, 255])))
                    .build()
                    .unwrap(),
                ),
                text: "Takumi Renders Animated image 🔥".to_string(),
              }
              .into(),
            ]),
          }
          .into(),
        ]),
      }
      .into();

      (node, DURATION_MS / TOTAL_FRAMES)
    })
    .collect::<Vec<_>>()
}

#[test]
fn fixtures_animation_bouncing_text_webp() {
  run_webp_animation_test(
    create_bouncing_text_nodes(),
    "tests/fixtures/animation_bouncing_text.webp",
    true,
    false,
    None,
  );
}

#[test]
fn fixtures_animation_bouncing_text_png() {
  run_png_animation_test(
    create_bouncing_text_nodes(),
    "tests/fixtures/animation_bouncing_text.png",
    None,
  );
}
