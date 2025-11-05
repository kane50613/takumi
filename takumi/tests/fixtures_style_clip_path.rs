use takumi::layout::{
  node::{ContainerNode, TextNode},
  style::{LengthUnit::*, *},
};

mod test_utils;
use test_utils::run_style_width_test;

#[test]
fn fixtures_clip_path_text_stroke_filled() {
  let text = "clip-path works in Takumi";

  let container = ContainerNode {
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
        .font_size(CssOption::some(Px(84.0)))
        .font_weight(FontWeight::from(700.0))
        .text_align(TextAlign::Center)
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .position(Position::Absolute)
            .top(CssOption::some(Percentage(50.0)))
            .left(CssOption::some(Percentage(50.0)))
            .translate(CssOption::some(SpacePair::from_single(Percentage(-50.0))))
            .color(ColorInput::Value(Color::white())) // White fill
            .clip_path(CssOption::some(
              BasicShape::from_str("inset(0 0 50% 0)").unwrap(),
            ))
            .build()
            .unwrap(),
        ),
        text: text.to_string(),
      }
      .into(),
      TextNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .position(Position::Absolute)
            .top(CssOption::some(Percentage(50.0)))
            .left(CssOption::some(Percentage(50.0)))
            .translate(CssOption::some(SpacePair::from_single(Percentage(-50.0))))
            .color(ColorInput::Value(Color::transparent())) // Transparent fill
            .text_stroke_width(Px(2.0))
            .text_stroke_color(CssOption::some(ColorInput::Value(Color([
              128, 128, 128, 255,
            ])))) // Semi-transparent white stroke
            .clip_path(CssOption::some(
              BasicShape::from_str("inset(50% 0 0 0)").unwrap(),
            ))
            .build()
            .unwrap(),
        ),
        text: text.to_string(),
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/clip_path_text_stroke_filled.webp",
  );
}

// Triangle clip-path similar to Vercel logo using polygon
#[test]
fn fixtures_clip_path_triangle_vercel() {
  let container = ContainerNode {
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
    children: Some(vec![
      // Triangle with clip-path
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Px(128.0))
            .height(Px(128.0))
            .background_color(ColorInput::Value(Color::black())) // Black triangle
            .clip_path(CssOption::some(
              BasicShape::from_str("polygon(0% 100%, 100% 100%, 50% 12.25%)").unwrap(),
            ))
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/clip_path_triangle_vercel.webp",
  );
}

// Alternative triangle with gradient background to show clipping more clearly
#[test]
fn fixtures_clip_path_triangle_gradient() {
  let container = ContainerNode {
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
    children: Some(vec![
      // Triangle with gradient background and clip-path
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Px(300.0))
            .height(Px(300.0))
            .background_image(CssOption::some(
              BackgroundImages::from_str(
                "linear-gradient(45deg, #ff3b30, #ff9500, #ffcc00, #34c759, #007aff, #5856d6)",
              )
              .unwrap(),
            ))
            .clip_path(CssOption::some(
              BasicShape::from_str("polygon(0% 100%, 100% 100%, 50% 12.25%)").unwrap(),
            ))
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/clip_path_triangle_gradient.webp",
  );
}

// Circle clip-path test
#[test]
fn fixtures_clip_path_circle() {
  let container = ContainerNode {
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
    children: Some(vec![
      // Circle with clip-path
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Px(200.0))
            .height(Px(200.0))
            .background_color(ColorInput::Value(Color([255, 0, 100, 255]))) // Pink background
            .clip_path(CssOption::some(
              BasicShape::from_str("circle(50%)").unwrap(),
            ))
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into(),
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/clip_path_circle.webp");
}

// Inset with border radius clip-path test
#[test]
fn fixtures_clip_path_inset_rounded() {
  let container = ContainerNode {
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
    children: Some(vec![
      // Inset with border radius and clip-path
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Px(200.0))
            .height(Px(200.0))
            .background_color(ColorInput::Value(Color([100, 200, 255, 255]))) // Light blue background
            .clip_path(CssOption::some(
              BasicShape::from_str("inset(50px 0 round 20px)").unwrap(),
            ))
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/clip_path_inset_rounded.webp",
  );
}
