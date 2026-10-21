use std::{collections::BTreeMap, sync::Arc};

use crate::{
  context::RenderContext,
  fonts::shaping::testing::{GEIST, register},
  geometry::{AvailableSpace, ComputedLayout, NodeId, Point, Size},
  layout::{
    node::Node,
    tree::{LayoutResults, RenderNode},
  },
  resources::font::Fonts,
  style::{SizingContext, StyleSheet},
  viewport::Viewport,
};

const STYLESHEET: &str = r"
  .table { display: table; font-family: Geist; font-size: 16px; border-spacing: 0 }
  .thead { display: table-header-group }
  .tbody { display: table-row-group }
  .tfoot { display: table-footer-group }
  .tr { display: table-row }
  .td { display: table-cell }
  .caption { display: table-caption }
  .colgroup { display: table-column-group }
  .col { display: table-column }
  .flex { display: flex }
  .block { display: block }
";

/// The render tree of `root`, its displays from [`STYLESHEET`] and `css`.
fn render(root: Node, css: &str) -> RenderNode {
  let stylesheet = StyleSheet::parse(&format!("{STYLESHEET}{css}")).expect("stylesheet parses");
  let mut fonts = Fonts::default();

  register(&mut fonts, GEIST, "Geist");
  let context = RenderContext::builder()
    .fonts(fonts.snapshot())
    .sizing(
      SizingContext::builder()
        .viewport(Viewport::default())
        .build(),
    )
    .stylesheet(Arc::new(stylesheet))
    .build();

  RenderNode::from_node(&context, root)
}

/// A box with an id, laid out.
#[derive(Debug)]
struct Placed {
  /// Its border box's top-left in the root's space.
  origin: Point<f32>,
  layout: ComputedLayout,
  /// Whether layout hid it, as a collapsed row or a hidden empty cell.
  hidden: bool,
}

/// Each box with an id, laid out in `width`.
fn boxes(root: &RenderNode, width: f32) -> BTreeMap<String, Placed> {
  fn collect(
    node: &RenderNode,
    node_id: NodeId,
    origin: Point<f32>,
    results: &LayoutResults,
    boxes: &mut BTreeMap<String, Placed>,
  ) {
    let layout = results.layout(node_id).expect("layout");
    let at = origin + layout.location;

    if let Some(id) = node.node.as_ref().and_then(|node| node.id()) {
      boxes.insert(
        id.to_owned(),
        Placed {
          origin: at,
          layout,
          hidden: results.is_hidden_for_paint(node_id),
        },
      );
    }
    for child in results.box_children(node_id).expect("children") {
      if let Some(render) = node
        .children
        .as_deref()
        .unwrap_or_default()
        .get(child.render_index)
        && child.inline_path.is_none()
      {
        collect(render, child.node_id, at, results, boxes);
      }
    }
  }

  let results = LayoutResults::compute(
    root,
    Size {
      width: AvailableSpace::Definite(width),
      height: AvailableSpace::MaxContent,
    },
  );
  let mut boxes = BTreeMap::new();

  collect(root, NodeId::ROOT, Point::ZERO, &results, &mut boxes);
  boxes
}

/// A box of `class_name` holding `children`.
fn part(class_name: &str, children: impl IntoIterator<Item = Node>) -> Node {
  Node::container(children.into_iter().collect::<Vec<_>>()).with_class_name(class_name)
}

fn table(rows: impl IntoIterator<Item = Node>) -> Node {
  part("table", rows).with_id("table")
}

fn row(cells: impl IntoIterator<Item = Node>) -> Node {
  part("tr", cells)
}

fn cell(id: &str, class_name: &str) -> Node {
  part(&format!("td {class_name}"), [Node::text(id)]).with_id(id)
}

/// A `td` element with `attributes`, as markup builds it.
fn td(id: &str, class_name: &str, attributes: &[(&str, &str)]) -> Node {
  cell(id, class_name).with_tag_name("td").with_attributes(
    attributes
      .iter()
      .map(|&(name, value)| (name.into(), value.into()))
      .collect(),
  )
}

/// An empty block of `class_name` with an id.
fn block(id: &str, class_name: &str) -> Node {
  part(&format!("block {class_name}"), []).with_id(id)
}

