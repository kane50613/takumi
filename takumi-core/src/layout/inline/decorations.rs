//! Text decoration lines and where skip-ink cuts them.

use crate::{
  geometry::{ComputedLayout, Point},
  layout::intercept::{Spans, skip_ink_ranges},
  resources::{
    font::PrimaryFontMetrics,
    glyph::{ResolvedGlyph, ResolvedOutlineGlyph},
  },
  style::{
    Affine, Color, SizedTextDecorationThickness, TextDecorationLines, TextDecorationSkipInk,
    TextDecorationStyle, TextUnderlinePosition,
  },
};
use std::{collections::HashMap, iter::repeat_n, sync::Arc};

use super::{
  BuiltInlineLayout, WalkedLine,
  line_box::{BoxFont, BoxKey},
  runs::ShapedRun,
};

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
  /// Where the border box sits in the space paint snaps to pixels in.
  pub paint_offset: Point<f32>,
  /// Whether the line paints above glyphs (line-through) vs below (under/overline).
  pub over: bool,
  /// Which decoration this is, so a backend can single one out.
  pub line: TextDecorationLines,
  /// How the line is drawn.
  pub style: TextDecorationStyle,
  /// The x-ranges `skip-ink` cuts out, sorted.
  pub skips: Spans,
}

/// Where a run's decoration lines paint: the border box's transform into the device's drawing
/// space, the device's transform onto its pixels, and where the border box sits in paint space.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DecorationSpace {
  pub(crate) transform: Affine,
  pub(crate) device: Affine,
  pub(crate) paint_offset: Point<f32>,
}

/// The font a decorating box measures its decorations with, as Blink's `UsedFont`: its primary
/// font at its size, scaled by `text-fit`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DecorationFont {
  metrics: PrimaryFontMetrics,
  size: f32,
  scale: f32,
}

impl DecorationFont {
  /// The font of a box styled with `font`, scaled by `scale`, or of `run` when the box has no
  /// primary font.
  fn of(font: BoxFont, scale: f32, run: &ShapedRun) -> Self {
    Self {
      metrics: font.metrics.unwrap_or_else(|| run.primary_metrics()),
      size: font.size,
      scale,
    }
  }

  /// Blink's `UsedFont::FloatAscent`.
  fn ascent(self) -> f32 {
    self.metrics.ascent * self.scale
  }

  /// Blink's `TextDecorationThickness::Resolve`, at least a pixel.
  fn thickness(self, thickness: SizedTextDecorationThickness) -> f32 {
    let auto = self.size * self.scale / 10.0;

    match thickness {
      SizedTextDecorationThickness::Auto => auto,
      SizedTextDecorationThickness::FromFont => self
        .metrics
        .underline
        .map_or(auto, |underline| underline.thickness * self.scale),
      SizedTextDecorationThickness::Value(value) => value.round(),
    }
    .max(1.0)
  }

  /// The underline's top below the box's top, after Blink's
  /// `TextDecorationOffset::ComputeUnderlineOffset`.
  fn underline_offset(
    self,
    position: TextUnderlinePosition,
    offset: Option<f32>,
    thickness: f32,
  ) -> f32 {
    let style_offset = offset.unwrap_or(0.0);
    let auto = || {
      let gap = match offset {
        Some(_) => 0.0,
        None => (thickness / 2.0).ceil().max(1.0),
      };

      (self.ascent() + gap + style_offset.round()).trunc()
    };

    match position {
      TextUnderlinePosition::Auto => auto(),
      TextUnderlinePosition::FromFont => self.metrics.underline.map_or_else(auto, |underline| {
        ((self.metrics.ascent + underline.position) * self.scale + style_offset).round()
      }),
      TextUnderlinePosition::Under => {
        (layout_unit((self.metrics.ascent + self.metrics.em_descent) * self.scale)
          + layout_unit(style_offset))
        .floor()
          + 1.0
      }
    }
  }
}

/// `value` rounded to Blink's `LayoutUnit`, a 64th of a pixel.
fn layout_unit(value: f32) -> f32 {
  (value * 64.0).round() / 64.0
}

/// A box whose decoration a run paints, as Blink's `DecoratingBox`: the font it measures the
/// decoration with and where its baseline sits.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DecoratingBox {
  font: DecorationFont,
  baseline: f32,
}

