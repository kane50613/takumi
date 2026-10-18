//! Blink's [`LogicalLineItem`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/logical_line_item.cc),
//! without ruby, initial letters, block-in-inline, or `text-fit`.

use std::{
  ops::{Index, IndexMut, Range},
  rc::Rc,
};

use super::{
  inline_item::{InlineItem, InlineItemType, LayoutObject, StyleVariant},
  inline_item_result::{InlineItemResult, LayoutResult, LineBoxStrut},
  inline_item_text_index::InlineItemTextIndex,
};
use crate::{
  fonts::shaping::shape_result_view::ShapeResultView,
  layout::{
    bfc_rect::BfcOffset,
    floats_utils::PositionedFloat,
    logical_rect::{LogicalOffset, LogicalRect, LogicalSize},
    tree::RenderNode,
  },
  layout_unit::LayoutUnit,
  style::Direction as TextDirection,
  text::bidi_paragraph::direction_from_level,
};

/// A line item without a bidi level: Blink's `0xff`.
pub(crate) const NO_BIDI_LEVEL: u8 = 0xff;

/// An inline box's fragment: what Blink's `InlineLayoutStateStack::BoxData::CreateBoxFragment`
/// builds, whose children stay in the line's flat list.
#[derive(Clone)]
pub(crate) struct InlineBoxFragment<'c> {
  pub(crate) node: &'c RenderNode,
  pub(crate) style_variant: StyleVariant,
  /// The border box's size.
  pub(crate) size: LogicalSize,
  pub(crate) borders: LineBoxStrut,
  pub(crate) padding: LineBoxStrut,
  pub(crate) has_line_left_edge: bool,
  pub(crate) has_line_right_edge: bool,
  /// The out-of-flow boxes inside, and their static offsets from the fragment.
  pub(crate) out_of_flow_children: Vec<(&'c RenderNode, LogicalOffset, LayoutUnit)>,
}

/// What a line item's fragment is: Blink's `LayoutResult` of one.
#[derive(Clone)]
pub(crate) enum LineFragment<'c> {
  /// An atomic inline, laid out.
  AtomicInline {
    node: &'c RenderNode,
    layout_result: LayoutResult,
  },
  /// A float placed with the line.
  Float {
    node: &'c RenderNode,
    positioned_float: PositionedFloat,
  },
  InlineBox(InlineBoxFragment<'c>),
}

impl<'c> LineFragment<'c> {
  pub(crate) fn node(&self) -> &'c RenderNode {
    match self {
      Self::AtomicInline { node, .. } | Self::Float { node, .. } => node,
      Self::InlineBox(fragment) => fragment.node,
    }
  }
}

/// Blink's `LogicalLineItem`: an item of a broken line, still mutable and in logical coordinates.
#[derive(Clone)]
pub(crate) struct LogicalLineItem<'a, 'c> {
  /// The item for a text fragment; none for an ellipsis.
  pub(crate) inline_item: Option<&'a InlineItem<'c>>,
  pub(crate) shape_result: Option<Rc<ShapeResultView>>,
  pub(crate) text_offset: Range<usize>,
  /// The text of generated text, such as a hyphen or an ellipsis.
  pub(crate) text_content: Option<String>,
  pub(crate) layout_result: Option<LineFragment<'c>>,
  /// The box an ellipsis belongs to, which has no item.
  pub(crate) layout_object: Option<LayoutObject<'c>>,
  pub(crate) style_variant: StyleVariant,
  pub(crate) out_of_flow_positioned_box: Option<&'c RenderNode>,
  pub(crate) unpositioned_float: Option<&'c RenderNode>,
  pub(crate) item_index: InlineItemTextIndex,
  /// The border box, at first in this child's own coordinates; `ComputeInlinePositions` makes it
  /// the line box's.
  pub(crate) rect: LogicalRect,
  /// A positioned float's offset in the block formatting context.
  pub(crate) bfc_offset: BfcOffset,
  /// The margin box's inline size.
  pub(crate) inline_size: LayoutUnit,
  pub(crate) margin_line_left: LayoutUnit,
  /// The box data index `PrepareForReorder` and `UpdateAfterReorder` track the item by.
  pub(crate) box_data_index: usize,
  /// For an inline box, its descendants and itself; zero for others.
  pub(crate) children_count: usize,
  pub(crate) bidi_level: u8,
  /// The containing block's direction, for an out-of-flow box.
  pub(crate) container_direction: TextDirection,
  /// Only trailing spaces, which take the paragraph's level (UAX #9 rule L1).
  pub(crate) has_only_bidi_trailing_spaces: bool,
  pub(crate) has_non_empty_inline_box_start: bool,
}

