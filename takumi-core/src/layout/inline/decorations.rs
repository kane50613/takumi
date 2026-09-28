//! Text decoration lines and where skip-ink cuts them.

use crate::{
  geometry::{ComputedLayout, Point},
  layout::intercept::{Spans, skip_ink_ranges},
  resources::glyph::{ResolvedGlyph, ResolvedOutlineGlyph},
  style::{
    Affine, AppliedTextDecoration, Color, SizedTextDecorationThickness, TextDecorationLines,
    TextDecorationSkipInk, TextDecorationStyle,
  },
};
use std::{collections::HashMap, sync::Arc};

use super::runs::ShapedRun;

/// A text decoration line as Blink's `DecorationGeometry` holds it, unsnapped.
pub struct DecorationLine {
  /// Left end and top.
  pub origin: Point<f32>,
  /// Length.
  pub width: f32,
  /// Thickness.
  pub thickness: f32,
  /// Decoration color, already resolved against `current-color`.
  pub color: Color,
  /// Maps the border box into the device's drawing space.
  pub transform: Affine,
  /// Maps the border box onto the output's pixels.
  pub output: Affine,
  /// Whether the line paints above glyphs (line-through) vs below (under/overline).
  pub over: bool,
  /// Which decoration this is, so a backend can single one out.
  pub line: TextDecorationLines,
  /// How the line is drawn.
  pub style: TextDecorationStyle,
  /// The x-ranges `skip-ink` cuts out, sorted.
  pub skips: Spans,
}

impl ShapedRun {
  /// The top and thickness of `decoration`'s `line`, when it draws one, after Blink's
  /// `TextDecorationInfo`.
  pub fn decoration_line(
    &self,
    decoration: &AppliedTextDecoration,
    line: TextDecorationLines,
    baseline_shift: f32,
  ) -> Option<(f32, f32)> {
    if !decoration.line.contains(line) {
      return None;
    }

    let ascent = self.metrics.ascent;
    let thickness = match decoration.thickness {
      SizedTextDecorationThickness::Value(value) => value,
      SizedTextDecorationThickness::FromFont => self.metrics.underline_size,
    };
    let baseline = self.baseline + baseline_shift;
    let top = match line {
      TextDecorationLines::UNDERLINE => {
        baseline + self.underline_offset_from_baseline(thickness, decoration.underline_offset)
      }
      TextDecorationLines::OVERLINE => baseline - ascent - thickness.floor(),
      TextDecorationLines::LINE_THROUGH => baseline - ascent / 3.0 - thickness / 2.0,
      _ => return None,
    };

    Some((top, thickness))
  }

  /// The outlines `skip-ink` gives way to, positioned from `origin`.
  fn ink_outlines<'g>(
    &self,
    resolved_glyphs: &'g HashMap<u32, Arc<ResolvedGlyph>>,
    origin: Point<f32>,
    baseline_shift: f32,
  ) -> Vec<(Point<f32>, &'g ResolvedOutlineGlyph)> {
    self
      .glyphs
      .iter()
      .filter(|glyph| glyph.skips_ink)
      .filter_map(|glyph| {
        let ResolvedGlyph::Outline(outline) = resolved_glyphs.get(&glyph.id)?.as_ref() else {
          return None;
        };

        Some((
          Point {
            x: origin.x + glyph.x,
            y: origin.y + glyph.y + baseline_shift,
          },
          outline,
        ))
      })
      .collect()
  }

  /// The lines the run's `text-decoration` paints, cut by `skip-ink`.
  pub fn decorations(
    &self,
    resolved_glyphs: &HashMap<u32, Arc<ResolvedGlyph>>,
    layout: ComputedLayout,
    baseline_shift: f32,
    transform: Affine,
    device: Affine,
  ) -> Vec<DecorationLine> {
    let content = layout.content_box_offset();
    let mut lines = self.decoration_lines(content, baseline_shift, transform, device * transform);

    if self.brush.decoration_skip_ink == TextDecorationSkipInk::None {
      return lines;
    }

    let mut outlines = None;

    for decoration in lines.iter_mut().filter(|decoration| !decoration.over) {
      let band = decoration.bounds();
      let outlines =
        outlines.get_or_insert_with(|| self.ink_outlines(resolved_glyphs, content, baseline_shift));

      decoration.skips = skip_ink_ranges(
        outlines.iter().copied(),
        band.top,
        band.bottom,
        decoration.thickness,
      );
    }
    lines
  }

  /// The run's decoration lines, uncut, with its content box at `content`.
  pub(crate) fn decoration_lines(
    &self,
    content: Point<f32>,
    baseline_shift: f32,
    transform: Affine,
    output: Affine,
  ) -> Vec<DecorationLine> {
    if self.decorated_advance() <= 0.0 {
      return Vec::new();
    }

    let decorations = self.brush.decorations.as_slice();

    decorations
      .iter()
      .flat_map(|decoration| {
        [
          (TextDecorationLines::UNDERLINE, false),
          (TextDecorationLines::OVERLINE, false),
          (TextDecorationLines::LINE_THROUGH, true),
        ]
        .map(|(line, over)| (decoration, line, over))
      })
      .filter_map(|(decoration, line, over)| {
        let (top, thickness) = self.decoration_line(decoration, line, baseline_shift)?;

        Some(DecorationLine {
          origin: Point {
            x: content.x + self.offset + self.decorated_offset(),
            y: content.y + top,
          },
          width: self.decorated_advance(),
          thickness,
          color: decoration.color,
          transform,
          output,
          over,
          line,
          style: decoration.style,
          skips: Spans::new(),
        })
      })
      .collect()
  }
}
