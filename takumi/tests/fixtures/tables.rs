use takumi::prelude::*;

use crate::test_utils::{CONTEXT, attrs, create_test_viewport, run_fixture_test_with_options};

fn cell(label: &str, content: Node) -> Node {
  Node::container([
    Node::text(label.to_string()).with_class_name("label"),
    content,
  ])
  .with_class_name("cell")
}

fn td(text: &str) -> Node {
  Node::container([Node::text(text.to_string())]).with_class_name("td")
}

fn th(text: &str) -> Node {
  Node::container([Node::text(text.to_string())]).with_class_name("td th")
}

fn tr(cells: impl Into<Vec<Node>>) -> Node {
  Node::container(cells).with_class_name("tr")
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
  .table {
    display: table;
    width: 100%;
    font-size: 13px;
    color: rgb(17, 24, 39);
  }
  .thead { display: table-header-group; }
  .tbody { display: table-row-group; }
  .tfoot { display: table-footer-group; }
  .tr { display: table-row; }
  .td {
    display: table-cell;
    padding: 3px 8px;
    border: 1px solid rgb(203, 213, 225);
  }
  .th {
    background: rgb(226, 232, 240);
    font-weight: bold;
  }
  .caption {
    display: table-caption;
    font-size: 11px;
    color: rgb(100, 116, 139);
  }
  .stripe { background: rgb(241, 245, 249); }
  .w80 { width: 80px; }
"#;

fn group(class: &str, rows: impl Into<Vec<Node>>) -> Node {
  Node::container(rows).with_class_name(class)
}

/// tfoot and thead written mid-source; the render must reorder them.
fn group_order_cell() -> Node {
  let table = Node::container([
    Node::container([Node::text("caption above the grid".to_string())]).with_class_name("caption"),
    group("tfoot", [tr([td("total"), td("42")])]),
    group(
      "tbody",
      [
        tr([td("alpha"), td("1")]),
        tr([td("beta"), td("2")]).with_class_name("tr stripe"),
        tr([td("gamma"), td("3")]),
      ],
    ),
    group("thead", [tr([th("name"), th("count")])]),
  ])
  .with_class_name("table");

  cell("group order, caption, striped row", table)
}

fn declared_width_cell() -> Node {
  let table = Node::container([
    tr([
      td("80px column").with_class_name("td w80"),
      td("auto column takes the rest"),
    ]),
    tr([td("a"), td("b")]),
  ])
  .with_class_name("table");

  cell("declared column width", table)
}

fn spans_cell() -> Node {
  let table = Node::container([
    tr([
      th("spans both columns").with_attributes(attrs(&[("colspan", "2")])),
      th("tall").with_attributes(attrs(&[("rowspan", "2")])),
    ]),
    tr([td("left"), td("right")]),
    tr([td("a"), td("b"), td("c")]),
  ])
  .with_class_name("table");

  cell("colspan / rowspan", table)
}

#[test]
fn test_tables() {
  let root = Node::container([group_order_cell(), declared_width_cell(), spans_cell()])
    .with_class_name("root");

  let options = RenderOptions::builder()
    .viewport(create_test_viewport())
    .node(root)
    .fonts(&CONTEXT)
    .stylesheet(StyleSheet::parse(CSS).unwrap().into())
    .build();

  run_fixture_test_with_options(options, "tables");
}
