//! Blink's [`PaginatedRootLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/paginated_root_layout_algorithm.cc)
//! without page boxes or `@page`, driven a page at a time so a caller can read the pages laid out so far.

use std::rc::Rc;

use rustc_hash::FxHashSet;

use crate::{
  Error, Result,
  geometry::{AvailableSpace, NodeId, Point, Size},
  layout::{
    block_break_token::BlockBreakToken,
    break_appeal::BreakAppeal,
    break_token::BreakToken,
    constraint_space::BlockFragmentation,
    fragmentation_layout_input::FragmentationLayoutInput,
    fragmentation_utils::{adjust_margins_for_fragmentation, fragment_index, is_break_inside},
    layout_view::LayoutView,
    length_utils::margins_for,
    page_area::PageAreas,
    paint_reach::PaintReach,
    pending_boxes::PendingBoxes,
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  layout_unit::LayoutUnit,
  style::{Direction, Length, SpacePair},
};

/// A render tree laid out across pages `page_size` one page at a time, each resuming where the
/// one before broke.
pub struct PagedLayout<'r> {
  tree: LayoutTree<'r>,
  page_size: Size<f32>,
  max_pages: usize,
  break_token: Option<Rc<BlockBreakToken>>,
  page_count: usize,
  page_areas: PageAreas,
  /// The root's last fragment and its origin, which page areas past the root's end attach to.
  last_root_fragment: Option<(usize, Point<LayoutUnit>)>,
  last_root_page: usize,
  is_finished: bool,
  /// How far above their pages the boxes of the tree paint, as `PaintReach::back` bounds it.
  reach_back: Option<f32>,
  /// The boxes no page has laid out yet that can paint on an earlier page.
  pending: PendingBoxes,
}

impl<'r> PagedLayout<'r> {
  /// Starts laying `root` out on at most `max_pages` pages `page_size`.
  pub fn new(root: &'r RenderNode, page_size: Size<f32>, max_pages: usize) -> Self {
    root.clear_layout_caches();

    let reach = PaintReach::of(root);

    Self {
      tree: LayoutTree::from_render_node(root),
      page_size,
      max_pages,
      break_token: None,
      page_count: 0,
      page_areas: PageAreas::default(),
      last_root_fragment: None,
      last_root_page: 0,
      is_finished: false,
      reach_back: reach.back,
      pending: PendingBoxes::new(&reach),
    }
  }

  /// Has the layout log what it finishes with, so that [`Self::release`] can free it.
  pub fn with_release(mut self) -> Self {
    self.tree.start_releases();
    self
  }

  /// Lays out the next page and returns whether there was one. Fails with [`Error::TooManyPages`]
  /// when a page past `max_pages` is needed.
  pub fn next_page(&mut self) -> Result<bool> {
    if self.is_finished {
      return Ok(false);
    }
    if self.page_count == self.max_pages {
      return Err(Error::TooManyPages(self.max_pages));
    }

    let page_block_size = LayoutUnit::from_float(self.page_size.height);
    let page = self.page_count;

    self.tree.begin_page(page + 1);
    let mut space = self
      .tree
      .root_space(self.page_size.map(AvailableSpace::Definite));
    let mut margins = margins_for(
      self.tree.layout_style(NodeId::ROOT),
      space.percentage_resolution_inline_size,
      Direction::Ltr,
    );

    adjust_margins_for_fragmentation(self.break_token.as_deref(), false, &mut margins);
    space.block_fragmentation = Some(BlockFragmentation {
      fragmentainer_block_size: page_block_size,
      fragmentainer_offset: margins.block_start,
      requires_content_before_breaking: false,
      should_propagate_child_break_values: true,
      min_break_appeal: BreakAppeal::LastResort,
      min_block_size_should_encompass_intrinsic_size: false,
    });

    let (root_fragment, candidates, laid_root) = match self.last_root_fragment {
      Some(root_fragment) if self.break_token.is_none() => (root_fragment, Vec::new(), false),
      _ => {
        let input = FragmentationLayoutInput {
          break_token: self.break_token.clone(),
          early_break: None,
        };
        let result = self.tree.layout_fragment(NodeId::ROOT, &space, &input);
        let fragment_index = fragment_index(self.break_token.as_deref());
        let space_left = page_block_size - margins.block_start;

        // Blink's page area breaks before a root that starts past the page end, leaving the page
        // blank, as `MovePastBreakpoint` must break before such a child.
        if self.break_token.is_none()
          && (space_left < LayoutUnit::ZERO
            || (space_left == LayoutUnit::ZERO
              && is_break_inside(result.break_info.break_token.as_deref())))
        {
          self.break_token = Some(Rc::new(BlockBreakToken::break_before(NodeId::ROOT, false)));
          self.page_count += 1;

          return Ok(true);
        }

        self.tree.place_child(
          NodeId::ROOT,
          fragment_index,
          page,
          Point::new(margins.inline_start, margins.block_start),
          &result,
        );

        // The pages stack in the block direction, which the out-of-flow boxes lay out across.
        let root_origin = Point::new(
          margins.inline_start,
          page_block_size * page as i32 + margins.block_start,
        );

        self.break_token = result.break_info.break_token.clone();
        (
          (fragment_index, root_origin),
          result.out_of_flow.clone(),
          true,
        )
      }
    };

    self.last_root_fragment = Some(root_fragment);
    if laid_root {
      self.last_root_page = page;
    }
    self.page_areas.lay_out(
      &mut self.tree,
      page,
      root_fragment,
      candidates,
      self.page_size,
      &space,
    );
    self.page_count += 1;
    self.is_finished = self.break_token.is_none() && !self.page_areas.resumes();
    self.update_pending();

    Ok(true)
  }

