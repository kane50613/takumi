//! Blink's block fragmentation in
//! [`TableLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/table_layout_algorithm.cc),
//! for pages alone, under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use crate::{
  geometry::{NodeId, Point},
  layout::{
    constraint_space::ConstraintSpace,
    fragmentation_layout_input::ChildBreakToken,
    layout_result::{LayoutResult, LayoutStatus},
    table::{TableLayoutAlgorithm, TableStructure, rows::Rows},
  },
  layout_unit::{BoxStrut, LayoutUnit},
  style::CaptionSide,
};
#[cfg(feature = "fragmentation")]
use crate::{
  layout::{
    break_appeal::BreakAppeal,
    break_token_algorithm_data::BreakTokenAlgorithmData,
    fragmentation_layout_input::FragmentationLayoutInput,
    fragmentation_utils::{
      adjust_margins_for_fragmentation, block_child_entries, break_before_child_if_needed,
      finish_fragmentation, fragment_index, fragmentation_layout_input_for_child, is_break_inside,
      is_early_break_target, setup_space_builder_for_fragmentation,
    },
    logical_rect::LogicalBoxStrut,
    table::TableBreakTokenData,
  },
  style::BoxDecorationBreak,
};

/// A child of the table's box the table lays out: its caption or section, by index in the
/// table's structure.
#[derive(Clone, Copy)]
pub(super) enum TableChild {
  Caption(usize),
  Section(usize),
}

impl TableChild {
  pub(super) fn node(self, structure: &TableStructure) -> NodeId {
    match self {
      Self::Caption(index) => structure.captions[index],
      Self::Section(index) => structure.sections[index].node,
    }
  }
}

/// Blink's `TableBoxExtent`: where the table box, everything but the captions, starts and ends in
/// the fragment.
#[derive(Clone, Copy)]
pub(super) struct TableBoxExtent {
  pub(super) start: LayoutUnit,
  pub(super) end: LayoutUnit,
}

impl TableBoxExtent {
  /// Blink's `BeginTableBoxLayout`, at the table box's block-start border edge.
  pub(super) fn begin(
    block_start_border_edge: LayoutUnit,
    border_padding_block_start: LayoutUnit,
  ) -> Self {
    Self {
      start: block_start_border_edge,
      end: block_start_border_edge + border_padding_block_start,
    }
  }

  /// Blink's `EndTableBoxLayout`: where the table box ends, its grid at least
  /// `minimal_grid_block_size` tall, with how much that grew it.
  pub(super) fn end_layout(
    &mut self,
    border_padding_block_end: LayoutUnit,
    spacing_after_last_section: LayoutUnit,
    minimal_grid_block_size: LayoutUnit,
  ) -> (LayoutUnit, LayoutUnit) {
    self.end += spacing_after_last_section + border_padding_block_end;

    let sections_total_size = self.end - self.start;
    let grid_block_size = sections_total_size.max(minimal_grid_block_size);

    self.end = self.start + grid_block_size;
    (self.end, grid_block_size - sections_total_size)
  }
}

/// How far the table's earlier fragments got.
#[derive(Clone, Copy, Default)]
pub(super) struct ResumedTableBox {
  pub(super) consumed_table_box_block_size: LayoutUnit,
  pub(super) has_entered_table_box: bool,
  pub(super) is_past_table_box: bool,
}

impl TableLayoutAlgorithm<'_, '_> {
  /// Blink's `TableChildIterator`: the captions and sections this fragment resumes or starts, top
  /// captions first and bottom captions last, with the tokens they resume at.
  pub(super) fn child_entries(&self) -> Vec<(TableChild, ChildBreakToken)> {
    let structure = Rc::clone(&self.structure);
    let caption_side = |index: usize| {
      self
        .tree
        .render_node(structure.captions[index])
        .context
        .style
        .inherited_data
        .caption_side
    };
    let children: Vec<TableChild> = (0..structure.captions.len())
      .filter(|&index| caption_side(index) == CaptionSide::Top)
      .map(TableChild::Caption)
      .chain((0..structure.sections.len()).map(TableChild::Section))
      .chain(
        (0..structure.captions.len())
          .filter(|&index| caption_side(index) == CaptionSide::Bottom)
          .map(TableChild::Caption),
      )
      .collect();

    self
      .resumed_child_entries(
        children
          .iter()
          .map(|child| child.node(&structure))
          .collect(),
      )
      .into_iter()
      .filter_map(|(node, token)| {
        children
          .iter()
          .find(|child| child.node(&structure) == node)
          .map(|&child| (child, token))
      })
      .collect()
  }
}

