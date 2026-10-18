//! Blink's [`InlineItemResult`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_item_result.cc)
//! and [`HyphenResult`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/hyphen_result.cc),
//! without ruby or `text-fit`.

use std::{ops::Range, rc::Rc};

use super::{
  inline_item::{InlineItem, InlineItemType},
  inline_item_text_index::InlineItemTextIndex,
  inline_node_data::StyleFonts,
  text_fit_scale::{TextFitBlockScale, TextFitScale},
};
use crate::{
  context::RenderContext,
  fonts::{
    font_height::FontHeight,
    shaping::{
      harfbuzz_shaper::HarfBuzzShaper, shape_result::ShapeResult,
      shape_result_view::ShapeResultView,
    },
  },
  geometry::Rect,
  layout::{exclusions::exclusion_space::ExclusionSpace, floats_utils::PositionedFloat},
  layout_unit::LayoutUnit,
  style::Direction as TextDirection,
  text::character::{HYPHEN, HYPHEN_MINUS},
};

/// Blink's `LineBoxStrut`: lengths on each side of a box, in line-relative directions.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct LineBoxStrut {
  pub(crate) inline_start: LayoutUnit,
  pub(crate) inline_end: LayoutUnit,
  pub(crate) line_over: LayoutUnit,
  pub(crate) line_under: LayoutUnit,
}

impl LineBoxStrut {
  /// The physical `edges` of a box in a container of `direction`, truncated to layout units as
  /// Blink's `LineBoxStrut(BoxStrut)` takes them.
  pub(crate) fn from_physical(edges: Rect<f32>, direction: TextDirection) -> Self {
    let (inline_start, inline_end) = direction.inline_sides(edges.left, edges.right);

    Self {
      inline_start: LayoutUnit::from_float(inline_start),
      inline_end: LayoutUnit::from_float(inline_end),
      line_over: LayoutUnit::from_float(edges.top),
      line_under: LayoutUnit::from_float(edges.bottom),
    }
  }

  /// Blink's `LineLeft`.
  pub(crate) fn line_left(&self, direction: TextDirection) -> LayoutUnit {
    match direction {
      TextDirection::Ltr => self.inline_start,
      TextDirection::Rtl => self.inline_end,
    }
  }

  /// Blink's `InlineSum`.
  pub(crate) fn inline_sum(&self) -> LayoutUnit {
    self.inline_start + self.inline_end
  }
}

/// What laying out an atomic inline gives its line: Blink's `LayoutResult` for one, in
/// line-relative directions.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct LayoutResult {
  /// The border box's inline size.
  pub(crate) inline_size: LayoutUnit,
  /// The border box's block size.
  pub(crate) block_size: LayoutUnit,
  /// The baseline below the border box's block start, if it has one.
  pub(crate) baseline: Option<LayoutUnit>,
}

impl LayoutResult {
  /// Blink's `LogicalBoxFragment::BaselineMetrics` for the alphabetic baseline: the margin box
  /// around the baseline, synthesized at the margin box's bottom without one.
  pub(crate) fn baseline_metrics(&self, margins: &LineBoxStrut) -> FontHeight {
    match self.baseline {
      Some(baseline) => FontHeight::new(
        baseline + margins.line_over,
        self.block_size - baseline + margins.line_under,
      ),
      None => FontHeight::new(
        self.block_size + margins.line_over + margins.line_under,
        LayoutUnit::ZERO,
      ),
    }
  }
}

/// Blink's `HyphenResult`: the shaped hyphen a hyphenated line ends with.
#[derive(Debug, Clone)]
pub(crate) struct HyphenResult {
  pub(crate) text: String,
  pub(crate) shape_result: Rc<ShapeResult>,
}

impl HyphenResult {
  /// Blink's `Shape`: `ComputedStyle::HyphenString`, U+2010 HYPHEN when the primary font has it,
  /// else U+002D HYPHEN-MINUS.
  pub(crate) fn new(context: &RenderContext, fonts: &StyleFonts) -> Self {
    let font = fonts.get(context);
    let has_hyphen = font
      .primary_font()
      .is_some_and(|font_data| font_data.has_glyph(HYPHEN));
    let text = String::from(if has_hyphen { HYPHEN } else { HYPHEN_MINUS });
    let shape_result =
      HarfBuzzShaper::new(text.clone()).shape_all(&font, context.style.inherited_data.direction);

    Self {
      text,
      shape_result: Rc::new(shape_result),
    }
  }

  /// Blink's `InlineSize`.
  pub(crate) fn inline_size(&self) -> LayoutUnit {
    self.shape_result.snapped_width().clamp_negative_to_zero()
  }
}

