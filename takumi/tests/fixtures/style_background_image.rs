use takumi::prelude::{Length::*, *};

use crate::test_utils::run_fixture_test;

fn centered_background_position() -> PositionValues {
  PositionValues::from_css_str("center center").unwrap()
}

fn create_container(background_images: BackgroundImages) -> Node {
  Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_image(Some(background_images)))
      .with(StyleDeclaration::background_position(
        centered_background_position(),
      )),
  )
}

fn create_container_with(
  background_images: BackgroundImages,
  background_size: Option<BackgroundSizes>,
  background_position: Option<PositionValues>,
  background_repeat: Option<BackgroundRepeats>,
) -> Node {
  Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_image(Some(background_images)))
      .with(StyleDeclaration::background_size(
        background_size.unwrap_or_default(),
      ))
      .with(StyleDeclaration::background_position(
        background_position.unwrap_or_else(centered_background_position),
      ))
      .with(StyleDeclaration::background_repeat(
        background_repeat.unwrap_or_default(),
      )),
  )
}

#[test]
fn test_style_background_image_gradient() {
  let background_images =
    BackgroundImages::from_css_str("linear-gradient(45deg, rgba(255,150,255,0.3), transparent)")
      .unwrap();

  let container = create_container(background_images).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_image(Some(
        BackgroundImages::from_css_str(
          "linear-gradient(45deg, rgba(255,150,255,0.3), transparent)",
        )
        .unwrap(),
      )))
      .with(StyleDeclaration::background_position(
        centered_background_position(),
      ))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::black(),
      ))),
  );

  run_fixture_test(container, "style_background_image_gradient");
}

#[test]
fn test_style_background_image_gradient_alt() {
  let background_images =
    BackgroundImages::from_css_str("linear-gradient(0deg, #ff3b30, #5856d6)").unwrap();

  let container = create_container(background_images);

  run_fixture_test(container, "style_background_image_gradient_alt");
}

#[test]
fn test_style_background_image_gradient_hard_stop() {
  let background_images =
    BackgroundImages::from_css_str("linear-gradient(to left, #252525 0%, #252525 20%, #f5f5f5 20%, #f5f5f5 40%, #00b7b7 40%, #00b7b7 60%, #b70000 60%, #b70000 80%, #fcd50e 80%)").unwrap();

  let container = create_container(background_images);

  run_fixture_test(container, "style_background_image_gradient_hard_stop");
}

#[test]
fn test_style_background_image_gradient_color_space_comparison() {
  let srgb = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0 / 3.0)))
      .with(StyleDeclaration::background_image(Some(
        BackgroundImages::from_css_str("linear-gradient(to right in srgb, red, blue)").unwrap(),
      ))),
  );

  let oklab = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(33.333)))
      .with(StyleDeclaration::background_image(Some(
        BackgroundImages::from_css_str("linear-gradient(to right in oklab, red, blue)").unwrap(),
      ))),
  );

  let oklch_longer = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(33.334)))
      .with(StyleDeclaration::background_image(Some(
        BackgroundImages::from_css_str("linear-gradient(to right in oklch longer hue, red, blue)")
          .unwrap(),
      ))),
  );

  let container = Node::container([srgb, oklab, oklch_longer]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column)),
  );

  run_fixture_test(
    container,
    "style_background_image_gradient_color_space_comparison",
  );
}

#[test]
fn test_style_background_image_radial_basic() {
  let background_images =
    BackgroundImages::from_css_str("radial-gradient(#e66465, #9198e5)").unwrap();

  let container = create_container(background_images);

  run_fixture_test(container, "style_background_image_radial_basic");
}

#[test]
fn test_style_background_image_radial_mixed() {
  let background_images = BackgroundImages::from_css_str("radial-gradient(ellipse at top, #e66465, transparent), radial-gradient(ellipse at bottom, #4d9f0c, transparent)").unwrap();

  let container = create_container(background_images);

  run_fixture_test(container, "style_background_image_radial_mixed");
}

#[test]
fn test_style_background_image_conic_basic() {
  let background_images = BackgroundImages::from_css_str(
    "conic-gradient(from 0deg at 50% 50%, #ff3b30 0%, #ffcc00 25%, #34c759 50%, #007aff 75%, #ff3b30 100%)",
  )
  .unwrap();

  let container = create_container(background_images);

  run_fixture_test(container, "style_background_image_conic_basic");
}

#[test]
fn test_style_background_image_linear_radial_mixed() {
  let background_images = BackgroundImages::from_css_str(
    "linear-gradient(45deg, #0000ff, #00ff00), radial-gradient(circle, #000000, transparent)",
  )
  .unwrap();

  let container = create_container(background_images);

  run_fixture_test(container, "style_background_image_linear_radial_mixed");
}

