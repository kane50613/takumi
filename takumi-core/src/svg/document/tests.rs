use super::*;
use crate::{
  style::{Color, FillRule},
  svg::style::Paint,
};

const OPTIONS: ParseOptions = ParseOptions {
  style: StyleContext {
    font_size: 12.0,
    dpi: 96.0,
  },
  style_sheet: None,
};

fn parse(markup: &str) -> Document {
  Document::parse(markup, &OPTIONS).unwrap()
}

fn by_id<'a>(document: &'a Document, id: &str) -> Node<'a> {
  document.element_by_id(id).unwrap()
}

#[test]
fn style_sheet_attribute_and_inline_style_cascade_in_order() {
  let document = parse(
    r##"<svg xmlns="http://www.w3.org/2000/svg">
      <style>#a { fill: #00f } rect { fill: #0f0; stroke: #f00 !important }</style>
      <rect id="a" fill="#ff0" style="stroke: #000; opacity: .5"/>
      <rect id="b" fill="#ff0"/>
    </svg>"##,
  );
  let a = by_id(&document, "a").style().unwrap();
  let b = by_id(&document, "b").style().unwrap();

  assert_eq!(a.fill, Paint::Color(Color([0, 0, 255, 255])));
  assert_eq!(a.stroke, Paint::Color(Color([255, 0, 0, 255])));
  assert_eq!(a.opacity, 0.5);
  assert_eq!(b.fill, Paint::Color(Color([0, 255, 0, 255])));
}

#[test]
fn inherited_properties_pass_down_and_the_others_reset() {
  let document = parse(
    r##"<svg xmlns="http://www.w3.org/2000/svg" fill-rule="evenodd" opacity=".5">
      <g id="g" stroke-width="3"><rect id="r"/></g>
    </svg>"##,
  );
  let rect = by_id(&document, "r").style().unwrap();

  assert_eq!(rect.fill_rule, FillRule::EvenOdd);
  assert_eq!(rect.stroke_width.number, 3.0);
  assert_eq!(rect.opacity, 1.0);
}

#[test]
fn use_copies_its_target_below_itself_and_inherits_from_it() {
  let document = parse(
    r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
      <defs><rect id="t" width="1" height="1"/></defs>
      <use id="u" xlink:href="#t" fill="#00f"/>
    </svg>"##,
  );
  let copy = by_id(&document, "u").first_element_child().unwrap();

  assert_eq!(copy.tag(), Some(Tag::Rect));
  assert_eq!(copy.element_id(), "");
  assert_eq!(
    copy.style().unwrap().fill,
    Paint::Color(Color([0, 0, 255, 255]))
  );
  assert_eq!(
    by_id(&document, "t").parent().unwrap().tag(),
    Some(Tag::Defs)
  );
}

#[test]
fn recursive_use_is_dropped() {
  let document = parse(
    r##"<svg xmlns="http://www.w3.org/2000/svg">
      <g id="g"><use id="u" href="#g"/></g>
    </svg>"##,
  );

  assert!(!by_id(&document, "u").has_children());
}

#[test]
fn unknown_elements_and_their_children_are_skipped() {
  let document = parse(
    r##"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject><rect id="r"/></foreignObject></svg>"##,
  );

  assert!(document.element_by_id("r").is_none());
}

#[test]
fn text_collapses_white_space_across_tspans() {
  let document = parse(
    r##"<svg xmlns="http://www.w3.org/2000/svg"><text id="t">  a  <tspan> b </tspan>
      c </text></svg>"##,
  );
  let text: String = by_id(&document, "t")
    .descendants()
    .filter(Node::is_text)
    .map(|node| node.text())
    .collect();

  assert_eq!(text, "a b c");
}

#[test]
fn the_root_must_be_svg() {
  assert!(matches!(
    Document::parse(r#"<g xmlns="http://www.w3.org/2000/svg"/>"#, &OPTIONS),
    Err(ParseError::NotSvg)
  ));
}

#[test]
fn the_host_color_is_needed_only_where_color_is_unset() {
  let current =
    parse(r#"<svg xmlns="http://www.w3.org/2000/svg"><rect fill="currentColor"/></svg>"#);
  let own = parse(
    r#"<svg xmlns="http://www.w3.org/2000/svg" color="red"><rect fill="currentColor"/></svg>"#,
  );

  assert!(current.uses_host_color());
  assert!(!own.uses_host_color());
}
