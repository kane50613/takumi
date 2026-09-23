use takumi::prelude::*;

use crate::test_utils::{ink_bounds, render_node, run_fixture_test};

const CANVAS_WIDTH: u32 = 480;
const CANVAS_HEIGHT: u32 = 200;

/// Synthetic italic skews glyph paths at draw time, so the ink of a tall
/// trailing glyph reaches past the advance-derived box that
/// `compute_node_paint_bounds` reports. An isolation layer (opacity) sized
/// from those bounds must not clip that overhang.
fn italic_root(opacity: f32) -> Node {
  isolated_on_canvas(
    Node::text("Illl".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::font_size(Length::Px(140.0).into()))
        .with(StyleDeclaration::font_style(FontStyle::italic()))
        .with(StyleDeclaration::color(ColorInput::Value(Color::black()))),
    ),
    opacity,
  )
}

fn descender_root(opacity: f32) -> Node {
  isolated_on_canvas(
    Node::text("ggg".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::font_size(Length::Px(120.0).into()))
        .with(StyleDeclaration::line_height(LineHeight::Length(
          Length::Em(0.8),
        )))
        .with(StyleDeclaration::color(ColorInput::Value(Color::black()))),
    ),
    opacity,
  )
}

fn isolated_on_canvas(text: Node, opacity: f32) -> Node {
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

fn render_canvas(node: Node) -> Bitmap {
  render_node(node, Viewport::new((CANVAS_WIDTH, CANVAS_HEIGHT)))
}

/// The tight line box ends above the g descenders; the ink below it must
/// survive an isolation layer sized from the node's paint bounds.
#[test]
fn test_isolated_text_keeps_descender_ink() {
  let (_, _, _, plain_bottom) = ink_bounds(&render_canvas(descender_root(1.0)));
  let (_, _, _, isolated_bottom) = ink_bounds(&render_canvas(descender_root(0.6)));

  run_fixture_test(
    descender_root(0.6),
    "paint-bounds-text-ink-descender-opacity",
  );

  assert!(
    plain_bottom.abs_diff(isolated_bottom) <= 1,
    "isolation clipped the descender: bottommost ink at y={isolated_bottom} with opacity vs y={plain_bottom} without",
  );
}

#[test]
fn test_isolated_text_keeps_synthetic_italic_overhang() {
  let (_, _, plain_right, _) = ink_bounds(&render_canvas(italic_root(1.0)));
  let (_, _, isolated_right, _) = ink_bounds(&render_canvas(italic_root(0.6)));

  run_fixture_test(italic_root(0.6), "paint-bounds-text-ink-italic-opacity");

  assert!(
    plain_right.abs_diff(isolated_right) <= 1,
    "isolation clipped the italic overhang: rightmost ink at x={isolated_right} with opacity vs x={plain_right} without",
  );
}
