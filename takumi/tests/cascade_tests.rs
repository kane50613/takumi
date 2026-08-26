mod test_utils;

use takumi::{measure, prelude::*};
use test_utils::CONTEXT;

fn measure_with_css(node: Node, css: &str) -> MeasuredNode {
  let stylesheet = StyleSheet::parse_loosy(css);
  measure(
    RenderOptions::builder()
      .viewport(Viewport::new((1200, 630)))
      .node(node)
      .stylesheet(stylesheet.into())
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap()
}

fn block(class: &str) -> Node {
  Node::container([])
    .with_class_name(class)
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)))
}

#[test]
fn important_wins_over_higher_specificity_normal() {
  let root = Node::container([block("box")]);
  let result = measure_with_css(
    root,
    r#"
      .box { width: 100px !important; }
      div.box { width: 200px; }
    "#,
  );
  assert_eq!(result.children[0].width, 100.0);
}

#[test]
fn empty_rule_blocks_do_not_disturb_the_cascade() {
  let root = Node::container([block("box")]);
  let result = measure_with_css(
    root,
    r#"
      .box {}
      .box { width: 120px; }
      div.box {}
    "#,
  );
  assert_eq!(result.children[0].width, 120.0);
}

#[test]
fn descendant_selector_matches_after_sibling_subtrees() {
  let plain = Node::container([block("probe")]);
  let outer = Node::container([block("probe")]).with_class_name("outer");
  let trailing = Node::container([block("probe")]);
  let root = Node::container([plain, outer, trailing]);
  let result = measure_with_css(root, r#".outer .probe { width: 150px; }"#);

  let default_width = result.children[0].children[0].width;
  assert_ne!(default_width, 150.0);
  assert_eq!(result.children[1].children[0].width, 150.0);
  assert_eq!(result.children[2].children[0].width, default_width);
}

fn tw_block(class: &str, tw: &str) -> Node {
  use std::str::FromStr;

  Node::container([])
    .with_class_name(class)
    .with_tw(TailwindValues::from_str(tw).expect("tailwind values should parse"))
}

#[test]
fn tw_sits_below_author_rules() {
  let root = Node::container([tw_block("box", "block w-64")]);
  let result = measure_with_css(root, r#".box { width: 100px; }"#);

  assert_eq!(result.children[0].width, 100.0);
}

/// `tw` is the last declared layer, so its important half loses to every
/// important author rule and still beats their normal ones.
#[test]
fn important_tw_beats_normal_author_rules() {
  let root = Node::container([
    tw_block("box", "block w-64!"),
    tw_block("shout", "block w-64!"),
  ]);
  let result = measure_with_css(
    root,
    r#"
      .box { width: 100px; }
      .shout { width: 100px !important; }
    "#,
  );

  assert_eq!(result.children[0].width, 256.0);
  assert_eq!(result.children[1].width, 100.0);
}

#[test]
fn tw_reads_theme_tokens_from_the_stylesheet() {
  let root = Node::container([
    tw_block("box", "block w-gutter"),
    tw_block("box", "block w-64"),
  ]);
  let result = measure_with_css(
    root,
    r#":root { --spacing-gutter: 10rem; --spacing: 0.5rem; }"#,
  );

  assert_eq!(result.children[0].width, 160.0);
  assert_eq!(result.children[1].width, 512.0);
}

/// `tw` is the last declared layer, so preflight wrapped in `@layer base`
/// resets defaults without beating utilities.
#[test]
fn tw_beats_named_layer_rules() {
  let root = Node::container([tw_block("box", "block w-64")]);
  let result = measure_with_css(
    root,
    r#"@layer base { *, ::after, ::before { box-sizing: border-box; margin: 0; padding: 0; border: 0 solid; } * { width: 50px; } }"#,
  );

  assert_eq!(result.children[0].width, 256.0);
}

/// A named layer's important half also outranks `tw`, which is declared last.
#[test]
fn important_layered_rules_beat_important_tw() {
  let result = measure_with_css(
    Node::container([tw_block("box", "block w-64!")]),
    "@layer base { .box { width: 120px !important; } }",
  );

  assert_eq!(result.children[0].width, 120.0);
}
