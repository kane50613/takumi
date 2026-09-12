use takumi::{prelude::*, render};

use crate::test_utils::CONTEXT;

const CANVAS: (u32, u32) = (480, 320);

/// Ink that reaches past the border box: an isolation layer sized from the
/// node's paint bounds must keep all of it.
const CASES: &[(&str, &str)] = &[
  (
    "box-shadow",
    ".ink { box-shadow: 40px 40px 20px 10px black; }",
  ),
  (
    "outline",
    ".ink { outline: 12px solid black; outline-offset: 24px; }",
  ),
  ("text-shadow", ".ink { text-shadow: 40px 40px 8px black; }"),
  (
    "text-stroke",
    ".ink { -webkit-text-stroke: 12px black; color: transparent; }",
  ),
  (
    "filter-blur",
    ".ink { filter: blur(16px); background: black; }",
  ),
  (
    "drop-shadow",
    ".ink { filter: drop-shadow(40px 40px 12px black); background: black; }",
  ),
  (
    "clip-path",
    ".ink { clip-path: circle(60%); background: black; }",
  ),
];

fn render_case(effect: &str, opacity: f32) -> Bitmap {
  let css = format!(
    ".ink {{ display: flex; width: 160px; height: 100px; margin: 80px; font-size: 72px; color: black; opacity: {opacity}; }} {effect}"
  );
  let root = Node::container([Node::container([Node::text("Ink")]).with_class_name("ink")])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Length::Px(CANVAS.0 as f32)))
        .with(StyleDeclaration::height(Length::Px(CANVAS.1 as f32)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        ))),
    );

  render(
    RenderOptions::builder()
      .viewport(Viewport::new(CANVAS))
      .node(root)
      .stylesheet(StyleSheet::parse_loosy(&css).into())
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap()
}

/// Right and bottom edges of the dark ink, inclusive.
fn ink_extent(image: &Bitmap) -> (u32, u32) {
  let width = image.width();

  image
    .as_raw()
    .as_chunks::<4>()
    .0
    .iter()
    .enumerate()
    .filter(|(_, pixel)| pixel[3] > 0 && pixel[0].min(pixel[1]).min(pixel[2]) < 160)
    .fold((0, 0), |(right, bottom), (index, _)| {
      let index = index as u32;
      (right.max(index % width), bottom.max(index / width))
    })
}

#[test]
fn test_isolation_keeps_ink_that_reaches_past_the_box() {
  for (name, effect) in CASES {
    let plain = ink_extent(&render_case(effect, 1.0));
    let isolated = ink_extent(&render_case(effect, 0.99));

    assert!(plain.0 > 0 && plain.1 > 0, "{name}: no ink painted");
    assert!(
      plain.0.abs_diff(isolated.0) <= 1 && plain.1.abs_diff(isolated.1) <= 1,
      "{name}: isolation clipped the ink to {isolated:?}, expected {plain:?}",
    );
  }
}
