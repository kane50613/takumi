//! Blink's [`BlockLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/block_layout_algorithm.cc):
//! a block container's children in a block formatting context, with margins collapsing, floats,
//! and clearance, an inline formatting context's lines among them. Without fragmentation, clamping
//! by height, `text-box-trim`, `margin-trim`, ruby annotations, list marker propagation, or quirks
//! mode.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::{mem::take, rc::Rc};

use crate::{
  context::RenderContext,
  geometry::{AvailableSpace, NodeId, Point, Size},
  layout::{
    bfc_rect::BfcOffset,
    constraint_space::{
      AdjoiningObjectTypes, AutoSizeBehavior, BaselineAlgorithmType, CacheSlot, ConstraintSpace,
    },
    content_alignment::BlockContentAlignment,
    exclusions::exclusion_space::ExclusionSpace,
    floats_utils::{Clear, Float, UnpositionedFloat, adjust_to_clearance, position_float},
    inline::{
      atomic_inline::BoxAtomicInline,
      inline_break_token::InlineBreakToken,
      inline_child_layout_context::InlineChildLayoutContext,
      inline_layout_algorithm::{InlineLayoutAlgorithm, LineBoxFragment},
      inline_node::InlineNode,
      inline_node_data::InlineNodeData,
      line_info::text_align,
      score_line_breaker::ScoreLineBreakContext,
      text_fit_utils::ParagraphScale,
      unpositioned_list_marker::UnpositionedListMarker,
    },
    layout_result::{
      InlineEdge, LayoutResult, LayoutStatus, LogicalStaticPosition, OutOfFlowCandidate,
    },
    layout_style::LayoutStyle,
    length_utils::{
      LengthResolver, child_available_size, child_percentage_size, margins_for,
      resolve_inline_auto_margins,
    },
    line_clamp_data::LineClampData,
    logical_rect::{AvailableLogicalSize, LogicalBoxStrut, LogicalOffset, LogicalSize},
    margin_strut::MarginStrut,
    min_max_sizes::{MinMaxSizes, MinMaxSizesResult},
    out_of_flow::ContainingBlock,
    relative_utils::compute_relative_offset,
    tree::LayoutTree,
  },
  layout_unit::LayoutUnit,
  style::{AlignItems, Direction, TextFitMode, TextFitTarget, TextWrapStyle},
};

/// Blink's `PreviousInflowPosition`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PreviousInflowPosition {
  pub(crate) logical_block_offset: LayoutUnit,
  pub(crate) margin_strut: MarginStrut,
  pub(crate) self_collapsing_child_had_clearance: bool,
}

/// Blink's `BlockLineClampData`, clamping by lines alone. Approximate: a block child's lines do not
/// count toward its container's clamp, so only the clamp container's own lines clamp.
#[derive(Debug, Clone, Copy, Default)]
struct BlockLineClampData {
  data: LineClampData,
  ignore_line_clamp: bool,
  previous_inflow_position_when_clamped: Option<PreviousInflowPosition>,
}

impl BlockLineClampData {
  /// Blink's `Setup`: the clamp a `line-clamp` sets, else the one the space passes down.
  fn setup(&mut self, line_clamp: Option<u32>, space: &ConstraintSpace) {
    self.data = match line_clamp {
      Some(_) if self.ignore_line_clamp => LineClampData::default(),
      Some(lines) => LineClampData {
        lines_until_clamp: Some(lines as i32),
      },
      None => space.line_clamp_data,
    };
  }

  /// Blink's `ShouldRelayoutWithNoForcedTruncate`: no line followed the clamp line.
  fn should_relayout_with_no_forced_truncate(&self) -> bool {
    self.previous_inflow_position_when_clamped.is_some() && self.data.lines_until_clamp == Some(0)
  }

  /// Blink's `UpdateAfterLayout`, the child laid out as `result` ending at `previous`.
  fn update_after_layout(
    &mut self,
    result: &LayoutResult,
    previous: &PreviousInflowPosition,
    is_line_box: bool,
  ) {
    let Some(old_lines_until_clamp) = self.data.lines_until_clamp else {
      return;
    };

    if is_line_box {
      self.data.lines_until_clamp = result.lines_until_clamp;
    } else if old_lines_until_clamp == 0 {
      // A lineless box right after the clamp point is content after it.
      self.data.lines_until_clamp = Some(-1);
    }

    if self.data.is_past_clamp_point() && self.previous_inflow_position_when_clamped.is_none() {
      self.previous_inflow_position_when_clamped = Some(*previous);
    }
  }
}

/// The `InlineNode` child of an inline formatting context root, laid out a line at a time.
struct InlineChild<'a, 'r> {
  node: InlineNode<'r>,
  items_data: &'a InlineNodeData<'r>,
  context: InlineChildLayoutContext<'a, 'r>,
  /// Blink's `previous_inline_break_token`: where the next line starts.
  break_token: Option<InlineBreakToken<'r>>,
  /// The outside list marker the box has yet to place beside a line.
  unpositioned_list_marker: Option<UnpositionedListMarker<'r>>,
  /// The lines laid out, in the content box.
  lines: Vec<LineBoxFragment<'a, 'r>>,
}

impl<'a, 'r> InlineChild<'a, 'r> {
  fn new(node: InlineNode<'r>, items_data: &'a InlineNodeData<'r>) -> Self {
    let mut context = InlineChildLayoutContext::default();

    // Blink's `LayoutWithOptimalInlineChildLayoutContext`.
    if node.block.context.style.inherited_data.text_wrap_style != TextWrapStyle::Auto
      && !items_data.is_score_line_break_disabled
    {
      context.score_line_break_context = Some(ScoreLineBreakContext::default());
    }

    Self {
      node,
      items_data,
      context,
      break_token: None,
      unpositioned_list_marker: UnpositionedListMarker::of(items_data),
      lines: Vec::new(),
    }
  }
}

/// Blink's `LayoutInputNode` as `HandleInflow` lays it out: a block, or the next line of the inline
/// content.
enum InflowChild<'i, 'a, 'r> {
  Block(NodeId),
  Inline(&'i mut InlineChild<'a, 'r>),
}

impl InflowChild<'_, '_, '_> {
  fn block(&self) -> Option<NodeId> {
    match self {
      Self::Block(node) => Some(*node),
      Self::Inline(_) => None,
    }
  }
}

/// Blink's `InflowChildData`.
#[derive(Debug, Clone, Copy)]
struct InflowChildData {
  bfc_offset_estimate: BfcOffset,
  margin_strut: MarginStrut,
  margins: LogicalBoxStrut,
  is_pushed_by_floats: bool,
}

/// Blink's `ToAdjoiningObjectTypes`.
fn adjoining_object_types(clear: Clear) -> AdjoiningObjectTypes {
  match clear {
    Clear::None => AdjoiningObjectTypes::empty(),
    Clear::Left => AdjoiningObjectTypes::FLOAT_LEFT,
    Clear::Right => AdjoiningObjectTypes::FLOAT_RIGHT,
    Clear::Both => AdjoiningObjectTypes::FLOAT_BOTH,
  }
}

/// Blink's `HasClearancePastAdjoiningFloats`.
fn has_clearance_past_adjoining_floats(
  types: AdjoiningObjectTypes,
  child_style: &LayoutStyle,
) -> bool {
  adjoining_object_types(child_style.clear).intersects(types)
}

/// Blink's `LogicalFromBfcLineOffset`.
fn logical_from_bfc_line_offset(
  child_bfc_line_offset: LayoutUnit,
  parent_bfc_line_offset: LayoutUnit,
  child_inline_size: LayoutUnit,
  parent_inline_size: LayoutUnit,
  direction: Direction,
) -> LayoutUnit {
  let relative_line_offset = child_bfc_line_offset - parent_bfc_line_offset;

  match direction {
    Direction::Ltr => relative_line_offset,
    Direction::Rtl => parent_inline_size - relative_line_offset - child_inline_size,
  }
}

/// Blink's `LogicalFromBfcOffsets`.
fn logical_from_bfc_offsets(
  child_bfc_offset: BfcOffset,
  parent_bfc_offset: BfcOffset,
  child_inline_size: LayoutUnit,
  parent_inline_size: LayoutUnit,
  direction: Direction,
) -> LogicalOffset {
  LogicalOffset::new(
    logical_from_bfc_line_offset(
      child_bfc_offset.line_offset,
      parent_bfc_offset.line_offset,
      child_inline_size,
      parent_inline_size,
      direction,
    ),
    child_bfc_offset.block_offset - parent_bfc_offset.block_offset,
  )
}

/// Blink's `ApplyClearance`.
fn apply_clearance(space: &ConstraintSpace, bfc_block_offset: &mut LayoutUnit) -> bool {
  if space.has_clearance_offset() && *bfc_block_offset < space.clearance_offset {
    *bfc_block_offset = space.clearance_offset;
    return true;
  }

  false
}

