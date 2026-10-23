//! Checks run on every document of the fragmentation tests: paged layout stepped a page at a time
//! agrees with the layout driven to the end, and so does the view of it while it steps.

use std::collections::HashSet;

use rustc_hash::FxHashMap;

use crate::{
  geometry::{NodeId, Size},
  layout::{
    layout_view::LayoutView,
    paged_layout::PagedLayout,
    tree::{LayoutResults, RenderNode},
  },
  paint_chunk::PaintChunk,
  scene::{NodePaint, PageWindows, PagedScene, SceneBounds, SceneRef, StackingContextNode},
};

/// What `view` answers for every box, as text, with the pages up to `page_count` to ask about.
fn answers(view: LayoutView<'_>, node_count: usize, page_count: usize) -> Vec<String> {
  (0..node_count)
    .map(NodeId::new)
    .map(|node| {
      let fragment_count = view.fragment_containers(node).count();
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
        view.fragment_containers(node).collect::<Vec<_>>(),
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

/// Lays `root` out a page at a time and to the end, and checks that they agree and that each page
/// paints from its window.
pub(crate) fn check(root: &RenderNode, page_size: Size<f32>) {
  check_pages(root, page_size, true);
}

/// [`check`] for a document whose pages paint from their windows only once every page is laid out.
pub(crate) fn check_when_finished(root: &RenderNode, page_size: Size<f32>) {
  check_pages(root, page_size, false);
}

fn check_pages(root: &RenderNode, page_size: Size<f32>, streams: bool) {
  let (expected, expected_pages) =
    LayoutResults::compute_pages(root, page_size, usize::MAX).expect("pages lay out");
  let mut layout = PagedLayout::new(root, page_size, usize::MAX);
  let mut finalized = Vec::new();

  while layout.next_page().expect("page lays out") {
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
  assert_eq!(layout.page_count(), expected_pages);

  assert_hoisting(&layout);
  assert_windows(root, page_size, streams);

  let live = answers(layout.view(), expected.node_count(), expected_pages);
  let results = layout.into_results();

  assert_eq!(snapshot(&results), snapshot(&expected));
  assert_eq!(
    live,
    answers(results.view(), results.node_count(), expected_pages),
    "the live view and the finished results agree"
  );
  for (page, fragment) in finalized.into_iter().enumerate() {
    assert_eq!(fragment, results.view().fragment_in(NodeId::ROOT, page));
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

fn page_chunks(
  contexts: &[StackingContextNode],
  page: usize,
  page_height: f32,
) -> Vec<(PaintChunk<'_>, String)> {
  let (top, bottom) = (page as f32 * page_height, (page + 1) as f32 * page_height);
  let reaches = |bounds: Option<SceneBounds>| {
    bounds.is_none_or(|bounds| {
      !bounds.is_empty() && (bounds.top as f32) < bottom && (bounds.bottom as f32) > top
    })
  };

  PaintChunk::in_paint_order(contexts)
    .into_iter()
    .filter(|chunk| reaches(chunk.node.paint_bounds))
    .map(|chunk| {
      let paint = chunk.node;
      let text = format!(
        "{:?} {} {:?} {:?} {:?} {:?} {:?}",
        paint.path,
        paint.fragment_index,
        chunk.part,
        paint.transform,
        paint.paint_bounds,
        paint.paint_offset,
        paint.container_size
      );

      (chunk, text)
    })
    .collect()
}

/// Where a scene's tree has the box at `path` and the inline content under it that no chunk of
/// `placed` paints, once the scene entered `paint`.
fn entered(root: &RenderNode, paint: &NodePaint, placed: &HashSet<Vec<usize>>) -> Vec<String> {
  let mut states = Vec::new();
  let mut stack = vec![(
    root.node_at_path(&paint.path).expect("node"),
    paint.path.clone(),
  )];

  while let Some((node, path)) = stack.pop() {
    states.push(format!(
      "{path:?} {:?} {}",
      node.context.paint_offset(),
      node.context.fragment_index()
    ));
    for (index, child) in node.children.iter().flatten().enumerate() {
      let mut child_path = path.clone();

      child_path.push(index);
      if !placed.contains(&child_path) {
        stack.push((child, child_path));
      }
    }
  }
  states
}

/// Paints each page from its window as the pages settle, and checks the chunks and the boxes'
/// states agree with the scene of every page at once.
fn assert_windows(root: &RenderNode, page_size: Size<f32>, streams: bool) {
  let whole = PagedScene::lay_out(root.clone(), page_size, usize::MAX, true)
    .expect("pages lay out")
    .into_scene();
  let placed: HashSet<Vec<usize>> = PaintChunk::in_paint_order(&whole.contexts)
    .iter()
    .map(|chunk| chunk.node.path.clone())
    .collect();
  let mut layout = PagedLayout::new(root, page_size, usize::MAX);
  let mut windows = PageWindows::new(page_size);
  let mut emitted = 0;

  loop {
    let stepped = layout.next_page().expect("page lays out");

    windows
      .refresh(
        root,
        layout.view(),
        layout.page_count(),
        layout.finalized_pages(),
      )
      .expect("pages refresh");

    let ready = if layout.is_finished() {
      layout.page_count()
    } else if streams {
      layout.settled_pages()
    } else {
      0
    };

    for page in emitted..ready {
      let pages = windows.pages_reaching(page);
      let window =
        SceneRef::window(root, layout.view(), page_size, &pages, true).expect("window builds");
      let expected = page_chunks(&whole.contexts, page, page_size.height);

      assert_eq!(
        page_chunks(&window.contexts, page, page_size.height)
          .into_iter()
          .map(|(_, text)| text)
          .collect::<Vec<_>>(),
        expected
          .iter()
          .map(|(_, text)| text.clone())
          .collect::<Vec<_>>(),
        "the chunks of page {page} from the window of pages {pages:?}"
      );
      for (chunk, _) in &expected {
        whole.view().enter(chunk.node).expect("whole scene enters");
        window.view().enter(chunk.node).expect("window enters");
        assert_eq!(
          entered(root, chunk.node, &placed),
          entered(&whole.root, chunk.node, &placed),
          "the boxes of {:?} on page {page}",
          chunk.node.path
        );
      }
    }
    emitted = emitted.max(ready);
    if !stepped {
      break;
    }
  }
  assert_eq!(emitted, layout.page_count());
}