impl Default for LogicalLineItem<'_, '_> {
  fn default() -> Self {
    Self {
      inline_item: None,
      shape_result: None,
      text_offset: 0..0,
      text_content: None,
      layout_result: None,
      layout_object: None,
      style_variant: StyleVariant::Standard,
      out_of_flow_positioned_box: None,
      unpositioned_float: None,
      item_index: InlineItemTextIndex::default(),
      rect: LogicalRect::default(),
      bfc_offset: BfcOffset::default(),
      inline_size: LayoutUnit::ZERO,
      margin_line_left: LayoutUnit::ZERO,
      box_data_index: 0,
      children_count: 0,
      bidi_level: NO_BIDI_LEVEL,
      container_direction: TextDirection::Ltr,
      has_only_bidi_trailing_spaces: false,
      has_non_empty_inline_box_start: false,
    }
  }
}

impl<'a, 'c> LogicalLineItem<'a, 'c> {
  /// A placeholder, without a fragment or a bidi level.
  pub(crate) fn placeholder(block_offset: LayoutUnit, block_size: LayoutUnit) -> Self {
    Self {
      rect: LogicalRect::new(LayoutUnit::ZERO, block_offset, LayoutUnit::ZERO, block_size),
      ..Self::default()
    }
  }

  /// A bidi control, without a fragment but with a bidi level.
  pub(crate) fn bidi_control(bidi_level: u8) -> Self {
    Self {
      bidi_level,
      ..Self::default()
    }
  }

  /// An in-flow fragment at `offset`, `inline_size` wide with its margins.
  pub(crate) fn with_layout_result(
    layout_result: LineFragment<'c>,
    offset: LogicalOffset,
    inline_size: LayoutUnit,
    children_count: usize,
    bidi_level: u8,
  ) -> Self {
    Self {
      layout_result: Some(layout_result),
      rect: LogicalRect {
        offset,
        ..LogicalRect::default()
      },
      inline_size,
      children_count,
      bidi_level,
      ..Self::default()
    }
  }

  /// An in-flow text fragment from `item_result`.
  pub(crate) fn text(
    inline_item: &'a InlineItem<'c>,
    item_result: &InlineItemResult<'a, 'c>,
    block_offset: LayoutUnit,
    inline_size: LayoutUnit,
    text_height: LayoutUnit,
  ) -> Self {
    Self {
      inline_item: Some(inline_item),
      shape_result: item_result.shape_result.clone(),
      text_offset: item_result.text_offset.clone(),
      rect: LogicalRect::new(
        LayoutUnit::ZERO,
        block_offset,
        LayoutUnit::ZERO,
        text_height,
      ),
      inline_size,
      bidi_level: inline_item.bidi_level(),
      has_only_bidi_trailing_spaces: item_result.has_only_bidi_trailing_spaces,
      ..Self::default()
    }
  }

  /// A generated text fragment, such as a hyphen.
  pub(crate) fn generated_text(
    inline_item: &'a InlineItem<'c>,
    shape_result: Rc<ShapeResultView>,
    text_content: String,
    block_offset: LayoutUnit,
    inline_size: LayoutUnit,
    text_height: LayoutUnit,
  ) -> Self {
    Self {
      inline_item: Some(inline_item),
      text_offset: shape_result.start_index() as usize..shape_result.end_index() as usize,
      shape_result: Some(shape_result),
      text_content: Some(text_content),
      rect: LogicalRect::new(
        LayoutUnit::ZERO,
        block_offset,
        LayoutUnit::ZERO,
        text_height,
      ),
      inline_size,
      bidi_level: inline_item.bidi_level(),
      ..Self::default()
    }
  }