  /// Frees what the layout finished with while laying out the first `pages` pages, which no page
  /// still to be laid out, painted, or summarized reads. Does nothing unless the layout is made
  /// [`Self::with_release`].
  pub fn release(&mut self, pages: usize) {
    self.tree.release(pages);
  }

  /// Whether the box `node` had no fragment to come once the first `pages` pages were laid out.
  pub fn has_ended_by(&self, node: NodeId, pages: usize) -> bool {
    self.tree.ended_by(node, pages)
  }

  /// The layout so far.
  pub fn view(&self) -> LayoutView<'_> {
    self.tree.view()
  }

  /// How many of the first pages no later page can paint onto, so a consumer may paint them from
  /// [`Self::view`].
  pub fn finalized_pages(&self) -> usize {
    if self.is_finished {
      return self.page_count;
    }
    if self.pending.pins_every_page() {
      return 0;
    }

    let mut pinned = self.page_count;
    let mut chain = vec![NodeId::ROOT];

    // The page areas after the root's last fragment attach to it, so it paints more with each.
    if self.break_token.is_none() && self.page_areas.resumes() {
      pinned = self.last_root_page;
    }

    if let Some(token) = &self.break_token {
      self.pin_pending_boxes(token, &mut chain, &mut pinned);
    }
    pinned
  }

  /// How many of the first pages a consumer may paint from [`Self::view`]: those
  /// [`Self::finalized_pages`] holds final, less the pages the boxes of the tree paint above their own.
  pub fn settled_pages(&self) -> usize {
    if self.is_finished {
      return self.page_count;
    }

    let Some(reach_back) = self.reach_back else {
      return 0;
    };
    // A page not laid out starts at or below the page after the last one laid out.
    let pages_back = (reach_back / self.page_size.height).ceil();
    let finalized = self.finalized_pages();

    if pages_back.is_nan() || pages_back >= finalized as f32 {
      return 0;
    }
    finalized - pages_back as usize
  }

  // A box a containing block holds back, or placed from the block's start in a later fragment,
  // keeps the block's first page open until it is laid out. A box clipping to rounded corners keeps
  // its own first page open until it ends.
  fn pin_pending_boxes(
    &self,
    token: &BlockBreakToken,
    chain: &mut Vec<NodeId>,
    pinned: &mut usize,
  ) {
    // A box that made no fragment yet starts on a page not laid out.
    if token.is_break_before {
      return;
    }
    if self
      .pending
      .is_containing_block(self.tree.render_node(token.node).context.box_id())
      || self.clips_to_rounded_box(token.node)
    {
      *pinned = (*pinned).min(self.start_page(chain));
    }

    for child in &token.child_break_tokens {
      let BreakToken::Block(child) = child else {
        continue;
      };

      if child.held_static_position.is_some() {
        *pinned = (*pinned).min(self.start_page(chain));
      } else {
        chain.push(child.node);
        self.pin_pending_boxes(child, chain, pinned);
        chain.pop();
      }
    }
  }

  /// Whether `node` clips its content to rounded corners, which the height of its stitched box,
  /// unknown until its last fragment, shapes on every page.
  fn clips_to_rounded_box(&self, node: NodeId) -> bool {
    let style = &self.tree.render_node(node).context.style;
    let square = SpacePair::from_single(Length::zero());
    let corners = &style.surround_data;

    style.resolve_overflows().should_clip_content()
      && [
        corners.border_top_left_radius,
        corners.border_top_right_radius,
        corners.border_bottom_right_radius,
        corners.border_bottom_left_radius,
      ]
      .into_iter()
      .any(|radius| radius != square)
  }

  fn update_pending(&mut self) {
    // A page area that goes on lays out its boxes in more fragments than the root's tokens show.
    let open = (!self.page_areas.resumes() && self.pending.has_unbounded())
      .then(|| open_nodes(self.break_token.as_deref()));

    self.pending.update(&self.tree, open.as_ref());
  }

  fn start_page(&self, chain: &[NodeId]) -> usize {
    let view = self.tree.view();
    let mut fragment_index = 0;

    for &node in chain.iter().rev() {
      match view
        .fragment_containers(node)
        .find(|&(index, _)| index == fragment_index)
      {
        Some((_, container)) => fragment_index = container,
        None => return 0,
      }
    }
    fragment_index
  }

  #[cfg(test)]
  pub(crate) fn tree(&self) -> &LayoutTree<'r> {
    &self.tree
  }

  /// How many pages are laid out.
  pub fn page_count(&self) -> usize {
    self.page_count
  }

  /// Whether the last page is laid out.
  pub fn is_finished(&self) -> bool {
    self.is_finished
  }

  /// The layout as it is.
  pub fn into_results(self) -> LayoutResults {
    self.tree.into_results()
  }
}

/// The boxes `root_token`'s box and the boxes under it have fragments still to come for.
fn open_nodes(root_token: Option<&BlockBreakToken>) -> FxHashSet<NodeId> {
  let mut open = FxHashSet::default();
  let mut stack: Vec<&BlockBreakToken> = root_token.into_iter().collect();

  while let Some(token) = stack.pop() {
    open.insert(token.node);
    stack.extend(
      token
        .child_break_tokens
        .iter()
        .filter_map(|child| match child {
          BreakToken::Block(child) => Some(&**child),
          BreakToken::Inline(_) => None,
        }),
    );
  }
  open
}