/// A child placed in the fragment: its border box's location and its layout.
struct PlacedChild {
  node: NodeId,
  location: Point<LayoutUnit>,
  result: Rc<LayoutResult>,
}

/// Blink's `BlockLayoutAlgorithm` with its `BoxFragmentBuilder`.
pub(crate) struct BlockLayoutAlgorithm<'t, 'r> {
  tree: &'t mut LayoutTree<'r>,
  node: NodeId,
  space: &'t ConstraintSpace,
  style: Rc<LayoutStyle>,
  border_padding: LogicalBoxStrut,
  inline_size: LayoutUnit,
  child_available_size: AvailableLogicalSize,
  child_percentage_size: AvailableLogicalSize,
  bfc_line_offset: LayoutUnit,
  bfc_block_offset: Option<LayoutUnit>,
  exclusion_space: ExclusionSpace,
  adjoining_object_types: AdjoiningObjectTypes,
  is_pushed_by_floats: bool,
  abort_when_bfc_block_offset_updated: bool,
  line_clamp_data: BlockLineClampData,
  intrinsic_block_size: LayoutUnit,
  first_baseline: Option<LayoutUnit>,
  last_baseline: Option<LayoutUnit>,
  children: Vec<PlacedChild>,
  out_of_flow: Vec<OutOfFlowCandidate>,
}

impl<'t, 'r> BlockLayoutAlgorithm<'t, 'r> {
  pub(crate) fn new(
    tree: &'t mut LayoutTree<'r>,
    node: NodeId,
    space: &'t ConstraintSpace,
  ) -> Self {
    let style = Rc::clone(tree.layout_style_rc(node));
    let resolver = LengthResolver::new(space, &style);
    let border_padding = resolver.border_padding;
    let inline_size = resolver
      .inline_size_for_fragment(&mut |size_type| tree.min_max_sizes(node, space, size_type));
    let initial_block_size = resolver.initial_block_size_for_fragment(None, Some(inline_size));
    let child_available_size = child_available_size(
      AvailableLogicalSize {
        inline_size: Some(inline_size),
        block_size: initial_block_size,
      },
      border_padding,
    );
    let child_percentage_size =
      child_percentage_size(space, tree.is_anonymous(node), child_available_size);

    Self {
      exclusion_space: (*space.exclusion_space).clone(),
      tree,
      node,
      space,
      style,
      border_padding,
      inline_size,
      child_available_size,
      child_percentage_size,
      bfc_line_offset: space.bfc_offset.line_offset,
      bfc_block_offset: None,
      adjoining_object_types: AdjoiningObjectTypes::empty(),
      is_pushed_by_floats: false,
      abort_when_bfc_block_offset_updated: false,
      line_clamp_data: BlockLineClampData::default(),
      intrinsic_block_size: LayoutUnit::ZERO,
      first_baseline: None,
      last_baseline: None,
      children: Vec::new(),
      out_of_flow: Vec::new(),
    }
  }

  fn direction(&self) -> Direction {
    self.space.direction
  }

  fn child_available_inline_size(&self) -> LayoutUnit {
    self
      .child_available_size
      .inline_size
      .unwrap_or(LayoutUnit::ZERO)
  }

  /// Blink's `BfcBlockOffset`: the resolved one, else the parent's estimate.
  fn bfc_block_offset_or_estimate(&self) -> LayoutUnit {
    self
      .bfc_block_offset
      .unwrap_or(self.space.bfc_offset.block_offset)
  }

  /// Blink's `ContainerBfcOffset`.
  fn container_bfc_offset(&self) -> BfcOffset {
    BfcOffset::new(self.bfc_line_offset, self.bfc_block_offset_or_estimate())
  }

  /// Blink's `NextBorderEdge`.
  fn next_border_edge(&self, previous: &PreviousInflowPosition) -> LayoutUnit {
    self.bfc_block_offset_or_estimate()
      + previous.logical_block_offset
      + previous.margin_strut.sum()
  }

  /// Blink's `NeedsAbortOnBfcBlockOffsetChange`.
  fn needs_abort_on_bfc_block_offset_change(&self) -> bool {
    self.abort_when_bfc_block_offset_updated
      && self.bfc_block_offset != Some(self.space.expected_bfc_block_offset())
  }

  /// Blink's `ResolveBfcBlockOffset`: false when the layout has to start over at the new offset.
  fn resolve_bfc_block_offset(
    &mut self,
    previous: &mut PreviousInflowPosition,
    bfc_block_offset: LayoutUnit,
    forced_bfc_block_offset: Option<LayoutUnit>,
  ) -> bool {
    if self.space.is_pushed_by_floats {
      self.is_pushed_by_floats = true;
    }
    if self.bfc_block_offset.is_some() {
      return true;
    }

    let mut bfc_block_offset = forced_bfc_block_offset.unwrap_or(bfc_block_offset);

    if apply_clearance(self.space, &mut bfc_block_offset) {
      self.is_pushed_by_floats = true;
    }
    self.bfc_block_offset = Some(bfc_block_offset);

    if self.needs_abort_on_bfc_block_offset_change() {
      return false;
    }

    previous.logical_block_offset = LayoutUnit::ZERO;
    previous.margin_strut = MarginStrut::default();
    true
  }

  /// [`Self::resolve_bfc_block_offset`] at `bfc_block_offset`, forced as the space says.
  fn resolve_bfc_block_offset_at(
    &mut self,
    previous: &mut PreviousInflowPosition,
    bfc_block_offset: LayoutUnit,
  ) -> bool {
    self.resolve_bfc_block_offset(
      previous,
      bfc_block_offset,
      self.space.forced_bfc_block_offset,
    )
  }

  /// [`Self::resolve_bfc_block_offset`] committing the pending margins.
  fn resolve_bfc_block_offset_here(&mut self, previous: &mut PreviousInflowPosition) -> bool {
    let next_border_edge = self.next_border_edge(previous);

    self.resolve_bfc_block_offset_at(previous, next_border_edge)
  }

  /// The layout as far as it got, to lay out again once the container moved it.
  fn abort(&self, status: LayoutStatus) -> LayoutResult {
    LayoutResult::aborted(status, self.bfc_block_offset, self.is_pushed_by_floats)
  }

  /// Blink's `Layout`, and its `HandleNonsuccessfulLayoutResult` for a clamp that hid no line.
  pub(crate) fn layout(mut self) -> LayoutResult {
    let items_data = self
      .tree
      .has_inline_children(self.node)
      .then(|| self.tree.inline_node_data(usize::from(self.node)));
    let status = self.start_layout().and_then(|mut previous| {
      let lines = match &items_data {
        Some(items_data) => Some(self.layout_lines(items_data, &mut previous)?),
        None => {
          self.layout_block_children(&mut previous)?;
          None
        }
      };

      Ok((previous, lines))
    });
    let (mut previous, lines) = match status {
      Ok(laid_out) => laid_out,
      Err(status) => return self.abort(status),
    };

    if self.should_relayout_ignoring_line_clamp() {
      return self.relayout_ignoring_line_clamp();
    }

    self.intrinsic_block_size = self.border_padding.block_start;
    self.finish_layout(&mut previous, items_data.as_deref().zip(lines))
  }

  /// Blink's `FinishLayout` asking for `RelayoutIgnoringLineClamp`: the clamp hid no line.
  pub(in crate::layout) fn should_relayout_ignoring_line_clamp(&self) -> bool {
    self.space.is_new_formatting_context
      && self
        .line_clamp_data
        .should_relayout_with_no_forced_truncate()
  }

  /// Blink's `RelayoutIgnoringLineClamp`.
  fn relayout_ignoring_line_clamp(self) -> LayoutResult {
    Self::new(self.tree, self.node, self.space)
      .ignoring_line_clamp()
      .layout()
  }

  /// The algorithm set up as `RelayoutIgnoringLineClamp` sets it up.
  pub(in crate::layout) fn ignoring_line_clamp(mut self) -> Self {
    self.line_clamp_data.ignore_line_clamp = true;
    self
  }

  /// Blink's `Layout` up to the children: the margins coming in, the offset resolved where none
  /// collapses through the block start, and the clamp set up.
  pub(in crate::layout) fn start_layout(&mut self) -> Result<PreviousInflowPosition, LayoutStatus> {
    let space = self.space;

    if !space.adjoining_object_types.is_empty() {
      self.abort_when_bfc_block_offset_updated = true;
      self.adjoining_object_types = space.adjoining_object_types;
    }

    let line_clamp = self.tree.render_node(self.node).context.style.line_clamp();

    self.line_clamp_data.setup(line_clamp, space);

    let content_edge = self.border_padding.block_start;
    let mut previous = PreviousInflowPosition {
      logical_block_offset: LayoutUnit::ZERO,
      margin_strut: space.margin_strut,
      self_collapsing_child_had_clearance: false,
    };

    // Margins do not collapse through border or padding, or into a new formatting context.
    if content_edge != LayoutUnit::ZERO || space.is_new_formatting_context {
      if !self.resolve_bfc_block_offset_here(&mut previous) {
        return Err(LayoutStatus::BfcBlockOffsetResolved);
      }
      previous.logical_block_offset = content_edge;
    }

    Ok(previous)
  }

