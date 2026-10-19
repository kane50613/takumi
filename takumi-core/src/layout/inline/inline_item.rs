//! Blink's [`InlineItem`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_item.cc).

use std::{
  fmt::{self, Debug, Formatter},
  rc::Rc,
};

use icu_properties::props::Script;

use crate::{
  context::RenderContext,
  fonts::{
    font_fallback_priority::FontFallbackPriority,
    shaping::{run_segmenter::RunSegmenterRange, shape_result::ShapeResult},
  },
  layout::{node::NodeKind, tree::RenderNode},
  style::{ComputedStyle, Direction as TextDirection},
  text::bidi_paragraph::direction_from_level,
};

/// The box an inline item comes from: Blink's `LayoutObject`.
#[derive(Clone, Copy)]
pub(crate) enum LayoutObject<'c> {
  /// A node's text: Blink's `LayoutText`.
  Text(&'c RenderNode),
  /// An inline box: Blink's `LayoutInline`.
  Inline(&'c RenderNode),
  /// An atomic inline, a float, an out-of-flow box, or a list marker: Blink's `LayoutBox`.
  Box(&'c RenderNode),
}

impl<'c> LayoutObject<'c> {
  pub(crate) fn node(self) -> &'c RenderNode {
    match self {
      Self::Text(node) | Self::Inline(node) | Self::Box(node) => node,
    }
  }

  pub(crate) fn context(self) -> &'c RenderContext {
    &self.node().context
  }

  /// Blink's `Style`.
  pub(crate) fn style(self) -> &'c ComputedStyle {
    &self.node().context.style
  }

  /// Blink's `IsLayoutImage`.
  pub(crate) fn is_image(self) -> bool {
    matches!(self, Self::Box(node) if node.node.as_ref().is_some_and(|node| matches!(node.kind, NodeKind::Image(_))))
  }

  /// Blink's `IsBR`.
  pub(crate) fn is_br(self) -> bool {
    matches!(self, Self::Text(node) if node.node.as_ref().and_then(|node| node.tag_name()).is_some_and(|tag| tag.eq_ignore_ascii_case("br")))
  }
}

impl Debug for LayoutObject<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let kind = match self {
      Self::Text(_) => "Text",
      Self::Inline(_) => "Inline",
      Self::Box(_) => "Box",
    };
    let tag = self.node().node.as_ref().and_then(|node| node.tag_name());

    write!(f, "{kind}({tag:?})")
  }
}

/// Blink's `InlineItemType`, without ruby and initial letters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InlineItemType {
  Text,
  Control,
  AtomicInline,
  BlockInInline,
  OpenTag,
  CloseTag,
  Floating,
  OutOfFlowPositioned,
  ListMarker,
  BidiControl,
}

/// Blink's `CollapseType`: how white space collapses at the end of an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CollapseType {
  #[default]
  NotCollapsible,
  OpaqueToCollapsing,
  Collapsible,
  Collapsed,
}

/// Blink's `TextItemType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum TextItemType {
  #[default]
  Normal,
  ForcedLineBreak,
  FlowControl,
  SymbolMarker,
  LayoutGenerated,
}

/// Blink's `StyleVariant`, without `::first-line`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum StyleVariant {
  #[default]
  Standard,
  StandardEllipsis,
}

/// Blink's `InlineItem`.
#[derive(Debug, Clone)]
pub(crate) struct InlineItem<'c> {
  start_offset: usize,
  end_offset: usize,
  pub(crate) shape_result: Option<Rc<ShapeResult>>,
  pub(crate) layout_object: Option<LayoutObject<'c>>,
  pub(crate) index: usize,
  pub(crate) item_type: InlineItemType,
  pub(crate) text_type: TextItemType,
  pub(crate) style_variant: StyleVariant,
  pub(crate) end_collapse_type: CollapseType,
  bidi_level: u8,
  /// The script and fallback priority `RunSegmenter` found, for text items.
  pub(crate) segment_data: Option<(Script, FontFallbackPriority)>,
  pub(crate) is_empty_item: bool,
  pub(crate) is_block_level: bool,
  pub(crate) is_end_collapsible_newline: bool,
  pub(crate) is_generated_for_line_break: bool,
  pub(crate) is_unsafe_to_reuse_shape_result: bool,
  /// Blink keeps this on `LayoutInline`, shared by its open and close tags.
  pub(crate) should_create_box_fragment: bool,
}

