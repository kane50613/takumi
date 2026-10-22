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
  scene::{PagedScene, PaintItemKind},
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
fn each_page_paints_the_lines_its_fragment_holds() {
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

  for (page, line_count) in [(0, 3), (1, 5), (0, 3)] {
    let scene = paged.page(page);
    let paint = scene
      .contexts
      .iter()
      .flat_map(|context| context.in_paint_order().into_iter().flatten())
      .find_map(|item| match &item.kind {
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
      .expect("the paragraph paints on every page");
    let node = scene
      .root
      .node_at_path(&paint.path)
      .expect("paragraph node");
    let layout = scene
      .results
      .fragment_layout(paint.node_id, paint.fragment_index)
      .expect("fragment layout");

    assert_eq!(paint.fragment_index, page);
    assert_eq!(
      node
        .fragment_items_in(layout)
        .expect("the fragment's lines")
        .line_boxes()
        .len(),
      line_count,
      "page {page}"
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