  /// The loop over the children in Blink's `Layout`.
  fn layout_block_children(
    &mut self,
    previous: &mut PreviousInflowPosition,
  ) -> Result<(), LayoutStatus> {
    for child in self.tree.child_nodes(self.node) {
      let child_style = Rc::clone(self.tree.layout_style_rc(child));

      if child_style.is_out_of_flow() {
        self.handle_out_of_flow_positioned(previous, child, &child_style);
      } else if child_style.is_floating() {
        self.handle_float(previous, child);
      } else if self.tree.creates_new_formatting_context(child) {
        self.handle_new_formatting_context(child, &child_style, previous)?;
      } else {
        self.handle_inflow(InflowChild::Block(child), &child_style, previous)?;
      }
    }

    Ok(())
  }

  /// Blink's `FinishLayout`.
  fn finish_layout(
    mut self,
    previous: &mut PreviousInflowPosition,
    lines: Option<(&InlineNodeData<'r>, Vec<LineBoxFragment<'_, 'r>>)>,
  ) -> LayoutResult {
    let space = self.space;
    let clamped = self.line_clamp_data.previous_inflow_position_when_clamped;

    // A clamp sizes the box as if nothing followed the clamp point.
    if let Some(clamped) = clamped {
      *previous = clamped;
    }

    let mut end_margin_strut = previous.margin_strut;
    let block_end = self.border_padding.block_end;

    if block_end != LayoutUnit::ZERO
      || previous.self_collapsing_child_had_clearance
      || space.is_new_formatting_context
    {
      // A new formatting context encloses its floats, unless it clamped.
      if space.is_new_formatting_context && clamped.is_none() {
        self.intrinsic_block_size = self
          .intrinsic_block_size
          .max(self.exclusion_space.clearance_offset(Clear::Both));
      }

      if self.bfc_block_offset.is_none() {
        if !self.resolve_bfc_block_offset_here(previous) {
          return self.abort(LayoutStatus::BfcBlockOffsetResolved);
        }
      } else {
        // The trailing margin strut is part of the block size only past something that separates
        // it from the incoming one.
        self.intrinsic_block_size = self
          .intrinsic_block_size
          .max(previous.logical_block_offset + end_margin_strut.sum());
      }

      self.intrinsic_block_size += block_end;
      end_margin_strut = MarginStrut::default();
    } else {
      self.intrinsic_block_size = self.intrinsic_block_size.max(previous.logical_block_offset);
    }

    let style = Rc::clone(&self.style);
    let resolver = LengthResolver::new(space, &style);
    let block_size = resolver
      .block_size_for_fragment(
        Some(self.intrinsic_block_size),
        Some(self.inline_size),
        None,
      )
      .unwrap_or(self.intrinsic_block_size);

    // Something solid to hang on to: a block size, or no way through.
    if self.bfc_block_offset.is_none()
      && block_size != LayoutUnit::ZERO
      && !self.resolve_bfc_block_offset_here(previous)
    {
      return self.abort(LayoutStatus::BfcBlockOffsetResolved);
    }

    // The last child's margins do not collapse through a block size that differs from the content,
    // or one the style fixes.
    if self.bfc_block_offset.is_some()
      && (block_size != self.intrinsic_block_size
        || resolver
          .initial_block_size_for_fragment(None, Some(self.inline_size))
          .is_some())
    {
      end_margin_strut = MarginStrut::default();
    }

    let is_self_collapsing = self.bfc_block_offset.is_none();
    let mut bfc_block_offset = self.bfc_block_offset;

    if is_self_collapsing {
      if let Some(forced) = space.forced_bfc_block_offset {
        bfc_block_offset = Some(forced);
        if space.is_pushed_by_floats {
          self.is_pushed_by_floats = true;
        }
      }
    } else {
      self.adjoining_object_types = AdjoiningObjectTypes::empty();
    }

    let content_offset = self.align_block_content(block_size);

    // Blink's `InlineNode` child keeps its lines until the fragment knows its size.
    if let Some((items_data, mut lines)) = lines {
      let available_space = self.inline_available_space();

      for line in &mut lines {
        line.move_by(BfcOffset::new(LayoutUnit::ZERO, content_offset));
      }
      self.tree.keep_inline_content(
        self.node,
        space,
        self.inline_size,
        (items_data, &lines),
        available_space,
      );
    }

    let size = LogicalSize::new(self.inline_size, block_size);
    let mut result = LayoutResult::of_box(size, &self.style, space);

    if self.tree.has_inline_children(self.node) {
      let candidates = self.tree.finish_inline_content(self.node, &result, space);

      self.out_of_flow.extend(candidates);
    }
    self.lay_out_out_of_flow_children(size);
    self.commit_children();

    if !space.is_new_formatting_context {
      result.exclusion_space = Rc::new(self.exclusion_space);
    }

    LayoutResult {
      intrinsic_block_size: self.intrinsic_block_size,
      bfc_line_offset: self.bfc_line_offset,
      bfc_block_offset,
      end_margin_strut,
      adjoining_object_types: self.adjoining_object_types,
      is_self_collapsing,
      is_pushed_by_floats: self.is_pushed_by_floats,
      first_baseline: self.first_baseline,
      last_baseline: self.last_baseline,
      use_last_baseline_for_inline_baseline: space.baseline_algorithm
        == BaselineAlgorithmType::InlineBlock,
      out_of_flow: self.out_of_flow,
      ..result
    }
  }

  /// Blink's `AlignBlockContent`: how far `align-content` moves the content of a box `block_size`
  /// tall down, its children, out-of-flow boxes, and baselines moved with it.
  fn align_block_content(&mut self, block_size: LayoutUnit) -> LayoutUnit {
    // Blink's `FinalizeTableCellLayout` aligns a cell by `vertical-align` instead.
    if self.space.is_table_cell {
      return LayoutUnit::ZERO;
    }

    let offset = BlockContentAlignment::for_block(self.style.align_content)
      .offset(block_size - self.intrinsic_block_size);

    if offset == LayoutUnit::ZERO {
      return offset;
    }

    let shift = Point::new(LayoutUnit::ZERO, offset);

    for child in &mut self.children {
      child.location = child.location + shift;
    }
    for candidate in &mut self.out_of_flow {
      *candidate = candidate.moved_by(shift);
    }
    for baseline in [&mut self.first_baseline, &mut self.last_baseline]
      .into_iter()
      .flatten()
    {
      *baseline += offset;
    }

    offset
  }

  /// Lays out the out-of-flow boxes this box contains once its size is known, as Blink's
  /// `HandleOofsAndSpecialDescendants`, keeping the rest for an ancestor.
  fn lay_out_out_of_flow_children(&mut self, size: LogicalSize) {
    if self.out_of_flow.is_empty() {
      return;
    }

    let claims = self.tree.claims(self.node);
    let candidates = take(&mut self.out_of_flow);
    let containing_block = ContainingBlock::padding_box(
      Size {
        width: size.inline_size,
        height: size.block_size,
      },
      self.style.border,
    );

    self.out_of_flow =
      self
        .tree
        .lay_out_out_of_flow(self.node, claims, candidates, containing_block, self.space);
  }

  /// Writes each placed child's geometry once the layout stands, which only a layout that places
  /// its boxes does.
  fn commit_children(&mut self) {
    if self.space.cache_slot != CacheSlot::Layout {
      return;
    }

    for child in take(&mut self.children) {
      self
        .tree
        .place_child(child.node, child.location, &child.result);
    }
  }

  /// The border-box location of a child `child_inline_size` wide at the logical `offset`.
  fn physical_offset(
    &self,
    offset: LogicalOffset,
    child_inline_size: LayoutUnit,
  ) -> Point<LayoutUnit> {
    let x = match self.direction() {
      Direction::Ltr => offset.inline_offset,
      Direction::Rtl => self.inline_size - offset.inline_offset - child_inline_size,
    };

    Point::new(x, offset.block_offset)
  }

  /// Blink's `BoxFragmentBuilder::AddResult`: the child moved by `position: relative`, its
  /// out-of-flow boxes along with it.
  fn add_result(&mut self, node: NodeId, result: Rc<LayoutResult>, offset: LogicalOffset) {
    let relative_offset = compute_relative_offset(
      self.tree.render_node(node),
      self.direction(),
      self
        .child_available_size
        .inline_size
        .map(LayoutUnit::to_float),
      self
        .child_available_size
        .block_size
        .map(LayoutUnit::to_float),
    );
    let location = self.physical_offset(offset, result.size.inline_size)
      + relative_offset.map(LayoutUnit::from_float);

    self.out_of_flow.extend(
      result
        .out_of_flow
        .iter()
        .map(|candidate| candidate.moved_by(location)),
    );
    self.children.push(PlacedChild {
      node,
      location,
      result,
    });
  }

  /// Blink's `HandleOutOfFlowPositioned`.
  fn handle_out_of_flow_positioned(
    &mut self,
    previous: &PreviousInflowPosition,
    child: NodeId,
    child_style: &LayoutStyle,
  ) {
    let mut static_position = LogicalStaticPosition {
      offset: LogicalOffset::new(
        self.border_padding.inline_start,
        previous.logical_block_offset,
      ),
      ..LogicalStaticPosition::default()
    };

    // The margin strut joins the static position only once the box is known not to collapse.
    if self.bfc_block_offset.is_some() {
      static_position.offset.block_offset += previous.margin_strut.sum();
    }

    if self
      .tree
      .render_node(child)
      .is_original_display_inline_type()
    {
      // An inline-level box's static position follows the floats before it.
      if self.bfc_block_offset.is_none() {
        self.adjoining_object_types |= AdjoiningObjectTypes::INLINE_OUT_OF_FLOW;
        self.abort_when_bfc_block_offset_updated = true;
      }

      let origin_bfc_block_offset = self
        .bfc_block_offset
        .unwrap_or(self.space.expected_bfc_block_offset())
        + static_position.offset.block_offset;
      let origin = BfcOffset::new(
        self.space.bfc_offset.line_offset + self.border_padding.line_left(self.direction()),
        origin_bfc_block_offset,
      );

      static_position.offset.inline_offset += calculate_out_of_flow_static_inline_level_offset(
        &self.tree.render_node(self.node).context,
        origin,
        &self.exclusion_space,
        self.child_available_inline_size(),
      );
    } else {
      // The alignment container for a block-level out-of-flow box is a zero-thickness line in the
      // inline direction. https://drafts.csswg.org/css-position-3/#staticpos-rect
      let available = self.child_available_inline_size();

      match child_style.justify_self {
        AlignItems::Center | AlignItems::SafeCenter => {
          static_position.inline_edge = InlineEdge::Center;
          static_position.offset.inline_offset += available / 2;
        }
        AlignItems::End | AlignItems::SafeEnd | AlignItems::FlexEnd | AlignItems::SafeFlexEnd => {
          static_position.inline_edge = InlineEdge::End;
          static_position.offset.inline_offset += available;
        }
        _ => {}
      }
    }

    self.out_of_flow.push(OutOfFlowCandidate {
      node: child,
      static_position: static_position.to_physical(self.direction(), self.inline_size),
    });
  }

  /// Blink's `HandleFloat`.
  fn handle_float(&mut self, previous: &PreviousInflowPosition, child: NodeId) {
    let space = self.space;
    let origin_bfc_offset = BfcOffset::new(
      space.bfc_offset.line_offset + self.border_padding.line_left(space.direction),
      if self.bfc_block_offset.is_some() {
        self.next_border_edge(previous)
      } else {
        space.expected_bfc_block_offset()
      },
    );
    let render_node = self.tree.render_node(child);
    let mut unpositioned_float = UnpositionedFloat::new(
      render_node,
      (self.child_available_size, self.child_percentage_size),
      origin_bfc_offset,
      space,
      self.line_clamp_data.data.float_state(),
    );

    if self.bfc_block_offset.is_none() {
      self.adjoining_object_types |= if unpositioned_float.float_type() == Float::Left {
        AdjoiningObjectTypes::FLOAT_LEFT
      } else {
        AdjoiningObjectTypes::FLOAT_RIGHT
      };
      // Floats placed at the expected offset move once the offset resolves elsewhere.
      if space.forced_bfc_block_offset.is_none() {
        self.abort_when_bfc_block_offset_updated = true;
      }
    }

    let tree = &mut *self.tree;
    let positioned = position_float(
      &mut unpositioned_float,
      &mut self.exclusion_space,
      &mut |_, float_space| tree.layout(child, float_space),
    );
    let result = positioned.layout_result;
    let bfc_offset = BfcOffset::new(
      space.bfc_offset.line_offset,
      self
        .bfc_block_offset
        .unwrap_or(space.expected_bfc_block_offset()),
    );
    let offset = logical_from_bfc_offsets(
      positioned.bfc_offset,
      bfc_offset,
      result.size.inline_size,
      self.inline_size,
      space.direction,
    );

    self.add_result(child, result, offset);
  }

  /// Blink's `CalculateMargins`: the child's margins, and how far `justify-self` or `auto` margins
  /// move it along the line.
  fn calculate_margins(
    &mut self,
    child: NodeId,
    child_style: &LayoutStyle,
    is_new_fc: bool,
  ) -> (LogicalBoxStrut, LayoutUnit) {
    let mut margins = margins_for(
      child_style,
      self.child_percentage_size.inline_size,
      self.direction(),
    );

    if is_new_fc {
      return (margins, LayoutUnit::ZERO);
    }

    let child_space = self.inline_size_space_for_child(child, child_style);
    let child_inline_size = {
      let resolver = LengthResolver::new(&child_space, child_style);
      let tree = &mut *self.tree;

      resolver.inline_size_for_fragment(&mut |size_type| {
        tree.min_max_sizes(child, &child_space, size_type)
      })
    };
    let available_space = self.child_available_inline_size();
    let (start, end) = self
      .direction()
      .inline_sides(child_style.margin.left, child_style.margin.right);
    let mut text_align_offset = LayoutUnit::ZERO;

    if start.is_auto() || end.is_auto() {
      resolve_inline_auto_margins(
        child_style,
        self.direction(),
        available_space,
        child_inline_size,
        &mut margins,
      );
    } else {
      text_align_offset =
        justify_self_offset(child_style, available_space, &margins, child_inline_size);
    }

    let additional_line_offset = match self.direction() {
      Direction::Rtl => {
        available_space - text_align_offset - child_inline_size - margins.inline_sum()
      }
      Direction::Ltr => text_align_offset,
    };

    (margins, additional_line_offset)
  }

  /// The space a child resolves its inline size in, as `CalculateMargins` builds it.
  fn inline_size_space_for_child(
    &self,
    child: NodeId,
    child_style: &LayoutStyle,
  ) -> ConstraintSpace {
    let mut space =
      ConstraintSpace::new(self.child_available_size.inline_size, child_style.direction);

    space.available_block_size = self.child_available_size.block_size;
    space.percentage_resolution_inline_size = self.child_percentage_size.inline_size;
    space.percentage_resolution_block_size = self.child_percentage_size.block_size;
    space.inline_auto_behavior = self.inline_auto_behavior_for_child(child, child_style);
    space.cache_slot = self.space.cache_slot;
    space
  }

  /// How a child's `auto` inline size resolves, from its `justify-self`.
  fn inline_auto_behavior_for_child(
    &self,
    child: NodeId,
    child_style: &LayoutStyle,
  ) -> AutoSizeBehavior {
    if self.tree.is_anonymous(child) && !self.tree.is_table(child) {
      return AutoSizeBehavior::StretchImplicit;
    }

    match child_style.justify_self {
      AlignItems::Stretch => AutoSizeBehavior::StretchExplicit,
      AlignItems::Normal => {
        if self.tree.is_table(child) || child_style.is_replaced {
          AutoSizeBehavior::FitContent
        } else {
          AutoSizeBehavior::StretchImplicit
        }
      }
      _ => AutoSizeBehavior::FitContent,
    }
  }

  /// Blink's `ComputeChildData`.
  fn compute_child_data(
    &mut self,
    previous: &PreviousInflowPosition,
    child: Option<NodeId>,
    child_style: &LayoutStyle,
    is_new_fc: bool,
  ) -> InflowChildData {
    // Blink's `CalculateMargins` gives an inline child none.
    let (margins, additional_line_offset) = child
      .map_or((LogicalBoxStrut::default(), LayoutUnit::ZERO), |child| {
        self.calculate_margins(child, child_style, is_new_fc)
      });
    let mut margin_strut = previous.margin_strut;

    margin_strut.append(margins.block_start);

    let direction = self.direction();
    let bfc_offset = BfcOffset::new(
      self.space.bfc_offset.line_offset
        + self.border_padding.line_left(direction)
        + additional_line_offset
        + margins.line_left(direction),
      self.bfc_block_offset_or_estimate() + previous.logical_block_offset,
    );

    InflowChildData {
      bfc_offset_estimate: bfc_offset,
      margin_strut,
      margins,
      is_pushed_by_floats: false,
    }
  }

  /// Blink's `CreateConstraintSpaceForChild`.
  #[allow(clippy::too_many_arguments)]
  fn create_constraint_space_for_child(
    &self,
    child: Option<NodeId>,
    child_style: &LayoutStyle,
    child_data: &InflowChildData,
    child_available_size: AvailableLogicalSize,
    is_new_fc: bool,
    child_bfc_block_offset: Option<LayoutUnit>,
    has_clearance_past_adjoining_floats: bool,
  ) -> ConstraintSpace {
    let space = self.space;
    let mut builder = ConstraintSpace::new(child_available_size.inline_size, child_style.direction);

    builder.inline_auto_behavior = child.map_or(AutoSizeBehavior::StretchImplicit, |child| {
      self.inline_auto_behavior_for_child(child, child_style)
    });
    builder.available_block_size = child_available_size.block_size;
    builder.percentage_resolution_inline_size = self.child_percentage_size.inline_size;
    builder.percentage_resolution_block_size = self.child_percentage_size.block_size;
    builder.is_new_formatting_context = is_new_fc;
    builder.cache_slot = space.cache_slot;
    // Approximate: only the lines count toward the clamp, a block child getting the one above.
    builder.line_clamp_data = match child {
      Some(_) => space.line_clamp_data,
      None => self.line_clamp_data.data,
    };

    let has_bfc_block_offset = self.bfc_block_offset.is_some();

    if !has_bfc_block_offset && let Some(forced) = space.forced_bfc_block_offset {
      builder.forced_bfc_block_offset = Some(forced);
    }
    if let (Some(offset), false) = (child_bfc_block_offset, is_new_fc) {
      builder.forced_bfc_block_offset = Some(offset);
    }
    if !has_bfc_block_offset && let Some(optimistic) = space.optimistic_bfc_block_offset {
      builder.optimistic_bfc_block_offset = Some(optimistic);
    }
    if (!has_bfc_block_offset && space.ancestor_has_clearance_past_adjoining_floats)
      || has_clearance_past_adjoining_floats
    {
      builder.ancestor_has_clearance_past_adjoining_floats = true;
    }

    let mut clearance_offset = if space.is_new_formatting_context {
      LayoutUnit::MIN
    } else {
      space.clearance_offset
    };

    if child.is_some() {
      clearance_offset =
        clearance_offset.max(self.exclusion_space.clearance_offset(child_style.clear));
    }
    builder.clearance_offset = clearance_offset;
    builder.baseline_algorithm = space.baseline_algorithm;

    if child_data.is_pushed_by_floats {
      builder.is_pushed_by_floats = true;
    }

    if !is_new_fc {
      builder.margin_strut = child_data.margin_strut;
      builder.bfc_offset = child_data.bfc_offset_estimate;
      builder.exclusion_space = Rc::new(self.exclusion_space.clone());
      if !has_bfc_block_offset {
        builder.adjoining_object_types = self.adjoining_object_types;
      }
    }

    if !space.is_new_formatting_context {
      builder.ignore_block_margins_for_stretch = (
        self.border_padding.block_start == LayoutUnit::ZERO,
        self.border_padding.block_end == LayoutUnit::ZERO,
      );
    }

    builder
  }

  /// Blink's `HandleNewFormattingContext`.
  fn handle_new_formatting_context(
    &mut self,
    child: NodeId,
    child_style: &LayoutStyle,
    previous: &mut PreviousInflowPosition,
  ) -> Result<(), LayoutStatus> {
    let space = self.space;
    let child_data = self.compute_child_data(previous, Some(child), child_style, true);
    let child_origin_line_offset =
      space.bfc_offset.line_offset + self.border_padding.line_left(self.direction());
    let adjoining_bfc_offset_estimate =
      child_data.bfc_offset_estimate.block_offset + child_data.margin_strut.sum();
    let non_adjoining_bfc_offset_estimate =
      child_data.bfc_offset_estimate.block_offset + previous.margin_strut.sum();
    let mut child_bfc_offset_estimate = adjoining_bfc_offset_estimate;
    let mut bfc_offset_already_resolved = false;
    let mut child_determined_bfc_offset = false;
    let mut child_margin_got_separated = false;
    let mut has_adjoining_floats = false;

    if self.bfc_block_offset.is_none() {
      has_adjoining_floats = self
        .adjoining_object_types
        .intersects(AdjoiningObjectTypes::FLOAT_BOTH);

      let has_clearance_past_adjoining_floats = space.ancestor_has_clearance_past_adjoining_floats
        || has_clearance_past_adjoining_floats(self.adjoining_object_types, child_style);

      if has_clearance_past_adjoining_floats {
        child_bfc_offset_estimate = self.next_border_edge(previous);
        child_margin_got_separated = true;
      } else if let Some(forced) = space.forced_bfc_block_offset {
        bfc_offset_already_resolved = true;
        child_bfc_offset_estimate = forced;
        child_margin_got_separated = child_bfc_offset_estimate != adjoining_bfc_offset_estimate;
      }

      child_determined_bfc_offset = true;

      if !self.resolve_bfc_block_offset_at(previous, child_bfc_offset_estimate) {
        return Err(LayoutStatus::BfcBlockOffsetResolved);
      }

      // Clearance may have moved it.
      child_bfc_offset_estimate = self.container_bfc_offset().block_offset;
    }

    let abort_if_cleared = child_data.margins.block_start != LayoutUnit::ZERO
      && !child_margin_got_separated
      && child_determined_bfc_offset;
    let mut laid_out = self.layout_new_formatting_context(
      child,
      child_style,
      &child_data,
      BfcOffset::new(child_origin_line_offset, child_bfc_offset_estimate),
      abort_if_cleared,
    );

    if laid_out.is_none() {
      // Pushed down by floats, so the child's margin should not have affected them.
      if child_determined_bfc_offset {
        let old_offset = self.bfc_block_offset;

        self.bfc_block_offset = None;
        self.resolve_bfc_block_offset(previous, non_adjoining_bfc_offset_estimate, None);

        if (bfc_offset_already_resolved || has_adjoining_floats)
          && old_offset != self.bfc_block_offset
        {
          return Err(LayoutStatus::BfcBlockOffsetResolved);
        }
      }

      child_bfc_offset_estimate = non_adjoining_bfc_offset_estimate;
      laid_out = self.layout_new_formatting_context(
        child,
        child_style,
        &child_data,
        BfcOffset::new(child_origin_line_offset, child_bfc_offset_estimate),
        false,
      );
    }

    let Some((result, child_bfc_offset)) = laid_out else {
      return Ok(());
    };
    let offset = logical_from_bfc_offsets(
      child_bfc_offset,
      self.container_bfc_offset(),
      result.size.inline_size,
      self.inline_size,
      self.direction(),
    );

    self.propagate_baseline_from_block_child(child, &result, offset.block_offset);
    self.add_result(child, Rc::clone(&result), offset);
    *previous = self.compute_inflow_position(
      previous,
      &child_data,
      Some(child_bfc_offset.block_offset),
      offset,
      &result,
      false,
    );

    Ok(())
  }

  /// Blink's `LayoutNewFormattingContext`: the child laid out in the first layout opportunity it
  /// fits, `None` when `abort_if_cleared` and floats push it down.
  fn layout_new_formatting_context(
    &mut self,
    child: NodeId,
    child_style: &LayoutStyle,
    child_data: &InflowChildData,
    mut origin_offset: BfcOffset,
    abort_if_cleared: bool,
  ) -> Option<(Rc<LayoutResult>, BfcOffset)> {
    let direction = self.direction();

    adjust_to_clearance(
      self.exclusion_space.clearance_offset(child_style.clear),
      &mut origin_offset,
    );

    let available_inline_size = self.child_available_inline_size();
    let opportunities = self.exclusion_space.all_layout_opportunities(
      origin_offset,
      available_inline_size,
      direction,
    );
    let line_left_margin = child_data.margins.line_left(direction);
    let line_right_margin = child_data.margins.line_right(direction);
    let mut last = None;

    for opportunity in opportunities {
      if abort_if_cleared && origin_offset.block_offset < opportunity.rect.block_start_offset() {
        return None;
      }

      let has_floats_on_line_left =
        opportunity.rect.line_start_offset() != origin_offset.line_offset;
      let has_floats_on_line_right =
        opportunity.rect.line_end_offset() != origin_offset.line_offset + available_inline_size;
      let can_expand_outside_opportunity = !has_floats_on_line_left && !has_floats_on_line_right;
      let mut line_left_offset = opportunity.rect.line_start_offset();
      let mut line_right_offset = opportunity.rect.line_end_offset();

      if can_expand_outside_opportunity {
        line_left_offset += line_left_margin;
        line_right_offset -= line_right_margin;
      } else {
        // Margins apply from the content box, not the opportunity.
        line_left_offset = line_left_offset
          .max(origin_offset.line_offset + line_left_margin.clamp_negative_to_zero());
        line_right_offset = line_right_offset.min(
          origin_offset.line_offset + available_inline_size
            - line_right_margin.clamp_negative_to_zero(),
        );
      }

      let opportunity_size = (line_right_offset - line_left_offset).clamp_negative_to_zero();
      let child_available_inline_size =
        (opportunity_size + child_data.margins.inline_sum()).clamp_negative_to_zero();
      let child_space = self.create_constraint_space_for_child(
        Some(child),
        child_style,
        child_data,
        AvailableLogicalSize {
          inline_size: Some(child_available_inline_size),
          block_size: self.child_available_size.block_size,
        },
        true,
        Some(opportunity.rect.start_offset.block_offset),
        false,
      );
      let result = self.tree.layout(child, &child_space);

      last = Some((
        Rc::clone(&result),
        opportunity,
        line_left_offset,
        line_right_offset,
      ));

      if result.size.block_size > opportunity.rect.block_size() {
        continue;
      }

      let mut auto_margins = child_data.margins;
      let mut text_align_offset = LayoutUnit::ZERO;
      let (start, end) = direction.inline_sides(child_style.margin.left, child_style.margin.right);
      let has_auto_margins = start.is_auto() || end.is_auto();

      if has_auto_margins {
        resolve_inline_auto_margins(
          child_style,
          direction,
          child_available_inline_size,
          result.size.inline_size,
          &mut auto_margins,
        );
      } else {
        text_align_offset = justify_self_offset(
          child_style,
          opportunity.rect.inline_size(),
          &child_data.margins,
          result.size.inline_size,
        );
      }

      let line_offset = match direction {
        Direction::Ltr => {
          line_left_offset
            + (auto_margins.line_left(direction) - line_left_margin)
            + text_align_offset
        }
        Direction::Rtl => {
          line_right_offset
            - text_align_offset
            - (auto_margins.line_right(direction) - line_right_margin)
            - result.size.inline_size
        }
      };
      let child_bfc_offset = BfcOffset::new(line_offset, opportunity.rect.block_start_offset());

      if has_floats_on_line_left
        && child_bfc_offset.line_offset < opportunity.rect.line_start_offset()
      {
        continue;
      }
      if has_floats_on_line_right
        && child_bfc_offset.line_offset + result.size.inline_size
          > opportunity.rect.line_end_offset()
      {
        continue;
      }
      if !can_expand_outside_opportunity && result.size.inline_size > opportunity.rect.inline_size()
      {
        continue;
      }

      return Some((result, child_bfc_offset));
    }

    // The last opportunity is unbounded, so a child that fits nowhere else fits there.
    last.map(|(result, opportunity, line_left_offset, _)| {
      (
        result,
        BfcOffset::new(line_left_offset, opportunity.rect.block_start_offset()),
      )
    })
  }

  /// Blink's `HandleInflow` and `FinishInflow`.
  fn handle_inflow(
    &mut self,
    mut child: InflowChild<'_, '_, 'r>,
    child_style: &LayoutStyle,
    previous: &mut PreviousInflowPosition,
  ) -> Result<(), LayoutStatus> {
    let has_clearance_past_adjoining_floats = self.bfc_block_offset.is_none()
      && child.block().is_some()
      && has_clearance_past_adjoining_floats(self.adjoining_object_types, child_style);
    let mut forced_bfc_block_offset = None;
    let mut is_pushed_by_floats = false;

    // Clearance past adjoining floats separates the margins before from the child, so the offset
    // resolves now and the child goes past the floats.
    if has_clearance_past_adjoining_floats {
      if !self.resolve_bfc_block_offset_here(previous) {
        return Err(LayoutStatus::BfcBlockOffsetResolved);
      }
      forced_bfc_block_offset = Some(self.exclusion_space.clearance_offset(child_style.clear));
      is_pushed_by_floats = true;
    }

    let mut child_data = self.compute_child_data(previous, child.block(), child_style, false);

    child_data.is_pushed_by_floats = is_pushed_by_floats;

    let child_space = self.create_constraint_space_for_child(
      child.block(),
      child_style,
      &child_data,
      self.child_available_size,
      false,
      forced_bfc_block_offset,
      has_clearance_past_adjoining_floats,
    );
    let (mut result, mut line) = self.layout_inflow(&child_space, &mut child);
    let mut child_bfc_block_offset = result.bfc_block_offset;
    let is_self_collapsing = result.is_self_collapsing;
    let normal_child_had_clearance = result.is_pushed_by_floats && !is_self_collapsing;

    // A child that resolved its offset before this one did makes this one resolve, and start over
    // if the floats before it moved.
    if result.status == LayoutStatus::BfcBlockOffsetResolved && self.bfc_block_offset.is_none() {
      self.abort_when_bfc_block_offset_updated = true;

      let mut bfc_block_offset = child_bfc_block_offset.unwrap_or(self.next_border_edge(previous));

      if normal_child_had_clearance {
        if self.space.clearance_offset == child_space.clearance_offset {
          self.is_pushed_by_floats = true;
        } else {
          bfc_block_offset = self.next_border_edge(previous);
        }
      }

      if !self.resolve_bfc_block_offset(previous, bfc_block_offset, None) {
        return Err(LayoutStatus::BfcBlockOffsetResolved);
      }
    }

    let mut self_collapsing_child_had_clearance =
      is_self_collapsing && has_clearance_past_adjoining_floats;

    if child_bfc_block_offset.is_none()
      && child_space.has_clearance_offset()
      && child_style.clear != Clear::None
    {
      // A self-collapsing child collapsed through: detect clearance from its hypothetical edge.
      let child_block_offset_estimate =
        self.bfc_block_offset_or_estimate() + result.end_margin_strut.sum();

      if child_block_offset_estimate < child_space.clearance_offset {
        self_collapsing_child_had_clearance = true;
      }
    }

    let child_had_clearance = self_collapsing_child_had_clearance || normal_child_had_clearance;

    if child_had_clearance && !self.resolve_bfc_block_offset_here(previous) {
      return Err(LayoutStatus::BfcBlockOffsetResolved);
    }

    let mut self_collapsing_child_needs_relayout = false;

    if child_bfc_block_offset.is_none() {
      if self.bfc_block_offset.is_some() && result.status == LayoutStatus::Success {
        let mut offset =
          child_data.bfc_offset_estimate.block_offset + result.end_margin_strut.sum();

        apply_clearance(&child_space, &mut offset);
        child_bfc_block_offset = Some(offset);

        if offset != child_space.expected_bfc_block_offset()
          && !result.adjoining_object_types.is_empty()
        {
          self_collapsing_child_needs_relayout = true;
        }
      }
    } else if !child_had_clearance && !is_self_collapsing {
      let offset = child_bfc_block_offset.unwrap_or_default();

      if !self.resolve_bfc_block_offset_at(previous, offset) {
        return Err(LayoutStatus::BfcBlockOffsetResolved);
      }
    }

    // A self-collapsing child past clearance leaves the incoming margin strut behind.
    if self_collapsing_child_had_clearance {
      let mut margin_strut = MarginStrut::default();

      margin_strut.append(child_data.margins.block_start);
      if child_data.margin_strut != margin_strut {
        child_data.margin_strut = margin_strut;
        self_collapsing_child_needs_relayout = true;
      }
    }

    if (result.status == LayoutStatus::BfcBlockOffsetResolved
      || self_collapsing_child_needs_relayout)
      && child_bfc_block_offset.is_some()
    {
      child_data.is_pushed_by_floats = result.is_pushed_by_floats;

      let new_space = self.create_constraint_space_for_child(
        child.block(),
        child_style,
        &child_data,
        self.child_available_size,
        false,
        child_bfc_block_offset,
        false,
      );

      (result, line) = self.layout_inflow(&new_space, &mut child);

      if result.status == LayoutStatus::BfcBlockOffsetResolved {
        child_bfc_block_offset = result.bfc_block_offset;

        let final_space = self.create_constraint_space_for_child(
          child.block(),
          child_style,
          &child_data,
          self.child_available_size,
          false,
          child_bfc_block_offset,
          false,
        );

        (result, line) = self.layout_inflow(&final_space, &mut child);
      }
    }

    if result.status != LayoutStatus::Success {
      return Err(result.status);
    }

    self.exclusion_space = (*result.exclusion_space).clone();
    self.adjoining_object_types = result.adjoining_object_types;

    if self.bfc_block_offset.is_none() {
      self.abort_when_bfc_block_offset_updated |= !result.adjoining_object_types.is_empty();
      if result.is_pushed_by_floats {
        self.is_pushed_by_floats = true;
      }
    }

    if let Some(line_box_bfc_block_offset) = result.line_box_bfc_block_offset {
      child_bfc_block_offset = Some(line_box_bfc_block_offset);
    }

    let mut offset = self.calculate_logical_offset(
      result.size.inline_size,
      result.bfc_line_offset,
      child_bfc_block_offset,
    );

    match (&mut child, line) {
      (InflowChild::Block(node), _) => {
        self.propagate_baseline_from_block_child(*node, &result, offset.block_offset);
        self.add_result(*node, Rc::clone(&result), offset);
      }
      (InflowChild::Inline(inline_child), Some(mut line)) => {
        // Where the line box sits in the block formatting context, which `offset` places, and
        // its out-of-flow boxes with it.
        let line_box_bfc_block_offset =
          child_bfc_block_offset.unwrap_or_else(|| self.bfc_block_offset_or_estimate());

        line.static_bfc_block_offset = line_box_bfc_block_offset;
        self.position_or_propagate_list_marker(inline_child, &line, &mut offset.block_offset);
        self.propagate_baseline_from_line_box(&line, offset.block_offset);

        // The line moves from the block formatting context into the content box.
        line.move_by(BfcOffset::new(
          -child_space.bfc_offset.line_offset,
          offset.block_offset - line_box_bfc_block_offset - self.border_padding.block_start,
        ));
        inline_child.break_token = line.break_token;
        inline_child.lines.push(line);
      }
      (InflowChild::Inline(_), None) => unreachable!("an inline child lays out a line"),
    }

    *previous = self.compute_inflow_position(
      previous,
      &child_data,
      child_bfc_block_offset,
      offset,
      &result,
      self_collapsing_child_had_clearance,
    );
    self.line_clamp_data.update_after_layout(
      &result,
      previous,
      matches!(child, InflowChild::Inline(_)),
    );

    Ok(())
  }

  /// Blink's `LayoutInflow`: a block child laid out by its algorithm, or the next line of the
  /// inline content by Blink's `InlineNode::Layout`.
  fn layout_inflow<'a>(
    &mut self,
    child_space: &ConstraintSpace,
    child: &mut InflowChild<'_, 'a, 'r>,
  ) -> (Rc<LayoutResult>, Option<LineBoxFragment<'a, 'r>>) {
    match child {
      InflowChild::Block(node) => (self.tree.layout(*node, child_space), None),
      InflowChild::Inline(inline_child) => {
        let mut atomic_inline = self.box_atomic_inline();
        let line = InlineLayoutAlgorithm::new(
          inline_child.node,
          inline_child.items_data,
          child_space,
          inline_child.break_token,
          &mut inline_child.context,
          &mut atomic_inline,
        )
        .layout();

        (Rc::new(line.layout_result(child_space)), Some(line))
      }
    }
  }

