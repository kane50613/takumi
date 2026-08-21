use takumi::{prelude::*, render};

use crate::test_utils::{CONTEXT, run_fixture_test};

const ELLIPSIS_CANVAS_WIDTH: u32 = 480;
const ELLIPSIS_CANVAS_HEIGHT: u32 = 200;

fn rightmost_dark_column(image: &Bitmap) -> u32 {
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
  let clipped = render(
    RenderOptions::builder()
      .viewport(Viewport::new((
        ELLIPSIS_CANVAS_WIDTH,
        ELLIPSIS_CANVAS_HEIGHT,
      )))
      .node(unbreakable_ellipsis_root(TextOverflow::Clip))
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap();
  let ellipsized = render(
    RenderOptions::builder()
      .viewport(Viewport::new((
        ELLIPSIS_CANVAS_WIDTH,
        ELLIPSIS_CANVAS_HEIGHT,
      )))
      .node(unbreakable_ellipsis_root(TextOverflow::Ellipsis))
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap();

  run_fixture_test(
    unbreakable_ellipsis_root(TextOverflow::Ellipsis),
    "text_ellipsis_nowrap_unbreakable",
  );

  let clipped_right = rightmost_dark_column(&clipped);
  let ellipsized_right = rightmost_dark_column(&ellipsized);

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
