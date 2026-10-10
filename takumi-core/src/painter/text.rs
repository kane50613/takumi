//! A block's inline text, painted after Blink's `TextFragmentPainter` in the order
//! [css-text-decor-3](https://drafts.csswg.org/css-text-decor-3/#painting-order) gives: shadows,
//! underlines and overlines, text, then line-through.

use std::rc::Rc;

use super::{
  BoxBorderPainter, BoxFrame, FillShape, OpacityLayer, PaintDevice, PaintRole, SnappedBox,
  background::{BackgroundClipArea, BoxBackground},
};
use crate::{
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, Point, Size},
  layout::{
    inline::{
      DecorationLine, DecorationLink, FragmentBackground, InlineBackgroundFragment,
      InlineOutlineRect, InlineRunLayout, OutlineIsland, PositionedInlineRun, ProcessedInlineSpan,
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
  /// The glyphs and their stroke in opaque black, colour glyphs by their alpha, for a
  /// [`GlyphDevice::fill_text_clip`] mask.
  Mask,
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

  /// Paints `background`'s `background-image` layers, clipped to `clip` under `transform`.
  fn fill_background_layers(
    &mut self,
    background: &StripBackground<'_>,
    clip: &FillShape,
    transform: Affine,
  );

  /// Fills `clip` under `transform` with `background`, its colour and layers, only where `mask`
  /// draws, as Blink's `BoxPainterBase::PaintFillLayerTextFillBox` keeps a background under a
  /// `DstIn` text layer.
  fn fill_text_clip(
    &mut self,
    background: &StripBackground<'_>,
    clip: &FillShape,
    transform: Affine,
    mask: &mut dyn FnMut(&mut dyn GlyphDevice),
  );
}

/// A background laid over a strip: an inline span's over the strip its fragments would make on
/// one line, or a box's over its border box.
pub struct StripBackground<'a> {
  /// The span or box.
  pub node: &'a RenderNode,
  /// Unique among the backgrounds one device paints.
  pub id: usize,
  /// Its background.
  pub background: BoxBackground<'a>,
  /// The strip, placed in the block.
  pub strip: BoxFrame,
}

impl StripBackground<'_> {
  /// Fills `clip` under `transform` with the colour, then the layers.
  pub fn fill(&self, clip: &FillShape, transform: Affine, device: &mut dyn GlyphDevice) {
    if let Some(color) = self.background.color {
      device.fill_shape(clip, color, transform);
    }
    if !self.background.layers.is_empty() {
      device.fill_background_layers(self, clip, transform);
    }
  }
}

/// A run as the paint passes see it.
struct PaintedRun<'r> {
  run: &'r PositionedInlineRun,
  decorations: Vec<DecorationLine>,
  style: &'r SizedFontStyle<'r>,
  /// The spans around the run, innermost first.
  chain: Option<&'r DecorationLink<'r>>,
  /// The run's baseline in the block's border box.
  baseline: f32,
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

/// Where an item of a line sits, in its block's border box: the line's baseline, and the item's
/// top and bottom.
#[derive(Clone, Copy)]
pub struct LineItem {
  /// The baseline of the line holding it.
  pub baseline: f32,
  /// Its top.
  pub top: f32,
  /// Its bottom.
  pub bottom: f32,
}

impl LineItem {
  /// `run`'s glyphs in the block at `layout`, between its ascent and descent, or `None` when it
  /// has none.
  pub fn of_run(run: &PositionedInlineRun, layout: ComputedLayout) -> Option<Self> {
    let shaped = &run.glyph_run;
    let baseline = run.glyph_offset(layout).y + shaped.glyphs.first()?.y;

    Some(Self {
      baseline,
      top: baseline - shaped.metrics.ascent,
      bottom: baseline + shaped.metrics.descent,
    })
  }

  /// The item with its block's border box moved down by `y`.
  pub fn below(self, y: f32) -> Self {
    Self {
      baseline: self.baseline + y,
      top: self.top + y,
      bottom: self.bottom + y,
    }
  }
}

