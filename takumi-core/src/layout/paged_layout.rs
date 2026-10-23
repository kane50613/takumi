//! Blink's [`PaginatedRootLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/paginated_root_layout_algorithm.cc)
//! without page boxes or `@page`, driven a page at a time so a caller can read the pages laid out so far.

use std::rc::Rc;

use crate::{
  Error, Result,
  geometry::{AvailableSpace, NodeId, Point, Size},
  layout::{
    block_break_token::BlockBreakToken,
    break_appeal::BreakAppeal,
    constraint_space::BlockFragmentation,
    fragmentation_layout_input::FragmentationLayoutInput,
    fragmentation_utils::{adjust_margins_for_fragmentation, fragment_index, is_break_inside},
    length_utils::margins_for,
    out_of_flow::PageAreas,
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  layout_unit::LayoutUnit,
  style::Direction,
};

/// One page [`PagedLayout::next_page`] laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaidPage {
  /// The page's index.
  pub index: usize,
  /// The index of the root's fragment on the page, none when the root starts after the page's start
  /// or ended on an earlier page.
  pub root_fragment: Option<usize>,
}

/// A render tree laid out across pages `page_size` one page at a time: each page resumes where the
/// one before broke, and its page area lays out the out-of-flow boxes the root left. The pages go
/// on while the root or the page area has more to lay out.
pub struct PagedLayout<'r> {
  tree: LayoutTree<'r>,
  page_size: Size<f32>,
  max_pages: usize,
  /// Where the root's next fragment resumes.
  break_token: Option<Rc<BlockBreakToken>>,
  page_count: usize,
  page_areas: PageAreas,
  /// The root's last fragment and its origin, which the page area of a page past the root's end
  /// attaches to.
  last_root_fragment: Option<(usize, Point<LayoutUnit>)>,
  is_finished: bool,
}

impl<'r> PagedLayout<'r> {
  /// Starts laying `root` out on pages `page_size`, at most `max_pages` of them.
  pub fn new(root: &'r RenderNode, page_size: Size<f32>, max_pages: usize) -> Self {
    root.clear_layout_caches();

    Self {
      tree: LayoutTree::from_render_node(root),
      page_size,
      max_pages,
      break_token: None,
      page_count: 0,
      page_areas: PageAreas::default(),
      last_root_fragment: None,
      is_finished: false,
    }
  }

  /// Lays out the next page, or returns none when the last one has been laid out. Fails with
  /// [`Error::TooManyPages`] when a page past `max_pages` is needed.
  pub fn next_page(&mut self) -> Result<Option<LaidPage>> {
    if self.is_finished {
      return Ok(None);
    }
    if self.page_count == self.max_pages {
      return Err(Error::TooManyPages(self.max_pages));
    }

    let page_block_size = LayoutUnit::from_float(self.page_size.height);
    let page = self.page_count;
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

          return Ok(Some(LaidPage {
            index: page,
            root_fragment: None,
          }));
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
    self.tree.lay_out_page_area(
      &mut self.page_areas,
      page,
      root_fragment,
      candidates,
      self.page_size,
      &space,
    );
    self.page_count += 1;
    self.is_finished = self.break_token.is_none() && !self.page_areas.resumes();

    Ok(Some(LaidPage {
      index: page,
      root_fragment: laid_root.then_some(root_fragment.0),
    }))
  }

  /// How many pages are laid out.
  pub fn page_count(&self) -> usize {
    self.page_count
  }

  /// Whether the last page is laid out.
  pub fn is_finished(&self) -> bool {
    self.is_finished
  }

  /// The layout as it is, for the pages laid out so far.
  pub fn into_results(self) -> LayoutResults {
    self.tree.into_results()
  }
}
