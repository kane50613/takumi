//! Blink's page area of a paginated layout: the out-of-flow boxes the root left, those the page
//! before left, and the fixed boxes that repeat on every page.

use std::{mem::take, rc::Rc};

use crate::{
  geometry::{NodeId, Point, Size},
  layout::{
    block_break_token::BlockBreakToken,
    box_fragment_builder::BoxFragmentBuilder,
    constraint_space::ConstraintSpace,
    layout_result::{LayoutResult, OutOfFlowCandidate},
    logical_rect::LogicalBoxStrut,
    out_of_flow::{ContainingBlock, ContainingBlockClaims, OutOfFlowLayoutPart},
    tree::LayoutTree,
  },
  layout_unit::LayoutUnit,
};

/// Blink's page areas of the pages laid out so far, and what each leaves the next.
#[derive(Default)]
pub(crate) struct PageAreas {
  /// Where the next page's area resumes: the out-of-flow boxes this one pushed to the next page or
  /// broke into it.
  break_token: Option<Rc<BlockBreakToken>>,
  fixed: Vec<FixedBox>,
}

/// A fixed box no box contains, which Blink's `LayoutRepeatableRoot` lays out once for every page
/// from the one it is found on.
struct FixedBox {
  node: NodeId,
  /// The box's border box in its page.
  offset: Point<LayoutUnit>,
  result: Rc<LayoutResult>,
  /// The page whose area found the box, the first it repeats on.
  first_page: usize,
}

impl PageAreas {
  /// Whether the next page has out-of-flow boxes to lay out, as Blink's `HasInsertedChildBreak`
  /// of the page area says.
  pub(crate) fn resumes(&self) -> bool {
    self.break_token.is_some()
  }

  /// Lays out the page area of page `page`, attached to the root's fragment `fragment_index` at
  /// `origin`: the out-of-flow boxes the root left as `root_candidates` and those the page before
  /// left, as `FragmentedOofInCb` does, then repeats the fixed boxes.
  pub(crate) fn lay_out(
    &mut self,
    tree: &mut LayoutTree<'_>,
    page: usize,
    (fragment_index, origin): (usize, Point<LayoutUnit>),
    root_candidates: Vec<OutOfFlowCandidate>,
    page_size: Size<f32>,
    space: &ConstraintSpace,
  ) {
    let page_block_size = LayoutUnit::from_float(page_size.height);
    let page_start = Point::new(LayoutUnit::ZERO, page_block_size * page as i32);
    let size = Size {
      width: LayoutUnit::from_float(page_size.width),
      height: page_block_size,
    };
    // The initial containing block is the first page's area, which a later page's sees above it.
    let initial_containing_block = ContainingBlock {
      origin: Point::new(-origin.x, -origin.y),
      size,
    };
    let page_containing_block = ContainingBlock {
      origin: page_start - origin,
      size,
    };
    let mut space = space.clone();

    if let Some(fragmentation) = &mut space.block_fragmentation {
      fragmentation.fragmentainer_offset = origin.y - page_start.y;
    }

    let mut builder = BoxFragmentBuilder::new(
      &space,
      tree.layout_style(NodeId::ROOT),
      LogicalBoxStrut::default(),
      self.break_token.take(),
    );
    let mut candidates = tree.resumed_out_of_flow(&builder);

    candidates.extend(root_candidates);
    // A fixed box is laid out against the page it is found on instead.
    let claims = ContainingBlockClaims {
      fixed: false,
      ..ContainingBlockClaims::ALL
    };

    while !candidates.is_empty() {
      let part = OutOfFlowLayoutPart::new(
        (NodeId::ROOT, fragment_index),
        claims,
        initial_containing_block,
        &space,
      )
      .with_final_block_size()
      .in_fragments(&mut builder);

      for candidate in tree.lay_out_out_of_flow(part, take(&mut candidates)) {
        let (offset, result) =
          tree.lay_out_out_of_flow_box(&candidate, page_containing_block, &space);

        candidates.extend(result.out_of_flow_moved_by(offset));
        self.fixed.push(FixedBox {
          node: candidate.node,
          offset: offset + origin - page_start,
          result,
          first_page: page,
        });
      }
    }

    self.break_token = builder
      .has_inserted_child_break()
      .then(|| Rc::new(builder.create_break_token(NodeId::ROOT)));

    for fixed in &self.fixed {
      fixed.place_repetition(tree, page, page_start, (fragment_index, origin));
    }
  }
}

impl FixedBox {
  /// Blink's repeated fixed box: places its repetition on page `page`, whose area starts at
  /// `page_start` in the stacked pages, in the root's fragment given.
  fn place_repetition(
    &self,
    tree: &mut LayoutTree<'_>,
    page: usize,
    page_start: Point<LayoutUnit>,
    (root_fragment_index, root_origin): (usize, Point<LayoutUnit>),
  ) {
    let repetition = page - self.first_page;

    tree.place_child(
      self.node,
      repetition,
      root_fragment_index,
      self.offset + page_start - root_origin,
      &self.result,
    );
    if repetition > 0 {
      tree.repeat_fragments(self.node, 0, repetition);
    }
    tree.add_hoisted_child(NodeId::ROOT, self.node);
  }
}