  /// An ellipsis, generated text of `layout_object` without an item.
  pub(crate) fn ellipsis(
    layout_object: LayoutObject<'c>,
    style_variant: StyleVariant,
    shape_result: Rc<ShapeResultView>,
    text_content: String,
    rect: LogicalRect,
    bidi_level: u8,
  ) -> Self {
    Self {
      text_offset: shape_result.start_index() as usize..shape_result.end_index() as usize,
      shape_result: Some(shape_result),
      text_content: Some(text_content),
      layout_object: Some(layout_object),
      style_variant,
      rect,
      inline_size: rect.size.inline_size,
      bidi_level,
      ..Self::default()
    }
  }

  /// A part of `source_item`'s text.
  pub(crate) fn sub_range(
    source_item: &Self,
    shape_result: Rc<ShapeResultView>,
    text_offset: Range<usize>,
  ) -> Self {
    Self {
      inline_item: source_item.inline_item,
      inline_size: shape_result.snapped_width(),
      shape_result: Some(shape_result),
      text_offset,
      text_content: source_item.text_content.clone(),
      rect: source_item.rect,
      bidi_level: source_item.bidi_level,
      ..Self::default()
    }
  }

  /// Blink's `OutOfFlowPositioned`.
  pub(crate) fn out_of_flow_positioned(
    inline_item: &InlineItem<'c>,
    container_direction: TextDirection,
  ) -> Self {
    Self {
      out_of_flow_positioned_box: inline_item.layout_object.map(LayoutObject::node),
      bidi_level: inline_item.bidi_level(),
      container_direction,
      ..Self::default()
    }
  }

  /// Blink's `PositionedFloat`.
  pub(crate) fn positioned_float(
    inline_item: &InlineItem<'c>,
    positioned_float: PositionedFloat,
  ) -> Self {
    Self {
      layout_result: inline_item.layout_object.map(|object| LineFragment::Float {
        node: object.node(),
        positioned_float,
      }),
      bfc_offset: positioned_float.bfc_offset,
      bidi_level: inline_item.bidi_level(),
      ..Self::default()
    }
  }

  /// Blink's `UnpositionedFloat`.
  pub(crate) fn unpositioned_float(
    inline_item: &InlineItem<'c>,
    item_index: InlineItemTextIndex,
  ) -> Self {
    Self {
      unpositioned_float: inline_item.layout_object.map(LayoutObject::node),
      bidi_level: inline_item.bidi_level(),
      item_index,
      ..Self::default()
    }
  }

  /// Blink's `IsItemType`.
  pub(crate) fn is_item_type(&self, item_type: InlineItemType) -> bool {
    self
      .inline_item
      .is_some_and(|item| item.item_type == item_type)
  }

  /// Blink's `IsFloating`.
  pub(crate) fn is_floating(&self) -> bool {
    matches!(self.layout_result, Some(LineFragment::Float { .. }))
  }

  /// Blink's `IsAtomicInline`.
  pub(crate) fn is_atomic_inline(&self) -> bool {
    matches!(self.layout_result, Some(LineFragment::AtomicInline { .. }))
  }

  /// Blink's `IsInlineBox`.
  pub(crate) fn is_inline_box(&self) -> bool {
    matches!(self.layout_result, Some(LineFragment::InlineBox(_)))
  }

  /// Blink's `HasInFlowFragment`.
  pub(crate) fn has_in_flow_fragment(&self) -> bool {
    self.inline_item.is_some()
      || self
        .layout_result
        .as_ref()
        .is_some_and(|result| !matches!(result, LineFragment::Float { .. }))
  }

  /// Blink's `HasInFlowOrFloatingFragment`.
  pub(crate) fn has_in_flow_or_floating_fragment(&self) -> bool {
    self.inline_item.is_some() || self.layout_result.is_some() || self.layout_object.is_some()
  }

  /// Blink's `HasOutOfFlowFragment`.
  pub(crate) fn has_out_of_flow_fragment(&self) -> bool {
    self.out_of_flow_positioned_box.is_some()
  }

  /// Blink's `HasFragment`.
  pub(crate) fn has_fragment(&self) -> bool {
    self.has_in_flow_or_floating_fragment() || self.has_out_of_flow_fragment()
  }

  /// Blink's `IsControl`.
  pub(crate) fn is_control(&self) -> bool {
    self.is_item_type(InlineItemType::Control)
  }

  /// Blink's `HasBidiLevel`.
  pub(crate) fn has_bidi_level(&self) -> bool {
    self.bidi_level != NO_BIDI_LEVEL
  }

  /// Blink's `IsPlaceholder`.
  pub(crate) fn is_placeholder(&self) -> bool {
    !self.has_fragment() && !self.has_bidi_level()
  }

