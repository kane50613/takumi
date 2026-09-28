//! A block's inline text, painted after Blink's `TextFragmentPainter` in the order
//! [css-text-decor-3](https://drafts.csswg.org/css-text-decor-3/#painting-order) gives: shadows,
//! underlines and overlines, text, then line-through.

use super::{
  BoxBorderPainter, BoxFrame, FillShape, OpacityLayer, PaintDevice, PaintRole,
  background::{BackgroundClipArea, BoxBackground},
};
use crate::{
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, Point, Size},
  layout::{
    inline::{
      DecorationLine, FragmentBackground, InlineBackgroundFragment, InlineOutlineRect,
      InlineRunLayout, OutlineIsland, PositionedInlineRun, ProcessedInlineSpan,
    },
    tree::RenderNode,
  },
  shadow::SizedShadow,
  style::{Affine, BackgroundClip},
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

  /// Paints `span`'s `background-image` layers, clipped to `clip` under `transform`.
  fn fill_background_layers(
    &mut self,
    span: &SpanBackground<'_>,
    clip: &FillShape,
    transform: Affine,
  );

  /// Draws `run`'s glyphs in the block at `frame` as [`GlyphFill::Background`] draws them, showing
  /// `span`'s background, its colour included, in place of the block's.
  fn draw_glyph_run_through(
    &mut self,
    run: &PositionedInlineRun,
    style: &SizedFontStyle,
    frame: BoxFrame,
    span: &SpanBackground<'_>,
  );
}

/// An inline span's background, laid over the strip its fragments would make on one line.
pub struct SpanBackground<'a> {
  /// The span.
  pub node: &'a RenderNode,
  /// The span's id, unique among the spans of its inline layout.
  pub span: usize,
  /// Its background.
  pub background: BoxBackground<'a>,
  /// The strip, placed in the block.
  pub strip: BoxFrame,
}

/// A run as the paint passes see it.
struct PaintedRun<'r> {
  run: &'r PositionedInlineRun,
  decorations: Vec<DecorationLine>,
  style: &'r SizedFontStyle<'r>,
  /// The background of a span around the run with `background-clip: text`.
  span: Option<SpanBackground<'r>>,
}

/// What a run's glyphs show.
#[derive(Clone, Copy)]
enum RunFill<'a> {
  /// What the device fills glyphs with.
  Glyphs(GlyphFill),
  /// The background of a span with `background-clip: text`.
  Span(&'a SpanBackground<'a>),
}

/// A pass over a line's runs, and which of their pieces it draws.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RunPass {
  /// Decorations and glyphs themselves.
  Proper,
  /// The shadow of decorations and glyphs.
  Shadow,
  /// The shadow of decorations alone.
  DecorationShadow,
  /// The shadow of glyphs alone.
  GlyphShadow,
}

impl RunPass {
  fn paints_decorations(self) -> bool {
    self != Self::GlyphShadow
  }

  fn paints_glyphs(self) -> bool {
    self != Self::DecorationShadow
  }
}

/// Some of an inline layout's lines: the runs, span backgrounds, and outline rects on them.
pub struct InlineLines<'l> {
  runs: Vec<&'l PositionedInlineRun>,
  background_fragments: Vec<&'l InlineBackgroundFragment<'l>>,
  outline_rects: Vec<InlineOutlineRect>,
}

impl InlineRunLayout<'_> {
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
  pub fn paint(
    &self,
    spans: &[ProcessedInlineSpan<'_>],
    style: &SizedFontStyle,
    fill: GlyphFill,
    frame: BoxFrame,
    device: &mut dyn GlyphDevice,
  ) {
    self
      .lines(frame.layout, |_| true)
      .paint(spans, style, fill, frame, device);
  }
}

impl InlineLines<'_> {
  /// Paints the text of the block at `frame`: span backgrounds and borders, the element's text shadows, each
  /// run's underline and overline, glyphs and line-through, then the spans' outlines.
  ///
  /// The shadows all paint before any text, so a shadow never lands on a neighbouring run's
  /// glyphs, as css-text-decor-3 asks of `text-shadow`.
  pub fn paint(
    &self,
    spans: &[ProcessedInlineSpan<'_>],
    style: &SizedFontStyle,
    fill: GlyphFill,
    frame: BoxFrame,
    device: &mut dyn GlyphDevice,
  ) {
    let at = frame.translation();

    for fragment in &self.background_fragments {
      device.with_opacity(fragment.opacity, None, |device| {
        if let Some(background) = &fragment.background {
          background.paint(fragment, frame, device);
        }

        device.set_role(PaintRole::Border);
        BoxBorderPainter::new(
          &fragment.border,
          Size {
            width: fragment.width,
            height: fragment.height,
          },
        )
        .paint(
          Point {
            x: frame.origin.x + fragment.x,
            y: frame.origin.y + fragment.y,
          },
          device,
        );
      });
    }

    let device_transform = device.transform();
    let runs: Vec<PaintedRun> = self
      .runs
      .iter()
      .filter_map(|&run| {
        let style = run.style(spans).unwrap_or(style);

        // A run of `visibility: hidden` text keeps its place on the line but paints nothing.
        style.parent.is_visible().then(|| PaintedRun {
          run,
          decorations: run.decorations(frame.layout, at, device_transform),
          style,
          span: self.span_background(run, spans, frame),
        })
      })
      .collect();

    // Neighbouring runs that cast the same shadows at the same `text-fit` scale share each shadow
    // pass, so the passes stay as few as the element's distinct `text-shadow` lists.
    for batch in runs.chunk_by(|left, right| {
      left.run.line_scale.scale == right.run.line_scale.scale
        && left
          .style
          .painted_text_shadows()
          .eq(right.style.painted_text_shadows())
    }) {
      let Some(first) = batch.first() else {
        continue;
      };
      let scale = first.run.line_scale.scale;

      for shadow in first.style.painted_text_shadows() {
        device.set_role(PaintRole::TextShadow);

        // Blink paints a scaled line's glyphs, shadows included, through `text-fit`'s scale, and
        // its decorations outside it.
        let passes: &[(SizedShadow, RunPass)] = if scale == 1.0 {
          &[(*shadow, RunPass::Shadow)]
        } else {
          &[
            (*shadow, RunPass::DecorationShadow),
            (shadow.scaled(scale), RunPass::GlyphShadow),
          ]
        };

        for (shadow, pass) in passes {
          device.begin_shadow(shadow);

          for painted in batch {
            painted.paint(RunFill::Glyphs(GlyphFill::Text), frame, *pass, device);
          }

          device.end_shadow();
        }
      }
    }

    for painted in &runs {
      let fill = painted
        .span
        .as_ref()
        .map_or(RunFill::Glyphs(fill), RunFill::Span);

      painted.paint(fill, frame, RunPass::Proper, device);
    }

    for island in OutlineIsland::of(&self.outline_rects) {
      island.paint(frame.origin, device);
    }
  }
}

