//! Shaped glyph runs resolved for painting and measuring.

use crate::{
  geometry::{ComputedLayout, PathCommand, Point},
  layout::{intercept::skips_ink, list_marker::SymbolMarker},
  resources::{
    font::{
      ExactFontMetrics, FontBlob, FontUnderline, PrimaryFontMetrics, RunFace, RunSynthesis,
      em_box_descent, zero_inline_size,
    },
    glyph::{ResolvedGlyphs, ResolvedOutlineGlyph},
  },
  style::{Affine, Color},
};
use skrifa::{FontRef, MetadataProvider, instance::LocationRef, raw::types::F2Dot14};
use std::{ops::Range, sync::Arc};

use super::{
  InlineBrush,
  background::InlineBackgroundFragment,
  decorations::{DecorationLine, DecorationPlacement, DecorationSpace},
  metrics::VisualInlineBox,
  outline::InlineOutlineRect,
  text_fit::LineScaleState,
};

/// A shaped glyph positioned within its run, in run-local coordinates.
#[derive(Clone, Copy, Debug)]
pub struct PositionedGlyph {
  /// Glyph id in the run's font.
  pub id: u32,
  /// Horizontal position from the line origin, the run's own offset included.
  pub x: f32,
  /// Vertical position from the line origin, the run's baseline included.
  pub y: f32,
  /// Whether `text-decoration-skip-ink` cuts a decoration around the glyph.
  pub skips_ink: bool,
}

/// Vertical font metrics for a shaped run, in px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunMetrics {
  /// Typographic ascent.
  pub ascent: f32,
  /// Typographic descent.
  pub descent: f32,
  /// Height of the run's leaded box: the used line height, grown to a fallback face's own
  /// height under `line-height: normal`.
  pub line_height: f32,
  /// Underline offset from the baseline.
  pub underline_offset: f32,
  /// Underline stroke thickness.
  pub underline_size: f32,
}

/// Whether `text-decoration-skip-ink` cuts a decoration around a glyph of the non-emoji cluster
/// `cluster` of `text`.
pub(super) fn glyph_skips_ink(text: &str, cluster: Range<usize>) -> bool {
  text
    .get(cluster)
    .and_then(|text| text.chars().next())
    .is_none_or(skips_ink)
}

/// What every [`ShapedRun::same_face`] pair agrees on, which narrows the faces worth comparing.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct FaceKey {
  font_id: u64,
  font_index: u32,
  font_size_bits: u32,
  source_span_id: Option<u64>,
}

/// A shaped, positioned glyph run, which Blink's `TextFragmentPainter` paints. Owns everything
/// both backends need to paint a run.
#[derive(Clone)]
pub struct ShapedRun {
  /// The run's glyphs, positioned from the line origin.
  pub glyphs: Vec<PositionedGlyph>,
  /// The run's start from the line origin, for what the glyphs do not place, such as decorations.
  pub offset: f32,
  /// Baseline position within the line.
  pub baseline: f32,
  /// Total horizontal advance of the run.
  pub advance: f32,
  /// Paint attributes carried by the run.
  pub brush: InlineBrush,
  /// Vertical font metrics for the run.
  pub metrics: RunMetrics,
  /// Font size the run was shaped at, in pixels.
  pub font_size: f32,
  /// Collection index for `skrifa::FontRef::from_index`, paired with [`Self::font_data`].
  pub font_index: u32,
  /// Byte range of the run's source text within its inline layout's
  /// [`BuiltInlineLayout::text`].
  pub text_range: Range<usize>,
  /// Each glyph's cluster bytes in the line's text, empty when unknown.
  pub cluster_ranges: Vec<Range<usize>>,
  /// User-space variation coordinates the run was shaped at, e.g. `[(*b"wght", 700.0)]`.
  /// A consumer that re-reads the font must apply these or it gets the default instance.
  pub variations: Vec<([u8; 4], f32)>,
  /// Stroke width in px for synthetic bold, when the face has no weight of its own to reach.
  pub synthetic_bold: Option<f32>,
  /// Synthetic oblique angle in degrees.
  pub synthetic_skew: Option<f32>,
  /// Normalized variation coordinates the run was shaped at.
  pub(crate) normalized_coords: Vec<F2Dot14>,
  /// The shape a symbol marker's run paints instead of its glyphs, which are spaces.
  pub(crate) symbol_marker: Option<SymbolMarker>,
  // Accessor, not a `pub` field: the backing font bytes stay out of the public API.
  pub(super) font_data: FontBlob,
}

