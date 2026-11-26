use takumi::{from_html, needs, prelude::*};
use takumi_core::paint_tree::{PaintTreeOptions, paint_tree};

use crate::test_utils::CONTEXT;

const PAGE: &str = "display: flex; width: 600px; height: 200px; padding: 20px; background: #3070c0; color: white; font-size: 28px;";

type Entry = (String, String, IgnoredReason, DeclarationOrigin);

fn options(node: Node, css: &str) -> RenderOptions<'static> {
  RenderOptions::builder()
    .viewport(Viewport::new((600, 200)))
    .node(node)
    .fonts(&CONTEXT)
    .stylesheet(StyleSheet::parse_loosy(css).into())
    .build()
}

fn entries(declarations: &[IgnoredDeclaration]) -> Vec<Entry> {
  declarations
    .iter()
    .map(|declaration| {
      (
        declaration.property.clone(),
        declaration.value.clone(),
        declaration.reason,
        declaration.origin,
      )
    })
    .collect()
}

/// What a render of `html` under `css` reports it ignored.
fn ignored(html: &str, css: &str) -> Vec<Entry> {
  let node = from_html(html, FromHtmlOptions::default()).unwrap();

  entries(&needs(options(node, css)).ignored_declarations)
}

fn entry(property: &str, value: &str, reason: IgnoredReason, origin: DeclarationOrigin) -> Entry {
  (property.to_owned(), value.to_owned(), reason, origin)
}

#[test]
fn supported_css_reports_nothing() {
  let html = format!(r#"<div class="card" style="{PAGE}">Supported CSS.</div>"#);
  let css = "@media (min-width: 100px) { .card { border-radius: 8px } } @supports (display: grid) { .card { gap: 4px } }";

  assert_eq!(ignored(&html, css), []);
}

#[test]
fn unsupported_properties_in_a_style_attribute() {
  let html = format!(
    r#"<div style="{PAGE} writing-mode: vertical-rl; column-count: 2; initial-letter: 2; text-emphasis: dot; shape-outside: circle(50%);">Unsupported CSS should not pass in silence.</div>"#
  );
  let attribute = |property, value| {
    entry(
      property,
      value,
      IgnoredReason::UnknownProperty,
      DeclarationOrigin::StyleAttribute,
    )
  };

  assert_eq!(
    ignored(&html, ""),
    [
      attribute("writing-mode", "vertical-rl"),
      attribute("column-count", "2"),
      attribute("initial-letter", "2"),
      attribute("text-emphasis", "dot"),
      attribute("shape-outside", "circle(50%)"),
    ]
  );
}

#[test]
fn an_invalid_value_in_a_style_attribute() {
  assert_eq!(
    ignored(
      r#"<div style="color: red; filter: blurry(4px)">Text</div>"#,
      ""
    ),
    [entry(
      "filter",
      "blurry(4px)",
      IgnoredReason::InvalidValue,
      DeclarationOrigin::StyleAttribute,
    )]
  );
}

#[test]
fn a_style_sheet_reports_its_at_rules_and_declarations() {
  let css = "@counter-style thumbs { system: cyclic; symbols: '+' }
    .card { filter: blurry(4px); widht: 10px }
    @media (min-width: 1px) { .card { text-emphasis: dot } }
    @keyframes spin { to { initial-letter: 2 } }";
  let sheet =
    |property, value, reason| entry(property, value, reason, DeclarationOrigin::StyleSheet);

  let mut reported = ignored(r#"<div class="card">Text</div>"#, css);

  reported.sort();

  let mut expected = vec![
    sheet("@counter-style", "thumbs", IgnoredReason::UnknownAtRule),
    sheet("filter", "blurry(4px)", IgnoredReason::InvalidValue),
    sheet("widht", "10px", IgnoredReason::UnknownProperty),
    sheet("text-emphasis", "dot", IgnoredReason::UnknownProperty),
    sheet("initial-letter", "2", IgnoredReason::UnknownProperty),
  ];

  expected.sort();

  assert_eq!(reported, expected);
}

#[test]
fn an_unknown_key_in_a_style_object() {
  let node: Node = serde_json::from_value(serde_json::json!({
    "type": "container",
    "style": { "display": "flex", "writingMode": "vertical-rl", "width": -4 },
    "children": [],
  }))
  .unwrap();

  let mut reported = entries(&needs(options(node, "")).ignored_declarations);

  reported.sort();

  assert_eq!(
    reported,
    [
      entry(
        "width",
        "-4",
        IgnoredReason::InvalidValue,
        DeclarationOrigin::StyleObject,
      ),
      entry(
        "writingMode",
        "vertical-rl",
        IgnoredReason::UnknownProperty,
        DeclarationOrigin::StyleObject,
      ),
    ]
  );
}

#[test]
fn each_declaration_is_reported_once() {
  let html = r#"<div><p style="writing-mode: vertical-rl">A</p><p style="writing-mode: vertical-rl">B</p><p class="x">C</p><p class="x">D</p></div>"#;

  assert_eq!(
    ignored(html, ".x { widht: 1px } .y { widht: 1px }"),
    [
      entry(
        "widht",
        "1px",
        IgnoredReason::UnknownProperty,
        DeclarationOrigin::StyleSheet,
      ),
      entry(
        "writing-mode",
        "vertical-rl",
        IgnoredReason::UnknownProperty,
        DeclarationOrigin::StyleAttribute,
      ),
    ]
  );
}

#[test]
fn the_paint_tree_lists_what_was_ignored() {
  let node = from_html(
    r#"<div style="writing-mode: vertical-rl">Text</div>"#,
    FromHtmlOptions::default(),
  )
  .unwrap();
  let tree = paint_tree(
    PaintTreeOptions::builder()
      .viewport(Viewport::new((200, 100)))
      .node(node)
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap();

  assert_eq!(
    entries(&tree.document.ignored_declarations),
    [entry(
      "writing-mode",
      "vertical-rl",
      IgnoredReason::UnknownProperty,
      DeclarationOrigin::StyleAttribute,
    )]
  );
}
