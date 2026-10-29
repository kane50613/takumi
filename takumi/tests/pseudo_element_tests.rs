use std::collections::BTreeMap;

use takumi::prelude::{Length::*, *};

use crate::test_utils::{block, measure_with_css, run_texts};

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
fn content_from_a_variable_or_a_split_rule_creates_a_pseudo_box() {
  let root = Node::container([Node::text("body".to_string())])
    .with_class_name("greet")
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)));
  let from_variable = measure_with_css(
    root.clone(),
    r#".greet { --label: "hello"; } .greet::before { content: var(--label); }"#,
  );
  let from_split_rule = measure_with_css(
    root,
    r#"*, ::before { color: red; } .greet::before { content: "hello"; }"#,
  );

  assert!(run_texts(&from_variable).concat().starts_with("hello"));
  assert!(run_texts(&from_split_rule).concat().starts_with("hello"));
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
    r#".greet::before { content: leader(dotted); color: red; }"#,
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

#[test]
fn tw_before_and_after_variants_generate_content() {
  let root = Node::container([Node::text("body".to_string())])
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)))
    .with_tw(
      "before:content-['hello_'] after:content-['_bye']"
        .parse()
        .unwrap(),
    );
  let text = run_texts(&measure_with_css(root, "")).concat();

  assert_eq!(text, "hello body bye");
}

/// A stylesheet rule without `content` still styles the box a `tw` utility gives content to.
#[test]
fn tw_content_keeps_the_stylesheet_rules_for_its_pseudo_element() {
  let root = Node::container([Node::text("body".to_string())])
    .with_class_name("greet")
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)))
    .with_tw("before:content-['hi_']".parse().unwrap());
  let text = run_texts(&measure_with_css(
    root,
    ".greet::before { text-transform: uppercase }",
  ))
  .concat();

  assert_eq!(text, "HI body");
}

#[test]
fn tw_structural_and_pseudo_element_variants_combine() {
  let item = |text: &str| {
    Node::container([Node::text(text.to_string())])
      .with_style(Style::default().with(StyleDeclaration::display(Display::Block)))
      .with_tw("first:before:content-['→_']".parse().unwrap())
  };
  let root = Node::container([item("one"), item("two")])
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)));

  assert_eq!(run_texts(&measure_with_css(root, "")), ["→ ", "one", "two"]);
}

/// Tailwind opens `before:` with an empty `content`, so a utility alone makes the box.
#[test]
fn tw_before_without_content_generates_an_empty_box() {
  let root = Node::container([])
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)))
    .with_tw(
      "before:block before:w-[10px] before:h-[10px]"
        .parse()
        .unwrap(),
    );
  let measured = measure_with_css(root, "");

  assert_eq!(measured.children.len(), 1);
  assert_eq!(
    (measured.children[0].width, measured.children[0].height),
    (10.0, 10.0)
  );
}

#[test]
fn tw_pseudo_elements_skip_replaced_elements() {
  let png_signature: &[u8] = &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  let root = Node::image(png_signature.to_vec())
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Px(50.0)))
        .with(StyleDeclaration::height(Px(50.0))),
    )
    .with_tw("before:content-['x'] after:block".parse().unwrap());
  let measured = measure_with_css(root, "");

  assert!(run_texts(&measured).is_empty());
  assert!(measured.children.is_empty());
}

/// A flex container's own text is an anonymous flex item beside its `::after`, as it is beside a
/// `::before`.
#[test]
fn own_text_is_a_flex_item_beside_after() {
  let root = Node::text("Default content".to_string()).with_class_name("row");
  let measured = measure_with_css(
    root,
    ".row { display: flex; align-items: center; height: 64px } \
     .row::after { content: ''; width: 16px; height: 16px }",
  );

  assert!(measured.runs.is_empty());
  assert_eq!(measured.children.len(), 2);
  assert_eq!(run_texts(&measured.children[0]), ["Default content"]);
}

/// White space a flex container holds is no flex item beside its `::after`, even preserved.
#[test]
fn own_white_space_is_no_flex_item_beside_after() {
  let css = ".row { display: flex; height: 64px } \
     .row::after { content: ''; width: 16px; height: 16px } \
     .pre { white-space: pre }";

  let collapsed = measure_with_css(Node::text("  ".to_string()).with_class_name("row"), css);
  let preserved = measure_with_css(Node::text("  ".to_string()).with_class_name("row pre"), css);

  assert_eq!(collapsed.children.len(), 1);
  assert_eq!(preserved.children.len(), 1);
}
