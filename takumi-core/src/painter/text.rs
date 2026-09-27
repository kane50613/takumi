//! A block's inline text, painted after Blink's `TextFragmentPainter` in the order
//! [css-text-decor-3](https://drafts.csswg.org/css-text-decor-3/#painting-order) gives: shadows,
//! underlines and overlines, text, then line-through.

use super::{BoxFrame, FillShape, PaintDevice, PaintRole};
use crate::{
  font_style::SizedFontStyle,
  geometry::ComputedLayout,
  layout::inline::{
    DecorationRect, InlineBackgroundFragment, InlineOutlineRect, InlineRunLayout, OutlineIsland,
    PositionedInlineRun, ProcessedInlineSpan,
  },
  shadow::SizedShadow,
  style::{Affine, FillRule},
};

/// What a device fills a run's glyphs with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphFill {
  /// The run's own paint: its colour or the font's colour layers, faux bold, and
  /// `-webkit-text-stroke`.
  Text,
  /// The box's background seen through the glyphs and their stroke, for
  /// `background-clip: text`, under the run's own paint.
  Background,
}

/// A device that can also draw text: glyph runs, and the shadows text and its decorations cast.
pub trait GlyphDevice: PaintDevice {
  /// Paints only the shadow of what is drawn until the matching [`GlyphDevice::end_shadow`]:
  /// each draw moved by the shadow's offset, filled with its colour, and blurred.
  ///
  /// Approximate: bitmap glyphs such as colour emoji cast no shadow, where Blink shadows their
  /// alpha.
  fn begin_shadow(&mut self, shadow: &SizedShadow);

  /// Stops painting shadows.
  fn end_shadow(&mut self);

  /// Draws `run`'s glyphs in the block at `frame`, filled as `fill` says.
  fn draw_glyph_run(
    &mut self,
    run: &PositionedInlineRun,
    style: &SizedFontStyle,
    fill: GlyphFill,
    frame: BoxFrame,
  );
}

/// Some of an inline layout's lines: the runs, span backgrounds, and outline rects on them.
pub struct InlineLines<'l> {
  runs: Vec<&'l PositionedInlineRun>,
  background_fragments: Vec<&'l InlineBackgroundFragment>,
  outline_rects: Vec<InlineOutlineRect>,
}

impl InlineRunLayout {
  /// The lines of the block at `layout` whose baseline, in its border box, `keep` accepts, as a
  /// page keeps the lines it owns.
  pub fn lines(&self, layout: ComputedLayout, keep: impl Fn(f32) -> bool) -> InlineLines<'_> {
    InlineLines {
      runs: self
        .runs
        .iter()
        .filter(|run| {
          run
            .glyph_run
            .glyphs
            .first()
            .is_none_or(|glyph| keep(run.glyph_offset(layout).y + glyph.y))
        })
        .collect(),
      background_fragments: self
        .background_fragments
        .iter()
        .filter(|fragment| keep(fragment.baseline))
        .collect(),
      outline_rects: self
        .outline_rects
        .iter()
        .copied()
        .filter(|rect| keep(rect.y + rect.height / 2.0))
        .collect(),
    }
  }

  /// Paints every line of the block at `frame`; see [`InlineLines::paint`].
  pub fn paint<D: GlyphDevice>(
    &self,
    spans: &[ProcessedInlineSpan<'_>],
    style: &SizedFontStyle,
    fill: GlyphFill,
    frame: BoxFrame,
    device: &mut D,
  ) {
    self
      .lines(frame.layout, |_| true)
      .paint(spans, style, fill, frame, device);
  }
}

impl InlineLines<'_> {
  /// Paints the text of the block at `frame`: span backgrounds, the element's text shadows, each
  /// run's underline and overline, glyphs and line-through, then the spans' outlines.
  ///
  /// The shadows all paint before any text, so a shadow never lands on a neighbouring run's
  /// glyphs, as css-text-decor-3 asks of `text-shadow`.
  pub fn paint<D: GlyphDevice>(
    &self,
    spans: &[ProcessedInlineSpan<'_>],
    style: &SizedFontStyle,
    fill: GlyphFill,
    frame: BoxFrame,
    device: &mut D,
  ) {
    let at = frame.translation();

    device.set_role(PaintRole::InlineBackground);

    for fragment in &self.background_fragments {
      device.with_opacity(fragment.opacity, |device| {
        device.fill_shape(
          &FillShape::Path {
            commands: fragment.path(),
            rule: FillRule::NonZero,
          },
          fragment.color,
          at,
        );
      });
    }

    let decorations: Vec<Vec<DecorationRect>> = self
      .runs
      .iter()
      .map(|run| {
        run.glyph_run.decorations(
          &run.resolved_glyphs,
          frame.layout,
          run.baseline_shift,
          run.transform(Affine::IDENTITY),
        )
      })
      .collect();

    let styles: Vec<&SizedFontStyle> = self
      .runs
      .iter()
      .map(|run| run.style(spans).unwrap_or(style))
      .collect();
    // A run of `visibility: hidden` text keeps its place on the line but paints nothing.
    let runs: Vec<_> = self
      .runs
      .iter()
      .zip(&decorations)
      .zip(&styles)
      .filter(|(_, style)| style.parent.is_visible())
      .collect();

    // Neighbouring runs that cast the same shadows share each shadow pass, so the passes stay as
    // few as the element's distinct `text-shadow` lists.
    for batch in runs.chunk_by(|(_, left), (_, right)| {
      left.painted_text_shadows().eq(right.painted_text_shadows())
    }) {
      let Some((_, first)) = batch.first() else {
        continue;
      };

      for shadow in first.painted_text_shadows() {
        device.set_role(PaintRole::TextShadow);
        device.begin_shadow(shadow);

        for ((run, decorations), style) in batch {
          run.paint(decorations, style, GlyphFill::Text, frame, true, device);
        }

        device.end_shadow();
      }
    }

    for ((run, decorations), style) in &runs {
      run.paint(decorations, style, fill, frame, false, device);
    }

    for island in OutlineIsland::of(self.outline_rects.clone()) {
      island.paint(spans, frame.origin, device);
    }
  }
}

impl PositionedInlineRun {
  /// The style of the span the run came from, when it came from one.
  pub(crate) fn style<'s>(
    &self,
    spans: &'s [ProcessedInlineSpan<'_>],
  ) -> Option<&'s SizedFontStyle<'s>> {
    let span_id = self.glyph_run.brush.source_span_id?;

    match spans.get(span_id as usize)? {
      ProcessedInlineSpan::Text { style, .. } => Some(style),
      _ => None,
    }
  }

  /// Paints the run at its span's opacity: underline and overline, glyphs, then line-through.
  /// A shadow pass keeps the text-shadow role for everything it draws.
  fn paint<D: GlyphDevice>(
    &self,
    decorations: &[DecorationRect],
    style: &SizedFontStyle,
    fill: GlyphFill,
    frame: BoxFrame,
    shadow_pass: bool,
    device: &mut D,
  ) {
    device.with_opacity(self.glyph_run.brush.opacity, |device| {
      if !shadow_pass {
        device.set_role(PaintRole::TextDecoration);
      }
      for decoration in decorations.iter().filter(|decoration| !decoration.over) {
        decoration.paint(frame.origin, device);
      }

      if !shadow_pass {
        device.set_role(PaintRole::Text);
      }
      device.draw_glyph_run(self, style, fill, frame);

      if !shadow_pass {
        device.set_role(PaintRole::TextDecoration);
      }
      for decoration in decorations.iter().filter(|decoration| decoration.over) {
        decoration.paint(frame.origin, device);
      }
    });
  }
}
