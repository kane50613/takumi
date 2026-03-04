use takumi::layout::{
  node::{ContainerNode, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn clip_path_text_stroke_filled() {
  let text = "clip-path works in Takumi";

  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([0, 0, 0, 255])))
        .display(Display::Flex)
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .flex_direction(FlexDirection::Column)
        .font_size(Some(Px(84.0)))
        .font_weight(FontWeight::from(700.0))
        .text_align(TextAlign::Center)
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        TextNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .display(Display::Block)
              .position(Position::Absolute)
              .top(Some(Percentage(50.0)))
              .left(Some(Percentage(50.0)))
              .translate(Some(SpacePair::from_single(Percentage(-50.0))))
              .color(ColorInput::Value(Color::white())) // White fill
              .clip_path(Some(BasicShape::from_str("inset(0 0 50% 0)").unwrap()))
              .build()
              .unwrap(),
          ),
          text: text.to_string(),
        }
        .into(),
        TextNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .display(Display::Block)
              .position(Position::Absolute)
              .top(Some(Percentage(50.0)))
              .left(Some(Percentage(50.0)))
              .translate(Some(SpacePair::from_single(Percentage(-50.0))))
              .color(ColorInput::Value(Color::transparent())) // Transparent fill
              .webkit_text_stroke_width(Some(Px(2.0)))
              .webkit_text_stroke_color(Some(ColorInput::Value(Color([128, 128, 128, 255])))) // Semi-transparent white stroke
              .clip_path(Some(BasicShape::from_str("inset(50% 0 0 0)").unwrap()))
              .build()
              .unwrap(),
          ),
          text: text.to_string(),
        }
        .into(),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "clip_path_text_stroke_filled");
}

// Triangle clip-path similar to Vercel logo using polygon
#[test]
fn clip_path_triangle_vercel() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([255, 255, 255, 255]))) // White background
        .display(Display::Flex)
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .flex_direction(FlexDirection::Column)
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        // Triangle with clip-path
        ContainerNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .width(Px(128.0))
              .height(Px(128.0))
              .background_color(ColorInput::Value(Color::black())) // Black triangle
              .clip_path(Some(
                BasicShape::from_str("polygon(0% 100%, 100% 100%, 50% 12.25%)").unwrap(),
              ))
              .build()
              .unwrap(),
          ),
          children: None,
        }
        .into(),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "clip_path_triangle_vercel");
}

// Alternative triangle with gradient background to show clipping more clearly
#[test]
fn clip_path_triangle_gradient() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([255, 255, 255, 255]))) // White background
        .display(Display::Flex)
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .flex_direction(FlexDirection::Column)
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        // Triangle with gradient background and clip-path
        ContainerNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .width(Px(300.0))
              .height(Px(300.0))
              .background_image(Some(
                BackgroundImages::from_str(
                  "linear-gradient(45deg, #ff3b30, #ff9500, #ffcc00, #34c759, #007aff, #5856d6)",
                )
                .unwrap(),
              ))
              .clip_path(Some(
                BasicShape::from_str("polygon(0% 100%, 100% 100%, 50% 12.25%)").unwrap(),
              ))
              .build()
              .unwrap(),
          ),
          children: None,
        }
        .into(),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "clip_path_triangle_gradient");
}

// Circle clip-path test
#[test]
fn clip_path_circle() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([255, 255, 255, 255]))) // White background
        .display(Display::Flex)
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .flex_direction(FlexDirection::Column)
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        // Circle with clip-path
        ContainerNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .width(Px(200.0))
              .height(Px(200.0))
              .background_color(ColorInput::Value(Color([255, 0, 100, 255]))) // Pink background
              .clip_path(Some(BasicShape::from_str("circle(50%)").unwrap()))
              .build()
              .unwrap(),
          ),
          children: None,
        }
        .into(),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "clip_path_circle");
}

// Inset with border radius clip-path test
#[test]
fn clip_path_inset_rounded() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([255, 255, 255, 255]))) // White background
        .display(Display::Flex)
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .flex_direction(FlexDirection::Column)
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        // Inset with border radius and clip-path
        ContainerNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .width(Px(200.0))
              .height(Px(200.0))
              .background_color(ColorInput::Value(Color([100, 200, 255, 255]))) // Light blue background
              .clip_path(Some(
                BasicShape::from_str("inset(50px 0 round 20px)").unwrap(),
              ))
              .build()
              .unwrap(),
          ),
          children: None,
        }
        .into(),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "clip_path_inset_rounded");
}

// Test: clip-path on parent clips absolutely-positioned children
#[test]
fn clip_path_inset_round_clips_children() {
  // Outer wrapper (white background, defines canvas)
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .build()
        .unwrap(),
    ),
    children: Some(
      [
        // Inner container with clip-path: inset(0px round 50px)
        ContainerNode {
          class_name: None,
          id: None,
          tag_name: None,
          preset: None,
          tw: None,
          style: Some(
            StyleBuilder::default()
              .position(Position::Absolute)
              .top(Some(Px(0.0)))
              .left(Some(Px(0.0)))
              .width(Percentage(100.0))
              .height(Percentage(100.0))
              .clip_path(Some(BasicShape::from_str("inset(0px round 50px)").unwrap()))
              .background_color(ColorInput::Value(Color([0, 0, 0, 255]))) // Black bg
              .build()
              .unwrap(),
          ),
          children: Some(
            [
              // Full-bleed red child — should be clipped to rounded rect
              ContainerNode {
                class_name: None,
                id: None,
                tag_name: None,
                preset: None,
                tw: None,
                style: Some(
                  StyleBuilder::default()
                    .position(Position::Absolute)
                    .top(Some(Px(0.0)))
                    .left(Some(Px(0.0)))
                    .width(Percentage(100.0))
                    .height(Percentage(100.0))
                    .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
                    .build()
                    .unwrap(),
                ),
                children: None,
              }
              .into(),
            ]
            .into(),
          ),
        }
        .into(),
      ]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "clip_path_inset_round_clips_children");
}