#[test]
fn test_style_background_image_builder_gradient_layers() {
  let background_images = BackgroundImages::from_css_str(
    "radial-gradient(circle at top left,rgba(236,253,245,0.9),transparent 30%),radial-gradient(circle at bottom right,rgba(45,212,191,0.22),transparent 28%),linear-gradient(135deg,#0f172a 0%,#134e4a 38%,#115e59 66%,#0f766e 100%)",
  )
  .unwrap();

  let container = create_container(background_images);

  run_fixture_test(container, "style_background_image_builder_gradient_layers");
}

#[test]
fn test_style_background_image_repeating_gradients() {
  let repeating_linear = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::flex_grow(Some(FlexGrow(1.0))))
      .with(StyleDeclaration::background_image(Some(
        BackgroundImages::from_css_str(
          "repeating-linear-gradient(90deg, rgba(99, 102, 241, 0.18) 0px, rgba(56, 189, 248, 0.14) 48px, rgba(99, 102, 241, 0.18) 96px)",
        )
        .unwrap(),
      )))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::from_rgb(0x0b1020),
      ))),
  );

  let repeating_radial = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::flex_grow(Some(FlexGrow(1.0))))
      .with(StyleDeclaration::background_image(Some(
        BackgroundImages::from_css_str(
          "repeating-radial-gradient(circle 320px at 50% 50%, rgba(56, 189, 248, 0.16) 0px, rgba(99, 102, 241, 0.10) 40px, rgba(56, 189, 248, 0.16) 80px)",
        )
        .unwrap(),
      )))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::from_rgb(0x0b1020),
      ))),
  );

  let repeating_conic = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::flex_grow(Some(FlexGrow(1.0))))
      .with(StyleDeclaration::background_image(Some(
        BackgroundImages::from_css_str(
          "repeating-conic-gradient(from 0deg at 50% 50%, rgba(99, 102, 241, 0.16) 0deg, rgba(56, 189, 248, 0.12) 90deg, rgba(99, 102, 241, 0.16) 180deg)",
        )
        .unwrap(),
      )))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::from_rgb(0x0b1020),
      ))),
  );

  let container = Node::container([repeating_linear, repeating_radial, repeating_conic])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0))),
    );

  run_fixture_test(container, "style_background_image_repeating_gradients");
}

#[test]
fn test_background_no_repeat_center_with_size_px() {
  let images =
    BackgroundImages::from_css_str("linear-gradient(90deg, rgba(255,0,0,1), rgba(0,0,255,1))")
      .unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_css_str("200px 120px").unwrap()),
    Some(PositionValues::from_css_str("center center").unwrap()),
    Some(BackgroundRepeats::from_css_str("no-repeat").unwrap()),
  );

  run_fixture_test(container, "style_background_no_repeat_center_200x120");
}

#[test]
fn test_background_repeat_tile_from_top_left() {
  let images =
    BackgroundImages::from_css_str("linear-gradient(90deg, rgba(0,200,0,1), rgba(0,0,0,0))")
      .unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_css_str("160px 100px").unwrap()),
    Some(PositionValues::from_css_str("0 0").unwrap()),
    Some(BackgroundRepeats::from_css_str("repeat").unwrap()),
  );

  run_fixture_test(container, "style_background_repeat_tile_from_top_left");
}

#[test]
fn test_background_repeat_space() {
  let images = BackgroundImages::from_css_str(
    "radial-gradient(circle, rgba(255,165,0,1) 0%, rgba(255,165,0,0) 70%)",
  )
  .unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_css_str("120px 120px").unwrap()),
    None,
    Some(BackgroundRepeats::from_css_str("space").unwrap()),
  );

  run_fixture_test(container, "style_background_repeat_space");
}

#[test]
fn test_background_repeat_round() {
  let images =
    BackgroundImages::from_css_str("radial-gradient(circle, rgba(0,0,0,1) 0%, rgba(0,0,0,0) 60%)")
      .unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_css_str("180px 120px").unwrap()),
    None,
    Some(BackgroundRepeats::from_css_str("round").unwrap()),
  );

  run_fixture_test(container, "style_background_repeat_round");
}

#[test]
fn test_background_position_percentage_with_no_repeat() {
  let images =
    BackgroundImages::from_css_str("linear-gradient(0deg, rgba(255,0,255,1), rgba(255,0,255,0))")
      .unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_css_str("220px 160px").unwrap()),
    Some(PositionValues::from_css_str("25% 75%").unwrap()),
    Some(BackgroundRepeats::from_css_str("no-repeat").unwrap()),
  );

  run_fixture_test(container, "style_background_position_percent_25_75");
}

#[test]
fn test_background_size_percentage_with_repeat() {
  let images = BackgroundImages::from_css_str(
    "linear-gradient(180deg, rgba(0,128,255,0.9), rgba(0,128,255,0))",
  )
  .unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_css_str("20% 20%").unwrap()),
    Some(PositionValues::from_css_str("0 0").unwrap()),
    Some(BackgroundRepeats::from_css_str("repeat").unwrap()),
  );

  run_fixture_test(container, "style_background_size_percent_20_20");
}