/// Blink's `InlineItemResult`: the part of an item a line holds, and how it measures there.
#[derive(Debug, Clone)]
pub(crate) struct InlineItemResult<'a, 'c> {
  pub(crate) item: &'a InlineItem<'c>,
  pub(crate) item_index: usize,
  pub(crate) text_offset: Range<usize>,
  pub(crate) inline_size: LayoutUnit,
  pub(crate) spacing_before: LayoutUnit,
  pub(crate) shape_result: Option<Rc<ShapeResultView>>,
  pub(crate) hyphen: Option<HyphenResult>,
  pub(crate) text_fit_scale: Option<TextFitScale>,
  pub(crate) layout_result: Option<LayoutResult>,
  pub(crate) positioned_float: Option<PositionedFloat>,
  /// The exclusion space before the float was placed, which a rewind restores.
  pub(crate) exclusion_space_before_position_float: Option<ExclusionSpace>,
  pub(crate) margins: LineBoxStrut,
  pub(crate) borders: LineBoxStrut,
  pub(crate) padding: LineBoxStrut,
  pub(crate) may_break_inside: bool,
  pub(crate) can_break_after: bool,
  pub(crate) has_only_pre_wrap_trailing_spaces: bool,
  pub(crate) has_only_bidi_trailing_spaces: bool,
  pub(crate) break_anywhere_if_overflow: bool,
  pub(crate) should_create_line_box: bool,
  pub(crate) has_unpositioned_floats: bool,
  pub(crate) is_hyphenated: bool,
}

impl<'a, 'c> InlineItemResult<'a, 'c> {
  pub(crate) fn new(
    item: &'a InlineItem<'c>,
    item_index: usize,
    text_offset: Range<usize>,
    break_anywhere_if_overflow: bool,
    should_create_line_box: bool,
    has_unpositioned_floats: bool,
  ) -> Self {
    Self {
      item,
      item_index,
      text_offset,
      inline_size: LayoutUnit::ZERO,
      spacing_before: LayoutUnit::ZERO,
      shape_result: None,
      hyphen: None,
      text_fit_scale: None,
      layout_result: None,
      positioned_float: None,
      exclusion_space_before_position_float: None,
      margins: LineBoxStrut::default(),
      borders: LineBoxStrut::default(),
      padding: LineBoxStrut::default(),
      may_break_inside: false,
      can_break_after: false,
      has_only_pre_wrap_trailing_spaces: false,
      has_only_bidi_trailing_spaces: false,
      break_anywhere_if_overflow,
      should_create_line_box,
      has_unpositioned_floats,
      is_hyphenated: false,
    }
  }

  pub(crate) fn start_offset(&self) -> usize {
    self.text_offset.start
  }

  pub(crate) fn end_offset(&self) -> usize {
    self.text_offset.end
  }

  pub(crate) fn length(&self) -> usize {
    self.text_offset.len()
  }

  pub(crate) fn start(&self) -> InlineItemTextIndex {
    InlineItemTextIndex::new(self.item_index, self.start_offset())
  }

  pub(crate) fn end(&self) -> InlineItemTextIndex {
    InlineItemTextIndex::new(self.item_index, self.end_offset())
  }

  /// Blink's `IsEmptyText`.
  pub(crate) fn is_empty_text(&self) -> bool {
    self.length() == 0 && self.item.item_type == InlineItemType::Text
  }

  /// Blink's `ShapeHyphen`.
  pub(crate) fn shape_hyphen(&mut self, fonts: &StyleFonts) {
    debug_assert!(self.hyphen.is_none());

    if let Some(object) = self.item.layout_object {
      self.hyphen = Some(HyphenResult::new(&object.node().context, fonts));
    }
  }
}

/// Blink's `InlineItemResults`.
pub(crate) type InlineItemResults<'a, 'c> = Vec<InlineItemResult<'a, 'c>>;

/// Blink's `FindTextScale`: the scale of the first text from `start_index` at the nesting level
/// `initial_nesting_level` starts it at, or none unless `should_scale`.
pub(crate) fn find_text_scale(
  should_scale: bool,
  line_items: &[InlineItemResult<'_, '_>],
  start_index: usize,
  initial_nesting_level: usize,
) -> TextFitBlockScale {
  if !should_scale {
    return TextFitBlockScale::default();
  }

  let mut level = initial_nesting_level;

  for item_result in line_items.iter().skip(start_index) {
    match item_result.item.item_type {
      InlineItemType::OpenTag => level += 1,
      InlineItemType::CloseTag => {
        if level == 0 {
          break;
        }

        level -= 1;
      }
      InlineItemType::Text if level == 0 => {
        return item_result.text_fit_scale.as_ref().map_or_else(
          TextFitBlockScale::default,
          |text_fit_scale| TextFitBlockScale {
            paint_scale: text_fit_scale.scale,
            scaled_font: text_fit_scale.font.clone(),
          },
        );
      }
      _ => {}
    }
  }

  TextFitBlockScale::default()
}