impl ShapedRun {
  /// The face its glyphs resolve against.
  pub(crate) fn face(&self) -> RunFace<'_> {
    RunFace {
      font_id: self.font_id(),
      font_index: self.font_index,
      font_size: self.font_size,
      normalized_coords: &self.normalized_coords,
      synthesis: RunSynthesis {
        embolden: self.synthetic_bold,
        skew: self.synthetic_skew,
      },
    }
  }

  /// The key every run [`Self::same_face`] as this one shares.
  pub(super) fn face_key(&self) -> FaceKey {
    FaceKey {
      font_id: self.font_id(),
      font_index: self.font_index,
      // Adding zero folds -0.0 into 0.0, which compare equal.
      font_size_bits: (self.font_size + 0.0).to_bits(),
      source_span_id: self.brush.source_span_id,
    }
  }

  /// Whether `other` draws in the same face, metrics, brush and symbol, wherever it draws.
  pub(super) fn same_face(&self, other: &Self) -> bool {
    self.brush == other.brush
      && self.metrics == other.metrics
      && self.font_size == other.font_size
      && self.font_index == other.font_index
      && self.font_id() == other.font_id()
      && self.variations == other.variations
      && self.synthetic_bold == other.synthetic_bold
      && self.synthetic_skew == other.synthetic_skew
      && self.normalized_coords == other.normalized_coords
      && self.symbol_marker == other.symbol_marker
  }

  /// Font bytes for `skrifa::FontRef::from_index`, paired with [`Self::font_index`].
  pub fn font_data(&self) -> &[u8] {
    self.font_data.as_ref()
  }

  /// The font file the run was shaped with.
  pub fn font_blob(&self) -> FontBlob {
    self.font_data.clone()
  }

  /// Stable identifier of the backing font blob, usable as a cache key.
  pub fn font_id(&self) -> u64 {
    self.font_data.id()
  }

  /// The run's font's metrics, standing in for a primary font a box lacks.
  pub(crate) fn primary_metrics(&self) -> PrimaryFontMetrics {
    let RunMetrics {
      ascent,
      descent,
      underline_offset,
      underline_size,
      ..
    } = self.metrics;
    let font = FontRef::from_index(self.font_data(), self.font_index).ok();
    let em_descent = em_box_descent(font.as_ref(), self.font_size, ascent, descent);
    let zero_width = font.as_ref().map_or(self.font_size / 2.0, |font| {
      zero_inline_size(font, self.font_size, LocationRef::default())
    });

    PrimaryFontMetrics {
      ascent: ascent.round(),
      descent: descent.round(),
      line_gap: 0.0,
      x_height: None,
      exact: ExactFontMetrics { ascent, descent },
      underline: Some(FontUnderline {
        position: -underline_offset,
        thickness: underline_size,
      }),
      em_descent,
      zero_width,
    }
  }
}

/// One glyph run positioned on its line, carrying everything both backends need to paint it.
#[non_exhaustive]
pub struct PositionedInlineRun {
  /// The shaped glyph run (metrics, brush, positioned glyphs, font).
  pub glyph_run: ShapedRun,
  /// Glyphs resolved to outlines/bitmaps, keyed by glyph id.
  pub resolved_glyphs: ResolvedGlyphs,
  /// Text-fit scale state for the line.
  pub(crate) line_scale: LineScaleState,
  /// Where the run's decorations go.
  pub(crate) decoration_placement: DecorationPlacement,
  /// Where the block's border box sits in the space paint snaps to pixels in.
  pub(crate) paint_offset: Point<f32>,
  /// How far `position: relative` on its spans moves it.
  pub(crate) relative_offset: Point<f32>,
  /// Where the run sits among its block's runs.
  #[cfg(feature = "paint-tree")]
  pub(crate) index: usize,
}

