//! Text decoration rectangles and skip-ink outlines.

use crate::{
  geometry::{ComputedLayout, PathCommand, Point},
  layout::intercept::skip_ink_spans,
  resources::glyph::ResolvedGlyph,
  style::{
    Affine, Color, SizedTextDecorationThickness, TextDecorationLines, TextDecorationSkipInk,
  },
};
use std::{collections::HashMap, sync::Arc};

use super::runs::ShapedRun;

/// A text decoration line (underline/overline/line-through) as a fillable rect.
pub struct DecorationRect {
  /// Rect width in pixels (run advance, snapped like the raster path).
  pub width: f32,
  /// Rect height in pixels (decoration thickness).
  pub height: f32,
  /// Decoration color, already resolved against `current-color`.
  pub color: Color,
  /// Affine transform into border-box space (`[a, b, c, d, e, f]`).
  pub transform: [f32; 6],
  /// Whether the line paints above glyphs (line-through) vs below (under/overline).
  pub over: bool,
  /// Which decoration this is, so a backend can single one out.
  pub line: TextDecorationLines,
}

impl ShapedRun {
  /// The active decoration lines for a glyph run, in border-box space.
  pub fn glyph_outlines<'g>(
    &self,
    resolved_glyphs: &'g HashMap<u32, Arc<ResolvedGlyph>>,
    origin: Point<f32>,
    baseline_shift: f32,
  ) -> Vec<(Point<f32>, &'g [PathCommand])> {
    self
      .glyphs
      .iter()
      .filter_map(|glyph| {
        let ResolvedGlyph::Outline(outline) = resolved_glyphs.get(&glyph.id)?.as_ref() else {
          return None;
        };

        Some((
          Point {
            x: origin.x + glyph.x,
            y: origin.y + glyph.y + baseline_shift,
          },
          outline.paths(),
        ))
      })
      .collect()
  }

  /// The rectangles the run's `text-decoration` paints.
  pub fn decorations(
    &self,
    resolved_glyphs: &HashMap<u32, Arc<ResolvedGlyph>>,
    layout: ComputedLayout,
    baseline_shift: f32,
    transform: Affine,
  ) -> Vec<DecorationRect> {
    let mut out = Vec::new();
    let brush = &self.brush;
    let lines = brush.decoration_line;
    if lines.is_empty() {
      return out;
    }
    let metrics = &self.metrics;
    // A fully trimmed run must not snap up to a 1px decoration.
    if self.decorated_advance() <= 0.0 {
      return out;
    }
    let start_x = layout.border.left + layout.padding.left + self.offset;
    let snapped_start_x = start_x.floor();
    let width = (start_x + self.decorated_advance()).ceil() - snapped_start_x;
    if width <= 0.0 {
      return out;
    }
    let baseline = self.baseline + baseline_shift;
    let top = layout.border.top + layout.padding.top;
    // Blink floors every decoration at 1px (`TextDecorationInfo::ResolvedThickness`).
    let thickness = |from_font: f32| {
      match brush.decoration_thickness {
        SizedTextDecorationThickness::Value(value) => value,
        SizedTextDecorationThickness::FromFont => from_font,
      }
      .max(1.0)
    };
    let mut emit = |x: f32,
                    span_width: f32,
                    y_offset: f32,
                    height: f32,
                    over: bool,
                    line: TextDecorationLines| {
      if height <= 0.0 || span_width <= 0.0 {
        return;
      }
      let matrix = transform * Affine::translation(x, top + y_offset);
      out.push(DecorationRect {
        width: span_width,
        height,
        color: brush.decoration_color,
        transform: matrix.to_cols_array(),
        over,
        line,
      });
    };

    if lines.contains(TextDecorationLines::UNDERLINE) {
      let y_offset = baseline + self.underline_offset_from_baseline();
      let height = thickness(metrics.underline_size);
      // `skip-ink` cuts the line where the glyphs cross it. The pieces carry the
      // same transform, so a backend paints them exactly as it paints one line.
      let spans = if brush.decoration_skip_ink == TextDecorationSkipInk::None {
        [(snapped_start_x, snapped_start_x + width)]
          .into_iter()
          .collect()
      } else {
        // The band runs from the content box, so the glyphs have to as well.
        let outlines = self.glyph_outlines(
          resolved_glyphs,
          Point {
            x: layout.border.left + layout.padding.left,
            y: 0.0,
          },
          baseline_shift,
        );

        skip_ink_spans(
          outlines.iter().copied(),
          snapped_start_x,
          snapped_start_x + width,
          y_offset,
          y_offset + height,
        )
      };

      for (start, end) in spans {
        emit(
          start,
          end - start,
          y_offset,
          height,
          false,
          TextDecorationLines::UNDERLINE,
        );
      }
    }
    if lines.contains(TextDecorationLines::OVERLINE) {
      emit(
        snapped_start_x,
        width,
        baseline - metrics.ascent - metrics.underline_offset,
        thickness(metrics.underline_size),
        false,
        TextDecorationLines::OVERLINE,
      );
    }
    if lines.contains(TextDecorationLines::LINE_THROUGH) {
      emit(
        snapped_start_x,
        width,
        baseline - metrics.strikethrough_offset,
        thickness(metrics.strikethrough_size),
        true,
        TextDecorationLines::LINE_THROUGH,
      );
    }
    out
  }
}