#[test]
fn test_background_image_grid_pattern() {
  let images = BackgroundImages::from_css_str(
    "linear-gradient(to right, grey 1px, transparent 1px), linear-gradient(to bottom, grey 1px, transparent 1px)",
  )
  .unwrap();

  let container = create_container_with(
    images.clone(),
    Some(BackgroundSizes::from_css_str("40px 40px").unwrap()),
    Some(PositionValues::from_css_str("0 0, 0 0").unwrap()),
    Some(BackgroundRepeats::from_css_str("repeat, repeat").unwrap()),
  )
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_image(Some(images)))
      .with(StyleDeclaration::background_size(
        BackgroundSizes::from_css_str("40px 40px").unwrap(),
      ))
      .with(StyleDeclaration::background_position(
        PositionValues::from_css_str("0 0, 0 0").unwrap(),
      ))
      .with(StyleDeclaration::background_repeat(
        BackgroundRepeats::from_css_str("repeat, repeat").unwrap(),
      ))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(container, "style_background_image_grid_pattern");
}

#[test]
fn test_background_image_dotted_pattern() {
  let images = BackgroundImages::from_css_str(
    "radial-gradient(circle at 25px 25px, lightgray 2%, transparent 0%), radial-gradient(circle at 75px 75px, lightgray 2%, transparent 0%)",
  )
  .unwrap();

  let container = create_container_with(
    images.clone(),
    Some(BackgroundSizes::from_css_str("100px 100px").unwrap()),
    None,
    Some(BackgroundRepeats::from_css_str("repeat").unwrap()),
  )
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_image(Some(images)))
      .with(StyleDeclaration::background_size(
        BackgroundSizes::from_css_str("100px 100px").unwrap(),
      ))
      .with(StyleDeclaration::background_position(
        centered_background_position(),
      ))
      .with(StyleDeclaration::background_repeat(
        BackgroundRepeats::from_css_str("repeat").unwrap(),
      ))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::black(),
      ))),
  );

  run_fixture_test(container, "style_background_image_dotted_pattern");
}

#[test]
fn test_background_size_contain() {
  let images = BackgroundImages::from_css_str("url(assets/images/yeecord.png)").unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_css_str("contain").unwrap()),
    Some(PositionValues::from_css_str("center center").unwrap()),
    Some(BackgroundRepeats::from_css_str("no-repeat").unwrap()),
  );

  run_fixture_test(container, "style_background_size_contain");
}

/// `background-size` with one auto axis re-derives that axis from the image's
/// intrinsic ratio after `round` rescales the other one. The SVG backend used
/// to skip that step and disagree with the raster one.
#[test]
fn test_background_size_auto_axis_round() {
  let images = BackgroundImages::from_css_str("url(assets/images/yeecord.png)").unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_css_str("auto 80px").unwrap()),
    Some(PositionValues::from_css_str("left top").unwrap()),
    Some(BackgroundRepeats::from_css_str("round").unwrap()),
  );

  // The fixture harness rewrites its goldens without comparing, so the geometry
  // is asserted here. Removing the file first keeps a stale one from a previous
  // run out of the assertion.
  let svg_path = "tests/fixtures-generated/style_background_size_auto_axis_round.svg";
  let _ = std::fs::remove_file(svg_path);

  run_fixture_test(container, "style_background_size_auto_axis_round");

  let svg = std::fs::read_to_string(svg_path).expect("the fixture wrote no svg");
  let pattern = svg
    .split_once("<pattern ")
    .expect("no background pattern in the svg")
    .1;
  let attr = |name: &str| {
    pattern
      .split_once(&format!("{name}=\""))
      .and_then(|(_, rest)| rest.split_once('"'))
      .map(|(value, _)| value.to_string())
      .unwrap_or_default()
  };

  // The fixed axis rounds to 90, the auto axis follows the intrinsic ratio and
  // then rounds to 92. Without the re-derivation the auto axis stays at 80.
  assert_eq!((attr("width"), attr("height")), ("92".into(), "90".into()));
}

#[test]
fn test_background_size_cover() {
  let images = BackgroundImages::from_css_str("url(assets/images/yeecord.png)").unwrap();
  let container = create_container_with(
    images,
    Some(BackgroundSizes::from_css_str("cover").unwrap()),
    Some(PositionValues::from_css_str("center center").unwrap()),
    Some(BackgroundRepeats::from_css_str("no-repeat").unwrap()),
  );

  run_fixture_test(container, "style_background_size_cover");
}

#[test]
fn test_style_background_image_repeating_hard_stop() {
  let background_images = BackgroundImages::from_css_str(
    "repeating-linear-gradient(45deg, #fbbf24 0px, #fbbf24 10px, #f59e0b 10px, #f59e0b 20px)",
  )
  .unwrap();

  let container = create_container(background_images);

  run_fixture_test(container, "style_background_image_repeating_hard_stop");
}