  /// The atomic inlines of the inline content laid out in the content box.
  fn box_atomic_inline(&mut self) -> BoxAtomicInline<'_, 'r> {
    let available_space = self.inline_available_space();
    let mut atomic_inline = BoxAtomicInline::new(self.tree, available_space);

    atomic_inline.cache_slot = self.space.cache_slot;
    atomic_inline
  }

  /// The space the inline content's atomic inlines lay out in: the content box's inline size, and
  /// the block size percentages resolve against.
  fn inline_available_space(&self) -> Size<AvailableSpace> {
    Size {
      width: AvailableSpace::Definite(self.child_available_inline_size().to_float()),
      height: self
        .child_percentage_size
        .block_size
        .map_or(AvailableSpace::MaxContent, |size| {
          AvailableSpace::Definite(size.to_float())
        }),
    }
  }

  /// Blink's `CalculateLogicalOffset`.
  fn calculate_logical_offset(
    &self,
    child_inline_size: LayoutUnit,
    child_bfc_line_offset: LayoutUnit,
    child_bfc_block_offset: Option<LayoutUnit>,
  ) -> LogicalOffset {
    if let (Some(child_bfc_block_offset), Some(_)) = (child_bfc_block_offset, self.bfc_block_offset)
    {
      return logical_from_bfc_offsets(
        BfcOffset::new(child_bfc_line_offset, child_bfc_block_offset),
        self.container_bfc_offset(),
        child_inline_size,
        self.inline_size,
        self.direction(),
      );
    }

    // A child, or this box, without a block offset yet sits at the block start.
    LogicalOffset::new(
      logical_from_bfc_line_offset(
        child_bfc_line_offset,
        self.bfc_line_offset,
        child_inline_size,
        self.inline_size,
        self.direction(),
      ),
      LayoutUnit::ZERO,
    )
  }

