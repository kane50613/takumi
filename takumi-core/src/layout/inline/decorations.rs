//! Text decoration lines and where skip-ink cuts them.

use crate::{
  geometry::{ComputedLayout, Point},
  layout::intercept::{Spans, skip_ink_ranges},
  resources::glyph::{ResolvedGlyph, ResolvedOutlineGlyph},
  style::{
    Affine, Color, SizedTextDecorationThickness, TextDecorationLines, TextDecorationSkipInk,
    TextDecorationStyle,
  },
};
use std::{collections::HashMap, sync::Arc};

use super::runs::ShapedRun;

/// A text decoration line as Blink's `DecorationGeometry` holds it: unsnapped, in the block's
/// border box.
pub struct DecorationLine {
  /// The line's left end and top.
  pub origin: Point<f32>,
  /// The line's length.
  pub width: f32,
  /// The line's thickness.
  pub thickness: f32,
  /// Decoration color, already resolved against `current-color`.
  pub color: Color,
  /// Maps the border box into the device's drawing space.
  pub transform: Affine,
  /// Maps the border box onto the output's pixels, where skip-ink rounds its cuts.
  pub output: Affine,
  /// Whether the line paints above glyphs (line-through) vs below (under/overline).
  pub over: bool,
  /// Which decoration this is, so a backend can single one out.
  pub line: TextDecorationLines,
  /// How the line is drawn.
  pub style: TextDecorationStyle,
  /// The x-ranges `text-decoration-skip-ink` cuts out of the line, sorted.
  pub skips: Spans,
}

impl ShapedRun {
  /// The top and thickness of an enabled decoration line, after Blink's `TextDecorationInfo`: an
  /// underline where [`ShapedRun::underline_offset_from_baseline`] puts it, an overline resting
  /// on the text's top, and a line-through centred a third of the ascent above the baseline.
  /// `from-font` takes the font's underline thickness for every line.
  pub fn decoration_line(
    &self,
    line: TextDecorationLines,
    baseline_shift: f32,
  ) -> Option<(f32, f32)> {
    if !self.brush.decoration_line.contains(line) {
      return None;
    }

    let ascent = self.metrics.ascent;
    let thickness = match self.brush.decoration_thickness {
      SizedTextDecorationThickness::Value(value) => value,
      SizedTextDecorationThickness::FromFont => self.metrics.underline_size,
    };
    let baseline = self.baseline + baseline_shift;
    let top = match line {
      TextDecorationLines::UNDERLINE => baseline + self.underline_offset_from_baseline(thickness),
      TextDecorationLines::OVERLINE => baseline - ascent - thickness.floor(),
      TextDecorationLines::LINE_THROUGH => baseline - ascent / 3.0 - thickness / 2.0,
      _ => return None,
    };

    Some((top, thickness))
  }

  /// The outlines of the run's glyphs that `skip-ink` gives way to, positioned from `origin`.
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

  /// The lines the run's `text-decoration` paints, `transform` mapping the block's border box
  /// into the device's drawing space and `device` mapping that space onto the output. `skip-ink`
  /// cuts an underline and an overline, not a line-through.
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
    let brush = &self.brush;
    // A fully trimmed run paints no decoration.
    if brush.decoration_line.is_empty() || self.decorated_advance() <= 0.0 {
      return Vec::new();
    }

    [
      (TextDecorationLines::UNDERLINE, false),
      (TextDecorationLines::OVERLINE, false),
      (TextDecorationLines::LINE_THROUGH, true),
    ]
    .into_iter()
    .filter_map(|(line, over)| {
      let (top, thickness) = self.decoration_line(line, baseline_shift)?;

      Some(DecorationLine {
        origin: Point {
          x: content.x + self.offset,
          y: content.y + top,
        },
        width: self.decorated_advance(),
        thickness,
        color: brush.decoration_color,
        transform,
        output,
        over,
        line,
        style: brush.decoration_style,
        skips: Spans::new(),
      })
    })
    .collect()
  }
}
