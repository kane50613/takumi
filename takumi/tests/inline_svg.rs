//! An inline `<svg>` in HTML styled by the page, as Blink styles the SVG elements of an HTML
//! document.

use takumi::{
  prelude::{FromHtml, FromHtmlOptions, Node, RenderOptions, StyleSheet, Viewport},
  render,
};

use crate::test_utils::CONTEXT;

/// The straight-alpha pixel at the center of a 10×10 render of `html` under `css`.
fn center(html: &str, css: &str) -> [u8; 4] {
  let node = Node::from_html(html, FromHtmlOptions::default()).unwrap();
  let bitmap = render(
    RenderOptions::builder()
      .viewport(Viewport::new((10, 10)))
      .node(node)
      .fonts(&CONTEXT)
      .stylesheet(StyleSheet::parse(css).unwrap().into())
      .build(),
  )
  .unwrap();
  let offset = (5 * bitmap.width() as usize + 5) * 4;

  bitmap.as_raw()[offset..offset + 4].try_into().unwrap()
}

const RECT: &str =
  r#"<svg width="10" height="10" style="display:block"><rect width="10" height="10"/></svg>"#;

#[test]
fn page_rules_style_the_elements_of_an_inline_svg() {
  assert_eq!(
    center(RECT, "rect { fill: rgb(0, 128, 0) }"),
    [0, 128, 0, 255]
  );
}

#[test]
fn an_inline_svg_inherits_the_style_of_its_box() {
  let html = format!(r#"<div style="fill: rgb(0, 0, 255)">{RECT}</div>"#);

  assert_eq!(center(&html, ""), [0, 0, 255, 255]);
}

#[test]
fn the_svg_style_element_wins_over_the_page_at_equal_specificity() {
  let html = r#"<svg width="10" height="10" style="display:block"><style>rect { fill: rgb(0, 255, 0) }</style><rect width="10" height="10"/></svg>"#;

  assert_eq!(
    center(html, "rect { fill: rgb(255, 0, 0) }"),
    [0, 255, 0, 255]
  );
}

#[test]
fn the_root_opacity_applies_once() {
  let html = format!(r#"<div style="background: white">{RECT}</div>"#);

  assert_eq!(center(&html, "svg { opacity: 0.5 }"), [128, 128, 128, 255]);
}

/// The columns of a 120×40 render of `html` that hold ink.
fn inked_columns(html: &str) -> Vec<usize> {
  let node = Node::from_html(html, FromHtmlOptions::default()).unwrap();
  let bitmap = render(
    RenderOptions::builder()
      .viewport(Viewport::new((120, 40)))
      .node(node)
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap();
  let pixels = bitmap.as_raw();

  (0..120)
    .filter(|x| (0..40).any(|y| pixels[(y * 120 + x) * 4 + 3] > 0))
    .collect()
}

/// A `tspan` opens the bidi controls its `unicode-bidi` and `direction` call for, as an inline
/// box does in Blink, so an override reverses its characters. Shaping right to left places a
/// glyph up to a pixel apart.
#[test]
fn a_tspan_override_reorders_its_characters() {
  let text = |body: &str| {
    inked_columns(&format!(
      r#"<svg width="120" height="40" style="display:block"><text x="10" y="30" font-size="24">{body}</text></svg>"#
    ))
  };
  let overridden =
    text(r#"a<tspan style="unicode-bidi: bidi-override; direction: rtl">bc</tspan>"#);
  let reversed = text("a<tspan>cb</tspan>");
  let apart = overridden
    .iter()
    .filter(|column| !reversed.contains(column))
    .chain(
      reversed
        .iter()
        .filter(|column| !overridden.contains(column)),
    )
    .count();

  assert_ne!(overridden, text("a<tspan>bc</tspan>"));
  assert!(apart <= 2, "{overridden:?} {reversed:?}");
}