  /// Blink's `ComputeInflowPosition`.
  fn compute_inflow_position(
    &self,
    previous: &PreviousInflowPosition,
    child_data: &InflowChildData,
    child_bfc_block_offset: Option<LayoutUnit>,
    logical_offset: LogicalOffset,
    result: &LayoutResult,
    self_collapsing_child_had_clearance: bool,
  ) -> PreviousInflowPosition {
    let is_self_collapsing = result.is_self_collapsing;
    let logical_block_offset = if is_self_collapsing {
      let mut logical_block_offset = previous.logical_block_offset;

      if self_collapsing_child_had_clearance {
        // The clearance a self-collapsing child had, explicitly: past the margin before it, by
        // what it takes to place it below the float.
        logical_block_offset += previous.margin_strut.sum();
        logical_block_offset += child_bfc_block_offset.unwrap_or_default()
          - result.end_margin_strut.sum()
          - self.next_border_edge(previous);
      }

      logical_block_offset
    } else {
      logical_offset.block_offset + result.size.block_size + result.clearance_after_line
    };
    let mut margin_strut = result.end_margin_strut;

    margin_strut.append(child_data.margins.block_end);

    PreviousInflowPosition {
      logical_block_offset,
      margin_strut,
      self_collapsing_child_had_clearance: self_collapsing_child_had_clearance
        || (previous.self_collapsing_child_had_clearance && is_self_collapsing),
    }
  }

