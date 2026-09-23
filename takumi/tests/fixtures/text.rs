use takumi::prelude::*;

use crate::test_utils::{ink_bounds, render_node, run_fixture_test};

const ELLIPSIS_CANVAS_WIDTH: u32 = 480;
const ELLIPSIS_CANVAS_HEIGHT: u32 = 200;

fn unbreakable_ellipsis_root(text_overflow: TextOverflow) -> Node {
  let text = Node::text("gsijdsoifgdhaetlelwtyuxxxxxx".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::text_overflow(text_overflow))
      .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap))
      .with_overflow(SpacePair::from_single(Overflow::Hidden))
      .with(StyleDeclaration::width(Length::Px(360.0)))
      .with(StyleDeclaration::color(ColorInput::Value(Color::black()))),
  );

  Node::container([text]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::font_size(Length::Px(48.0).into()))
      .with(StyleDeclaration::width(Length::Px(
        ELLIPSIS_CANVAS_WIDTH as f32,
      )))
      .with(StyleDeclaration::height(Length::Px(
        ELLIPSIS_CANVAS_HEIGHT as f32,
      )))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  )
}

/// A single unbreakable token has no break opportunity, but browsers still
/// ellipsize it at a cluster boundary. The clipped variant runs ink to the
/// box edge; the ellipsis variant must stop short of it.
#[test]
fn test_nowrap_ellipsis_without_break_opportunity() {
  let viewport = Viewport::new((ELLIPSIS_CANVAS_WIDTH, ELLIPSIS_CANVAS_HEIGHT));
  let clipped = render_node(unbreakable_ellipsis_root(TextOverflow::Clip), viewport);
  let ellipsized = render_node(unbreakable_ellipsis_root(TextOverflow::Ellipsis), viewport);

  run_fixture_test(
    unbreakable_ellipsis_root(TextOverflow::Ellipsis),
    "text_ellipsis_nowrap_unbreakable",
  );

  let (_, _, clipped_right, _) = ink_bounds(&clipped);
  let (_, _, ellipsized_right, _) = ink_bounds(&ellipsized);

  assert!(
    clipped_right >= 355,
    "expected the clip variant to run ink to the box edge, rightmost ink at x={clipped_right}",
  );
  assert!(
    ellipsized_right < 355,
    "expected the ellipsis variant to stop short of the box edge, rightmost ink at x={ellipsized_right}",
  );
  assert_ne!(clipped.as_raw(), ellipsized.as_raw());
}
