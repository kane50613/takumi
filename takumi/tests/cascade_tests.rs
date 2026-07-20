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
  let root = Node::container([plain, outer]);
  let result = measure_with_css(root, r#".outer .probe { width: 150px; }"#);

  let default_width = result.children[0].children[0].width;
  assert_ne!(default_width, 150.0);
  assert_eq!(result.children[1].children[0].width, 150.0);
}