  /// Blink's `PropagateBaselineFromBlockChild`.
  fn propagate_baseline_from_block_child(
    &mut self,
    child: NodeId,
    result: &LayoutResult,
    block_offset: LayoutUnit,
  ) {
    let algorithm = self.space.baseline_algorithm;

    // An inline-block takes no baselines from its tables.
    if self.tree.is_table(child) && algorithm == BaselineAlgorithmType::InlineBlock {
      return;
    }

    if self.first_baseline.is_none()
      && let Some(first) = result.first_baseline
    {
      self.first_baseline = Some(block_offset + first);
    }

    let use_last_baseline =
      algorithm == BaselineAlgorithmType::Default || result.use_last_baseline_for_inline_baseline;
    let mut last = if use_last_baseline {
      result.last_baseline
    } else {
      result.first_baseline
    };

    // Some scroll containers force an inline-block's baseline to their block-end margin edge.
    if algorithm == BaselineAlgorithmType::InlineBlock
      && result.forces_inline_baseline_synthesis(self.tree.layout_style(child))
    {
      let margins = margins_for(
        self.tree.layout_style(child),
        self.child_percentage_size.inline_size,
        self.direction(),
      );

      last = Some(result.size.block_size + margins.block_end);
    }

    if let Some(last) = last {
      self.last_baseline = Some(block_offset + last);
    }
  }

