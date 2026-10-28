//! Block fragmentation across pages, after Blink's `fragmentation_test.cc`.

use std::{collections::BTreeMap, sync::Arc};

use crate::{
  Error,
  context::RenderContext,
  fonts::shaping::testing::{GEIST, register},
  geometry::{NodeId, PathCommand, Point, Size},
  layout::{
    node::Node,
    paged_layout::PagedLayout,
    paged_layout_oracles as oracles,
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  paint_property::ClipKind,
  resources::font::Fonts,
  scene::{NodePaint, PagedScene, PaintItemKind, Scene},
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
      // A box inside inline boxes sits at its path through them.
      let render = node
        .children
        .as_deref()
        .and_then(|children| children.get(child.render_index))
        .and_then(|render| {
          child
            .inline_path
            .iter()
            .flatten()
            .try_fold(render, |render, &index| {
              render.children.as_deref()?.get(index)
            })
        });

      if let Some(render) = render {
        collect(render, child.node_id, &containers, results, boxes);
      }
    }
  }

  let page_size = Size {
    width: 300.0,
    height: page_height,
  };

  oracles::check(root, page_size);

  let (results, page_count) =
    LayoutResults::compute_pages(root, page_size, usize::MAX).expect("pages lay out");
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
  let paged = PagedScene::lay_out(
    tree,
    Size {
      width: 300.0,
      height: 200.0,
    },
    usize::MAX,
    false,
  )
  .expect("pages lay out");

  assert_eq!(paged.page_count(), 2);

  let scene = &paged.into_scene();

  // The root paints once on each page.
  assert_eq!(
    scene.contexts[0]
      .roots()
      .iter()
      .map(|root| root.fragment_index)
      .collect::<Vec<_>>(),
    [0, 1]
  );

  let paints = paints_of(scene, "p");

  // One scene paints both fragments, the second where the second page starts.
  assert_eq!(
    paints
      .iter()
      .map(|paint| (paint.fragment_index, paint.paint_offset.y))
      .collect::<Vec<_>>(),
    [(0, 140.0), (1, 200.0)]
  );

  assert_eq!(
    paints
      .iter()
      .map(|paint| line_count(scene, paint))
      .collect::<Vec<_>>(),
    [3, 5]
  );
}

/// The paints of each fragment of the box with `id`, in paint order.
fn paints_of(scene: &Scene, id: &str) -> Vec<NodePaint> {
  scene
    .contexts
    .iter()
    .flat_map(|context| {
      context
        .roots()
        .iter()
        .chain(
          context
            .in_paint_order()
            .into_iter()
            .flatten()
            .filter_map(|item| match &item.kind {
              PaintItemKind::Node(paint) => Some(paint),
              _ => None,
            }),
        )
    })
    .filter(|paint| {
      scene
        .root
        .node_at_path(&paint.path)
        .and_then(|node| node.node.as_ref())
        .and_then(|node| node.id())
        == Some(id)
    })
    .cloned()
    .collect()
}

/// The number of lines in the fragment `paint` paints.
fn line_count(scene: &Scene, paint: &NodePaint) -> usize {
  let layout = scene.view().enter(paint).expect("fragment layout");

  scene
    .root
    .node_at_path(&paint.path)
    .and_then(|node| node.fragment_items_in(layout))
    .expect("the fragment's lines")
    .line_boxes()
    .len()
}

/// Paged layout stops once the content needs more pages than its limit, in flow or out of it.
#[test]
fn paged_layout_stops_at_its_page_limit() {
  let page = Size {
    width: 300.0,
    height: 200.0,
  };
  let breaks = render(
    root([
      block("a", "h100"),
      block("b", "h100 page"),
      block("c", "h100 page"),
    ]),
    ".h100 { height: 100px } .page { break-before: page }",
  );
  let far = render(
    root([block("far", "far")]),
    ".far { position: absolute; top: 1000px; height: 10px }",
  );

  assert_eq!(
    LayoutResults::compute_pages(&breaks, page, 3)
      .expect("three pages fit")
      .1,
    3
  );
  assert!(matches!(
    LayoutResults::compute_pages(&breaks, page, 2),
    Err(Error::TooManyPages(2))
  ));
  assert!(matches!(
    LayoutResults::compute_pages(&far, page, 4),
    Err(Error::TooManyPages(4))
  ));

  let mut stepped = PagedLayout::new(&breaks, page, 2);

  assert!(stepped.next_page().expect("first page"));
  assert!(stepped.next_page().expect("second page"));
  assert!(matches!(stepped.next_page(), Err(Error::TooManyPages(2))));
}

/// A flex item stretched past the page where its lines end lays out none on the later pages.
#[test]
fn lines_that_ended_stay_on_their_page_in_a_stretched_box() {
  let tree = render(
    root([Node::container([
      block("tall", "h600"),
      Node::container([Node::text("Beside")])
        .with_tag_name("p")
        .with_id("p"),
    ])
    .with_class_name("flex")]),
    ".h600 { height: 600px; width: 100px } .flex { display: flex }",
  );
  let paged = PagedScene::lay_out(
    tree,
    Size {
      width: 300.0,
      height: 200.0,
    },
    usize::MAX,
    false,
  )
  .expect("pages lay out");
  let scene = &paged.into_scene();

  assert_eq!(
    paints_of(scene, "p")
      .iter()
      .map(|paint| line_count(scene, paint))
      .collect::<Vec<_>>(),
    [1, 0, 0]
  );
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
    usize::MAX,
  )
  .expect("pages lay out");
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
    usize::MAX,
  )
  .expect("pages lay out");
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