fn assert_close(actual: f32, expected: f32) {
  assert!(
    (actual - expected).abs() <= 0.01,
    "expected {expected}, got {actual}"
  );
}

#[test]
fn a_flex_box_in_a_row_lays_out_in_an_anonymous_cell() {
  let tree = render(
    table([row([
      Node::container([Node::text("flex")])
        .with_class_name("flex")
        .with_id("flex"),
      cell("b", ""),
    ])]),
    ".td { padding: 3px }",
  );
  let boxes = boxes(&tree, 400.0);
  let flex = &boxes["flex"].layout;

  assert!(flex.size.width > 0.0 && flex.size.height > 0.0, "{flex:?}");
}

#[test]
fn the_header_lays_out_first_and_the_footer_last() {
  let tree = render(
    table([
      part("tfoot", [row([cell("foot", "")])]),
      part("tbody", [row([cell("body", "")])]),
      part("thead", [row([cell("head", "")])]),
    ]),
    "",
  );
  let boxes = boxes(&tree, 400.0);

  assert!(boxes["head"].origin.y < boxes["body"].origin.y);
  assert!(boxes["body"].origin.y < boxes["foot"].origin.y);
}

#[test]
fn captions_lay_out_above_and_below_the_grid() {
  let tree = render(
    table([
      part("caption bottom", [Node::text("after")]).with_id("after"),
      row([cell("a", "")]),
      part("caption", [Node::text("before")]).with_id("before"),
    ]),
    ".bottom { caption-side: bottom }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["before"].origin.y, 0.0);
  assert_close(boxes["a"].origin.y, boxes["before"].layout.size.height);
  assert_close(
    boxes["after"].origin.y,
    boxes["a"].origin.y + boxes["a"].layout.size.height,
  );
}

#[test]
fn a_middle_cell_centers_its_content() {
  let tree = render(
    table([row([
      part("td", [block("tall", "tall")]),
      part("td middle", [block("short", "short")]),
    ])]),
    ".tall { height: 60px } .short { height: 20px } .middle { vertical-align: middle }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["short"].origin.y, 20.0);
}

#[test]
fn align_content_outranks_vertical_align() {
  let tree = render(
    table([row([
      part("td", [block("tall", "tall")]),
      part("td end", [block("short", "short")]),
    ])]),
    ".tall { height: 60px } .short { height: 20px } .end { vertical-align: top; align-content: end }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["short"].origin.y, 40.0);
}

#[test]
fn a_hidden_child_of_a_row_takes_no_column() {
  let tree = render(
    table([row([
      cell("a", "narrow"),
      block("gone", "gone"),
      cell("b", "narrow"),
    ])]),
    ".narrow { width: 30px; padding: 0 } .gone { display: none }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["b"].origin.x, 30.0);
  assert_close(boxes["table"].layout.size.width, 60.0);
}

#[test]
fn a_cell_inside_a_cell_lays_out_in_an_anonymous_table() {
  let tree = render(
    table([row([part("td", [cell("inner", "")]).with_id("outer")])]),
    "",
  );
  let boxes = boxes(&tree, 400.0);
  let (outer, inner) = (&boxes["outer"], &boxes["inner"]);

  assert!(inner.layout.size.width > 0.0);
  assert!(outer.layout.size.height >= inner.layout.size.height);
}

#[test]
fn colspan_and_rowspan_cover_columns_and_rows() {
  let tree = render(
    table([
      row([
        td("tall", "narrow", &[("rowspan", "2")]),
        td("wide", "", &[("colspan", "2")]),
      ]),
      row([cell("b", "narrow"), cell("c", "narrow")]),
    ]),
    ".td { padding: 0 } .narrow { width: 30px }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["b"].origin.x, 30.0);
  assert_close(boxes["c"].origin.x, 60.0);
  assert_close(boxes["wide"].layout.size.width, 60.0);
  assert_close(
    boxes["tall"].layout.size.height,
    boxes["wide"].layout.size.height + boxes["b"].layout.size.height,
  );
}

#[test]
fn jsx_camel_case_span_counts_the_same_as_the_html_attribute() {
  let wide_width = |name: &str| {
    let tree = render(
      table([
        row([td("wide", "", &[(name, "2")])]),
        row([cell("b", "narrow"), cell("c", "narrow")]),
      ]),
      ".td { padding: 0 } .narrow { width: 30px }",
    );

    boxes(&tree, 400.0)["wide"].layout.size.width
  };

  assert_close(wide_width("colSpan"), 60.0);
  assert_close(wide_width("colspan"), 60.0);
}

#[test]
fn spans_on_a_box_that_is_not_a_td_are_ignored() {
  let tree = render(
    table([
      row([cell("a", "").with_attributes(BTreeMap::from([("colspan".into(), "2".into())]))]),
      row([cell("b", "narrow"), cell("c", "narrow")]),
    ]),
    ".td { padding: 0 } .narrow { width: 30px }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["a"].layout.size.width, 30.0);
}

#[test]
fn oversized_spans_clamp() {
  let tree = render(
    table([row([
      td("a", "", &[("colspan", "65535")]),
      td("b", "", &[("colspan", "65535")]),
    ])]),
    "",
  );
  let boxes = boxes(&tree, 400.0);

  assert!(boxes["b"].origin.x >= boxes["a"].origin.x + boxes["a"].layout.size.width - 0.01);
}

#[test]
fn a_rowspan_ends_with_its_row_group() {
  let tree = render(
    table([
      part("thead", [row([td("head", "", &[("rowspan", "3")])])]),
      part("tbody", [row([cell("body", "")])]),
    ]),
    "",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["body"].origin.y, boxes["head"].layout.size.height);
}

#[test]
fn a_cells_declared_width_sizes_its_column() {
  let tree = render(
    table([row([cell("a", "wide")]), row([cell("b", "")])]),
    ".td { padding: 0 } .wide { width: 80px }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["b"].layout.size.width, 80.0);
}

#[test]
fn a_column_element_sizes_its_column() {
  let tree = render(
    table([
      part("colgroup", [part("col wide", [])]),
      row([cell("a", ""), cell("b", "")]),
    ]),
    ".td { padding: 0 } .wide { width: 120px }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["a"].layout.size.width, 120.0);
  assert_close(boxes["b"].origin.x, 120.0);
}

#[test]
fn border_spacing_insets_the_edges_and_separates_the_cells() {
  let tree = render(
    table([
      row([cell("a", "narrow"), cell("b", "narrow")]),
      row([cell("c", "narrow"), cell("d", "narrow")]),
    ])
    .with_class_name("table spaced"),
    ".spaced { border-spacing: 4px 6px } .td { padding: 0 } .narrow { width: 10px }",
  );
  let boxes = boxes(&tree, 400.0);
  let row_height = boxes["a"].layout.size.height;

  assert_close(boxes["a"].origin.x, 4.0);
  assert_close(boxes["a"].origin.y, 6.0);
  assert_close(boxes["b"].origin.x, 18.0);
  assert_close(boxes["c"].origin.y, 12.0 + row_height);
  assert_close(boxes["table"].layout.size.width, 32.0);
  assert_close(boxes["table"].layout.size.height, 18.0 + 2.0 * row_height);
}

#[test]
fn a_fixed_table_shares_its_width_evenly() {
  let tree = render(
    table([row([cell("a", ""), cell("bbbbbbbb", ""), cell("c", "")])])
      .with_class_name("table fixed"),
    ".fixed { table-layout: fixed; width: 300px }",
  );
  let boxes = boxes(&tree, 400.0);

  for id in ["a", "bbbbbbbb", "c"] {
    assert_close(boxes[id].layout.size.width, 100.0);
  }
}

#[test]
fn a_fixed_table_reads_widths_from_its_first_row_only() {
  let tree = render(
    table([
      row([cell("a", "narrow"), cell("b", "")]),
      row([cell("c", "wide"), cell("d", "")]),
    ])
    .with_class_name("table fixed"),
    ".fixed { table-layout: fixed; width: 300px } .td { padding: 0 } .narrow { width: 50px } .wide { width: 200px }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["c"].layout.size.width, 50.0);
  assert_close(boxes["d"].layout.size.width, 250.0);
}

#[test]
fn a_fixed_table_without_a_width_lays_out_automatically() {
  let tree = render(
    table([row([cell("a", "narrow")]), row([cell("b", "wide")])]).with_class_name("table fixed"),
    ".fixed { table-layout: fixed } .td { padding: 0 } .narrow { width: 50px } .wide { width: 200px }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["a"].layout.size.width, 200.0);
}

#[test]
fn a_collapsed_line_carries_one_border_split_between_its_cells() {
  let tree = render(
    table([row([cell("a", "bordered"), cell("b", "bordered")])]).with_class_name("table collapse"),
    ".collapse { border-collapse: collapse } .bordered { border: 2px solid black; padding: 0 }",
  );
  let boxes = boxes(&tree, 400.0);
  let (a, b) = (&boxes["a"], &boxes["b"]);

  assert_close(a.layout.border.right, 1.0);
  assert_close(b.layout.border.left, 1.0);
  assert_close(b.origin.x, a.origin.x + a.layout.size.width);
}

#[test]
fn the_wider_border_wins_a_collapsed_line() {
  let tree = render(
    table([
      row([cell("a", "bordered heavy")]),
      row([cell("b", "bordered")]),
    ])
    .with_class_name("table collapse"),
    ".collapse { border-collapse: collapse } .bordered { border: 2px solid black } .heavy { border-bottom-width: 6px }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["b"].layout.border.top, 3.0);
}

#[test]
fn a_hidden_border_clears_a_collapsed_line() {
  let tree = render(
    table([row([cell("a", "bordered hidden"), cell("b", "bordered")])])
      .with_class_name("table collapse"),
    ".collapse { border-collapse: collapse } .bordered { border: 2px solid black } .hidden { border-right-style: hidden }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["b"].layout.border.left, 0.0);
}

#[test]
fn a_rows_border_collapses_into_its_cells() {
  let tree = render(
    table([
      row([cell("a", "")]),
      row([cell("b", "")]).with_class_name("tr ruled"),
    ])
    .with_class_name("table collapse"),
    ".collapse { border-collapse: collapse } .ruled { border-top: 4px solid black }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["b"].layout.border.top, 2.0);
  assert_close(boxes["a"].layout.border.bottom, 2.0);
}

#[test]
fn a_collapsed_row_takes_no_height() {
  let tree = render(
    table([
      row([cell("a", "")]),
      row([cell("b", "")])
        .with_class_name("tr collapsed")
        .with_id("collapsed"),
      row([cell("c", "")]),
    ]),
    ".collapsed { visibility: collapse }",
  );
  let boxes = boxes(&tree, 400.0);

  assert!(boxes["collapsed"].hidden);
  assert_close(
    boxes["c"].origin.y,
    boxes["a"].origin.y + boxes["a"].layout.size.height,
  );
}

#[test]
fn an_empty_cell_hides_under_empty_cells_hide() {
  let tree = render(
    table([row([part("td", []).with_id("empty"), cell("full", "")])]).with_class_name("table hide"),
    ".hide { empty-cells: hide }",
  );
  let boxes = boxes(&tree, 400.0);

  assert!(boxes["empty"].hidden);
  assert!(!boxes["full"].hidden);
}

#[test]
fn a_right_to_left_table_starts_its_columns_on_the_right() {
  let tree = render(
    table([row([cell("a", "narrow"), cell("b", "narrow")])]).with_class_name("table rtl"),
    ".rtl { direction: rtl } .td { padding: 0 } .narrow { width: 30px }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["a"].origin.x, 30.0);
  assert_close(boxes["b"].origin.x, 0.0);
}

#[test]
fn a_table_height_grows_its_rows() {
  let tree = render(
    table([row([cell("a", "")]), row([cell("b", "")])]).with_class_name("table tall"),
    ".tall { height: 200px }",
  );
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["a"].layout.size.height, 100.0);
  assert_close(boxes["b"].origin.y, 100.0);
}

#[test]
fn a_table_of_columns_alone_lays_out() {
  let tree = render(table([part("col", [])]), "");
  let boxes = boxes(&tree, 400.0);

  assert_close(boxes["table"].layout.size.width, 0.0);
}