impl InlineRunLayout<'_> {
  /// The runs, span backgrounds and outline rects of the block at `layout` that `keep` accepts, as
  /// a page keeps the items it shows.
  pub fn lines(&self, layout: ComputedLayout, keep: impl Fn(LineItem) -> bool) -> InlineLines<'_> {
    InlineLines {
      runs: self
        .runs
        .iter()
        .filter(|run| LineItem::of_run(run, layout).is_none_or(&keep))
        .collect(),
      background_fragments: self
        .background_fragments
        .iter()
        .filter(|fragment| {
          keep(LineItem {
            baseline: fragment.baseline,
            top: fragment.y,
            bottom: fragment.y + fragment.height,
          })
        })
        .collect(),
      outline_rects: self
        .outline_rects
        .iter()
        .copied()
        .filter(|rect| {
          keep(LineItem {
            baseline: rect.y + rect.height / 2.0,
            top: rect.y,
            bottom: rect.y + rect.height,
          })
        })
        .collect(),
    }
  }

  /// Paints every line of the block at `frame`; see [`InlineLines::paint`].
  pub fn paint(
    &self,
    spans: &[ProcessedInlineSpan<'_>],
    style: &SizedFontStyle,
    frame: BoxFrame,
    device: &mut dyn GlyphDevice,
  ) {
    self
      .lines(frame.layout, |_| true)
      .paint(spans, style, frame, device);
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
    frame: BoxFrame,
    device: &mut dyn GlyphDevice,
  ) {
    let runs = self.painted_runs(spans, style, frame, device);

    for fragment in &self.background_fragments {
      device.with_opacity(fragment.opacity, None, |device| {
        if let Some(background) = &fragment.background {
          background.paint(fragment, frame, &runs, device);
        }

        let snapped = fragment.snapped_box();

        device.set_role(PaintRole::Border);
        BoxBorderPainter::new(&fragment.border, snapped.size()).paint(
          Point {
            x: frame.origin.x + fragment.x,
            y: frame.origin.y + fragment.y,
          } + snapped.offset(),
          device,
        );
      });
    }

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
            painted.paint(GlyphFill::Text, frame, *pass, device);
          }

          device.end_shadow();
        }
      }
    }

    for painted in &runs {
      painted.paint(GlyphFill::Text, frame, RunPass::Proper, device);
    }

    for island in OutlineIsland::of(&self.outline_rects) {
      island.paint(frame.origin, device);
    }
  }
}

impl InlineLines<'_> {
  /// Paints the glyphs and decorations of the visible runs of the block at `frame` into a
  /// [`GlyphDevice::fill_text_clip`] mask.
  pub fn paint_mask(
    &self,
    spans: &[ProcessedInlineSpan<'_>],
    style: &SizedFontStyle,
    frame: BoxFrame,
    device: &mut dyn GlyphDevice,
  ) {
    for painted in self.painted_runs(spans, style, frame, device) {
      painted.paint_mask(frame, device);
    }
  }

  /// The runs that paint, `visibility: hidden` ones left out, each in its span's style or `style`.
  fn painted_runs<'r>(
    &'r self,
    spans: &'r [ProcessedInlineSpan<'_>],
    style: &'r SizedFontStyle,
    frame: BoxFrame,
    device: &dyn GlyphDevice,
  ) -> Vec<PaintedRun<'r>> {
    let at = frame.translation();
    let device_transform = device.transform();

    self
      .runs
      .iter()
      .filter_map(|&run| {
        let style = run.style(spans).unwrap_or(style);

        // A run of `visibility: hidden` text keeps its place on the line but paints nothing.
        style.parent.is_visible().then(|| PaintedRun {
          run,
          decorations: run.decorations(frame.layout, at, device_transform),
          style,
          chain: run.span_chain(spans),
          baseline: run.glyph_offset(frame.layout).y + run.glyph_run.baseline,
        })
      })
      .collect()
  }
}

