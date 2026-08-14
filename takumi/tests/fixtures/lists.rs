use takumi::prelude::*;

use crate::test_utils::{CONTEXT, attrs, create_test_viewport, run_fixture_test_with_options};

fn item(text: &str) -> Node {
  Node::container([Node::text(text.to_string())]).with_class_name("item")
}

fn list(class: &str, items: impl Into<Vec<Node>>) -> Node {
  Node::container(items).with_class_name(format!("list {class}"))
}

fn cell(label: &str, content: Node) -> Node {
  Node::container([
    Node::text(label.to_string()).with_class_name("label"),
    content,
  ])
  .with_class_name("cell")
}

const CSS: &str = r#"
  .root {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    padding: 16px;
    width: 100%;
    height: 100%;
    background: rgb(243, 244, 246);
    align-content: flex-start;
    font-family: "Geist";
  }
  .cell {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 282px;
    height: 190px;
    background: white;
    border-radius: 8px;
    padding: 10px 12px;
  }
  .label {
    font-size: 11px;
    color: rgb(100, 116, 139);
    font-family: "Geist Mono";
  }
  .list {
    display: block;
    padding-left: 40px;
    font-size: 16px;
    color: rgb(17, 24, 39);
  }
  .item {
    display: list-item;
  }
  .decimal { list-style-type: decimal; }
  .disc { list-style-type: disc; }
  .circle { list-style-type: circle; }
  .roman { list-style-type: upper-roman; }
  .alpha { list-style-type: lower-alpha; }
  .padded { list-style-type: decimal-leading-zero; }
  .arrow { list-style-type: "→ "; }
  .hidden { list-style-type: none; }
  .inside { list-style-position: inside; }
  .image { list-style-image: radial-gradient(rgb(59, 130, 246), rgb(37, 99, 235)); }
  .nested { padding-left: 24px; }
  .block { display: block; }
"#;

fn nested_ordered_cell() -> Node {
  let inner = list(
    "decimal nested",
    [item("inner line 1"), item("inner line 2")],
  );
  let outer = list(
    "decimal",
    [
      item("line 1"),
      Node::container([Node::text("line 2".to_string()), inner]).with_class_name("item"),
    ],
  );

  cell("nested ordered lists", outer)
}

fn bullets_cell() -> Node {
  let nested = list("circle nested", [item("circle"), item("circle")]);

  cell("bullets", list("disc", [item("disc"), nested]))
}

fn counter_styles_cell() -> Node {
  let lists = Node::container([
    list("roman", [item("upper roman"), item("upper roman")]),
    list("alpha", [item("lower alpha"), item("lower alpha")]),
    list("padded", [item("leading zero")]),
    list("arrow", [item("string marker")]),
    list("hidden", [item("none")]),
  ]);

  cell("counter styles", lists)
}

fn inside_cell() -> Node {
  let lists = Node::container([
    list("decimal inside", [item("inside"), item("inside, wrapping")]),
    list("disc inside", [item("inside")]),
  ]);

  cell("list-style-position: inside", lists)
}

fn ordinals_cell() -> Node {
  let started =
    list("decimal", [item("start at 3"), item("4")]).with_attributes(attrs(&[("start", "3")]));
  let valued = list(
    "decimal",
    [
      item("1"),
      Node::container([Node::text("value=9".to_string())])
        .with_class_name("item")
        .with_attributes(attrs(&[("value", "9")])),
      item("10"),
    ],
  );

  cell("start / value", Node::container([started, valued]))
}

fn block_content_cell() -> Node {
  let items = list(
    "decimal",
    [
      Node::container([
        Node::container([Node::text("block child".to_string())]).with_class_name("block")
      ])
      .with_class_name("item"),
      item("text child"),
    ],
  );

  cell("block-level item content", items)
}

fn marker_image_cell() -> Node {
  let lists = Node::container([
    list("image", [item("gradient marker"), item("gradient marker")]),
    list("image inside", [item("inside")]),
  ]);

  cell("list-style-image", lists)
}

#[test]
fn test_list_markers() {
  let root = Node::container([
    nested_ordered_cell(),
    bullets_cell(),
    counter_styles_cell(),
    inside_cell(),
    ordinals_cell(),
    marker_image_cell(),
    block_content_cell(),
  ])
  .with_class_name("root");

  let options = RenderOptions::builder()
    .viewport(create_test_viewport())
    .node(root)
    .fonts(&CONTEXT)
    .stylesheet(StyleSheet::parse(CSS).unwrap().into())
    .build();

  run_fixture_test_with_options(options, "list_markers");
}
