use std::collections::BTreeMap;

use takumi::prelude::*;

use crate::test_utils::{measure_with_css, run_texts};

fn element<const N: usize>(class: &str, children: [Node; N]) -> Node {
  Node::container(children)
    .with_class_name(class)
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)))
}

fn counted(css: &str, root: Node) -> Vec<String> {
  run_texts(&measure_with_css(root, css))
    .into_iter()
    .map(str::to_owned)
    .collect()
}

const ITEMS: &str = ".item { counter-increment: c } .item::before { content: counter(c) }";

#[test]
fn siblings_share_the_counter_the_first_increment_creates() {
  let root = element(
    "root",
    [
      element("item", []),
      element("item", []),
      element("item", []),
    ],
  );

  assert_eq!(counted(ITEMS, root), ["1", "2", "3"]);
}

#[test]
fn counters_joins_the_nested_scopes_outermost_first() {
  let css = ".list { counter-reset: item } .item { counter-increment: item }
    .item::before { content: counters(item, '.') }";
  let nested = element("list", [element("item", []), element("item", [])]);
  let root = element(
    "list",
    [
      element("item", []),
      element("item", [nested]),
      element("item", []),
    ],
  );

  assert_eq!(counted(css, root), ["1", "2", "2.1", "2.2", "3"]);
}

#[test]
fn after_reads_the_counter_its_descendants_leave() {
  let css = ".root { counter-reset: c } .item { counter-increment: c }
    .root::after { content: counter(c) }";
  let root = element("root", [element("item", []), element("item", [])]);

  assert_eq!(counted(css, root), ["2"]);
}

#[test]
fn style_containment_keeps_its_counters_inside() {
  let css = format!("{ITEMS} .scope {{ contain: style }}");
  let scope = element("scope", [element("item", []), element("item", [])]);
  let root = element("root", [element("item", []), scope, element("item", [])]);

  assert_eq!(counted(&css, root), ["1", "1", "2", "2"]);
}

#[test]
fn counter_set_replaces_the_value_and_styles_write_it() {
  let css = ".item { counter-increment: c } .jump { counter-set: c 9 }
    .item::before { content: counter(c, upper-roman) }";
  let jump = element("item", []).with_class_name("item jump");
  let root = element("root", [element("item", []), jump, element("item", [])]);

  assert_eq!(counted(css, root), ["I", "IX", "X"]);
}

#[test]
fn an_element_without_a_box_is_not_counted() {
  let css = format!("{ITEMS} .hidden {{ display: none }}");
  let hidden = Node::container([]).with_class_name("item hidden");
  let root = element("root", [element("item", []), hidden, element("item", [])]);

  assert_eq!(counted(&css, root), ["1", "2"]);
}

#[test]
fn list_items_count_the_list_item_counter_from_the_list_start() {
  let css =
    "li { display: list-item; list-style-type: none } li::before { content: counter(list-item) }";
  let item = || Node::container([]).with_tag_name("li");
  let list = Node::container([item(), item()])
    .with_tag_name("ol")
    .with_attributes(BTreeMap::from([("start".into(), "5".into())]));

  assert_eq!(counted(css, list), ["5", "6"]);
}

#[test]
fn a_counter_nothing_instantiates_reads_zero() {
  let css = ".root::before { content: counter(missing) '|' counters(missing, '.') }";

  assert_eq!(counted(css, element("root", [])), ["0|0"]);
}

#[test]
fn a_reversed_list_counts_the_list_item_counter_down() {
  let css =
    "li { display: list-item; list-style-type: none } li::before { content: counter(list-item) }";
  let item = || Node::container([]).with_tag_name("li");
  let list = Node::container([item(), item(), item()])
    .with_tag_name("ol")
    .with_attributes(BTreeMap::from([("reversed".into(), "".into())]));

  assert_eq!(counted(css, list), ["3", "2", "1"]);
}

#[test]
fn an_li_that_is_not_a_list_item_leaves_the_list_item_counter_alone() {
  let css = "li { display: list-item; list-style-type: none } .block { display: block }
    li::before { content: counter(list-item) }";
  let item = || Node::container([]).with_tag_name("li");
  let block = item().with_class_name("block");
  let list = Node::container([item(), block, item()]).with_tag_name("ol");

  assert_eq!(counted(css, list), ["1", "1", "2"]);
}
