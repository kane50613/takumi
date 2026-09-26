use takumi::{prelude::*, render};

use crate::test_utils::{CONTEXT, InkBounds, ink_bounds};

const CANVAS: (u32, u32) = (480, 320);

/// Ink that reaches past the border box: an isolation layer sized from the
/// node's paint bounds must keep all of it. The expected extents are those of
/// a viewport-sized layer, which cannot clip; text cases vary by font, so they
/// only compare the isolated render against the plain one.
struct Case {
  name: &'static str,
  css: &'static str,
  expected: Option<InkBounds>,
}

const CASES: &[Case] = &[
  Case {
    name: "box-shadow",
    css: ".ink { box-shadow: 40px 40px 20px 10px black; }",
    expected: Some((87, 101, 292, 232)),
  },
  Case {
    name: "outline",
    css: ".ink { outline: 12px solid black; outline-offset: 24px; }",
    expected: Some((44, 44, 275, 215)),
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
    expected: Some((75, 75, 244, 184)),
  },
  Case {
    name: "drop-shadow",
    css: ".ink { filter: drop-shadow(40px 40px 12px black); background: black; }",
    expected: Some((80, 80, 281, 221)),
  },
  Case {
    name: "clip-path",
    css: ".ink { clip-path: circle(60%); background: black; }",
    expected: Some((80, 80, 239, 179)),
  },
  Case {
    name: "child-drop-shadow",
    css: ".ink { filter: brightness(1); } .child { width: 120px; height: 60px; filter: drop-shadow(40px 40px 12px black); background: black; }",
    expected: Some((80, 80, 241, 186)),
  },
  Case {
    name: "box-shadow-negative",
    css: ".ink { box-shadow: -40px -40px 20px 10px black; }",
    expected: Some((27, 27, 212, 152)),
  },
  Case {
    name: "drop-shadow-negative",
    css: ".ink { filter: drop-shadow(-40px -40px 12px black); background: black; }",
    expected: Some((38, 38, 239, 179)),
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

#[test]
fn test_isolation_keeps_ink_that_reaches_past_the_box() {
  for Case {
    name,
    css,
    expected,
  } in CASES
  {
    let plain = ink_bounds(&render_case(css, 1.0));
    let isolated = ink_bounds(&render_case(css, 0.99));

    assert!(plain.2 > 0 && plain.3 > 0, "{name}: no ink painted");
    if let Some(expected) = expected {
      assert_eq!(plain, *expected, "{name}: the layer clipped the ink");
    }
    let edges = [
      (plain.0, isolated.0),
      (plain.1, isolated.1),
      (plain.2, isolated.2),
      (plain.3, isolated.3),
    ];
    assert!(
      edges.iter().all(|(a, b)| a.abs_diff(*b) <= 1),
      "{name}: isolation clipped the ink to {isolated:?}, expected {plain:?}",
    );
  }
}
