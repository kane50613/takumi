use std::f32::consts::PI;

use takumi::layout::{
  node::{ContainerNode, NodeKind, TextNode},
  style::{Length::*, *},
};
use takumi::rendering::{
  AnimationFrame, RenderOptions, RenderOptionsBuilder, SequentialSceneBuilder,
  render_sequence_animation,
};

use crate::test_utils::{CONTEXT, create_test_viewport_with_size, run_animation_fixture_test};

const BOUNCING_TEXT_FPS: u32 = 20;
const BOUNCING_TEXT_DURATION_MS: u32 = 900;
const KEYFRAME_INTERPOLATION_FPS: u32 = 20;
const KEYFRAME_INTERPOLATION_DURATION_MS: u32 = 1200;

fn bouncing_text_frames() -> Vec<NodeKind> {
  let frame_count = BOUNCING_TEXT_DURATION_MS * BOUNCING_TEXT_FPS / 1000;
  let denominator = frame_count.saturating_sub(1).max(1) as f32;
  (0..frame_count)
    .map(|frame_index| {
      let progress = frame_index as f32 / denominator;
      let bounce = (progress * 2.0 * PI).sin().abs();
      let y_offset = -140.0 * bounce;

      bouncing_text_node(y_offset)
    })
    .collect()
}

fn bouncing_text_node(y_offset: f32) -> NodeKind {
  ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center)),
    ),
    children: Some(
      [ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(Style::default().with(StyleDeclaration::transform(Some(
          [Transform::Translate(Px(0.0), Px(y_offset))].into(),
        )))),
        children: Some([bouncing_text_label()].into()),
      }
      .into()]
      .into(),
    ),
  }
  .into()
}

fn bouncing_text_label() -> NodeKind {
  TextNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::font_size(Px(56.0).into()))
        .with(StyleDeclaration::font_family(Some(FontFamily::from(
          "monospace",
        ))))
        .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
        .with(StyleDeclaration::color(ColorInput::Value(Color([
          10, 10, 10, 255,
        ])))),
    ),
    text: "Takumi Renders Animated image 🔥".to_string(),
  }
  .into()
}

fn keyframe_interpolation_node() -> NodeKind {
  ContainerNode {
    class_name: Some("root".into()),
    id: None,
    tag_name: Some("div".into()),
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([242, 244, 247, 255]),
        ))),
    ),
    children: Some(
      [ContainerNode {
        class_name: Some("stage".into()),
        id: None,
        tag_name: Some("section".into()),
        preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with(StyleDeclaration::display(Display::Flex))
            .with(StyleDeclaration::flex_direction(FlexDirection::Column))
            .with(StyleDeclaration::justify_content(JustifyContent::Center))
            .with(StyleDeclaration::align_items(AlignItems::Center)),
        ),
        children: Some(
          [
            TextNode {
              class_name: Some("eyebrow".into()),
              id: None,
              tag_name: Some("p".into()),
              preset: None,
              tw: None,
              style: None,
              text: "Stylesheet keyframes".to_string(),
            }
            .into(),
            ContainerNode {
              class_name: Some("track".into()),
              id: None,
              tag_name: Some("div".into()),
              preset: None,
              tw: None,
              style: None,
              children: Some(
                [ContainerNode {
                  class_name: Some("chip".into()),
                  id: None,
                  tag_name: Some("div".into()),
                  preset: None,
                  tw: None,
                  style: None,
                  children: Some(
                    [TextNode {
                      class_name: Some("label".into()),
                      id: None,
                      tag_name: Some("span".into()),
                      preset: None,
                      tw: None,
                      style: None,
                      text: "Takumi".to_string(),
                    }
                    .into()]
                    .into(),
                  ),
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
      .into()]
      .into(),
    ),
  }
  .into()
}

fn keyframe_interpolation_frames() -> Vec<AnimationFrame> {
  let scene = SequentialSceneBuilder::default()
    .options(keyframe_interpolation_options())
    .duration_ms(KEYFRAME_INTERPOLATION_DURATION_MS)
    .build()
    .unwrap();

  render_sequence_animation(&[scene], KEYFRAME_INTERPOLATION_FPS).unwrap()
}

fn keyframe_interpolation_options() -> RenderOptions<'static, NodeKind> {
  RenderOptionsBuilder::default()
    .viewport(create_test_viewport_with_size(800, 400))
    .node(keyframe_interpolation_node())
    .global(&CONTEXT)
    .stylesheets(vec![keyframe_interpolation_stylesheet().to_string()])
    .build()
    .unwrap()
}

fn keyframe_interpolation_stylesheet() -> &'static str {
  r#"
    .stage {
      width: 760px;
      height: 320px;
      padding: 36px;
      row-gap: 28px;
      border-radius: 28px;
      background: rgb(15, 23, 42);
      box-shadow: 0 20px 60px rgba(15, 23, 42, 0.22);
    }

    .eyebrow {
      color: rgb(148, 163, 184);
      font-size: 28px;
    }

    .track {
      width: 620px;
      height: 124px;
      padding: 18px;
      border-radius: 999px;
      background: rgb(30, 41, 59);
    }

    .chip {
      width: 120px;
      height: 88px;
      border-radius: 999px;
      background: rgb(56, 189, 248);
      display: flex;
      justify-content: center;
      align-items: center;
      animation-name: glide;
      animation-duration: 600ms;
      animation-timing-function: ease-in;
      animation-iteration-count: infinite;
      animation-direction: alternate;
      animation-fill-mode: both;
    }

    .label {
      color: white;
      font-size: 34px;
      font-weight: 700;
    }

    @keyframes glide {
      0% {
        width: 120px;
        background-color: rgb(56, 189, 248);
      }

      40% {
        width: 280px;
        background-color: rgb(168, 85, 247);
      }

      100% {
        width: 520px;
        background-color: rgb(244, 114, 182);
      }
    }
  "#
}

#[test]
fn animation_bouncing_text() {
  run_animation_fixture_test(
    bouncing_text_frames(),
    "animation_bouncing_text",
    BOUNCING_TEXT_DURATION_MS,
    BOUNCING_TEXT_FPS,
  );
}

#[test]
fn animation_keyframe_interpolation() {
  run_animation_fixture_test(
    keyframe_interpolation_frames(),
    "animation_keyframe_interpolation",
    KEYFRAME_INTERPOLATION_DURATION_MS,
    KEYFRAME_INTERPOLATION_FPS,
  );
}
