//! Checks run on every document of the fragmentation tests: paged layout stepped a page at a time
//! agrees with the layout driven to the end.

use crate::{
  geometry::{NodeId, Size},
  layout::{
    paged_layout::PagedLayout,
    tree::{LayoutResults, RenderNode},
  },
};

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

/// Lays `root` out a page at a time and a second time to the end, and checks they agree.
pub(crate) fn check(root: &RenderNode, page_size: Size<f32>) {
  let (expected, expected_pages) =
    LayoutResults::compute_pages(root, page_size, usize::MAX).expect("pages lay out");
  let mut layout = PagedLayout::new(root, page_size, usize::MAX);
  let mut laid = Vec::new();

  while let Some(page) = layout.next_page().expect("page lays out") {
    assert_eq!(page.index, laid.len(), "pages come in order");
    laid.push(page);
  }

  assert!(layout.is_finished());
  assert_eq!(laid.len(), expected_pages);
  assert_eq!(layout.page_count(), expected_pages);

  let results = layout.into_results();

  assert_eq!(snapshot(&results), snapshot(&expected));
  for page in laid {
    assert_eq!(
      page.root_fragment,
      results.fragment_in(NodeId::ROOT, page.index),
      "the root's fragment on page {}",
      page.index
    );
  }
}
