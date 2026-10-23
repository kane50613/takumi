//! Checks run on every document of the fragmentation tests: paged layout stepped a page at a time
//! agrees with the layout driven to the end, and so does the view of it while it steps.

use rustc_hash::FxHashMap;

use crate::{
  geometry::{NodeId, Size},
  layout::{
    layout_view::LayoutView,
    paged_layout::PagedLayout,
    tree::{LayoutResults, RenderNode},
  },
};

/// What `view` answers for every box, as text, with the pages up to `page_count` to ask about.
fn answers(view: LayoutView<'_>, node_count: usize, page_count: usize) -> Vec<String> {
  (0..node_count)
    .map(NodeId::new)
    .map(|node| {
      let fragment_count = view.container_fragment_indices(node).count();
      let mut answer = format!("{:?}", view.layout(node));

      for fragment in 0..=fragment_count {
        answer += &format!(
          " {:?} {} {:?}",
          view.fragment_layout(node, fragment),
          view.is_repeated_fragment(node, fragment),
          view.table_fragment(node, fragment)
        );
      }
      answer += &format!(
        " {:?} {} {}",
        view.container_fragment_indices(node).collect::<Vec<_>>(),
        view.is_hidden_for_paint(node),
        view.is_line_clamp_clipped_float(node),
      );
      for child in view.box_children(node).expect("children") {
        answer += &format!(
          " ({} {:?} {:?} {:?})",
          child.render_index,
          child.inline_path,
          child.node_id,
          view.hoisted_cb(node, child)
        );
      }
      for container in 0..=page_count {
        answer += &format!(
          " {:?} {:?}",
          view.fragments_in(node, container).collect::<Vec<_>>(),
          view
            .ordered_children_in(node, container)
            .expect("children")
            .iter()
            .map(|child| child.node_id)
            .collect::<Vec<_>>()
        );
      }
      answer
    })
    .collect()
}

/// Every node's fragments and box children, as text.
fn snapshot(results: &LayoutResults) -> Vec<String> {
  (0..results.node_count())
    .map(NodeId::new)
    .map(|node| {
      format!(
        "{:?} {:?}",
        results.fragments(node).expect("fragments"),
        results.box_children(node).expect("children")
      )
    })
    .collect()
}

/// Lays `root` out a page at a time and a second time to the end, and checks they agree, and that
/// the live view of the first agrees with the finished results.
pub(crate) fn check(root: &RenderNode, page_size: Size<f32>) {
  let (expected, expected_pages) =
    LayoutResults::compute_pages(root, page_size, usize::MAX).expect("pages lay out");
  let mut layout = PagedLayout::new(root, page_size, usize::MAX);
  let mut laid = Vec::new();
  let mut finalized = Vec::new();

  while let Some(page) = layout.next_page().expect("page lays out") {
    assert_eq!(page.index, laid.len(), "pages come in order");
    laid.push(page);

    let finalized_pages = layout.finalized_pages();

    assert!(finalized_pages <= layout.page_count());
    assert!(
      finalized_pages >= finalized.len(),
      "a page that was final stays final"
    );
    finalized = (0..finalized_pages)
      .map(|page| layout.view().fragment_in(NodeId::ROOT, page))
      .collect();
  }

  assert!(layout.is_finished());
  assert_eq!(laid.len(), expected_pages);
  assert_eq!(layout.page_count(), expected_pages);

  assert_hoisting(&layout);

  let live = answers(layout.view(), expected.node_count(), expected_pages);
  let results = layout.into_results();

  assert_eq!(snapshot(&results), snapshot(&expected));
  assert_eq!(
    live,
    answers(results.view(), results.node_count(), expected_pages),
    "the live view and the finished results agree"
  );
  for page in laid {
    assert_eq!(
      page.root_fragment,
      results.fragment_in(NodeId::ROOT, page.index),
      "the root's fragment on page {}",
      page.index
    );
  }
  for (page, fragment) in finalized.into_iter().enumerate() {
    assert_eq!(fragment, results.fragment_in(NodeId::ROOT, page));
  }
}

/// Checks the view names the box that positioned each box the way the finished layout did: the
/// last node in layout order whose hoisted children hold it, unless that is the box's parent.
fn assert_hoisting(layout: &PagedLayout<'_>) {
  let tree = layout.tree();
  let view = layout.view();
  let mut containing_blocks = FxHashMap::default();

  for index in 0..tree.node_count() {
    for &child in &tree.node_state(NodeId::new(index)).hoisted_children {
      containing_blocks.insert(usize::from(child), index);
    }
  }
  for index in 0..tree.node_count() {
    let node = NodeId::new(index);

    for child in view.box_children(node).expect("children") {
      assert_eq!(
        view.hoisted_cb(node, child),
        containing_blocks
          .get(&usize::from(child.node_id))
          .filter(|&&container| container != index)
          .map(|&container| NodeId::new(container)),
      );
    }
  }
}
