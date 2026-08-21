use takumi::{prelude::*, render};

use crate::test_utils::{CONTEXT, run_fixture_test};

const CANVAS_WIDTH: u32 = 480;
const CANVAS_HEIGHT: u32 = 200;

/// Synthetic italic skews glyph paths at draw time, so the ink of a tall
/// trailing glyph reaches past the advance-derived box that
/// `compute_node_paint_bounds` reports. An isolation layer (opacity) sized
/// from those bounds must not clip that overhang.
fn root(opacity: f32) -> Node {
  let text = Node::text("Illl".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::font_size(Length::Px(140.0).into()))
      .with(StyleDeclaration::font_style(FontStyle::italic()))
      .with(StyleDeclaration::color(ColorInput::Value(Color::black()))),
  );

  let isolated = Node::container([text]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::opacity(PercentageNumber(opacity))),
  );

  Node::container([isolated]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::align_items(AlignItems::FlexStart))
      .with(StyleDeclaration::width(Length::Px(CANVAS_WIDTH as f32)))
      .with(StyleDeclaration::height(Length::Px(CANVAS_HEIGHT as f32)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  )
}

fn render_root(opacity: f32) -> Bitmap {
  render(
    RenderOptions::builder()
      .viewport(Viewport::new((CANVAS_WIDTH, CANVAS_HEIGHT)))
      .node(root(opacity))
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap()
}

fn rightmost_ink_column(image: &Bitmap) -> u32 {
  let width = image.width();

  image
    .as_raw()
    .as_chunks::<4>()
    .0
    .iter()
    .enumerate()
    .filter_map(|(index, pixel)| {
      let dark = pixel[3] > 0 && pixel[0].min(pixel[1]).min(pixel[2]) < 160;
      dark.then(|| index as u32 % width)
    })
    .max()
    .unwrap_or(0)
}

fn descender_root(opacity: f32) -> Node {
  let text = Node::text("ggg".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::font_size(Length::Px(120.0).into()))
      .with(StyleDeclaration::line_height(LineHeight::Length(
        Length::Em(0.8),
      )))
      .with(StyleDeclaration::color(ColorInput::Value(Color::black()))),
  );

  let isolated = Node::container([text]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::opacity(PercentageNumber(opacity))),
  );

  Node::container([isolated]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::align_items(AlignItems::FlexStart))
      .with(StyleDeclaration::width(Length::Px(CANVAS_WIDTH as f32)))
      .with(StyleDeclaration::height(Length::Px(CANVAS_HEIGHT as f32)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  )
}

fn bottommost_ink_row(image: &Bitmap) -> u32 {
  let width = image.width();

  image
    .as_raw()
    .as_chunks::<4>()
    .0
    .iter()
    .enumerate()
    .filter_map(|(index, pixel)| {
      let dark = pixel[3] > 0 && pixel[0].min(pixel[1]).min(pixel[2]) < 160;
      dark.then(|| index as u32 / width)
    })
    .max()
    .unwrap_or(0)
}

/// The tight line box ends above the g descenders; the ink below it must
/// survive an isolation layer sized from the node's paint bounds.
#[test]
fn test_isolated_text_keeps_descender_ink() {
  let plain = render(
    RenderOptions::builder()
      .viewport(Viewport::new((CANVAS_WIDTH, CANVAS_HEIGHT)))
      .node(descender_root(1.0))
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap();
  let isolated = render(
    RenderOptions::builder()
      .viewport(Viewport::new((CANVAS_WIDTH, CANVAS_HEIGHT)))
      .node(descender_root(0.6))
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap();

  run_fixture_test(
    descender_root(0.6),
    "paint-bounds-text-ink-descender-opacity",
  );

  let plain_bottom = bottommost_ink_row(&plain);
  let isolated_bottom = bottommost_ink_row(&isolated);

  assert!(
    plain_bottom.abs_diff(isolated_bottom) <= 1,
    "isolation clipped the descender: bottommost ink at y={isolated_bottom} with opacity vs y={plain_bottom} without",
  );
}

#[test]
fn test_isolated_text_keeps_synthetic_italic_overhang() {
  let plain = render_root(1.0);
  let isolated = render_root(0.6);

  run_fixture_test(root(0.6), "paint-bounds-text-ink-italic-opacity");

  let plain_right = rightmost_ink_column(&plain);
  let isolated_right = rightmost_ink_column(&isolated);

  assert!(
    plain_right.abs_diff(isolated_right) <= 1,
    "isolation clipped the italic overhang: rightmost ink at x={isolated_right} with opacity vs x={plain_right} without",
  );
}