#[cfg(feature = "fragmentation")]
impl TableLayoutAlgorithm<'_, '_> {
  /// Blink's `ComputeCaptionMargins` for a caption resumed at `token`: no block-start margin past
  /// its first fragment, and no block-end margin before its last.
  pub(super) fn adjust_caption_margins_for_fragmentation(
    &self,
    token: &ChildBreakToken,
    margins: &mut BoxStrut,
  ) {
    let mut logical = LogicalBoxStrut {
      inline_start: margins.left,
      inline_end: margins.right,
      block_start: margins.top,
      block_end: margins.bottom,
    };

    adjust_margins_for_fragmentation(token.as_deref(), false, &mut logical);
    margins.top = logical.block_start;
    margins.bottom = logical.block_end;
  }

  fn resumed_child_entries(&self, children: Vec<NodeId>) -> Vec<(NodeId, ChildBreakToken)> {
    block_child_entries(children, self.input.break_token.as_deref())
  }

  /// Blink's `FragmentIndex` of the table fragment this layout makes.
  pub(super) fn fragment_index(&self) -> usize {
    fragment_index(self.input.break_token.as_deref())
  }

  fn table_break_data(&self) -> Option<&TableBreakTokenData> {
    let token = self
      .input
      .break_token
      .as_deref()
      .filter(|token| is_break_inside(Some(token)))?;

    match token.algorithm_data.as_ref()? {
      BreakTokenAlgorithmData::Table(data) => Some(data),
      BreakTokenAlgorithmData::TableRow { .. } => None,
    }
  }

  /// The rows an earlier fragment computed, with the sections' block size before the table's own
  /// grew them.
  pub(super) fn resumed_rows(&self) -> Option<(Rows, LayoutUnit)> {
    self
      .table_break_data()
      .map(|data| ((*data.rows).clone(), data.total_table_min_block_size))
  }

  pub(super) fn resumed_table_box(&self) -> Option<ResumedTableBox> {
    self.table_break_data().map(|data| ResumedTableBox {
      consumed_table_box_block_size: data.consumed_table_box_block_size,
      has_entered_table_box: data.has_entered_table_box,
      is_past_table_box: data.is_past_table_box,
    })
  }

  pub(super) fn previously_consumed_block_size(&self) -> LayoutUnit {
    self.builder.previously_consumed_block_size()
  }

  /// Blink's `FragmentainerSpaceLeftForChildren` where the page's size is known.
  pub(super) fn fragmentainer_space_at_start(&self) -> Option<LayoutUnit> {
    self
      .space
      .has_block_fragmentation()
      .then(|| self.builder.fragmentainer_space_left(true))
  }

  pub(super) fn involved_in_block_fragmentation(&self) -> bool {
    self.builder.involved_in_block_fragmentation()
  }

  pub(super) fn should_clone_box_end_decorations(&self) -> bool {
    self.builder.should_clone_box_end_decorations
  }

  /// Whether `box-decoration-break: clone` repeats the table's border and padding on each
  /// fragment.
  pub(super) fn should_clone_box_decorations(&self) -> bool {
    self.builder.box_decoration_break == BoxDecorationBreak::Clone
  }

  pub(super) fn breaks_before_early_break_target(&mut self, child: NodeId) -> bool {
    let Some(early_break) = self.input.early_break.as_deref() else {
      return false;
    };

    if !is_early_break_target(early_break, &self.builder, Some(child)) {
      return false;
    }
    self
      .builder
      .add_break_before_child(Some(child), BreakAppeal::Perfect, false);
    true
  }

  /// Blink's `SetupSpaceBuilderForFragmentation` for `child` at `offset` in the table.
  pub(super) fn setup_child_space_for_fragmentation(
    &self,
    space: &mut ConstraintSpace,
    child: NodeId,
    offset: LayoutUnit,
  ) {
    setup_space_builder_for_fragmentation(
      self.space,
      &self.builder,
      self.tree.is_monolithic(child),
      offset,
      space,
    );
  }