impl<'c> InlineItem<'c> {
  pub(crate) fn new(
    item_type: InlineItemType,
    start: usize,
    end: usize,
    layout_object: Option<LayoutObject<'c>>,
  ) -> Self {
    debug_assert!(end >= start);

    let mut item = Self {
      start_offset: start,
      end_offset: end,
      shape_result: None,
      layout_object,
      index: 0,
      item_type,
      text_type: TextItemType::Normal,
      style_variant: StyleVariant::Standard,
      end_collapse_type: CollapseType::NotCollapsible,
      bidi_level: 0,
      segment_data: None,
      is_empty_item: false,
      is_block_level: false,
      is_end_collapsible_newline: false,
      is_generated_for_line_break: false,
      is_unsafe_to_reuse_shape_result: false,
      should_create_box_fragment: false,
    };

    item.compute_box_properties();
    item
  }

  /// Blink's copy constructor with new offsets and shape result.
  pub(crate) fn with_offsets(
    &self,
    start: usize,
    end: usize,
    shape_result: Option<Rc<ShapeResult>>,
  ) -> Self {
    debug_assert!(end >= start);

    Self {
      start_offset: start,
      end_offset: end,
      shape_result,
      ..self.clone()
    }
  }

  /// Blink's `ComputeBoxProperties`.
  fn compute_box_properties(&mut self) {
    debug_assert!(!self.is_empty_item);

    match self.item_type {
      InlineItemType::Text | InlineItemType::AtomicInline | InlineItemType::Control => {}
      InlineItemType::OpenTag => {
        self.is_empty_item = self
          .layout_object
          .is_some_and(|object| inline_box_edges_empty(object.node()).0);
      }
      InlineItemType::CloseTag => {
        self.is_empty_item = self
          .layout_object
          .is_some_and(|object| inline_box_edges_empty(object.node()).1);
      }
      // Unknown until laid out; not empty is the safer guess.
      InlineItemType::BlockInInline => {}
      _ => {
        if matches!(
          self.item_type,
          InlineItemType::OutOfFlowPositioned | InlineItemType::Floating
        ) {
          self.is_block_level = true;
        }

        self.is_empty_item = true;
      }
    }
  }

  /// Blink's `IsForcedLineBreak`.
  pub(crate) fn is_forced_line_break(&self) -> bool {
    self.text_type == TextItemType::ForcedLineBreak
  }

  /// Blink's `IsFloatingOrOutOfFlowPositioned`.
  pub(crate) fn is_floating_or_out_of_flow_positioned(&self) -> bool {
    matches!(
      self.item_type,
      InlineItemType::Floating | InlineItemType::OutOfFlowPositioned
    )
  }

  pub(crate) fn is_symbol_marker(&self) -> bool {
    self.text_type == TextItemType::SymbolMarker
  }

  pub(crate) fn set_is_symbol_marker(&mut self) {
    debug_assert!(matches!(
      self.text_type,
      TextItemType::Normal | TextItemType::SymbolMarker
    ));

    self.text_type = TextItemType::SymbolMarker;
  }

  /// Blink's `IsOpaqueForTextProcessing`.
  pub(crate) fn is_opaque_for_text_processing(&self) -> bool {
    self.length() == 0 || self.is_floating_or_out_of_flow_positioned()
  }

  pub(crate) fn start_offset(&self) -> usize {
    self.start_offset
  }

  pub(crate) fn end_offset(&self) -> usize {
    self.end_offset
  }

  pub(crate) fn length(&self) -> usize {
    self.end_offset - self.start_offset
  }

  /// Blink's `IsValidOffset`.
  pub(crate) fn is_valid_offset(&self, offset: usize) -> bool {
    (offset >= self.start_offset && offset < self.end_offset)
      || (self.start_offset == self.end_offset && offset == self.start_offset)
  }

  /// Blink's `Direction`.
  pub(crate) fn direction(&self) -> TextDirection {
    direction_from_level(self.bidi_level)
  }

  pub(crate) fn bidi_level(&self) -> u8 {
    self.bidi_level
  }

  /// Blink's `BidiLevelForReorder`: list markers stay out of reordering.
  pub(crate) fn bidi_level_for_reorder(&self) -> u8 {
    if self.item_type == InlineItemType::ListMarker {
      0
    } else {
      self.bidi_level
    }
  }

  /// Blink's `IsImage`.
  pub(crate) fn is_image(&self) -> bool {
    self.layout_object.is_some_and(LayoutObject::is_image)
  }

  /// Blink's `SetOffset`, dropping the shape result.
  pub(crate) fn set_offset(&mut self, start: usize, end: usize) {
    debug_assert!(end >= start);

    self.start_offset = start;
    self.end_offset = end;
    self.shape_result = None;
  }

  /// Blink's `SetEndOffset`, dropping the shape result.
  pub(crate) fn set_end_offset(&mut self, end_offset: usize) {
    debug_assert!(end_offset >= self.start_offset);

    self.end_offset = end_offset;
    self.shape_result = None;
  }

