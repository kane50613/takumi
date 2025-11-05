use takumi::layout::{
  node::{ContainerNode, NodeKind},
  style::{LengthUnit::*, *},
};

mod test_utils;
use test_utils::run_style_width_test;

fn create_container(background_images: BackgroundImages) -> ContainerNode<NodeKind> {
  ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_image(CssOption::some(background_images))
        .build()
        .unwrap(),
    ),
    children: None,
  }
}

fn create_container_with(
  background_images: BackgroundImages,
  background_size: Option<BackgroundSizes>,
  background_position: Option<BackgroundPositions>,
  background_repeat: Option<BackgroundRepeats>,
) -> ContainerNode<NodeKind> {
  ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_image(CssOption::some(background_images))
        .background_size(CssOption(background_size))
        .background_position(CssOption(background_position))
        .background_repeat(CssOption(background_repeat))
        .build()
        .unwrap(),
    ),
    children: None,
  }
}

#[test]
fn test_style_background_image_gradient_basic() {
  let background_images =
    BackgroundImages::from_str("linear-gradient(45deg, #007aff, #34c759)").unwrap();

  let container = create_container(background_images);

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_image_gradient.webp",
  );
}

#[test]
fn test_style_background_image_gradient_alt() {
  let background_images =
    BackgroundImages::from_str("linear-gradient(0deg, #ff3b30, #5856d6)").unwrap();

  let container = create_container(background_images);

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_image_gradient_alt.webp",
  );
}

#[test]
fn test_style_background_image_gradient_hard_stop() {
  let background_images =
    BackgroundImages::from_str("linear-gradient(to left, #252525 0%, #252525 20%, #f5f5f5 20%, #f5f5f5 40%, #00b7b7 40%, #00b7b7 60%, #b70000 60%, #b70000 80%, #fcd50e 80%)").unwrap();

  let container = create_container(background_images);

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_image_gradient_hard_stop.webp",
  );
}

#[test]
fn test_style_background_image_radial_basic() {
  let background_images = BackgroundImages::from_str("radial-gradient(#e66465, #9198e5)").unwrap();

  let container = create_container(background_images);

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_image_radial_basic.webp",
  );
}

#[test]
fn test_style_background_image_radial_mixed() {
  let background_images = BackgroundImages::from_str("radial-gradient(ellipse at top, #e66465, transparent), radial-gradient(ellipse at bottom, #4d9f0c, transparent)").unwrap();

  let container = create_container(background_images);

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_image_radial_mixed.webp",
  );
}

#[test]
fn test_style_background_image_linear_radial_mixed() {
  let background_images = BackgroundImages::from_str(
    "linear-gradient(45deg, #0000ff, #00ff00), radial-gradient(circle, #000000, transparent)",
  )
  .unwrap();

  let container = create_container(background_images);

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_image_linear_radial_mixed.webp",
  );
}

#[test]
fn test_background_no_repeat_center_with_size_px() {
  let images =
    BackgroundImages::from_str("linear-gradient(90deg, rgba(255,0,0,1), rgba(0,0,255,1))").unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_str("200px 120px").unwrap()),
    Some(BackgroundPositions::from_str("center center").unwrap()),
    Some(BackgroundRepeats::from_str("no-repeat").unwrap()),
  );

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_no_repeat_center_200x120.webp",
  );
}

#[test]
fn test_background_repeat_tile_from_top_left() {
  let images =
    BackgroundImages::from_str("linear-gradient(90deg, rgba(0,200,0,1), rgba(0,0,0,0))").unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_str("160px 100px").unwrap()),
    Some(BackgroundPositions::from_str("0 0").unwrap()),
    Some(BackgroundRepeats::from_str("repeat").unwrap()),
  );

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_repeat_tile_from_top_left.webp",
  );
}

#[test]
fn test_background_repeat_space() {
  let images = BackgroundImages::from_str(
    "radial-gradient(circle, rgba(255,165,0,1) 0%, rgba(255,165,0,0) 70%)",
  )
  .unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_str("120px 120px").unwrap()),
    None,
    Some(BackgroundRepeats::from_str("space").unwrap()),
  );

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_repeat_space.webp",
  );
}

#[test]
fn test_background_repeat_round() {
  let images =
    BackgroundImages::from_str("radial-gradient(circle, rgba(0,0,0,1) 0%, rgba(0,0,0,0) 60%)")
      .unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_str("180px 120px").unwrap()),
    None,
    Some(BackgroundRepeats::from_str("round").unwrap()),
  );

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_repeat_round.webp",
  );
}

#[test]
fn test_background_position_percentage_with_no_repeat() {
  let images =
    BackgroundImages::from_str("linear-gradient(0deg, rgba(255,0,255,1), rgba(255,0,255,0))")
      .unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_str("220px 160px").unwrap()),
    Some(BackgroundPositions::from_str("25% 75%").unwrap()),
    Some(BackgroundRepeats::from_str("no-repeat").unwrap()),
  );

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_position_percent_25_75.webp",
  );
}

#[test]
fn test_background_size_percentage_with_repeat() {
  let images =
    BackgroundImages::from_str("linear-gradient(180deg, rgba(0,128,255,0.9), rgba(0,128,255,0))")
      .unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_str("20% 20%").unwrap()),
    Some(BackgroundPositions::from_str("0 0").unwrap()),
    Some(BackgroundRepeats::from_str("repeat").unwrap()),
  );

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_size_percent_20_20.webp",
  );
}

#[test]
fn test_background_image_grid_pattern() {
  let images = BackgroundImages::from_str(
    "linear-gradient(to right, grey 1px, transparent 1px), linear-gradient(to bottom, grey 1px, transparent 1px)",
  )
  .unwrap();

  let mut container = create_container_with(
    images,
    Some(BackgroundSizes::from_str("40px 40px").unwrap()),
    Some(BackgroundPositions::from_str("0 0, 0 0").unwrap()),
    Some(BackgroundRepeats::from_str("repeat, repeat").unwrap()),
  );

  container.style.as_mut().unwrap().background_color = ColorInput::Value(Color::white()).into();

  assert_eq!(
    container.style.as_ref().unwrap().background_repeat,
    CssOption::some(BackgroundRepeats(vec![
      BackgroundRepeat::repeat(),
      BackgroundRepeat::repeat()
    ]))
    .into()
  );

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_image_grid_pattern.webp",
  );
}

#[test]
fn test_background_image_noise_v1_with_gradient() {
  let images = BackgroundImages::from_str(
    "radial-gradient(circle at 25% 25%, rgba(255, 0, 128, 0.6), transparent 50%), radial-gradient(circle at 75% 75%, rgba(0, 128, 255, 0.6), transparent 50%), linear-gradient(135deg, rgba(138, 43, 226, 0.4), rgba(30, 144, 255, 0.4), rgba(255, 20, 147, 0.4)), noise-v1(opacity(0.8))",
  )
  .unwrap();

  let mut container = create_container_with(
    images,
    Some(BackgroundSizes::from_str("100% 100%, 100% 100%, 100% 100%, 100% 100%").unwrap()),
    Some(BackgroundPositions::from_str("0 0, 0 0, 0 0, 0 0").unwrap()),
    Some(BackgroundRepeats::from_str("no-repeat, no-repeat, no-repeat, no-repeat").unwrap()),
  );

  container.style.as_mut().unwrap().background_color = ColorInput::Value(Color::white()).into();

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_image_noise_v1_blend.webp",
  );
}
