//! Block fragmentation across pages, after Blink's `fragmentation_test.cc`.

use std::{collections::BTreeMap, sync::Arc};

use crate::{
  context::RenderContext,
  fonts::shaping::testing::{GEIST, register},
  geometry::{NodeId, Point, Size},
  layout::{
    node::Node,
    tree::{LayoutResults, RenderNode},
  },
  resources::font::Fonts,
  scene::{NodePaint, PagedScene, PaintItemKind},
  style::{SizingContext, StyleSheet},
  viewport::Viewport,
};

/// One fragment of a box with an id: the page it is on and its border box there.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Placed {
  page: usize,
  origin: Point<f32>,
  height: f32,
}

fn render(root: Node, css: &str) -> RenderNode {
  let stylesheet = StyleSheet::parse(&format!(
    ".block {{ display: block }} p {{ display: block; margin: 0; font-family: Geist; font-size: 16px; line-height: 20px; white-space: pre-line }} {css}"
  ))
  .expect("stylesheet parses");
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

/// Each fragment of each box with an id, `root` laid out on pages 300 wide and `page_height`
/// tall, with the page count.
fn pages(root: &RenderNode, page_height: f32) -> (BTreeMap<String, Vec<Placed>>, usize) {
  fn collect(
    node: &RenderNode,
    node_id: NodeId,
    containers: &[(usize, Point<f32>)],
    results: &LayoutResults,
    boxes: &mut BTreeMap<String, Vec<Placed>>,
  ) {
    let fragments: Vec<(usize, Point<f32>, f32)> = results
      .fragments(node_id)
      .expect("fragments")
      .iter()
      .map(|fragment| {
        let (page, origin) = containers[fragment.container_fragment_index];

        (
          page,
          origin + fragment.layout.location,
          fragment.layout.size.height,
        )
      })
      .collect();

    if let Some(id) = node.node.as_ref().and_then(|node| node.id()) {
      boxes.insert(
        id.to_owned(),
        fragments
          .iter()
          .map(|&(page, origin, height)| Placed {
            page,
            origin,
            height,
          })
          .collect(),
      );
    }

    let containers: Vec<(usize, Point<f32>)> = fragments
      .iter()
      .map(|&(page, origin, _)| (page, origin))
      .collect();

    for child in results.box_children(node_id).expect("children") {
      if let Some(render) = node
        .children
        .as_deref()
        .unwrap_or_default()
        .get(child.render_index)
        && child.inline_path.is_none()
      {
        collect(render, child.node_id, &containers, results, boxes);
      }
    }
  }

  let (results, page_count) = LayoutResults::compute_pages(
    root,
    Size {
      width: 300.0,
      height: page_height,
    },
  );
  let containers: Vec<(usize, Point<f32>)> =
    (0..page_count).map(|page| (page, Point::ZERO)).collect();
  let mut boxes = BTreeMap::new();

  collect(root, NodeId::ROOT, &containers, &results, &mut boxes);
  (boxes, page_count)
}

fn block(id: &str, style: &str) -> Node {
  Node::container(Vec::<Node>::new())
    .with_class_name(format!("block {style}"))
    .with_id(id)
}

fn root(children: impl IntoIterator<Item = Node>) -> Node {
  Node::container(children.into_iter().collect::<Vec<_>>()).with_class_name("block")
}

fn placed(page: usize, y: f32, height: f32) -> Placed {
  Placed {
    page,
    origin: Point::new(0.0, y),
    height,
  }
}

#[test]
fn a_block_that_reaches_past_the_page_end_continues_on_the_next() {
  let tree = render(
    root([block("a", "h100"), block("b", "h100"), block("c", "h100")]),
    ".h100 { height: 100px }",
  );
  let (boxes, page_count) = pages(&tree, 250.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["a"], [placed(0, 0.0, 100.0)]);
  assert_eq!(boxes["b"], [placed(0, 100.0, 100.0)]);
  assert_eq!(boxes["c"], [placed(0, 200.0, 50.0), placed(1, 0.0, 50.0)]);
}

#[test]
fn break_inside_avoid_moves_a_block_to_the_next_page() {
  let tree = render(
    root([
      block("a", "h100"),
      block("b", "h100"),
      block("c", "h100 avoid"),
    ]),
    ".h100 { height: 100px } .avoid { break-inside: avoid }",
  );
  let (boxes, _) = pages(&tree, 250.0);

  assert_eq!(boxes["c"], [placed(1, 0.0, 100.0)]);
}

#[test]
fn break_before_page_starts_a_block_on_a_new_page() {
  let tree = render(
    root([block("a", "h50"), block("b", "h50 forced")]),
    ".h50 { height: 50px } .forced { break-before: page }",
  );
  let (boxes, page_count) = pages(&tree, 250.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["a"], [placed(0, 0.0, 50.0)]);
  assert_eq!(boxes["b"], [placed(1, 0.0, 50.0)]);
}

#[test]
fn a_margin_after_an_unforced_break_is_truncated() {
  let tree = render(
    root([block("a", "h240"), block("b", "h50 spaced")]),
    ".h240 { height: 240px } .h50 { height: 50px } .spaced { margin-top: 30px }",
  );
  let (boxes, _) = pages(&tree, 250.0);

  assert_eq!(boxes["b"], [placed(1, 0.0, 50.0)]);
}

#[test]
fn lines_split_between_pages_as_widows_ask() {
  let lines = "1\n2\n3\n4\n5\n6\n7\n8";
  let paragraph = |widows: &str| {
    render(
      root([
        block("a", "h140"),
        Node::container([Node::text(lines)])
          .with_tag_name("p")
          .with_class_name(widows)
          .with_id("p"),
      ]),
      ".h140 { height: 140px } .six { widows: 6 }",
    )
  };
  let (boxes, _) = pages(&paragraph(""), 200.0);

  // Three lines fit after the block, five follow.
  assert_eq!(boxes["p"], [placed(0, 140.0, 60.0), placed(1, 0.0, 100.0)]);

  let (boxes, _) = pages(&paragraph("six"), 200.0);

  // Six widows leave two lines on the first page.
  assert_eq!(boxes["p"], [placed(0, 140.0, 60.0), placed(1, 0.0, 120.0)]);
}

#[test]
fn each_fragment_of_a_paged_scene_paints_its_own_lines() {
  let tree = render(
    root([
      block("a", "h140"),
      Node::container([Node::text("1\n2\n3\n4\n5\n6\n7\n8")])
        .with_tag_name("p")
        .with_id("p"),
    ]),
    ".h140 { height: 140px }",
  );
  let mut paged = PagedScene::lay_out(
    tree,
    Size {
      width: 300.0,
      height: 200.0,
    },
    false,
  )
  .expect("pages lay out");

  assert_eq!(paged.page_count(), 2);

  let scene = paged.page(1);

  assert_eq!(scene.origin, Point::new(0.0, 200.0));
  // The root paints once on each page.
  assert_eq!(
    scene.contexts[0]
      .roots()
      .iter()
      .map(|root| root.fragment_index)
      .collect::<Vec<_>>(),
    [0, 1]
  );

  let paints: Vec<NodePaint> = scene
    .contexts
    .iter()
    .flat_map(|context| context.in_paint_order().into_iter().flatten())
    .filter_map(|item| match &item.kind {
      PaintItemKind::Node(paint)
        if scene
          .root
          .node_at_path(&paint.path)
          .and_then(|node| node.node.as_ref())
          .and_then(|node| node.id())
          == Some("p") =>
      {
        Some(paint.clone())
      }
      _ => None,
    })
    .collect();

  // One scene paints both fragments, the second where the second page starts.
  assert_eq!(
    paints
      .iter()
      .map(|paint| (paint.fragment_index, paint.paint_offset.y))
      .collect::<Vec<_>>(),
    [(0, 140.0), (1, 200.0)]
  );

  for (paint, line_count) in paints.iter().zip([3, 5]) {
    let layout = scene
      .results
      .fragment_layout(paint.node_id, paint.fragment_index)
      .expect("fragment layout");
    let fragments = scene
      .fragments
      .as_ref()
      .expect("a paged scene places fragments");

    fragments.enter(&mut scene.root, paint, layout.location);

    let node = scene
      .root
      .node_at_path(&paint.path)
      .expect("paragraph node");

    assert_eq!(
      node
        .fragment_items_in(layout)
        .expect("the fragment's lines")
        .line_boxes()
        .len(),
      line_count,
      "fragment {}",
      paint.fragment_index
    );
  }
}

#[test]
fn a_sliced_box_leaves_its_border_out_where_it_breaks() {
  let tree = render(
    root([block("a", "h100"), block("b", "framed")]),
    ".h100 { height: 100px } .framed { height: 200px; border: 5px solid black; padding: 4px }",
  );
  let (results, page_count) = LayoutResults::compute_pages(
    &tree,
    Size {
      width: 300.0,
      height: 200.0,
    },
  );
  let b = results.box_children(NodeId::ROOT).expect("children")[1].node_id;
  let fragments = results.fragments(b).expect("fragments");

  assert_eq!(page_count, 2);
  assert_eq!(fragments.len(), 2);
  assert_eq!(
    (
      fragments[0].layout.border.top,
      fragments[0].layout.border.bottom
    ),
    (5.0, 0.0)
  );
  assert_eq!(
    (
      fragments[1].layout.border.top,
      fragments[1].layout.border.bottom
    ),
    (0.0, 5.0)
  );
  assert_eq!(fragments[1].layout.padding.top, 0.0);
}

#[test]
fn a_later_page_keeps_the_out_of_flow_boxes_an_earlier_one_positioned() {
  let body = Node::container([
    block("abs", "h20 abs"),
    block("a", "h20"),
    block("b", "h20 forced"),
  ])
  .with_class_name("block");
  let tree = render(
    root([body]),
    ".h20 { height: 20px } .abs { position: absolute; top: 96px } .forced { break-before: page }",
  );
  let (results, page_count) = LayoutResults::compute_pages(
    &tree,
    Size {
      width: 300.0,
      height: 192.0,
    },
  );
  let body = results.box_children(NodeId::ROOT).expect("root children")[0].node_id;
  let abs = &results.box_children(body).expect("body children")[0];

  assert_eq!(page_count, 2);
  // The page area positioned it, which the second page's layout does not forget.
  assert_eq!(abs.hoisted_cb, Some(NodeId::ROOT));
}

#[test]
fn an_absolute_box_among_lines_sits_on_the_page_of_its_line() {
  let paragraph = Node::container([Node::text("1\n2\n3\n4\n5"), block("abs", "abs")])
    .with_tag_name("p")
    .with_class_name("rel")
    .with_id("p");
  let tree = render(
    root([paragraph]),
    ".rel { position: relative } .abs { position: absolute; width: 10px; height: 10px }",
  );
  let (boxes, _) = pages(&tree, 60.0);

  assert_eq!(
    boxes["abs"]
      .iter()
      .map(|placed| placed.page)
      .collect::<Vec<_>>(),
    [1]
  );
}

/// A table of `rows`, each a row of one cell that holds `cell`.
fn table(rows: impl IntoIterator<Item = (&'static str, Node)>) -> Node {
  Node::container(
    rows
      .into_iter()
      .map(|(id, cell)| {
        Node::container([Node::container([cell]).with_class_name("td")])
          .with_class_name("tr")
          .with_id(id)
      })
      .collect::<Vec<_>>(),
  )
  .with_class_name("table")
  .with_id("table")
}

const TABLE_CSS: &str = ".table { display: table; border-spacing: 0 } .tr { display: table-row } \
  .td { display: table-cell; padding: 0 } .h60 { height: 60px } .h300 { height: 300px } \
  .caption { display: table-caption; height: 40px }";

#[test]
fn a_table_row_breaks_inside_unless_it_avoids_it() {
  let rows = || ["r0", "r1", "r2", "r3", "r4"].map(|id| (id, block(&format!("{id}-cell"), "h60")));
  let (boxes, page_count) = pages(&render(root([table(rows())]), TABLE_CSS), 200.0);

  // The fourth row's cell content breaks, and the row with it.
  assert_eq!(page_count, 2);
  assert_eq!(boxes["r3"], [placed(0, 180.0, 20.0), placed(1, 0.0, 40.0)]);
  assert_eq!(
    boxes["table"],
    [placed(0, 0.0, 200.0), placed(1, 0.0, 100.0)]
  );

  let (boxes, _) = pages(
    &render(
      root([table(rows())]),
      &format!("{TABLE_CSS} .tr {{ break-inside: avoid }}"),
    ),
    200.0,
  );

  assert_eq!(boxes["r2"], [placed(0, 120.0, 60.0)]);
  assert_eq!(boxes["r3"], [placed(1, 0.0, 60.0)]);
  assert_eq!(boxes["r4"], [placed(1, 60.0, 60.0)]);
  assert_eq!(
    boxes["table"],
    [placed(0, 0.0, 200.0), placed(1, 0.0, 120.0)]
  );
}

#[test]
fn a_tall_cell_breaks_its_row_across_pages() {
  let tree = render(
    root([table([("row", block("content", "h300"))])]),
    TABLE_CSS,
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["row"], [placed(0, 0.0, 200.0), placed(1, 0.0, 100.0)]);
  assert_eq!(
    boxes["content"],
    [placed(0, 0.0, 200.0), placed(1, 0.0, 100.0)]
  );
}

#[test]
fn a_top_caption_comes_before_the_rows_it_breaks_with() {
  let caption = Node::container(Vec::<Node>::new())
    .with_class_name("caption")
    .with_id("caption");
  let rows = Node::container(
    ["r0", "r1", "r2"]
      .map(|id| {
        Node::container([
          Node::container([block(&format!("{id}-cell"), "h60")]).with_class_name("td")
        ])
        .with_class_name("tr")
        .with_id(id)
      })
      .to_vec(),
  )
  .with_class_name("tbody");
  let tree = render(
    root([Node::container([caption, rows])
      .with_class_name("table")
      .with_id("table")]),
    &format!("{TABLE_CSS} .tbody {{ display: table-row-group }}"),
  );
  let (boxes, _) = pages(&tree, 150.0);

  assert_eq!(boxes["caption"], [placed(0, 0.0, 40.0)]);
  assert_eq!(boxes["r0"], [placed(0, 40.0, 60.0)]);
  assert_eq!(boxes["r1"], [placed(0, 100.0, 50.0), placed(1, 0.0, 10.0)]);
}

/// A table with a header group of `header_class`, a body of five 60px rows that avoid breaking,
/// and, with `footer_class`, a footer group.
fn table_with_groups(header_class: &str, footer_class: Option<&str>) -> Node {
  let group = |class: &str, id: &str, rows: Vec<Node>| {
    Node::container(rows)
      .with_class_name(class.to_owned())
      .with_id(id)
  };
  let row = |id: String, height: &str| {
    Node::container([Node::container([block(&format!("{id}-cell"), height)]).with_class_name("td")])
      .with_class_name("tr")
      .with_id(id.as_str())
  };
  let mut sections = vec![
    group(
      &format!("thead {header_class}"),
      "thead",
      vec![row("h".into(), "h20")],
    ),
    group(
      "tbody",
      "tbody",
      (0..5)
        .map(|index| row(format!("r{index}"), "h60"))
        .collect(),
    ),
  ];

  if let Some(footer_class) = footer_class {
    sections.push(group(
      &format!("tfoot {footer_class}"),
      "tfoot",
      vec![row("f".into(), "h20")],
    ));
  }
  Node::container(sections)
    .with_class_name("table")
    .with_id("table")
}

const GROUP_CSS: &str = ".thead { display: table-header-group } .tbody { display: table-row-group } \
  .tfoot { display: table-footer-group } .tbody .tr { break-inside: avoid } .h20 { height: 20px } \
  .h80 { height: 80px } .avoid { break-inside: avoid }";

#[test]
fn a_short_table_header_repeats_on_every_page() {
  let tree = render(
    root([table_with_groups("avoid", None)]),
    &format!("{TABLE_CSS} {GROUP_CSS}"),
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["thead"], [placed(0, 0.0, 20.0), placed(1, 0.0, 20.0)]);
  assert_eq!(boxes["h"], [placed(0, 0.0, 20.0), placed(1, 0.0, 20.0)]);
  assert_eq!(boxes["r2"], [placed(0, 140.0, 60.0)]);
  assert_eq!(boxes["r3"], [placed(1, 20.0, 60.0)]);
}

#[test]
fn a_table_header_repeats_only_when_it_avoids_breaks_and_is_short() {
  for header_class in ["", "avoid h80"] {
    let tree = render(
      root([table_with_groups(header_class, None)]),
      &format!("{TABLE_CSS} {GROUP_CSS} .h80 .h20 {{ height: 80px }}"),
    );
    let (boxes, _) = pages(&tree, 200.0);

    assert_eq!(boxes["thead"].len(), 1, "{header_class:?} does not repeat");
  }
}

#[test]
fn a_short_table_footer_repeats_at_the_end_of_every_page() {
  let tree = render(
    root([table_with_groups("avoid", Some("avoid"))]),
    &format!("{TABLE_CSS} {GROUP_CSS}"),
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  // The header and footer take 40px of each page, so two rows fit on each, and the footer follows
  // the last row on the page.
  assert_eq!(page_count, 3);
  assert_eq!(boxes["r1"], [placed(0, 80.0, 60.0)]);
  assert_eq!(boxes["r2"], [placed(1, 20.0, 60.0)]);
  assert_eq!(boxes["r4"], [placed(2, 20.0, 60.0)]);
  assert_eq!(
    boxes["tfoot"],
    [
      placed(0, 140.0, 20.0),
      placed(1, 140.0, 20.0),
      placed(2, 80.0, 20.0)
    ]
  );
  assert_eq!(boxes["f"].len(), 3);
}

/// The fragments of the first child of `root`'s first child, laid out on pages 300 wide and
/// `page_height` tall: their containers' fragment indices, block offsets, and block sizes, with
/// the page count.
fn nested_fragments(root: &RenderNode, page_height: f32) -> (Vec<(usize, f32, f32)>, usize) {
  let (results, page_count) = LayoutResults::compute_pages(
    root,
    Size {
      width: 300.0,
      height: page_height,
    },
  );
  let parent = results.box_children(NodeId::ROOT).expect("root children")[0].node_id;
  let child = results.box_children(parent).expect("children")[0].node_id;
  let fragments = results
    .fragments(child)
    .expect("fragments")
    .iter()
    .map(|fragment| {
      (
        fragment.container_fragment_index,
        fragment.layout.location.y,
        fragment.layout.size.height,
      )
    })
    .collect();

  (fragments, page_count)
}

#[test]
fn an_absolute_box_taller_than_a_page_breaks_across_pages() {
  let container = Node::container([block("abs", "abs")]).with_class_name("block relative");
  let tree = render(
    root([container, block("a", "h300")]),
    ".relative { position: relative } .abs { position: absolute; top: 0; height: 300px } \
     .h300 { height: 300px }",
  );
  let (fragments, page_count) = nested_fragments(&tree, 200.0);

  assert_eq!(page_count, 2);
  // Both fragments belong to the containing block's only fragment, the second a page further.
  assert_eq!(fragments, [(0, 0.0, 200.0), (0, 200.0, 100.0)]);
}

#[test]
fn an_absolute_box_past_the_last_page_adds_pages() {
  let body = Node::container([block("abs", "abs")]).with_class_name("block");
  let tree = render(
    root([body]),
    ".abs { position: absolute; top: 0; height: 500px }",
  );
  let (fragments, page_count) = nested_fragments(&tree, 200.0);

  assert_eq!(page_count, 3);
  assert_eq!(fragments.len(), 3);
}

#[test]
fn a_fixed_box_repeats_on_every_page() {
  let body =
    Node::container([block("fixed", "fixed"), block("a", "h500")]).with_class_name("block");
  let tree = render(
    root([body]),
    ".fixed { position: fixed; top: 10px; height: 20px } .h500 { height: 500px }",
  );
  let (fragments, page_count) = nested_fragments(&tree, 200.0);

  assert_eq!(page_count, 3);
  // Each page's root fragment holds one, at the same place on its page.
  assert_eq!(
    fragments,
    [(0, 10.0, 20.0), (1, 10.0, 20.0), (2, 10.0, 20.0)]
  );
}

#[test]
fn an_absolute_box_inside_a_clipping_box_adds_no_pages() {
  let clip = Node::container([block("tall", "tall")]).with_class_name("block clip");
  let body = Node::container([clip]).with_class_name("block");
  let tree = render(
    root([body]),
    ".clip { position: absolute; overflow: clip; width: 10px; height: 10px } \
     .tall { position: absolute; height: 700px }",
  );
  let (_, page_count) = nested_fragments(&tree, 200.0);

  assert_eq!(page_count, 1);
}