  /// Blink's `IsOpaqueToBidiReordering`: placeholders and inline boxes keep their places.
  pub(crate) fn is_opaque_to_bidi_reordering(&self) -> bool {
    self.is_placeholder() || self.is_inline_box()
  }

  /// Blink's `GetLayoutObject`'s node.
  pub(crate) fn node(&self) -> Option<&'c RenderNode> {
    self
      .inline_item
      .and_then(|item| item.layout_object)
      .map(LayoutObject::node)
      .or_else(|| self.layout_result.as_ref().map(LineFragment::node))
      .or_else(|| self.layout_object.map(LayoutObject::node))
  }

  /// Blink's `ResolvedDirection`.
  pub(crate) fn resolved_direction(&self) -> TextDirection {
    debug_assert!(self.has_bidi_level() || self.is_inline_box());

    if self.has_bidi_level() {
      direction_from_level(self.bidi_level)
    } else {
      TextDirection::Ltr
    }
  }
}

/// Blink's `LogicalLineItems`.
#[derive(Clone, Default)]
pub(crate) struct LogicalLineItems<'a, 'c> {
  pub(crate) children: Vec<LogicalLineItem<'a, 'c>>,
}

impl<'a, 'c> LogicalLineItems<'a, 'c> {
  pub(crate) fn len(&self) -> usize {
    self.children.len()
  }

  pub(crate) fn is_empty(&self) -> bool {
    self.children.is_empty()
  }

  pub(crate) fn add_child(&mut self, item: LogicalLineItem<'a, 'c>) {
    self.children.push(item);
  }

  /// Blink's `InsertChild`.
  pub(crate) fn insert_child(&mut self, index: usize, item: LogicalLineItem<'a, 'c>) {
    self.will_insert_child(index);
    self.children.insert(index, item);
  }

  /// Blink's `WillInsertChild`: boxes spanning the insertion point grow by one.
  fn will_insert_child(&mut self, insert_before: usize) {
    for (index, child) in self.children.iter_mut().enumerate() {
      if index >= insert_before {
        break;
      }

      if child.children_count != 0 && index + child.children_count > insert_before {
        child.children_count += 1;
      }
    }
  }

  /// Blink's `FirstInFlowChild`.
  pub(crate) fn first_in_flow_child(&self) -> Option<&LogicalLineItem<'a, 'c>> {
    self
      .children
      .iter()
      .find(|child| child.has_in_flow_fragment())
  }

  /// Blink's `LastInFlowChild`.
  pub(crate) fn last_in_flow_child(&self) -> Option<&LogicalLineItem<'a, 'c>> {
    self
      .children
      .iter()
      .rev()
      .find(|child| child.has_in_flow_fragment())
  }

  /// Blink's `MoveInInlineDirection` of every child.
  pub(crate) fn move_in_inline_direction(&mut self, delta: LayoutUnit) {
    self.move_in_inline_direction_range(delta, 0, self.children.len());
  }

  /// Blink's `MoveInInlineDirection` of `start..end`.
  pub(crate) fn move_in_inline_direction_range(
    &mut self,
    delta: LayoutUnit,
    start: usize,
    end: usize,
  ) {
    for child in &mut self.children[start..end] {
      child.rect.offset.inline_offset += delta;
    }
  }

  /// Blink's `MoveInBlockDirection` of every child.
  pub(crate) fn move_in_block_direction(&mut self, delta: LayoutUnit) {
    self.move_in_block_direction_range(delta, 0, self.children.len());
  }

  /// Blink's `MoveInBlockDirection` of `start..end`.
  pub(crate) fn move_in_block_direction_range(
    &mut self,
    delta: LayoutUnit,
    start: usize,
    end: usize,
  ) {
    for child in &mut self.children[start..end] {
      child.rect.offset.block_offset += delta;
    }
  }
}

impl<'a, 'c> Index<usize> for LogicalLineItems<'a, 'c> {
  type Output = LogicalLineItem<'a, 'c>;

  fn index(&self, index: usize) -> &Self::Output {
    &self.children[index]
  }
}

impl IndexMut<usize> for LogicalLineItems<'_, '_> {
  fn index_mut(&mut self, index: usize) -> &mut Self::Output {
    &mut self.children[index]
  }
}