impl<'c> FragmentBackground<'c> {
  /// Whether the span's background shows only through its text.
  fn clips_text(&self) -> bool {
    self.node.context.style.background_data.background_clip == BackgroundClip::Text
  }

  /// The span's background on `fragment` of the block at `frame`.
  fn background(
    &self,
    fragment: &InlineBackgroundFragment,
    frame: BoxFrame,
  ) -> StripBackground<'c> {
    StripBackground {
      node: self.node,
      id: fragment.span,
      background: BoxBackground::new(
        &self.node.context,
        self.strip,
        fragment.border,
        self.node.context.paint_offset + self.strip_origin,
      ),
      strip: BoxFrame::new(self.strip, frame.origin + self.strip_origin),
    }
  }

  /// Paints the color and layers on `fragment` of the block at `frame`, clipped by the span's
  /// `background-clip` to the fragment, as Blink's `BoxPainterBase::PaintFillLayers` clips both.
  /// A background clipped to the text fills the fragment under a mask of the `runs` inside it, as
  /// `BoxPainterBase::PaintFillLayerTextFillBox` does.
  fn paint(
    &self,
    fragment: &InlineBackgroundFragment,
    frame: BoxFrame,
    runs: &[PaintedRun],
    device: &mut dyn GlyphDevice,
  ) {
    let context = &self.node.context;
    let snapped = fragment.snapped_box();
    let at = frame.origin + self.fragment.location + snapped.offset();
    let transform = Affine::translation(at.x, at.y);
    let background = self.background(fragment, frame);

    device.set_role(PaintRole::InlineBackground);

    if self.clips_text() {
      device.fill_text_clip(
        &background,
        &FillShape::Rect(snapped.size()),
        transform,
        &mut |mask| {
          for painted in runs.iter().filter(|painted| painted.lies_in(fragment)) {
            painted.paint_mask(frame, mask);
          }
        },
      );
      return;
    }

    if let Some(clip) = BackgroundClipArea::new(context, self.fragment, fragment.border, &snapped)
      .shape(snapped.size())
    {
      background.fill(&clip, transform, device);
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

  /// The spans around the run, innermost first.
  fn span_chain<'s, 'c>(
    &self,
    spans: &'s [ProcessedInlineSpan<'c>],
  ) -> Option<&'s DecorationLink<'c>> {
    spans
      .get(self.glyph_run.brush.source_span_id? as usize)?
      .text_chain()
      .map(Rc::as_ref)
  }
}

impl PaintedRun<'_> {
  /// Whether the run sits inside `fragment`: within its span, on its line.
  fn lies_in(&self, fragment: &InlineBackgroundFragment) -> bool {
    self.chain.is_some_and(|chain| {
      chain
        .ancestors()
        .any(|link| link.decoration.id == fragment.span)
    }) && (fragment.y..=fragment.y + fragment.height).contains(&self.baseline)
  }

  /// Paints the run's glyphs and decorations into a `background-clip: text` mask, as Blink's
  /// `kTextClip` phase does: in black, without shadows or opacity.
  fn paint_mask(&self, frame: BoxFrame, device: &mut dyn GlyphDevice) {
    for decoration in &self.decorations {
      decoration.paint_mask(device);
    }
    device.draw_glyph_run(self.run, self.style, GlyphFill::Mask, frame);
  }

  /// Paints what `pass` draws of the run at its span's opacity: underline and overline, glyphs
  /// showing `fill`, then line-through. A shadow pass keeps the text-shadow role for everything it
  /// draws.
  fn paint(&self, fill: GlyphFill, frame: BoxFrame, pass: RunPass, device: &mut dyn GlyphDevice) {
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
        device.draw_glyph_run(run, style, fill, frame);
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

impl InlineBackgroundFragment<'_> {
  /// The fragment's border box pixel-snapped where its span paints.
  fn snapped_box(&self) -> SnappedBox {
    SnappedBox::new(
      self.owner.context.paint_offset
        + Point {
          x: self.x,
          y: self.y,
        },
      Size {
        width: self.width,
        height: self.height,
      },
    )
  }
}