impl<'l> InlineLines<'l> {
  /// The background `run`'s glyphs show when a span around it sets `background-clip: text`: the
  /// innermost such span's on the run's line, in the block at `frame`.
  fn span_background(
    &self,
    run: &PositionedInlineRun,
    spans: &[ProcessedInlineSpan<'_>],
    frame: BoxFrame,
  ) -> Option<SpanBackground<'l>> {
    let Some(ProcessedInlineSpan::Text {
      decorations: Some(chain),
      ..
    }) = spans.get(run.glyph_run.brush.source_span_id? as usize)
    else {
      return None;
    };
    let span = chain
      .ancestors()
      .find(|link| link.decoration.owner.context.style.background_clip == BackgroundClip::Text)?;
    let baseline = run.glyph_offset(frame.layout).y + run.glyph_run.baseline;

    self
      .background_fragments
      .iter()
      .find(|fragment| {
        fragment.span == span.decoration.id
          && (fragment.y..=fragment.y + fragment.height).contains(&baseline)
      })
      .and_then(|fragment| Some(fragment.background?.background(fragment, frame)))
  }
}

impl<'c> FragmentBackground<'c> {
  /// Whether the span's background shows only through its text.
  fn clips_text(&self) -> bool {
    self.node.context.style.background_clip == BackgroundClip::Text
  }

  /// The span's background on `fragment` of the block at `frame`.
  fn background(&self, fragment: &InlineBackgroundFragment, frame: BoxFrame) -> SpanBackground<'c> {
    SpanBackground {
      node: self.node,
      span: fragment.span,
      background: BoxBackground::new(
        &self.node.context,
        self.strip,
        fragment.border,
        self.strip_origin,
      ),
      strip: BoxFrame::new(self.strip, frame.origin + self.strip_origin),
    }
  }

  /// Paints the color and layers on `fragment` of the block at `frame`, clipped by the span's
  /// `background-clip` to the fragment, as Blink's `BoxPainterBase::PaintFillLayers` clips both.
  /// A background clipped to the text shows through the glyphs instead.
  fn paint(
    &self,
    fragment: &InlineBackgroundFragment,
    frame: BoxFrame,
    device: &mut dyn GlyphDevice,
  ) {
    if self.clips_text() {
      return;
    }

    let context = &self.node.context;
    let Some(clip) =
      BackgroundClipArea::new(context, self.fragment, fragment.border).shape(self.fragment.size)
    else {
      return;
    };
    let transform = Affine::translation(
      frame.origin.x + self.fragment.location.x,
      frame.origin.y + self.fragment.location.y,
    );

    device.set_role(PaintRole::InlineBackground);
    if fragment.color.0[3] != 0 {
      device.fill_shape(&clip, fragment.color, transform);
    }
    if self.has_layers() {
      device.fill_background_layers(&self.background(fragment, frame), &clip, transform);
    }
  }

  /// Whether the span has `background-image` layers.
  fn has_layers(&self) -> bool {
    self
      .node
      .context
      .style
      .background_image
      .as_deref()
      .is_some_and(|images| !images.is_empty())
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
}

impl PaintedRun<'_> {
  /// Paints what `pass` draws of the run at its span's opacity: underline and overline, glyphs
  /// showing `fill`, then line-through. A shadow pass keeps the text-shadow role for everything it
  /// draws.
  fn paint(&self, fill: RunFill<'_>, frame: BoxFrame, pass: RunPass, device: &mut dyn GlyphDevice) {
    let PaintedRun {
      run,
      decorations,
      style,
      ..
    } = self;
    let shadow_pass = pass != RunPass::Proper;
    let paints_decorations = pass.paints_decorations();

    device.with_opacity(run.glyph_run.brush.opacity, None, |device| {
      if paints_decorations {
        if !shadow_pass {
          device.set_role(PaintRole::TextDecoration);
        }
        for decoration in decorations.iter().filter(|decoration| !decoration.over) {
          decoration.paint(device);
        }
      }

      if pass.paints_glyphs() {
        if !shadow_pass {
          device.set_role(PaintRole::Text);
        }
        match fill {
          RunFill::Glyphs(fill) => device.draw_glyph_run(run, style, fill, frame),
          RunFill::Span(span) => device.draw_glyph_run_through(run, style, frame, span),
        }
      }

      if paints_decorations {
        if !shadow_pass {
          device.set_role(PaintRole::TextDecoration);
        }
        for decoration in decorations.iter().filter(|decoration| decoration.over) {
          decoration.paint(device);
        }
      }
    });
  }
}
