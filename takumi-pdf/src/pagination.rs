//! The content laid out across pages, which takumi-core's block fragmentation breaks, and the
//! page counters filled in from where it put each box.

use takumi_core::layout::node::Node;

use crate::{
  bands::Repeatable,
  counters::{
    BoxPages, FlowCounters, has_page_counters, has_target_counters, substitute_target_counters,
  },
  interactive::Interactive,
  options::PdfError,
  page::PageFrame,
  tree::{PreparedTree, TreeInputs},
};

/// How many pages one render may lay its content out on. A document tall enough to pass this is
/// not a document anyone reads; it is a way to spend a renderer's memory.
pub(crate) const MAX_PAGES: usize = 20_000;

/// How many times pagination reruns to settle page counters. A counter widens
/// the line it sits on, which can move a break, which can renumber the counter.
/// The second pass numbers a laid-out document, the third confirms the numbers
/// did not move, and a document still moving after that keeps the last pass's
/// numbers.
const COUNTER_PASSES: usize = 3;

/// The content laid out across pages, with the `fixed` boxes that repeat on every page and the
/// interactive targets the pages draw from. The pages stack in the content's coordinates, each the
/// page area tall.
pub(crate) struct Paginated {
  pub(crate) content: PreparedTree,
  pub(crate) repeated: Vec<Repeatable>,
  pub(crate) starts: Vec<f32>,
  pub(crate) interactive: Interactive,
  window: f32,
}

/// One page's window into the content.
pub(crate) struct PageSlice {
  /// 0-based; display page numbers are `index + 1`.
  pub(crate) index: usize,
  pub(crate) start: f32,
  /// Where the next page begins.
  pub(crate) end: f32,
}

impl Paginated {
  /// Lays the content out on pages, refilling the counters it
  /// holds until the numbers stop moving. A counter inside a repeated box is
  /// left to that box, which lays out again per page with its own numbers.
  pub(crate) fn build(
    mut node: Node,
    inputs: &TreeInputs<'_>,
    frame: &PageFrame,
  ) -> Result<Self, PdfError> {
    if !has_page_counters(&node) && !has_target_counters(&node) {
      return Self::build_once(node, inputs, frame);
    }
    let mut previous: Vec<String> = Vec::new();
    let mut passes = 0;

    loop {
      let paginated = Self::build_once(node.clone(), inputs, frame)?;
      let mut written = Vec::new();

      passes += 1;
      paginated.substitute_counters(&mut node, &mut written);
      if written == previous {
        return Ok(paginated);
      }
      previous = written;
      // The numbers are still moving, and this was the last pass allowed. They
      // are laid out once more so the page shows the numbers it was laid out with.
      if passes == COUNTER_PASSES {
        return Self::build_once(node, inputs, frame);
      }
    }
  }

  fn build_once(node: Node, inputs: &TreeInputs<'_>, frame: &PageFrame) -> Result<Self, PdfError> {
    let (content, repeated, page_count) = inputs.prepare_paged(node, frame)?;
    let window = frame.window_height;
    let starts = (0..page_count).map(|page| page as f32 * window).collect();
    let interactive = Interactive::collect(&content);

    Ok(Self {
      content,
      repeated,
      starts,
      interactive,
      window,
    })
  }

  /// Fills both kinds of counter from this laid-out pass, collecting what it
  /// wrote so the caller can see whether the numbers moved.
  fn substitute_counters(&self, node: &mut Node, written: &mut Vec<String>) {
    let pages = self.starts.len();
    let page_at = |top: &f32| self.page_index(*top) + 1;
    // `fill_root` wraps the tree in a page-wide box, which takes preorder 0.
    let page_of = self
      .interactive
      .extents
      .iter()
      .filter_map(|(order, extent)| {
        let pages = BoxPages {
          start: page_at(&extent.top),
          next: extent.flow_bottom.as_ref().map(page_at),
        };

        Some((order.checked_sub(1)?, pages))
      })
      .collect();
    let repeated: Vec<_> = self
      .repeated
      .iter()
      .filter_map(Repeatable::source_orders)
      .map(|orders| orders.start.saturating_sub(1)..orders.end.saturating_sub(1))
      .collect();

    FlowCounters::new(&page_of, pages, &repeated).substitute(node, None, written);

    let anchors = &self.interactive.anchors;
    let target_page = |id: &str| anchors.get(id).map(|anchor| page_at(&anchor.top));

    substitute_target_counters(node, None, &target_page, written);
  }

  /// 0-based index of the page `top` falls on.
  pub(crate) fn page_index(&self, top: f32) -> usize {
    self
      .starts
      .partition_point(|start| *start <= top)
      .saturating_sub(1)
  }

  pub(crate) fn pages(&self) -> impl Iterator<Item = PageSlice> + '_ {
    self
      .starts
      .iter()
      .enumerate()
      .map(|(index, &start)| PageSlice {
        index,
        start,
        end: start + self.window,
      })
  }
}