  pub(super) fn lay_out_child(
    &mut self,
    child: NodeId,
    space: &ConstraintSpace,
    token: &ChildBreakToken,
  ) -> Rc<LayoutResult> {
    let input =
      fragmentation_layout_input_for_child(child, token.clone(), self.input.early_break.as_deref());

    self.tree.layout_fragment(child, space, &input)
  }

  /// Blink's `BreakBeforeChildIfNeeded` for `child` at `offset` in the table.
  pub(super) fn break_before_child_if_needed(
    &mut self,
    child: NodeId,
    result: &LayoutResult,
    offset: LayoutUnit,
    has_container_separation: bool,
  ) -> Result<bool, LayoutStatus> {
    if !self.space.has_block_fragmentation() {
      return Ok(false);
    }

    let style = Rc::clone(self.tree.layout_style_rc(child));
    let fragmentainer_block_offset = self.builder.fragmentainer_offset(true) + offset;

    break_before_child_if_needed(
      self.space,
      &mut self.builder,
      (child, &style),
      result,
      fragmentainer_block_offset,
      has_container_separation,
    )
  }

  /// Places `child`'s fragment resumed at `token` at `location`, and takes in what it tells the
  /// table about breaking.
  pub(super) fn place_table_child(
    &mut self,
    child: NodeId,
    token: &ChildBreakToken,
    location: Point<LayoutUnit>,
    result: &LayoutResult,
  ) {
    let child_fragment_index = fragment_index(token.as_deref());

    self.tree.place_child(
      child,
      child_fragment_index,
      self.fragment_index(),
      location,
      result,
    );
    if !self.space.has_block_fragmentation() {
      return;
    }

    let style = Rc::clone(self.tree.layout_style_rc(child));

    self
      .builder
      .add_result(self.space, &style, result, location.y);
  }

  /// The fragment index `child` resumed at `token` lays out.
  pub(super) fn child_fragment_index(token: &ChildBreakToken) -> usize {
    fragment_index(token.as_deref())
  }

  pub(super) fn resumes_past_block_end(token: &ChildBreakToken) -> bool {
    token.as_deref().is_some_and(|token| token.is_at_block_end)
  }

  pub(super) fn has_inflow_child_break_inside(&self) -> bool {
    self.space.has_block_fragmentation() && self.builder.has_inflow_child_break_inside()
  }

  pub(super) fn set_has_seen_all_children(&mut self) {
    self.builder.has_seen_all_children = true;
  }

  /// Blink's `FinishFragmentation`, then the sides the table box itself includes, as Blink sets
  /// them after: this fragment's block size and intrinsic block size.
  pub(super) fn finish_fragmentation(
    &mut self,
    block_size: LayoutUnit,
    intrinsic_block_size: LayoutUnit,
    sides: (bool, bool),
  ) -> Result<(LayoutUnit, LayoutUnit), LayoutStatus> {
    if !self.builder.involved_in_block_fragmentation() {
      return Ok((block_size, intrinsic_block_size));
    }

    self.builder.fragments_total_block_size = block_size;
    self.builder.intrinsic_block_size = intrinsic_block_size;
    finish_fragmentation(&mut self.builder)?;
    (
      self.builder.includes_block_start,
      self.builder.includes_block_end,
    ) = sides;
    Ok((
      self.builder.fragment_block_size,
      self.builder.intrinsic_block_size,
    ))
  }

  /// Keeps what the next fragment resumes the table from.
  pub(super) fn keep_break_token_data(
    &mut self,
    rows: Rc<Rows>,
    total_table_min_block_size: LayoutUnit,
    table_box: ResumedTableBox,
  ) {
    if !self.builder.involved_in_block_fragmentation() {
      return;
    }

    self.builder.break_token_data = Some(BreakTokenAlgorithmData::Table(Rc::new(
      TableBreakTokenData {
        rows,
        total_table_min_block_size,
        consumed_table_box_block_size: table_box.consumed_table_box_block_size,
        has_entered_table_box: table_box.has_entered_table_box,
        is_past_table_box: table_box.is_past_table_box,
      },
    )));
  }

  pub(super) fn finish_break_info(&mut self, result: &mut LayoutResult) {
    result.break_info = self.builder.finish_break_info(self.node);
    self.builder.exclude_broken_sides(result);
  }

