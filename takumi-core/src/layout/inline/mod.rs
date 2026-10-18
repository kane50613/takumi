//! Inline layout, ported from Blink's `core/layout/inline`.

use crate::{
  font_style::BoxLineHeight,
  geometry::{Point, Size},
  style::{
    AppliedTextDecorations, Color, FontSynthesis, ResolvedFontVariations, TextDecorationSkipInk,
    VerticalAlign,
  },
};

pub(crate) mod atomic_inline;
mod background;
mod block_layout;
mod cache;
mod decorations;
mod fragment_items;
pub(crate) mod inline_box_state;
pub(crate) mod inline_break_token;
pub(crate) mod inline_child_layout_context;
pub(crate) mod inline_item;
pub(crate) mod inline_item_result;
pub(crate) mod inline_item_segment;
pub(crate) mod inline_item_text_index;
pub(crate) mod inline_items_builder;
pub(crate) mod inline_layout_algorithm;
pub(crate) mod inline_node;
pub(crate) mod inline_node_data;
mod items;
pub(crate) mod justification_utils;
pub(crate) mod leading_floats;
pub(crate) mod line_break_candidate;
pub(crate) mod line_break_point;
pub(crate) mod line_breaker;
pub(crate) mod line_info;
pub(crate) mod line_truncator;
pub(crate) mod line_widths;
pub(crate) mod logical_line_builder;
pub(crate) mod logical_line_item;
mod metrics;
pub(crate) mod min_max_sizes;
mod outline;
pub(crate) mod paragraph_line_breaker;
pub(crate) mod previous_character;
mod runs;
pub(crate) mod score_line_breaker;
#[cfg(test)]
pub(crate) mod testing;
mod text_fit;
pub(crate) mod text_fit_scale;
pub(crate) mod text_fit_utils;

pub(crate) use self::items::InlineContentKind;
pub(crate) use self::{
  background::PaddingBox,
  fragment_items::{ContentBox, InkBox},
  items::InlineOutOfFlow,
  outline::{OutlineIsland, RightAngleContour},
};
pub use self::{
  background::{FragmentBackground, InlineBackgroundFragment},
  decorations::DecorationLine,
  fragment_items::FragmentItems,
  items::{DecorationLink, InlineBoxItem, ProcessedInlineSpan},
  metrics::{InlineBoxKind, InlinePass, VisualInlineBox},
  outline::InlineOutlineRect,
  runs::{
    HangingWhitespace, InlineRunLayout, MeasuredInlineBox, MeasuredInlineRun, PositionedGlyph,
    PositionedInlineRun, RunMetrics, ShapedRun,
  },
};
pub(crate) use cache::InlineLayoutCache;

/// Where an out-of-flow box sits before insets move it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StaticPosition {
  /// The inline-start and block-start corner of its margin box.
  pub(crate) point: Point<f32>,
  /// Whether the inline start is the right edge, in a right-to-left paragraph.
  pub(crate) from_end: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// The size an inline layout measures at and where its first and last lines sit.
pub struct InlineMeasurement {
  pub(crate) size: Size<f32>,
  /// Baseline of the first line from the content-box top.
  pub(crate) first_baseline: Option<f32>,
  /// Baseline of the last line from the content-box top.
  pub(crate) last_baseline: Option<f32>,
  /// Whether a height or line limit may have dropped lines.
  pub(crate) clamped: bool,
}

#[derive(Clone, PartialEq, Debug)]
/// Paint attributes carried per glyph run through the inline layout.
pub struct InlineBrush {
  /// Span this run originated from, if any.
  pub source_span_id: Option<u64>,
  /// Whether this run is the synthetic direction mark, which never paints.
  pub(crate) is_direction_mark: bool,
  /// Run opacity.
  pub opacity: f32,
  /// Text fill color.
  pub color: Color,
  /// The decorations the run paints.
  pub decorations: AppliedTextDecorations,
  /// Whether decorations skip over glyph ink.
  pub decoration_skip_ink: TextDecorationSkipInk,
  /// `-webkit-text-stroke` colour, which a span may set for itself.
  pub stroke_color: Color,
  /// `-webkit-text-stroke` width in pixels.
  pub stroke_width: f32,
  pub(crate) font_synthesis: FontSynthesis,
  /// The variations the run shaped with.
  pub(crate) font_variations: ResolvedFontVariations,
  pub(crate) line_height: BoxLineHeight,
  /// `letter-spacing` in pixels when it is fixed rather than a percentage, which `text-fit`
  /// leaves unscaled.
  pub(crate) fixed_letter_spacing: f32,
  /// `word-spacing` in pixels when it is fixed rather than a percentage.
  pub(crate) fixed_word_spacing: f32,
  pub(crate) vertical_align: VerticalAlign,
}

impl Default for InlineBrush {
  fn default() -> Self {
    Self {
      source_span_id: None,
      is_direction_mark: false,
      opacity: 1.0,
      color: Color::black(),
      decorations: AppliedTextDecorations::default(),
      decoration_skip_ink: TextDecorationSkipInk::default(),
      stroke_color: Color::black(),
      stroke_width: 0.0,
      font_synthesis: FontSynthesis::default(),
      font_variations: ResolvedFontVariations::default(),
      line_height: BoxLineHeight::Normal,
      fixed_letter_spacing: 0.0,
      fixed_word_spacing: 0.0,
      vertical_align: VerticalAlign::default(),
    }
  }
}