/// A paragraph of a line and a `position: relative` span that holds the absolute box `abs`.
fn paragraph_with_inline_containing_block(abs: Node) -> Node {
  let span = Node::container([Node::text("a"), abs]).with_class_name("inline rel");

  root([Node::container([span]).with_tag_name("p").with_id("p")])
}

const INLINE_CONTAINING_BLOCK_CSS: &str = ".inline { display: inline } \
  .rel { position: relative } .abs { position: absolute; width: 10px }";

#[test]
fn an_absolute_box_in_an_inline_box_taller_than_a_page_breaks_across_pages() {
  let tree = render(
    paragraph_with_inline_containing_block(block("abs", "abs")),
    &format!("{INLINE_CONTAINING_BLOCK_CSS} .abs {{ top: 0; height: 300px }}"),
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["abs"], [placed(0, -1.0, 201.0), placed(1, 0.0, 99.0)]);
}

#[test]
fn an_absolute_box_in_an_inline_box_below_the_page_moves_to_the_next_page() {
  let tree = render(
    paragraph_with_inline_containing_block(block("abs", "abs")),
    &format!("{INLINE_CONTAINING_BLOCK_CSS} .abs {{ top: 250px; height: 50px }}"),
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["abs"], [placed(1, 49.0, 50.0)]);
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

/// A table of ten rows 60 tall, `r0` to `r9`, each cell holding a block `r0-cell` to `r9-cell`.
fn ten_row_table() -> RenderNode {
  let rows = ["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9"]
    .map(|id| (id, block(&format!("{id}-cell"), "h60")));

  render(root([table(rows)]), TABLE_CSS)
}

fn node_with_id(tree: &LayoutTree<'_>, id: &str) -> NodeId {
  (0..tree.node_count())
    .map(NodeId::new)
    .find(|&node| tree.render_node(node).node.as_ref().and_then(Node::id) == Some(id))
    .expect("a box with the id")
}

/// A table's rows free what they kept once their page is drawn, while the table goes on.
#[test]
fn a_tables_finished_rows_free_their_layouts_before_the_table_ends() {
  let tree = ten_row_table();
  let page_size = Size {
    width: 300.0,
    height: 200.0,
  };
  let mut layout = PagedLayout::new(&tree, page_size, usize::MAX).with_release();

  assert!(layout.next_page().expect("page lays out"));
  layout.release(1);
  assert!(layout.next_page().expect("page lays out"));

  let tree = layout.tree();
  let is_released = |id: &str| tree.node_state(node_with_id(tree, id)).cache.is_released();

  assert!(!is_released("table"));
  assert!(is_released("r0"));
  assert!(is_released("r0-cell"));
  assert!(!is_released("r9"));
}

/// The rows a table measured for its first page, but placed on none yet, drop their measures.
#[test]
fn a_tables_rows_still_to_come_drop_their_measures() {
  let tree = ten_row_table();
  let page_size = Size {
    width: 300.0,
    height: 200.0,
  };
  let mut layout = PagedLayout::new(&tree, page_size, usize::MAX).with_release();

  assert!(layout.next_page().expect("page lays out"));
  layout.release(0);

  let tree = layout.tree();
  let cell_has_measures = |content: &str| {
    let (cell, _) = tree
      .node_state(node_with_id(tree, content))
      .box_parent
      .expect("a cell holds the content");

    tree
      .node_state(NodeId::new(cell as usize))
      .cache
      .has_measures()
  };

  assert!(cell_has_measures("r0-cell"));
  assert!(!cell_has_measures("r9-cell"));
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
    usize::MAX,
  )
  .expect("pages lay out");
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
  // The box breaks the containing block, whose second fragment holds the box's second.
  assert_eq!(fragments, [(0, 0.0, 200.0), (1, 0.0, 100.0)]);
}

#[test]
fn an_absolute_box_at_the_bottom_waits_for_the_last_fragment_of_its_containing_block() {
  let container =
    Node::container([block("abs", "abs"), block("a", "h300")]).with_class_name("block relative");
  let tree = render(
    root([container]),
    ".relative { position: relative } .abs { position: absolute; bottom: 0; height: 50px } \
     .h300 { height: 300px }",
  );
  let (fragments, page_count) = nested_fragments(&tree, 200.0);

  assert_eq!(page_count, 2);
  // The box ends at the bottom of the whole containing block, on its second page.
  assert_eq!(fragments, [(1, 50.0, 50.0)]);
}

