mod test_utils;

use std::collections::BTreeMap;

use takumi::prelude::{Length::*, *};
use test_utils::{block, measure_with_css, run_texts};

#[test]
fn before_string_content_inserts_text_at_start() {
  let root = Node::container([Node::text("body".to_string())])
    .with_class_name("greet")
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)));
  let result = measure_with_css(root, r#".greet::before { content: "hello"; }"#);
  let text = run_texts(&result).concat();
  assert!(
    text.starts_with("hello"),
    "expected leading 'hello' in {text:?}"
  );
}

#[test]
fn after_string_content_inserts_text_at_end() {
  let root = Node::container([Node::text("body".to_string())])
    .with_class_name("greet")
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)));
  let result = measure_with_css(root, r#".greet::after { content: "bye"; }"#);
  let text = run_texts(&result).concat();
  assert!(text.ends_with("bye"), "expected trailing 'bye' in {text:?}");
}

#[test]
fn before_and_after_around_existing_text() {
  let root = Node::container([Node::text("middle".to_string())])
    .with_class_name("box")
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)));
  let result = measure_with_css(
    root,
    r#"
      .box::before { content: "[ "; }
      .box::after  { content: " ]"; }
    "#,
  );
  let text = run_texts(&result).concat();
  assert!(text.contains("[ "), "expected '[ ' prefix in {text:?}");
  assert!(text.contains("middle"), "expected 'middle' in {text:?}");
  assert!(text.contains(" ]"), "expected ' ]' suffix in {text:?}");
}

#[test]
fn content_normal_or_none_creates_no_pseudo_box() {
  let root = block("greet");
  let result_none = measure_with_css(root.clone(), r#".greet::before { content: none; }"#);
  let result_normal = measure_with_css(root, r#".greet::before { content: normal; }"#);

  assert!(run_texts(&result_none).is_empty());
  assert!(run_texts(&result_normal).is_empty());
}

#[test]
fn attr_resolves_against_originating_attributes() {
  let root = Node::container([])
    .with_class_name("badge")
    .with_attributes(BTreeMap::from([("data-label".into(), "alpha".into())]))
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)));
  let result = measure_with_css(root, r#".badge::before { content: attr(data-label); }"#);
  let runs = run_texts(&result);
  assert!(runs.contains(&"alpha"), "runs = {runs:?}");
}

#[test]
fn attr_resolves_id_against_structured_metadata() {
  let root = Node::container([])
    .with_id("hero")
    .with_class_name("badge")
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)));
  let result = measure_with_css(root, r#".badge::before { content: attr(id); }"#);
  let runs = run_texts(&result);
  assert!(runs.contains(&"hero"), "runs = {runs:?}");
}

#[test]
fn attr_uses_fallback_when_attribute_is_missing() {
  let root = block("badge");
  let result = measure_with_css(
    root,
    r#".badge::before { content: attr(missing, "fallback-text"); }"#,
  );
  let runs = run_texts(&result);
  assert!(runs.contains(&"fallback-text"), "runs = {runs:?}");
}

#[test]
fn empty_string_content_creates_no_pseudo_box() {
  let root = block("greet");
  let result = measure_with_css(root, r#".greet::before { content: ""; }"#);
  assert!(run_texts(&result).is_empty());
}

#[test]
fn unsupported_content_value_creates_no_pseudo_box() {
  let root = block("greet");
  let result = measure_with_css(
    root,
    r#".greet::before { content: counter(foo); color: red; }"#,
  );
  assert!(run_texts(&result).is_empty());
}

#[test]
fn pseudo_does_not_apply_to_replaced_image_element() {
  // data: URI keeps the test self-contained.
  let png_bytes: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a,
    0x0a, // PNG signature only — measurement
         // will fall back to 0x0 once decode fails, which is fine for this test.
  ];
  let root = Node::image(png_bytes.to_vec())
    .with_class_name("logo")
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Px(50.0)))
        .with(StyleDeclaration::height(Px(50.0))),
    );
  let result = measure_with_css(root, r#".logo::before { content: "x"; }"#);
  assert!(run_texts(&result).is_empty());
}

#[test]
fn display_none_on_pseudo_creates_no_box() {
  let root = block("greet");
  let result = measure_with_css(root, r#".greet::before { content: "x"; display: none; }"#);
  assert!(run_texts(&result).is_empty());
}

#[test]
fn display_flex_pseudo_downgrades_to_block() {
  let root = Node::container([Node::text("body".to_string())])
    .with_class_name("card")
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::width(Px(200.0))),
    );
  let block = measure_with_css(
    root.clone(),
    r#".card::before { content: "header"; display: block; }"#,
  );
  let flex = measure_with_css(
    root,
    r#".card::before { content: "header"; display: flex; }"#,
  );
  assert_eq!(
    flex.height, block.height,
    "flex pseudo should match block; got flex={flex:?} block={block:?}"
  );
}

#[test]
fn display_block_pseudo_creates_block_level_box() {
  let root = Node::container([Node::text("body".to_string())])
    .with_class_name("card")
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::width(Px(200.0))),
    );
  let inline = measure_with_css(
    root.clone(),
    r#"
      .card::before { content: "header"; }
    "#,
  );
  let block = measure_with_css(
    root,
    r#"
      .card::before { content: "header"; display: block; }
    "#,
  );

  // `display: block` should force the pseudo onto its own line, making the
  // container strictly taller than the inline-flow variant.
  assert!(
    block.height > inline.height,
    "expected block ({}) > inline ({})",
    block.height,
    inline.height
  );
}

#[test]
fn gradient_content_renders_with_default_object_size() {
  let root = block("hero").with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Px(400.0))),
  );
  let result = measure_with_css(
    root,
    r#".hero::before { content: linear-gradient(red, blue); display: block; }"#,
  );
  // css-images-3 §5.1 default object size for gradients: 300x150.
  assert_eq!(result.height, 150.0, "result = {result:?}");
}