  /// Blink's `LayoutInlineChild`: the lines it lays the inline content of `items_data` out in, at
  /// the scale a consistent `text-fit` measures.
  pub(in crate::layout) fn layout_lines<'a>(
    &mut self,
    items_data: &'a InlineNodeData<'r>,
    previous: &mut PreviousInflowPosition,
  ) -> Result<Vec<LineBoxFragment<'a, 'r>>, LayoutStatus> {
    let node = InlineNode::new(self.tree.render_node(self.node));
    let paragraph_scale = self.measure_paragraph_scale(node, items_data)?;
    let mut inline_child = InlineChild::new(node, items_data);

    inline_child
      .context
      .enable_measuring_mode_if_necessary(Some(paragraph_scale));
    self.layout_inline_children(&mut inline_child, previous)?;

    Ok(inline_child.lines)
  }

  /// The scale every line of a consistent `text-fit` takes, as Blink's `LayoutInlineChild`
  /// measures it with a layout of its own.
  fn measure_paragraph_scale(
    &mut self,
    node: InlineNode<'r>,
    items_data: &InlineNodeData<'r>,
  ) -> Result<ParagraphScale, LayoutStatus> {
    let text_fit = node.block.context.style.misc_inherited1_data.text_fit;
    let grow_consistent =
      text_fit.mode == TextFitMode::Grow && text_fit.target == TextFitTarget::Consistent;
    let shrink_consistent =
      text_fit.mode == TextFitMode::Shrink && text_fit.target == TextFitTarget::Consistent;

    if !grow_consistent && !shrink_consistent {
      return Ok(ParagraphScale::default());
    }

    let available_inline_size = self.child_available_inline_size();
    let mut measuring = BlockLayoutAlgorithm::new(&mut *self.tree, self.node, self.space);
    let mut inline_child = InlineChild::new(node, items_data);

    measuring.line_clamp_data = self.line_clamp_data;
    inline_child
      .context
      .enable_measuring_mode_if_necessary(None);

    if let Err(status) = measuring
      .start_layout()
      .and_then(|mut previous| measuring.layout_inline_children(&mut inline_child, &mut previous))
    {
      // The container reads the offset the measuring layout resolved.
      if status == LayoutStatus::BfcBlockOffsetResolved {
        self.bfc_block_offset = measuring.bfc_block_offset;
      }

      return Err(status);
    }

    let paragraph_scale =
      node.measure_per_block_scale(items_data, &inline_child.lines, available_inline_size);

    if (paragraph_scale.scale < 1.0 && !shrink_consistent)
      || (paragraph_scale.scale > 1.0 && !grow_consistent)
    {
      return Ok(ParagraphScale::default());
    }

    Ok(paragraph_scale)
  }

  /// The loop over the children in Blink's `Layout` for an inline formatting context: a line at a
  /// time until the content ends.
  fn layout_inline_children(
    &mut self,
    inline_child: &mut InlineChild<'_, 'r>,
    previous: &mut PreviousInflowPosition,
  ) -> Result<(), LayoutStatus> {
    let style = Rc::clone(&self.style);

    loop {
      self.handle_inflow(InflowChild::Inline(&mut *inline_child), &style, previous)?;

      if inline_child.break_token.is_none() {
        return Ok(());
      }
    }
  }

  /// Blink's `PositionOrPropagateListMarker` for a line: the outside marker goes beside the first
  /// line box, which moves down from `block_offset` when the marker reaches higher.
  fn position_or_propagate_list_marker(
    &mut self,
    inline_child: &mut InlineChild<'_, 'r>,
    line: &LineBoxFragment<'_, 'r>,
    block_offset: &mut LayoutUnit,
  ) {
    if line.line_box_bfc_block_offset.is_none() {
      return;
    }

    let Some(marker) = inline_child.unpositioned_list_marker.take() else {
      return;
    };

    marker.add_to_box(line, block_offset, &mut self.box_atomic_inline());
  }

  /// Blink's `PropagateBaselineFromLineBox`.
  fn propagate_baseline_from_line_box(
    &mut self,
    line: &LineBoxFragment<'_, 'r>,
    block_offset: LayoutUnit,
  ) {
    if line.is_empty_line_box || self.line_clamp_data.data.is_past_clamp_point() {
      return;
    }

    let Some(baseline) = line.baseline() else {
      return;
    };

    if self.first_baseline.is_none() {
      self.first_baseline = Some(block_offset + baseline);
    }
    self.last_baseline = Some(block_offset + baseline);
  }
}

