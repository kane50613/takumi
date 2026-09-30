//! A laid-out run's text and glyphs, as the document describes them.

use std::ops::Range;

use crate::{
  geometry::{ComputedLayout, PathCommand, Point},
  layout::inline::{PositionedInlineRun, ProcessedInlineSpan},
  path_data::path_data,
  resources::glyph::{ResolvedBitmapGlyph, ResolvedGlyph},
  style::{Affine, Color},
};

/// How one glyph of a run paints.
pub(super) enum GlyphPaint<'g> {
  /// An outline filled with the run's colour, stroked by `embolden` for faux bold.
  Outline {
    /// The outline.
    paths: &'g [PathCommand],
    /// The faux-bold stroke width.
    embolden: Option<f32>,
  },
  /// Colour font layers, bottom first.
  Layers(Vec<(Color, &'g [PathCommand])>),
  /// An embedded bitmap, such as colour emoji.
  #[cfg_attr(not(feature = "png"), expect(dead_code))]
  Bitmap(&'g ResolvedBitmapGlyph),
}

/// A glyph of a run and where it sits.
pub(super) struct PlacedGlyph<'g> {
  /// Maps the glyph's own space into the block's border box, the line's `text-fit` scale included.
  pub(super) transform: Affine,
  /// The glyph's origin from the run's baseline start, before the line's scale.
  pub(super) offset: Point<f32>,
  /// How it paints.
  pub(super) paint: GlyphPaint<'g>,
}

impl PositionedInlineRun {
  /// The start of the run's baseline in the border box of the block at `layout`, before the
  /// line's `text-fit` scale.
  pub(super) fn origin(&self, layout: ComputedLayout) -> Point<f32> {
    let glyph_offset = self.glyph_offset(layout);

    Point {
      x: glyph_offset.x + self.glyph_run.offset,
      y: glyph_offset.y + self.glyph_run.baseline,
    }
  }

  /// The run's glyphs in the block at `layout`, in order.
  pub(super) fn placed_glyphs(
    &self,
    layout: ComputedLayout,
  ) -> impl Iterator<Item = PlacedGlyph<'_>> {
    let run_transform = self.transform(Affine::IDENTITY);
    let glyph_offset = self.glyph_offset(layout);
    let shaped = &self.glyph_run;

    shaped.glyphs.iter().filter_map(move |glyph| {
      let paint = match self.resolved_glyphs.get(&glyph.id)?.as_ref() {
        ResolvedGlyph::Outline(outline) => {
          let layers = self.resolve_color_layers(outline, shaped.brush.color);

          if layers.is_empty() {
            GlyphPaint::Outline {
              paths: outline.paths(),
              embolden: outline.embolden().filter(|embolden| *embolden > 0.0),
            }
          } else {
            GlyphPaint::Layers(layers)
          }
        }
        ResolvedGlyph::Bitmap(bitmap) => GlyphPaint::Bitmap(bitmap),
      };

      Some(PlacedGlyph {
        transform: run_transform
          * Affine::translation(glyph_offset.x + glyph.x, glyph_offset.y + glyph.y),
        offset: Point {
          x: glyph.x - shaped.offset,
          y: glyph.y - shaped.baseline,
        },
        paint,
      })
    })
  }

  /// The outlines of the glyphs the run's colour fills, as SVG path data from its baseline start.
  pub(super) fn outline(&self, layout: ComputedLayout) -> String {
    self
      .placed_glyphs(layout)
      .filter_map(|glyph| match glyph.paint {
        GlyphPaint::Outline { paths, .. } => Some(path_data(
          paths,
          Affine::translation(glyph.offset.x, glyph.offset.y),
        )),
        _ => None,
      })
      .collect()
  }

  /// The faux-bold stroke width the run's outlines take.
  pub(super) fn embolden(&self, layout: ComputedLayout) -> Option<f32> {
    self
      .placed_glyphs(layout)
      .find_map(|glyph| match glyph.paint {
        GlyphPaint::Outline { embolden, .. } => embolden,
        _ => None,
      })
  }

  /// The run's text without the bidi marks layout inserts.
  pub(super) fn text(&self, text: &str, spans: &[ProcessedInlineSpan<'_>]) -> String {
    let Range { start, end } = self.text_range(spans);
    let end = end.min(text.len());
    let start = text.ceil_char_boundary(start.min(end));
    let end = text.floor_char_boundary(end);

    text[start..end]
      .chars()
      .filter(|c| !matches!(c, '\u{200E}' | '\u{200F}'))
      .collect()
  }

  /// The run's byte range narrowed to the span it was shaped for, since several spans can share
  /// one run.
  fn text_range(&self, spans: &[ProcessedInlineSpan<'_>]) -> Range<usize> {
    let range = self.glyph_run.text_range.clone();

    match self
      .glyph_run
      .brush
      .source_span_id
      .and_then(|id| spans.get(id as usize))
    {
      Some(ProcessedInlineSpan::Text { byte_range, .. }) => {
        range.start.max(byte_range.start)..range.end.min(byte_range.end)
      }
      _ => range,
    }
  }
}
