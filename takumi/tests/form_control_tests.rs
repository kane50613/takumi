use takumi::prelude::*;

use crate::test_utils::{CONTEXT, create_test_viewport, run_texts};

const CSS: &str = "div { font-family: Geist; font-size: 16px } input { font-family: Geist }";

/// `html`, parsed with the element presets, under [`CSS`].
fn measure_html(html: &str) -> MeasuredNode {
  takumi::measure(
    RenderOptions::builder()
      .viewport(create_test_viewport())
      .node(Node::from_html(html, FromHtmlOptions::default()).unwrap())
      .stylesheet(StyleSheet::parse_loosy(CSS).into())
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap()
}

/// The boxes of the `<input>` elements in `<div>{inputs}</div>`.
fn inputs(inputs: &str) -> Vec<MeasuredNode> {
  measure_html(&format!("<div>{inputs}</div>")).children
}

fn editor(input: &MeasuredNode) -> &MeasuredNode {
  assert_eq!(input.children.len(), 1, "{input:#?}");
  &input.children[0]
}

fn assert_within(actual: f32, expected: f32, tolerance: f32) {
  assert!(
    (actual - expected).abs() <= tolerance,
    "expected {expected} +/- {tolerance}, got {actual}"
  );
}

#[test]
fn a_text_field_shows_its_value_inside_its_border_and_padding() {
  let [input] = inputs(r#"<input type="text" disabled value="-100.11" style="width: 150px">"#)
    .try_into()
    .unwrap();

  // `html.css` gives a text field a 2px border and 1px 2px padding, outside its content box.
  assert_eq!(input.width, 158.0);
  assert_eq!(editor(&input).width, 150.0);
  assert_eq!(run_texts(&input), ["-100.11"]);
}

#[test]
fn values_are_sanitized_and_passwords_masked() {
  let measured = inputs(concat!(
    r#"<input type="password" value="secret">"#,
    r#"<input value="a&#10;b&#13;c">"#,
    r#"<input type="email" value=" a@b.co ">"#,
    r#"<input type="number" value="abc">"#,
    r#"<input type="number" value="-1.5e3">"#,
  ));
  let texts: Vec<Vec<&str>> = measured.iter().map(run_texts).collect();

  assert_eq!(
    texts,
    [
      vec!["\u{2022}".repeat(6).as_str()],
      vec!["abc"],
      vec!["a@b.co"],
      vec![],
      vec!["-1.5e3"],
    ]
  );
}

#[test]
fn an_empty_text_field_keeps_a_line_and_its_baseline() {
  let [filled, empty] = inputs(r#"<input value="Hg"><input>"#).try_into().unwrap();

  assert_eq!(empty.height, filled.height);
  assert_eq!(empty.transform[5], filled.transform[5]);
}

#[test]
fn a_text_field_sits_on_the_baseline_of_its_line() {
  let measured = measure_html(r#"<div>Hg<input value="Hg" style="font-size: 16px"></div>"#);
  let input = &measured.children[0];
  let editor = editor(input);
  let line_text = &measured.runs[0];

  assert_within(
    editor.transform[5] + editor.runs[0].y,
    measured.transform[5] + line_text.y,
    0.01,
  );
}

#[test]
fn a_text_field_centers_its_editor_without_regard_to_overflow() {
  let [tall, short] = inputs(concat!(
    r#"<input value="x" style="height: 60px">"#,
    r#"<input value="x" style="height: 4px">"#,
  ))
  .try_into()
  .unwrap();

  for (input, content_height) in [(&tall, 60.0), (&short, 4.0)] {
    let editor = editor(input);
    let content_top = input.transform[5] + 3.0;

    assert_within(
      editor.transform[5] + editor.height / 2.0,
      content_top + content_height / 2.0,
      0.5,
    );
  }

  assert!(editor(&short).transform[5] < short.transform[5] + 3.0);
}

#[test]
fn the_size_attribute_sets_the_width_of_text_fields_but_not_number_fields() {
  let widths: Vec<f32> = inputs(concat!(
    "<input>",
    r#"<input size="5">"#,
    r#"<input size="0">"#,
    r#"<input size="x">"#,
    r#"<input size=" +5px">"#,
    r#"<input type="number" size="5">"#,
  ))
  .iter()
  .map(|input| input.width)
  .collect();
  let [default, five, zero, invalid, lenient, number] = widths.try_into().unwrap();

  assert!(five < default);
  assert_eq!(zero, default);
  assert_eq!(invalid, default);
  assert_eq!(lenient, five);
  assert_eq!(number, default);
}

#[test]
fn a_text_field_lays_out_as_a_block_container_whatever_its_display() {
  let [inline] = inputs(r#"<input style="display: inline; width: 100px">"#)
    .try_into()
    .unwrap();
  let [flex] = inputs(r#"<input style="display: flex">"#)
    .try_into()
    .unwrap();

  assert_eq!(inline.width, 108.0);
  assert_eq!(flex.width, 1200.0);
  assert_eq!(editor(&flex).width, 1192.0);
}

#[test]
fn a_hidden_input_takes_no_box() {
  let measured = measure_html(r#"<div><input type="hidden" value="secret"></div>"#);

  assert!(run_texts(&measured).is_empty());
  assert_eq!(measured.height, 0.0);
}

#[test]
fn a_text_field_ellipsizes_a_value_wider_than_it() {
  let [input] = inputs(
    r#"<input disabled value="What is Lorem Ipsum? Lorem Ipsum is dummy text" style="width: 80px; text-overflow: ellipsis">"#,
  )
  .try_into()
  .unwrap();
  let texts = run_texts(&input);

  assert!(
    texts.last().is_some_and(|text| text.ends_with('…')),
    "{texts:?}"
  );
  assert!(!texts.concat().contains("dummy text"), "{texts:?}");
}