impl DecoratingBox {
  /// Blink's `DecoratingBox::ContentOffsetInContainer`, the top of its content area.
  fn top(self) -> f32 {
    self.baseline - self.font.ascent()
  }
}

/// Where a run's decorations go, after Blink's `TextDecorationInfo`: in the border box, which
/// `text-fit` leaves unscaled, against the run's text and the boxes that decorate it.
#[derive(Clone, Debug, Default)]
pub(crate) struct DecorationPlacement {
  /// Maps the run's glyphs into the border box.
  glyphs: Affine,
  /// Where the run's text baseline lands.
  baseline: f32,
  /// Top of the run's text.
  text_top: f32,
  /// The decorated span's left end.
  left: f32,
  /// The decorated span's width.
  width: f32,
  /// The box each of the run's decorations sits against, outermost first.
  boxes: Vec<DecoratingBox>,
}

impl BuiltInlineLayout<'_> {
  /// Where `run`, shaped from `glyph_run`'s spans and placed after `static_inline_prefix` of
  /// static advance on `line`, paints its decorations in `layout`.
  pub(crate) fn decoration_placement(
    &self,
    line: &WalkedLine,
    run: &ShapedRun,
    source_span_id: Option<u64>,
    static_inline_prefix: f32,
    layout: ComputedLayout,
  ) -> DecorationPlacement {
    let decorations = run.brush.decorations.as_slice();

    if decorations.is_empty() {
      return DecorationPlacement::default();
    }

    let chain = self.span_chain(source_span_id);
    let state = line.setup.run_scale(line.baseline_shift_in(chain));
    let glyphs = state.transform(Affine::IDENTITY, static_inline_prefix);
    let content = layout.content_box_offset();
    let line_baseline = content.y + line.setup.resolved_metrics.resolved_baseline;
    let root = DecoratingBox {
      font: DecorationFont::of(self.font, state.scale, run),
      baseline: line_baseline,
    };
    let mut spans: Vec<DecoratingBox> = chain
      .into_iter()
      .flat_map(|link| link.ancestors())
      .filter(|link| {
        link
          .decoration
          .owner
          .context
          .style
          .text_decoration_line
          .is_some_and(|line| !line.is_empty())
      })
      .map(|link| DecoratingBox {
        font: DecorationFont::of(link.decoration.font, 1.0, run),
        baseline: line_baseline + line.state.offsets.of(BoxKey::Span(link.decoration.id)),
      })
      .collect();

    spans.reverse();

    let text_font = chain.map_or(self.font, |link| link.decoration.font);
    let text = DecorationFont::of(text_font, state.scale, run);
    let baseline = content.y + run.baseline + line.baseline_shift_in(chain);
    let left = content.x + run.offset + run.decorated_offset();
    let (left, _) = glyphs.transform_point(left, 0.0);
    let (right, _) = glyphs.transform_point(
      content.x + run.offset + run.decorated_offset() + run.decorated_advance(),
      0.0,
    );

    DecorationPlacement {
      glyphs,
      baseline,
      text_top: baseline - text.ascent(),
      left,
      width: right - left,
      boxes: repeat_n(root, decorations.len().saturating_sub(spans.len()))
        .chain(spans)
        .collect(),
    }
  }
}

impl ShapedRun {
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

  /// The lines the run's `text-decoration` paints at `placement`, cut by `skip-ink`, as Blink's
  /// `TextPainter::ClipDecorationsStripe` cuts them from the unscaled glyphs.
  pub(crate) fn decorations(
    &self,
    resolved_glyphs: &HashMap<u32, Arc<ResolvedGlyph>>,
    layout: ComputedLayout,
    placement: &DecorationPlacement,
    baseline_shift: f32,
    space: DecorationSpace,
  ) -> Vec<DecorationLine> {
    let DecorationSpace {
      transform,
      device,
      paint_offset,
    } = space;
    let content = layout.content_box_offset();
    let mut lines = self.decoration_lines(placement, transform, device * transform, paint_offset);

    if self.brush.decoration_skip_ink == TextDecorationSkipInk::None {
      return lines;
    }

    let glyph_baseline = content.y + self.baseline + baseline_shift;
    let mut outlines = None;

    for decoration in lines.iter_mut().filter(|decoration| !decoration.over) {
      let band = decoration.bounds();
      let outlines =
        outlines.get_or_insert_with(|| self.ink_outlines(resolved_glyphs, content, baseline_shift));

      decoration.skips = skip_ink_ranges(
        outlines.iter().copied(),
        glyph_baseline + band.top - placement.baseline,
        glyph_baseline + band.bottom - placement.baseline,
        decoration.thickness,
      )
      .into_iter()
      .map(|(start, end)| {
        (
          placement.glyphs.transform_point(start, 0.0).0,
          placement.glyphs.transform_point(end, 0.0).0,
        )
      })
      .collect();
    }
    lines
  }