impl PositionedInlineRun {
  /// The lines the run's `text-decoration` paints in `layout`, drawn through `base` onto a device
  /// at `device`.
  pub fn decorations(
    &self,
    layout: ComputedLayout,
    base: Affine,
    device: Affine,
  ) -> Vec<DecorationLine> {
    self.glyph_run.decorations(
      &self.resolved_glyphs,
      layout,
      &self.decoration_placement,
      DecorationSpace {
        transform: base * Affine::translation(self.relative_offset.x, self.relative_offset.y),
        device,
        paint_offset: self.paint_offset + self.relative_offset,
      },
    )
  }

  /// The run's transform composed onto `base`.
  pub fn transform(&self, base: Affine) -> Affine {
    let Point { x, y } = self.relative_offset;

    self.line_scale.transform(base * Affine::translation(x, y)) * Affine::translation(-x, -y)
  }

  /// Per-glyph inline-offset origin: the content box's top-left, moved by the relative offset.
  pub fn glyph_offset(&self, layout: ComputedLayout) -> Point<f32> {
    let offset = layout.content_box_offset();
    Point {
      x: offset.x + self.relative_offset.x,
      y: offset.y + self.relative_offset.y,
    }
  }

  /// Resolves a COLR glyph to color and path layers for vector emission.
  pub fn resolve_color_layers<'g>(
    &self,
    outline: &'g ResolvedOutlineGlyph,
    foreground: Color,
  ) -> Vec<(Color, &'g [PathCommand])> {
    let Some(layers) = outline.color_layers() else {
      return Vec::new();
    };
    let font = FontRef::from_index(self.glyph_run.font_data(), self.glyph_run.font_index).ok();
    let palettes = font.as_ref().map(MetadataProvider::color_palettes);
    let palette = palettes.as_ref().and_then(|palettes| palettes.get(0));

    layers
      .iter()
      .filter_map(|layer| {
        Some((
          layer.color(palette.as_ref(), foreground)?,
          layer.paths.as_slice(),
        ))
      })
      .collect()
  }
}

/// A positioned inline paint item shared by the backends.
#[non_exhaustive]
pub struct InlineRunLayout<'c> {
  /// Glyph runs in line/visual order.
  pub runs: Vec<PositionedInlineRun>,
  /// In-flow and out-of-flow inline boxes, positioned, sorted by id.
  pub inline_boxes: Vec<VisualInlineBox>,
  /// Outlined spans' line fragments, sorted by span then line.
  pub outline_rects: Vec<InlineOutlineRect>,
  /// Inline-span background fragments, in paint order (outer spans first).
  pub background_fragments: Vec<InlineBackgroundFragment<'c>>,
}

/// A measured glyph run: its text (borrowed from the layout) and local bounding box, with text-fit
/// line scaling applied.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasuredInlineRun<'a> {
  /// The run's text content, borrowed from the layout.
  pub text: &'a str,
  /// Left edge, relative to the inline formatting context's origin.
  pub x: f32,
  /// Top edge, relative to the inline formatting context's origin.
  pub y: f32,
  /// Run width.
  pub width: f32,
  /// Run height.
  pub height: f32,
  /// URI of the nearest enclosing anchor's `href`, if any.
  pub link: Option<Arc<str>>,
}

/// A measured inline box's local bounding box, with text-fit line scaling applied to in-flow boxes'
/// x position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasuredInlineBox {
  /// Left edge, relative to the inline formatting context's origin.
  pub x: f32,
  /// Top edge, relative to the inline formatting context's origin.
  pub y: f32,
  /// Box width.
  pub width: f32,
  /// Box height.
  pub height: f32,
}

pub(super) fn slice_text_at_char_boundaries(text: &str, byte_range: Range<usize>) -> &str {
  if byte_range.start >= byte_range.end || byte_range.start >= text.len() {
    return "";
  }

  let end = byte_range.end.min(text.len());
  let start = text.ceil_char_boundary(byte_range.start.min(end));
  let end = text.floor_char_boundary(end);
  if start >= end {
    return "";
  }

  &text[start..end]
}