#[test]
fn an_absolute_box_as_tall_as_its_containing_block_covers_all_its_pages() {
  let container =
    Node::container([block("abs", "abs"), block("a", "h300")]).with_class_name("block relative");
  let tree = render(
    root([container]),
    ".relative { position: relative } .abs { position: absolute; top: 0; height: 100% } \
     .h300 { height: 300px }",
  );
  let (fragments, page_count) = nested_fragments(&tree, 200.0);

  assert_eq!(page_count, 2);
  // The box is one fragment, which the pages paint across both.
  assert_eq!(fragments, [(1, -200.0, 300.0)]);
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

const FLOAT_CSS: &str = ".float { float: left; width: 50px } .h20 { height: 20px } \
  .h80 { height: 80px } .h150 { height: 150px } .h300 { height: 300px } \
  .avoid { break-inside: avoid } .clear { clear: both } .m30 { margin-top: 30px } \
  .after { break-after: page } .border { border-bottom: 10px solid }";

#[test]
fn a_resumed_float_drops_its_block_start_margin() {
  let tree = render(root([block("float", "float h300 m30")]), FLOAT_CSS);
  let (boxes, _) = pages(&tree, 200.0);

  assert_eq!(
    boxes["float"],
    [placed(0, 30.0, 170.0), placed(1, 0.0, 130.0)]
  );
}

#[test]
fn a_forced_break_before_an_absolute_box_moves_its_static_position_to_the_next_page() {
  let wrap = Node::container([block("a", "h20 after"), block("abs", "abs")])
    .with_class_name("block")
    .with_id("wrap");
  let tree = render(
    root([wrap]),
    &format!("{FLOAT_CSS} .abs {{ position: absolute; width: 10px; height: 10px }}"),
  );
  let (boxes, _) = pages(&tree, 200.0);

  assert_eq!(boxes["abs"], [placed(1, 0.0, 10.0)]);
}

#[test]
fn a_forced_break_before_a_float_leaves_the_container_unbroken_by_inflow_content() {
  let wrap = Node::container([block("a", "h20 after"), block("float", "float h20")])
    .with_class_name("block border")
    .with_id("wrap");
  let tree = render(root([wrap]), FLOAT_CSS);
  let (boxes, _) = pages(&tree, 200.0);

  assert_eq!(boxes["float"], [placed(1, 0.0, 20.0)]);
  // The float breaks in a parallel flow, so the first fragment keeps its block-end border.
  assert_eq!(boxes["wrap"][0], placed(0, 0.0, 30.0));
}

#[test]
fn a_float_taller_than_a_page_breaks_across_pages() {
  let tree = render(root([block("float", "float h300")]), FLOAT_CSS);
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(
    boxes["float"],
    [placed(0, 0.0, 200.0), placed(1, 0.0, 100.0)]
  );
}

#[test]
fn a_float_that_avoids_breaking_moves_to_the_next_page_alone() {
  let tree = render(
    root([
      block("a", "h150"),
      block("float", "float h80 avoid"),
      block("b", "h20"),
    ]),
    FLOAT_CSS,
  );
  let (boxes, _) = pages(&tree, 200.0);

  assert_eq!(boxes["float"], [placed(1, 0.0, 80.0)]);
  // The float flows in parallel, so the block after it stays on the first page.
  assert_eq!(boxes["b"], [placed(0, 150.0, 20.0)]);
}

#[test]
fn a_block_that_clears_a_broken_float_follows_it_to_the_next_page() {
  let tree = render(
    root([block("float", "float h300"), block("c", "h20 clear")]),
    FLOAT_CSS,
  );
  let (boxes, _) = pages(&tree, 200.0);

  assert_eq!(boxes["c"], [placed(1, 100.0, 20.0)]);
}

const BLOCK_IN_INLINE_CSS: &str = ".inline { display: inline } .h50 { height: 50px } \
  .h300 { height: 300px }";

fn paragraph_with_block_in_inline(block_in_inline: Node) -> Node {
  let span =
    Node::container([Node::text("a"), block_in_inline, Node::text("z")]).with_class_name("inline");

  root([Node::container([span]).with_tag_name("p").with_id("p")])
}

#[test]
fn a_block_in_inline_breaks_inside_its_line() {
  let tree = render(
    paragraph_with_block_in_inline(block("block", "h300")),
    BLOCK_IN_INLINE_CSS,
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(
    boxes["block"],
    [placed(0, 20.0, 180.0), placed(1, 0.0, 120.0)]
  );
  // The line after the block follows it to the next page.
  assert_eq!(boxes["p"], [placed(0, 0.0, 200.0), placed(1, 0.0, 140.0)]);
}

#[test]
fn a_block_in_inline_whose_content_overflows_resumes_in_a_parallel_flow() {
  let block_in_inline = Node::container([block("tall", "h300")])
    .with_class_name("block h50")
    .with_id("block");
  let tree = render(
    paragraph_with_block_in_inline(block_in_inline),
    BLOCK_IN_INLINE_CSS,
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["block"], [placed(0, 20.0, 50.0), placed(1, 0.0, 0.0)]);
  assert_eq!(
    boxes["tall"],
    [placed(0, 20.0, 180.0), placed(1, 0.0, 120.0)]
  );
  // The line after the block stays on the first page.
  assert_eq!(boxes["p"][0], placed(0, 0.0, 90.0));
}

#[test]
fn a_float_among_lines_breaks_across_pages() {
  let paragraph = Node::container([block("float", "float h300"), Node::text("a")])
    .with_tag_name("p")
    .with_id("p");
  let tree = render(root([paragraph]), FLOAT_CSS);
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(
    boxes["float"],
    [placed(0, 0.0, 200.0), placed(1, 0.0, 100.0)]
  );
}

#[test]
fn a_float_among_lines_that_avoids_breaking_moves_to_the_next_page_alone() {
  let paragraph = Node::container([block("float", "float h80 avoid"), Node::text("x")])
    .with_tag_name("p")
    .with_id("p");
  let tree = render(root([block("a", "h150"), paragraph]), FLOAT_CSS);
  let (boxes, _) = pages(&tree, 200.0);

  assert_eq!(boxes["float"], [placed(1, 0.0, 80.0)]);
  // The float flows in parallel, so the line beside it stays on the first page.
  assert_eq!(boxes["p"][0], placed(0, 150.0, 20.0));
}

#[test]
fn a_box_that_clones_its_decorations_grows_by_those_of_each_fragment() {
  let tree = render(
    root([block("box", "clone")]),
    ".clone { box-decoration-break: clone; box-sizing: content-box; padding: 20px 0; height: 300px }",
  );
  let (boxes, _) = pages(&tree, 200.0);

  // The 300px content and the padding of both fragments make 380px.
  assert_eq!(boxes["box"], [placed(0, 0.0, 200.0), placed(1, 0.0, 180.0)]);
}

#[test]
fn a_fixed_box_repeats_on_the_pages_an_absolute_box_adds() {
  let body =
    Node::container([block("fixed", "fixed"), block("abs", "abs")]).with_class_name("block");
  let tree = render(
    root([body]),
    ".fixed { position: fixed; top: 10px; height: 20px } \
     .abs { position: absolute; top: 0; height: 500px }",
  );
  let (fragments, page_count) = nested_fragments(&tree, 200.0);

  assert_eq!(page_count, 3);
  // The pages past the root's own fragment place it from that fragment.
  assert_eq!(
    fragments,
    [(0, 10.0, 20.0), (0, 210.0, 20.0), (0, 410.0, 20.0)]
  );
}

/// A body of the box `abs` laid out before `content`, on pages 300 wide and 200 tall.
fn page_area_fragments(abs_css: &str, content: Node) -> (Vec<(usize, f32, f32)>, usize) {
  let body = Node::container([block("abs", "abs"), content]).with_class_name("block");
  let tree = render(
    root([body]),
    &format!(
      ".abs {{ position: absolute; {abs_css} }} .h20 {{ height: 20px }} .h300 {{ height: 300px }}"
    ),
  );

  nested_fragments(&tree, 200.0)
}

#[test]
fn an_initial_containing_block_box_below_the_page_moves_to_the_page_it_is_on() {
  let (fragments, page_count) = page_area_fragments("top: 250px; height: 50px", block("a", "h300"));

  assert_eq!(page_count, 2);
  // The root's second fragment is the page's, and holds the box 50px down.
  assert_eq!(fragments, [(1, 50.0, 50.0)]);
}

#[test]
fn an_initial_containing_block_box_past_the_root_adds_pages() {
  let (fragments, page_count) = page_area_fragments("top: 850px; height: 50px", block("a", "h20"));

  assert_eq!(page_count, 5);
  // The root's only fragment holds the box below it, where the last page paints it.
  assert_eq!(fragments, [(0, 850.0, 50.0)]);
}

#[test]
fn an_initial_containing_block_box_taller_than_a_page_breaks_across_pages() {
  let (fragments, page_count) = page_area_fragments("top: 0; height: 450px", block("a", "h20"));

  assert_eq!(page_count, 3);
  assert_eq!(
    fragments,
    [(0, 0.0, 200.0), (0, 200.0, 200.0), (0, 400.0, 50.0)]
  );
}

#[test]
fn an_initial_containing_block_box_taller_than_a_page_resumes_in_the_root_fragment_of_its_page() {
  let (fragments, page_count) =
    page_area_fragments("top: 150px; height: 300px", block("a", "h300"));

  assert_eq!(page_count, 3);
  // The third page is past the root's second, whose fragment holds the box's last.
  assert_eq!(
    fragments,
    [(0, 150.0, 50.0), (1, 0.0, 200.0), (1, 200.0, 50.0)]
  );
}

#[test]
fn a_box_at_the_bottom_of_the_initial_containing_block_sits_at_the_bottom_of_the_first_page() {
  let (fragments, page_count) = page_area_fragments("bottom: 0; height: 50px", block("a", "h300"));

  assert_eq!(page_count, 2);
  assert_eq!(fragments, [(0, 150.0, 50.0)]);
}

#[test]
fn a_fixed_box_repeats_from_the_root_fragment_of_each_page() {
  let body =
    Node::container([block("fixed", "fixed"), block("a", "h300")]).with_class_name("block");
  let tree = render(
    root([body]),
    ".fixed { position: fixed; top: 10px; height: 20px } .h300 { height: 300px } \
     .abs { position: absolute; top: 450px; height: 100px }",
  );
  let (fragments, page_count) = nested_fragments(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(fragments, [(0, 10.0, 20.0), (1, 10.0, 20.0)]);
}

#[test]
fn a_fixed_box_starting_on_a_later_page_repeats_from_that_page() {
  let body =
    Node::container([block("a", "h20 forced"), block("fixed", "fixed")]).with_class_name("block");
  let tree = render(
    root([body]),
    ".fixed { position: fixed; top: 10px; height: 20px } .h20 { height: 20px } \
     .forced { break-after: page } .abs { position: absolute; top: 450px; height: 10px }",
  );
  let (results, page_count) = LayoutResults::compute_pages(
    &tree,
    Size {
      width: 300.0,
      height: 200.0,
    },
    usize::MAX,
  )
  .expect("pages lay out");
  let body = results.box_children(NodeId::ROOT).expect("root children")[0].node_id;
  let fixed = results.box_children(body).expect("body children")[1].node_id;

  assert_eq!(page_count, 2);
  // Blink's page area of the first page never sees a box that starts on the second.
  assert_eq!(
    results
      .fragments(fixed)
      .expect("fragments")
      .iter()
      .map(|fragment| (
        fragment.container_fragment_index,
        fragment.layout.location.y
      ))
      .collect::<Vec<_>>(),
    [(1, 10.0)]
  );
}

#[test]
fn a_fixed_box_repeats_on_the_pages_an_absolute_box_past_the_root_adds() {
  let body = Node::container([
    block("fixed", "fixed"),
    block("a", "h300"),
    block("abs", "abs"),
  ])
  .with_class_name("block");
  let tree = render(
    root([body]),
    ".fixed { position: fixed; top: 10px; height: 20px } .h300 { height: 300px } \
     .abs { position: absolute; top: 450px; height: 100px }",
  );
  let (fragments, page_count) = nested_fragments(&tree, 200.0);

  assert_eq!(page_count, 3);
  assert_eq!(
    fragments,
    [(0, 10.0, 20.0), (1, 10.0, 20.0), (1, 210.0, 20.0)]
  );
}

#[test]
fn a_fixed_box_inside_a_repeated_fixed_box_repeats_too() {
  let outer = Node::container([block("inner", "inner")]).with_class_name("block fixed");
  let body = Node::container([outer, block("a", "h500")]).with_class_name("block");
  let tree = render(
    root([body]),
    ".fixed { position: fixed; top: 10px; height: 20px } \
     .inner { position: fixed; top: 50px; height: 7px } .h500 { height: 500px }",
  );
  let (results, page_count) = LayoutResults::compute_pages(
    &tree,
    Size {
      width: 300.0,
      height: 200.0,
    },
    usize::MAX,
  )
  .expect("pages lay out");
  let inner_fragments = (0..)
    .map_while(|index| results.fragments(NodeId::new(index)).ok())
    .find(|fragments| {
      fragments
        .first()
        .is_some_and(|fragment| fragment.layout.size.height == 7.0)
    })
    .map_or(0, <[_]>::len);

  assert_eq!(page_count, 3);
  assert_eq!(inner_fragments, 3);
}

#[test]
fn an_absolute_box_inside_a_clipping_table_adds_no_pages() {
  let table = Node::container([block("tall", "tall")]).with_class_name("table clip");
  let body = Node::container([table]).with_class_name("block");
  let tree = render(
    root([body]),
    ".table { display: table; position: relative; overflow: clip; width: 10px; height: 10px } \
     .tall { position: absolute; height: 700px }",
  );
  let (_, page_count) = nested_fragments(&tree, 200.0);

  assert_eq!(page_count, 1);
}

#[test]
fn a_table_that_clones_its_decorations_leaves_them_out_of_its_rows() {
  let tree = render(
    root([table([("row", block("content", "h60"))]).with_class_name("table clone")]),
    &format!(
      "{TABLE_CSS} .clone {{ box-decoration-break: clone; box-sizing: content-box; padding: 20px 0; \
       height: 300px }}"
    ),
  );
  let (boxes, _) = pages(&tree, 200.0);

  // The row takes the 300px content box alone, and each fragment repeats the 40px padding.
  assert_eq!(
    boxes["row"],
    [placed(0, 20.0, 160.0), placed(1, 20.0, 140.0)]
  );
  assert_eq!(
    boxes["table"],
    [placed(0, 0.0, 200.0), placed(1, 0.0, 180.0)]
  );
}

const FLEX_CSS: &str = ".column { display: flex; flex-direction: column } \
  .row { display: flex; flex-wrap: wrap } .full { width: 100% } .h120 { height: 120px } \
  .h150 { height: 150px } .h300 { height: 300px } .avoid { break-inside: avoid } \
  .before { break-before: page }";

fn flex(class: &str, items: impl IntoIterator<Item = Node>) -> Node {
  Node::container(items.into_iter().collect::<Vec<_>>())
    .with_class_name(class)
    .with_id("flex")
}

#[test]
fn a_column_flex_item_breaks_across_pages() {
  let tree = render(
    root([flex("column", [block("a", "h150"), block("b", "h150")])]),
    FLEX_CSS,
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["a"], [placed(0, 0.0, 150.0)]);
  assert_eq!(boxes["b"], [placed(0, 150.0, 50.0), placed(1, 0.0, 100.0)]);
  assert_eq!(
    boxes["flex"],
    [placed(0, 0.0, 200.0), placed(1, 0.0, 100.0)]
  );
}

#[test]
fn a_column_flex_item_after_a_forced_break_starts_the_next_page() {
  let tree = render(
    root([flex(
      "column",
      [block("a", "h120"), block("b", "h120 before")],
    )]),
    FLEX_CSS,
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["b"], [placed(1, 0.0, 120.0)]);
}

#[test]
fn a_flex_row_item_breaks_inside_where_the_page_ends() {
  let tree = render(
    root([flex(
      "row",
      [block("a", "full h120"), block("b", "full h120")],
    )]),
    FLEX_CSS,
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["b"], [placed(0, 120.0, 80.0), placed(1, 0.0, 40.0)]);
}

#[test]
fn a_flex_row_that_avoids_breaking_inside_moves_to_the_next_page() {
  let tree = render(
    root([flex(
      "row",
      [block("a", "full h120"), block("b", "full h120 avoid")],
    )]),
    FLEX_CSS,
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["b"], [placed(1, 0.0, 120.0)]);
  assert_eq!(
    boxes["flex"],
    [placed(0, 0.0, 200.0), placed(1, 0.0, 120.0)]
  );
}

const GRID_CSS: &str = ".grid { display: grid } .h150 { height: 150px } \
  .avoid { break-inside: avoid } .before { break-before: page }";

fn grid(items: impl IntoIterator<Item = Node>) -> Node {
  Node::container(items.into_iter().collect::<Vec<_>>())
    .with_class_name("grid")
    .with_id("grid")
}

#[test]
fn a_grid_item_breaks_across_pages() {
  let tree = render(
    root([grid([block("a", "h150"), block("b", "h150")])]),
    GRID_CSS,
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["a"], [placed(0, 0.0, 150.0)]);
  assert_eq!(boxes["b"], [placed(0, 150.0, 50.0), placed(1, 0.0, 100.0)]);
  assert_eq!(
    boxes["grid"],
    [placed(0, 0.0, 200.0), placed(1, 0.0, 100.0)]
  );
}

#[test]
fn a_grid_row_that_avoids_breaking_inside_moves_to_the_next_page() {
  let tree = render(
    root([grid([block("a", "h150"), block("b", "h150 avoid")])]),
    GRID_CSS,
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["b"], [placed(1, 0.0, 150.0)]);
}

#[test]
fn a_grid_row_after_a_forced_break_starts_the_next_page() {
  let tree = render(
    root([grid([block("a", "h150"), block("b", "h150 before")])]),
    GRID_CSS,
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(boxes["b"], [placed(1, 0.0, 150.0)]);
}

#[test]
fn a_box_knows_whether_a_descendant_resolves_a_percentage_block_size_against_it() {
  let depends = |child: Node, css: &str| {
    render(root([child]), css).has_descendant_that_depends_on_percentage_block_size()
  };
  let css = ".half { height: 50% } .fixed { height: 20px } .abs { position: absolute } \
    .rel { position: relative; top: 10% } .inline { display: inline }";

  assert!(depends(block("a", "half"), css));
  assert!(!depends(block("a", "fixed"), css));
  // An out-of-flow box resolves its percentages against the final size.
  assert!(!depends(block("a", "half abs"), css));
  // A relatively positioned box moves by a percentage of the block size.
  assert!(depends(block("a", "fixed rel"), css));
  // An inline box passes on its content's dependence.
  assert!(depends(
    Node::container([block("a", "half")]).with_class_name("inline"),
    css
  ));
  // A box of a fixed size keeps its children's percentages to itself.
  assert!(!depends(
    Node::container([block("b", "half")]).with_class_name("block fixed"),
    css
  ));
}

#[test]
fn a_flex_item_with_a_percentage_child_keeps_its_flexed_size_past_a_break() {
  let item = |first: &str| {
    Node::container([block("first", first), block("second", "h180 before")])
      .with_class_name("block grow")
      .with_id("item")
  };
  let css = ".column { display: flex; flex-direction: column; height: 200px } \
    .grow { flex: 1 1 0 } .h50 { height: 50px } .quarter { height: 25% } .h180 { height: 180px } \
    .before { break-before: page }";
  let fragments = |first: &str| {
    let tree = render(root([flex("column", [item(first)])]), css);

    pages(&tree, 100.0).0["item"].clone()
  };

  // The item grows to hold the content a forced break pushed down.
  assert_eq!(
    fragments("h50"),
    [
      placed(0, 0.0, 100.0),
      placed(1, 0.0, 100.0),
      placed(2, 0.0, 80.0)
    ]
  );
  // A child that resolves a percentage against it would change with it, so it keeps its size.
  assert_eq!(
    fragments("quarter"),
    [
      placed(0, 0.0, 100.0),
      placed(1, 0.0, 100.0),
      placed(2, 0.0, 0.0)
    ]
  );
}

const OUT_OF_FLOW_CSS: &str = ".relative { position: relative } \
  .abs { position: absolute; top: 0; height: 300px } .h300 { height: 300px }";

#[test]
fn an_absolute_box_taller_than_a_page_breaks_across_the_pages_of_a_flex_container() {
  let tree = render(
    root([Node::container([block("abs", "abs"), block("a", "h300")])
      .with_class_name("row relative")
      .with_id("flex")]),
    &format!("{FLEX_CSS} {OUT_OF_FLOW_CSS}"),
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(
    boxes["flex"],
    [placed(0, 0.0, 200.0), placed(1, 0.0, 100.0)]
  );
  assert_eq!(boxes["abs"], [placed(0, 0.0, 200.0), placed(1, 0.0, 100.0)]);
}

#[test]
fn an_absolute_box_taller_than_a_page_breaks_across_the_pages_of_a_grid() {
  let tree = render(
    root([Node::container([block("abs", "abs"), block("a", "h300")])
      .with_class_name("grid relative")
      .with_id("grid")]),
    &format!("{GRID_CSS} {OUT_OF_FLOW_CSS}"),
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(
    boxes["grid"],
    [placed(0, 0.0, 200.0), placed(1, 0.0, 100.0)]
  );
  assert_eq!(boxes["abs"], [placed(0, 0.0, 200.0), placed(1, 0.0, 100.0)]);
}

#[test]
fn an_absolute_box_taller_than_a_page_breaks_across_the_pages_of_a_table() {
  let cell = Node::container([block("abs", "abs"), block("a", "h300")]).with_class_name("block");
  let tree = render(
    root([table([("row", cell)]).with_class_name("table relative")]),
    &format!("{TABLE_CSS} {OUT_OF_FLOW_CSS}"),
  );
  let (boxes, page_count) = pages(&tree, 200.0);

  assert_eq!(page_count, 2);
  assert_eq!(
    boxes["table"],
    [placed(0, 0.0, 200.0), placed(1, 0.0, 100.0)]
  );
  assert_eq!(boxes["abs"], [placed(0, 0.0, 200.0), placed(1, 0.0, 100.0)]);
}

/// How many pages are final after each page of `root` is laid out on pages 300 wide.
fn finalized_after_each_page(root: &RenderNode, page_height: f32) -> Vec<usize> {
  let page_size = Size {
    width: 300.0,
    height: page_height,
  };
  let mut layout = PagedLayout::new(root, page_size, usize::MAX);
  let mut finalized = Vec::new();

  while layout.next_page().expect("page lays out") {
    finalized.push(layout.finalized_pages());
  }
  oracles::check(root, page_size);
  finalized
}

/// A box a containing block holds back keeps the block's pages open until it ends.
#[test]
fn a_held_box_keeps_the_pages_from_its_containing_block_open() {
  let tree = render(
    root([
      block("a", "h300"),
      Node::container([block("held", "held")]).with_class_name("block rel"),
    ]),
    ".h300 { height: 300px } .rel { position: relative; height: 500px } \
     .held { position: absolute; bottom: 0; height: 10px }",
  );

  // The block starts on the second page and ends on the fourth.
  assert_eq!(finalized_after_each_page(&tree, 200.0), [1, 1, 1, 4]);
}

/// An absolute box found on a later page keeps its containing block's first pages open.
#[test]
fn an_absolute_box_found_on_a_later_page_keeps_its_containing_blocks_pages_open() {
  let tree = render(
    root([
      block("before", "h100"),
      Node::container([block("a", "h500"), block("late", "late")]).with_class_name("block rel"),
    ]),
    ".h100 { height: 100px } .h500 { height: 500px } .rel { position: relative } \
     .late { position: absolute; top: 10px; left: 0; width: 10px; height: 10px }",
  );

  // The box is found on the third page and placed from the block's start on the first.
  assert_eq!(finalized_after_each_page(&tree, 200.0), [0, 0, 3]);
}

/// A box the styles do not bound can paint on any page until the layout has laid it out.
#[test]
fn a_box_with_a_transform_keeps_every_page_open_until_it_is_laid_out() {
  let tree = render(
    root([block("a", "h500"), block("up", "up"), block("b", "h500")]),
    ".h500 { height: 500px } .up { height: 10px; transform: translateY(-300px) }",
  );

  assert_eq!(finalized_after_each_page(&tree, 200.0), [0, 0, 3, 4, 5, 6]);
}

/// An absolute box a later page finds paints on the first page, the initial containing block's.
#[test]
fn an_absolute_box_found_on_a_later_page_keeps_every_page_open_until_then() {
  let tree = render(
    root([
      block("a", "h300"),
      Node::container([block("late", "late")]).with_class_name("block"),
    ]),
    ".h300 { height: 300px } .late { position: absolute; top: 0; left: 0; width: 10px; height: 10px }",
  );
  let (results, _) = LayoutResults::compute_pages(
    &tree,
    Size {
      width: 300.0,
      height: 200.0,
    },
    usize::MAX,
  )
  .expect("pages lay out");
  // The box is the last in the tree, found on the second page and placed a page above it.
  let late = results
    .fragments(NodeId::new(results.node_count() - 1))
    .expect("fragments");

  assert_eq!(late.len(), 1);
  assert_eq!(late[0].container_fragment_index, 1);
  assert_eq!(late[0].layout.location.y, -200.0);
  assert_eq!(finalized_after_each_page(&tree, 200.0), [0, 2]);
}

/// A box painted far from its page belongs to the window of the pages it reaches.
#[test]
fn a_box_painted_pages_away_paints_in_their_windows() {
  let tree = render(
    root([
      block("a", "h700"),
      block("up", "up"),
      block("b", "h700"),
      block("down", "down"),
    ]),
    ".h700 { height: 700px } \
     .up { height: 50px; width: 20px; background: red; transform: translateY(-1100px) } \
     .down { height: 50px; width: 20px; background: red; position: relative; top: -900px }",
  );

  oracles::check_when_finished(
    &tree,
    Size {
      width: 300.0,
      height: 200.0,
    },
  );
}

/// A box a containing block holds back belongs to the fragment of its containing block that lays
/// it out.
#[test]
fn a_box_hoisted_past_its_parent_paints_in_the_window_of_its_containing_block() {
  let tree = render(
    root([Node::container([
      Node::container([block("held", "held"), block("late", "late")]).with_class_name("block"),
      block("tail", "h700"),
    ])
    .with_class_name("block rel")]),
    ".rel { position: relative } .h700 { height: 700px } \
     .held { position: absolute; bottom: 0; width: 20px; height: 30px; background: red } \
     .late { position: absolute; top: 450px; width: 20px; height: 30px; background: red }",
  );

  oracles::check_when_finished(
    &tree,
    Size {
      width: 300.0,
      height: 200.0,
    },
  );
}

/// The root's last fragment stays open while page areas go on after it.
#[test]
fn the_roots_last_fragment_stays_open_while_page_areas_go_on() {
  let tree = render(
    root([block("a", "h50"), block("tall", "tall")]),
    ".h50 { height: 50px } .tall { position: absolute; top: 500px; height: 1000px; width: 10px }",
  );

  assert_eq!(
    finalized_after_each_page(&tree, 200.0),
    [0, 0, 0, 0, 0, 0, 0, 8]
  );
}

const CLIPPED_OVERFLOW_CSS: &str = ".box { height: 100px } .h50 { height: 50px } .h150 { height: 150px } \
  .h400 { height: 400px } .hidden { overflow: hidden } .clip { overflow: clip } \
  .flex { display: flex } .grid { display: grid }";

/// A 100px box on the first page, then a 100px box styled `class` holding a 400px child, on pages
/// 100px tall.
fn clipped_overflow_pages(class: &str) -> (BTreeMap<String, Vec<Placed>>, usize) {
  let tree = render(
    root([
      block("first", "box"),
      Node::container([block("inner", "h400")])
        .with_class_name(format!("block box {class}"))
        .with_id("clipper"),
    ]),
    CLIPPED_OVERFLOW_CSS,
  );

  pages(&tree, 100.0)
}

#[test]
fn the_content_an_overflow_hidden_box_clips_adds_no_pages() {
  let (boxes, page_count) = clipped_overflow_pages("hidden");

  assert_eq!(page_count, 2);
  assert_eq!(boxes["clipper"], [placed(1, 0.0, 100.0)]);
  assert_eq!(boxes["inner"], [placed(1, 0.0, 400.0)]);
}

#[test]
fn the_content_an_overflow_clip_box_clips_adds_no_pages() {
  let (boxes, page_count) = clipped_overflow_pages("clip");

  assert_eq!(page_count, 2);
  assert_eq!(boxes["inner"], [placed(1, 0.0, 400.0)]);
}

#[test]
fn content_overflowing_a_visible_box_continues_on_later_pages() {
  let (boxes, page_count) = clipped_overflow_pages("");

  assert_eq!(page_count, 5);
  assert_eq!(
    boxes["clipper"],
    [
      placed(1, 0.0, 100.0),
      placed(2, 0.0, 0.0),
      placed(3, 0.0, 0.0),
      placed(4, 0.0, 0.0)
    ]
  );
}

#[test]
fn a_clipping_box_that_breaks_stops_breaking_on_the_page_it_ends() {
  let tree = render(
    root([
      block("first", "h50"),
      Node::container([block("inner", "h400")])
        .with_class_name("block h150 hidden")
        .with_id("clipper"),
      block("after", "h50"),
    ]),
    CLIPPED_OVERFLOW_CSS,
  );
  let (boxes, page_count) = pages(&tree, 100.0);

  assert_eq!(page_count, 3);
  assert_eq!(
    boxes["clipper"],
    [placed(0, 50.0, 50.0), placed(1, 0.0, 100.0)]
  );
  assert_eq!(
    boxes["inner"],
    [placed(0, 50.0, 50.0), placed(1, 0.0, 350.0)]
  );
  assert_eq!(boxes["after"], [placed(2, 0.0, 50.0)]);
}

#[test]
fn the_content_a_clipping_flex_container_clips_adds_no_pages() {
  let (boxes, page_count) = clipped_overflow_pages("flex hidden");

  assert_eq!(page_count, 2);
  assert_eq!(boxes["clipper"], [placed(1, 0.0, 100.0)]);
}

#[test]
fn the_content_a_clipping_grid_container_clips_adds_no_pages() {
  let (boxes, page_count) = clipped_overflow_pages("grid hidden");

  assert_eq!(page_count, 2);
  assert_eq!(boxes["clipper"], [placed(1, 0.0, 100.0)]);
}

/// The block-axis extent, in the stacked pages, of the clip each fragment of the box with `id`
/// clips its content to.
fn content_clip_extents(scene: &Scene, id: &str) -> Vec<(f32, f32)> {
  paints_of(scene, id)
    .iter()
    .map(|paint| {
      let clip = scene
        .properties
        .clip(paint.properties.contents.clip.expect("an overflow clip"));
      let ClipKind::Shape(shape) = &clip.kind else {
        panic!("a shape clip");
      };
      let bounds = PathCommand::build_path(&shape.to_commands())
        .expect("a clip path")
        .bounds();
      let (_, top) = clip.transform.transform_point(bounds.left(), bounds.top());
      let (_, bottom) = clip
        .transform
        .transform_point(bounds.left(), bounds.bottom());

      (top, bottom)
    })
    .collect()
}

/// A box clipping to rounded corners keeps its pages open until it ends, since its whole height
/// shapes the corners on every page. A square clip leaves them alone.
#[test]
fn a_rounded_clip_keeps_its_pages_open_until_it_ends() {
  for (container, finalized) in [
    ("clipping rounded", [0, 0, 0, 4]),
    ("clipping", [1, 2, 3, 4]),
  ] {
    let tree = render(
      root([
        Node::container([block("child", "tall")]).with_class_name(format!("block {container}"))
      ]),
      ".clipping { overflow: hidden } .rounded { border-radius: 50% } .tall { height: 400px }",
    );

    assert_eq!(
      finalized_after_each_page(&tree, 100.0),
      finalized,
      "{container}"
    );
  }
}

#[test]
fn a_broken_overflow_clip_spans_the_stitched_box() {
  for (child, container) in [
    ("translated", "clipping"),
    ("translated positioned", "clipping"),
    ("translated", "clipping rounded"),
  ] {
    let tree = render(
      root([Node::container([block("child", child)])
        .with_class_name(format!("block {container}"))
        .with_id("container")]),
      ".clipping { height: 200px; overflow: hidden; position: relative }
       .rounded { border: 4px solid black; border-radius: 12px }
       .translated { height: 120px; transform: translateY(50px) }
       .positioned { position: absolute; width: 200px }",
    );
    let paged = PagedScene::lay_out(
      tree,
      Size {
        width: 300.0,
        height: 100.0,
      },
      usize::MAX,
      false,
    )
    .expect("pages lay out");
    let scene = &paged.into_scene();
    let inset = if container.contains("rounded") {
      4.0
    } else {
      0.0
    };

    // Each page clips to the whole container, so the child pushed past the first page by its
    // transform shows on the second.
    assert_eq!(
      content_clip_extents(scene, "container"),
      [(inset, 200.0 - inset), (inset, 200.0 - inset)],
      "{child} in {container}"
    );
  }
}