  /// Blink's `Style`, from the layout object.
  pub(crate) fn style(&self) -> Option<&'c ComputedStyle> {
    self.layout_object.map(LayoutObject::style)
  }

  /// Blink's `IsCollapsibleSpaceOnly`.
  pub(crate) fn is_collapsible_space_only(&self) -> bool {
    self.item_type == InlineItemType::Text
      && self.end_collapse_type == CollapseType::Collapsible
      && self.length() == 1
  }

  /// Blink's `CreateRunSegmenterRange`.
  pub(crate) fn create_run_segmenter_range(&self) -> RunSegmenterRange {
    debug_assert_eq!(self.item_type, InlineItemType::Text);

    let (script, font_fallback_priority) = self
      .segment_data
      .unwrap_or((Script::Common, FontFallbackPriority::Text));

    RunSegmenterRange {
      start: self.start_offset,
      end: self.end_offset,
      script,
      font_fallback_priority,
    }
  }

  /// Blink's `EqualsRunSegment`.
  pub(crate) fn equals_run_segment(&self, other: &Self) -> bool {
    self.segment_data == other.segment_data
  }

  /// Blink's `SetBidiLevel`, dropping the shape result when the direction changes.
  pub(crate) fn set_bidi_level(&mut self, level: u8) {
    if (level & 1) != (self.bidi_level & 1) {
      self.shape_result = None;
    }

    self.bidi_level = level;
  }
}

/// Blink's `InlineItems`.
pub(crate) type InlineItems<'c> = Vec<InlineItem<'c>>;

/// Blink's `Split`: splits item `index` at `offset`, dropping its shape result.
pub(crate) fn split(items: &mut InlineItems<'_>, index: usize, offset: usize) {
  let source = &mut items[index];

  debug_assert!(offset > source.start_offset);
  debug_assert!(offset < source.end_offset);

  source.shape_result = None;

  let mut copy = source.clone();

  source.end_offset = offset;
  copy.start_offset = offset;
  items.insert(index + 1, copy);
}

/// Blink's `SetSegmentData`: gives every text item `range`'s script and fallback priority.
pub(crate) fn set_segment_data(range: &RunSegmenterRange, items: &mut InlineItems<'_>) {
  for item in items {
    if item.item_type == InlineItemType::Text {
      item.segment_data = Some((range.script, range.font_fallback_priority));
    }
  }
}

/// Blink's `SetBidiLevel`: gives items from `index` up to `end_offset` bidi `level`, splitting the
/// item `end_offset` falls in. Returns the index of the next item.
pub(crate) fn set_bidi_level(
  items: &mut InlineItems<'_>,
  mut index: usize,
  end_offset: usize,
  level: u8,
  mut num_out_of_flow: usize,
) -> usize {
  loop {
    let item = &mut items[index];

    item.set_bidi_level(level);

    if num_out_of_flow != 0 && item.is_floating_or_out_of_flow_positioned() {
      num_out_of_flow -= 1;
    }

    if item.end_offset >= end_offset {
      break;
    }

    index += 1;
  }

  if items[index].end_offset > end_offset {
    split(items, index, end_offset);

    return index + 1;
  }

  debug_assert_eq!(end_offset, items[index].end_offset);

  // Trailing empty close tags and out-of-flow boxes take the level of the item before.
  index += 1;

  while index < items.len() {
    let item = &mut items[index];
    let is_trailing =
      item.length() == 0 && (item.item_type == InlineItemType::CloseTag || num_out_of_flow != 0);

    if !is_trailing {
      break;
    }

    item.set_bidi_level(level);

    if num_out_of_flow != 0 && item.is_floating_or_out_of_flow_positioned() {
      num_out_of_flow -= 1;
    }

    index += 1;
  }

  debug_assert_eq!(num_out_of_flow, 0);

  index
}

/// Blink's `UpdateIndex`.
pub(crate) fn update_index(items: &mut InlineItems<'_>) {
  for (index, item) in items.iter_mut().enumerate() {
    item.index = index;
  }
}

/// Blink's `IsInlineBoxStartEmpty` and `IsInlineBoxEndEmpty`: whether each inline edge of `node`
/// has no border, padding, or margin.
pub(crate) fn inline_box_edges_empty(node: &RenderNode) -> (bool, bool) {
  let direction = node.context.style.inherited_data.direction;
  let sides = [node.border_px(), node.padding_px(), node.margin_px()]
    .map(|edges| direction.inline_sides(edges.left, edges.right));

  (
    sides.iter().all(|&(start, _)| start == 0.0),
    sides.iter().all(|&(_, end)| end == 0.0),
  )
}
