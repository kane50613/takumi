use takumi::{prelude::*, render};

use crate::test_utils::CONTEXT;

const CANVAS: (u32, u32) = (480, 320);

/// Ink that reaches past the border box: an isolation layer sized from the
/// node's paint bounds must keep all of it. The expected extents are those of
/// a viewport-sized layer, which cannot clip; text cases vary by font, so they
/// only compare the isolated render against the plain one.
struct Case {
  name: &'static str,
  css: &'static str,
  expected: Option<(u32, u32)>,
}

const CASES: &[Case] = &[
  Case {
    name: "box-shadow",
    css: ".ink { box-shadow: 40px 40px 20px 10px black; }",
    expected: Some((292, 232)),
  },
  Case {
    name: "outline",
    css: ".ink { outline: 12px solid black; outline-offset: 24px; }",
    expected: Some((275, 215)),
  },
  Case {
    name: "text-shadow",
    css: ".ink { text-shadow: 40px 40px 8px black; }",
    expected: None,
  },
  Case {
    name: "text-stroke",
    css: ".ink { -webkit-text-stroke: 12px black; color: transparent; }",
    expected: None,
  },
  Case {
    name: "filter-blur",
    css: ".ink { filter: blur(16px); background: black; }",
    expected: Some((244, 184)),
  },
  Case {
    name: "drop-shadow",
    css: ".ink { filter: drop-shadow(40px 40px 12px black); background: black; }",
    expected: Some((281, 221)),
  },
  Case {
    name: "clip-path",
    css: ".ink { clip-path: circle(60%); background: black; }",
    expected: Some((239, 179)),
  },
  Case {
    name: "child-drop-shadow",
    css: ".ink { filter: brightness(1); } .child { width: 120px; height: 60px; filter: drop-shadow(40px 40px 12px black); background: black; }",
    expected: Some((241, 186)),
  },
  Case {
    name: "child-text-stroke",
    css: ".ink { -webkit-text-stroke: 1px black; } .child { -webkit-text-stroke: 12px black; color: transparent; }",
    expected: None,
  },
];

fn render_case(effect: &str, opacity: f32) -> Bitmap {
  let css = format!(
    ".ink {{ display: flex; width: 160px; height: 100px; margin: 80px; font-size: 72px; color: black; opacity: {opacity}; }} {effect}"
  );
  let child = Node::container([Node::text("Ink")]).with_class_name("child");
  let root = Node::container([Node::container([child]).with_class_name("ink")]).with_style(
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
  for Case {
    name,
    css,
    expected,
  } in CASES
  {
    let plain = ink_extent(&render_case(css, 1.0));
    let isolated = ink_extent(&render_case(css, 0.99));

    assert!(plain.0 > 0 && plain.1 > 0, "{name}: no ink painted");
    if let Some(expected) = expected {
      assert_eq!(plain, *expected, "{name}: the layer clipped the ink");
    }
    assert!(
      plain.0.abs_diff(isolated.0) <= 1 && plain.1.abs_diff(isolated.1) <= 1,
      "{name}: isolation clipped the ink to {isolated:?}, expected {plain:?}",
    );
  }
}