/// Blink's `CalculateOutOfFlowStaticInlineLevelOffset`: where a zero-sized line from `origin`
/// would start beside the floats, aligned and indented as the lines of `container` are.
fn calculate_out_of_flow_static_inline_level_offset(
  container: &RenderContext,
  origin: BfcOffset,
  exclusion_space: &ExclusionSpace,
  child_available_inline_size: LayoutUnit,
) -> LayoutUnit {
  let style = &container.style;
  let direction = style.inherited_data.direction;
  let is_ltr = direction == Direction::Ltr;
  let opportunity = exclusion_space.find_layout_opportunity(
    origin,
    child_available_inline_size,
    direction,
    LayoutUnit::ZERO,
  );
  let child_line_offset = if is_ltr {
    opportunity.rect.line_start_offset()
  } else {
    opportunity.rect.line_end_offset()
  };
  let relative_line_offset = child_line_offset - origin.line_offset;
  let mut inline_offset = if is_ltr {
    relative_line_offset
  } else {
    child_available_inline_size - relative_line_offset
  };
  let line_offset =
    text_align(style, false).line_offset_for_text_align(direction, opportunity.rect.inline_size());

  inline_offset += if is_ltr {
    line_offset
  } else {
    opportunity.rect.inline_size() - line_offset
  };

  let text_indent = style.misc_inherited1_data.text_indent;

  if !text_indent.hanging {
    inline_offset += text_indent
      .amount
      .minimum_value_for(&container.sizing, child_available_inline_size);
  }

  inline_offset
}

/// Blink's `WebkitTextAlignAndJustifySelfOffset`, for `justify-self` alone.
fn justify_self_offset(
  child_style: &LayoutStyle,
  available_space: LayoutUnit,
  margins: &LogicalBoxStrut,
  child_inline_size: LayoutUnit,
) -> LayoutUnit {
  let free_space = available_space - child_inline_size - margins.inline_sum();
  let (position, is_safe) = match child_style.justify_self {
    AlignItems::Center => (InlineEdge::Center, false),
    AlignItems::SafeCenter => (InlineEdge::Center, true),
    AlignItems::End | AlignItems::FlexEnd => (InlineEdge::End, false),
    AlignItems::SafeEnd | AlignItems::SafeFlexEnd => (InlineEdge::End, true),
    _ => return LayoutUnit::ZERO,
  };
  // `normal` alignment is safe; a stated one is unsafe unless it says otherwise.
  let free_space = if is_safe {
    free_space.clamp_negative_to_zero()
  } else {
    free_space
  };

  match position {
    InlineEdge::Center => free_space / 2,
    InlineEdge::End => free_space,
    InlineEdge::Start => LayoutUnit::ZERO,
  }
}

/// Blink's `BlockLayoutAlgorithm::ComputeMinMaxSizes`: the min-content and max-content sizes of a
/// block container with block-level children, floats beside each other on their "line".
pub(crate) fn block_min_max_sizes(
  tree: &mut LayoutTree<'_>,
  node: NodeId,
  space: &ConstraintSpace,
) -> MinMaxSizesResult {
  let style = Rc::clone(tree.layout_style_rc(node));
  let resolver = LengthResolver::new(space, &style);
  let border_padding = resolver.border_padding;
  // Blink's intrinsic fragment geometry: an indefinite inline size, and the block size the style
  // gives without content.
  let child_available_size = child_available_size(
    AvailableLogicalSize {
      inline_size: None,
      block_size: resolver.initial_block_size_for_fragment(None, None),
    },
    border_padding,
  );
  let child_percentage_size =
    child_percentage_size(space, tree.is_anonymous(node), child_available_size);
  let direction = style.direction;
  let children = tree.child_nodes(node);
  let mut sizes = MinMaxSizes::default();
  let mut depends_on_block_constraints = false;
  let mut float_left_inline_size = LayoutUnit::ZERO;
  let mut float_right_inline_size = LayoutUnit::ZERO;

  for child in children {
    let child_style = Rc::clone(tree.layout_style_rc(child));

    if child_style.is_out_of_flow() {
      continue;
    }

    let child_clear = child_style.clear;
    let child_is_new_fc = tree.creates_new_formatting_context(child);
    let is_floating = child_style.is_floating();

    // A float or new formatting context with clearance starts a new "line".
    if is_floating || child_is_new_fc {
      let float_inline_size = float_left_inline_size + float_right_inline_size;

      if child_clear != Clear::None {
        sizes.max_size = sizes.max_size.max(float_inline_size);
      }
      if matches!(child_clear, Clear::Both | Clear::Left) {
        float_left_inline_size = LayoutUnit::ZERO;
      }
      if matches!(child_clear, Clear::Both | Clear::Right) {
        float_right_inline_size = LayoutUnit::ZERO;
      }
    }

    let mut child_space = ConstraintSpace::new(None, child_style.direction);

    child_space.available_block_size = child_available_size.block_size;
    child_space.percentage_resolution_block_size = child_percentage_size.block_size;
    child_space.is_new_formatting_context = child_is_new_fc;
    child_space.cache_slot = CacheSlot::Measure;

    let child_result = tree.min_and_max_content_contribution(child, &child_space);
    let margins = margins_for(&child_style, None, direction);
    let max_inline_contribution = if is_floating {
      let float_inline_size = child_result.sizes.max_size + margins.inline_sum();

      // A float that negative margins move outside the content box takes no room.
      if float_inline_size > LayoutUnit::ZERO {
        if child_style.float == Float::Left {
          float_left_inline_size += float_inline_size;
        } else {
          float_right_inline_size += float_inline_size;
        }
      }

      float_left_inline_size + float_right_inline_size
    } else if child_is_new_fc {
      let margin_line_left = margins.line_left(direction);
      let margin_line_right = margins.line_right(direction);
      let line_left_inset = if margin_line_left > LayoutUnit::ZERO {
        float_left_inline_size.max(margin_line_left)
      } else {
        float_left_inline_size + margin_line_left
      };
      let line_right_inset = if margin_line_right > LayoutUnit::ZERO {
        float_right_inline_size.max(margin_line_right)
      } else {
        float_right_inline_size + margin_line_right
      };

      child_result.sizes.max_size + (line_left_inset + line_right_inset)
    } else {
      child_result.sizes.max_size + margins.inline_sum()
    };

    sizes.max_size = sizes.max_size.max(max_inline_contribution);
    sizes.min_size = sizes
      .min_size
      .max(child_result.sizes.min_size + margins.inline_sum());
    depends_on_block_constraints |= child_result.depends_on_block_constraints;

    // Anything but a float starts a new "line".
    if !is_floating {
      float_left_inline_size = LayoutUnit::ZERO;
      float_right_inline_size = LayoutUnit::ZERO;
    }
  }

  sizes += border_padding.inline_sum();
  MinMaxSizesResult::new(sizes, depends_on_block_constraints)
}