  /// Blink's `RelayoutAndBreakEarlier`: the table laid out again to break at the breakpoint it
  /// found.
  pub(super) fn relayout_and_break_earlier(self, status: LayoutStatus) -> LayoutResult {
    let early_break = self.builder.early_break.clone();

    if status == LayoutStatus::NeedsEarlierBreak && early_break.is_some() {
      let input = FragmentationLayoutInput {
        break_token: self.input.break_token.clone(),
        early_break,
      };

      return TableLayoutAlgorithm::new(self.tree, self.node, self.space, &input).map_or_else(
        || LayoutResult::new(Default::default()),
        TableLayoutAlgorithm::layout,
      );
    }

    let mut result = LayoutResult::new(Default::default());

    result.status = status;
    result.break_info.early_break = early_break;
    result
  }
}

/// The fragmentation hooks without the `fragmentation` feature, where nothing breaks.
#[cfg(not(feature = "fragmentation"))]
impl TableLayoutAlgorithm<'_, '_> {
  pub(super) fn adjust_caption_margins_for_fragmentation(
    &self,
    _token: &ChildBreakToken,
    _margins: &mut BoxStrut,
  ) {
  }

  fn resumed_child_entries(&self, children: Vec<NodeId>) -> Vec<(NodeId, ChildBreakToken)> {
    children.into_iter().map(|child| (child, ())).collect()
  }

  pub(super) fn fragment_index(&self) -> usize {
    0
  }

  pub(super) fn resumed_rows(&self) -> Option<(Rows, LayoutUnit)> {
    None
  }

  pub(super) fn resumed_table_box(&self) -> Option<ResumedTableBox> {
    None
  }

  pub(super) fn previously_consumed_block_size(&self) -> LayoutUnit {
    LayoutUnit::ZERO
  }

  pub(super) fn fragmentainer_space_at_start(&self) -> Option<LayoutUnit> {
    None
  }

  pub(super) fn involved_in_block_fragmentation(&self) -> bool {
    false
  }

  pub(super) fn should_clone_box_end_decorations(&self) -> bool {
    false
  }

  pub(super) fn should_clone_box_decorations(&self) -> bool {
    false
  }

  pub(super) fn breaks_before_early_break_target(&mut self, _child: NodeId) -> bool {
    false
  }

  pub(super) fn setup_child_space_for_fragmentation(
    &self,
    _space: &mut ConstraintSpace,
    _child: NodeId,
    _offset: LayoutUnit,
  ) {
  }

  pub(super) fn lay_out_child(
    &mut self,
    child: NodeId,
    space: &ConstraintSpace,
    _token: &ChildBreakToken,
  ) -> Rc<LayoutResult> {
    self.tree.layout(child, space)
  }

  pub(super) fn break_before_child_if_needed(
    &mut self,
    _child: NodeId,
    _result: &LayoutResult,
    _offset: LayoutUnit,
    _has_container_separation: bool,
  ) -> Result<bool, LayoutStatus> {
    Ok(false)
  }

  pub(super) fn place_table_child(
    &mut self,
    child: NodeId,
    _token: &ChildBreakToken,
    location: Point<LayoutUnit>,
    result: &LayoutResult,
  ) {
    self.tree.place_child(child, 0, 0, location, result);
  }

  pub(super) fn child_fragment_index(_token: &ChildBreakToken) -> usize {
    0
  }

  pub(super) fn resumes_past_block_end(_token: &ChildBreakToken) -> bool {
    false
  }

  pub(super) fn has_inflow_child_break_inside(&self) -> bool {
    false
  }

  pub(super) fn set_has_seen_all_children(&mut self) {}

  pub(super) fn finish_fragmentation(
    &mut self,
    block_size: LayoutUnit,
    intrinsic_block_size: LayoutUnit,
    _sides: (bool, bool),
  ) -> Result<(LayoutUnit, LayoutUnit), LayoutStatus> {
    Ok((block_size, intrinsic_block_size))
  }

  pub(super) fn keep_break_token_data(
    &mut self,
    _rows: Rc<Rows>,
    _total_table_min_block_size: LayoutUnit,
    _table_box: ResumedTableBox,
  ) {
  }

  pub(super) fn finish_break_info(&mut self, _result: &mut LayoutResult) {}

  pub(super) fn relayout_and_break_earlier(self, status: LayoutStatus) -> LayoutResult {
    let mut result = LayoutResult::new(Default::default());

    result.status = status;
    result
  }
}