  /// The run's decoration lines at `placement`, uncut.
  pub(crate) fn decoration_lines(
    &self,
    placement: &DecorationPlacement,
    transform: Affine,
    output: Affine,
    paint_offset: Point<f32>,
  ) -> Vec<DecorationLine> {
    if self.decorated_advance() <= 0.0 {
      return Vec::new();
    }

    self
      .brush
      .decorations
      .as_slice()
      .iter()
      .zip(&placement.boxes)
      .flat_map(|(decoration, decorating)| {
        let thickness = decorating.font.thickness(decoration.thickness);

        [
          (TextDecorationLines::UNDERLINE, false),
          (TextDecorationLines::OVERLINE, false),
          (TextDecorationLines::LINE_THROUGH, true),
        ]
        .into_iter()
        .filter(|(line, _)| decoration.line.contains(*line))
        .map(move |(line, over)| {
          let top = match line {
            TextDecorationLines::UNDERLINE => {
              decorating.top()
                + decorating.font.underline_offset(
                  decoration.underline_position,
                  decoration.underline_offset,
                  thickness,
                )
            }
            TextDecorationLines::OVERLINE => placement.text_top - thickness.floor(),
            _ => placement.text_top + 2.0 * decorating.font.ascent() / 3.0 - thickness / 2.0,
          };

          DecorationLine {
            origin: Point {
              x: placement.left,
              y: top,
            },
            width: placement.width,
            thickness,
            color: decoration.color,
            transform,
            output,
            paint_offset,
            over,
            line,
            style: decoration.style,
            skips: Spans::new(),
          }
        })
      })
      .collect()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::resources::font::{ExactFontMetrics, FontUnderline};

  fn font(scale: f32) -> DecorationFont {
    DecorationFont {
      metrics: PrimaryFontMetrics {
        ascent: 40.0,
        descent: 10.0,
        line_gap: 0.0,
        x_height: None,
        exact: ExactFontMetrics {
          ascent: 40.0,
          descent: 10.0,
          line_gap: 0.0,
        },
        underline: Some(FontUnderline {
          position: 5.0,
          thickness: 2.0,
        }),
        em_descent: 20.0,
      },
      size: 100.0,
      scale,
    }
  }

  #[test]
  fn an_underline_sits_by_its_position_below_the_top() {
    // `auto` leaves a gap of half the thickness, at least a pixel, under the baseline.
    assert_eq!(
      font(1.0).underline_offset(TextUnderlinePosition::Auto, None, 1.0),
      41.0
    );
    assert_eq!(
      font(1.0).underline_offset(TextUnderlinePosition::Auto, None, 5.0),
      43.0
    );
    assert_eq!(
      font(1.0).underline_offset(TextUnderlinePosition::FromFont, None, 2.0),
      45.0
    );
    // A pixel past the bottom of the em box.
    assert_eq!(
      font(1.0).underline_offset(TextUnderlinePosition::Under, None, 2.0),
      61.0
    );
  }

  #[test]
  fn a_set_underline_offset_drops_the_auto_gap() {
    assert_eq!(
      font(1.0).underline_offset(TextUnderlinePosition::Auto, Some(3.0), 4.0),
      43.0
    );
    assert_eq!(
      font(1.0).underline_offset(TextUnderlinePosition::Under, Some(-4.0), 2.0),
      57.0
    );
  }

  #[test]
  fn text_fit_measures_decorations_with_the_scaled_font() {
    let font = font(1.5);
    let thickness = font.thickness(SizedTextDecorationThickness::Auto);

    assert_eq!(thickness, 15.0);
    assert_eq!(
      font.underline_offset(TextUnderlinePosition::Auto, None, thickness),
      68.0
    );
    assert_eq!(
      font.thickness(SizedTextDecorationThickness::Value(2.4)),
      2.0
    );
  }
}
