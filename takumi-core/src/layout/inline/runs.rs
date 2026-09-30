//! Shaped glyph runs resolved for painting and measuring.

use crate::{
  context::RenderContext,
  geometry::{ComputedLayout, PathCommand, Point},
  layout::intercept::skips_ink,
  resources::{
    font::{FontError, run_synthesis, run_variations},
    glyph::{ResolvedColorLayer, ResolvedGlyph, ResolvedOutlineGlyph},
  },
  style::{Affine, Color, TextUnderlinePosition},
};
use parley::{GlyphRun, fontique::Blob};
use skrifa::{FontRef, MetadataProvider, raw::TableProvider};
use std::{collections::HashMap, ops::Range, sync::Arc};

use super::{
  BuiltInlineLayout, InlineBrush, PlacedItem,
  background::{CoverExtent, DecorationAccumulator, InlineBackgroundFragment},
  items::ProcessedInlineSpan,
  metrics::{VisualInlineBox, resolve_visual_inline_box},
  outline::InlineOutlineRect,
  text_fit::{LineScaleState, RunBox},
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
#[derive(Clone, Copy, Debug)]
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

/// The source cluster behind a positioned glyph.
struct GlyphCluster {
  range: Range<usize>,
  emoji: bool,
}

/// Per-glyph source clusters for a [`GlyphRun`], aligned to its positioned glyphs.
fn glyph_clusters(
  glyph_run: &GlyphRun<'_, InlineBrush>,
  positioned: &[PositionedGlyph],
) -> Vec<GlyphCluster> {
  let mut full: Vec<(u32, GlyphCluster)> = Vec::new();

  for cluster in glyph_run.run().visual_clusters() {
    let range = cluster.text_range();
    let before = full.len();

    for glyph in cluster.glyphs() {
      full.push((
        glyph.id,
        GlyphCluster {
          range: range.clone(),
          emoji: cluster.is_emoji(),
        },
      ));
    }
    // A glyph-less cluster is a ligature continuation: fold its text into the
    // carrying glyph so the ligature maps to its full source text.
    if full.len() == before
      && let Some((_, last)) = full.last_mut()
    {
      last.range.start = last.range.start.min(range.start);
      last.range.end = last.range.end.max(range.end);
    }
  }
  let count = positioned.len();

  let matches_at = |start: usize| {
    full.get(start..start + count).is_some_and(|window| {
      window
        .iter()
        .zip(positioned)
        .all(|((id, _), glyph)| *id == glyph.id)
    })
  };
  let Some(start) = (0..=full.len().saturating_sub(count)).find(|&s| matches_at(s)) else {
    return Vec::new();
  };
  full
    .drain(start..start + count)
    .map(|(_, cluster)| cluster)
    .collect()
}

/// A shaped, positioned glyph run — the core-owned replacement for
/// `parley::GlyphRun`. Owns everything both backends need to paint a run; carries
/// no borrow into the parley layout.
pub struct ShapedRun {
  /// The run's glyphs, positioned from the line origin.
  pub glyphs: Vec<PositionedGlyph>,
  /// Horizontal offset of the run's start from the line origin. A glyph's `x`
  /// already carries it; this is for placing what the glyphs do not, such as a
  /// decoration spanning the run.
  pub offset: f32,
  /// Baseline position within the line.
  pub baseline: f32,
  /// Total horizontal advance of the run.
  pub advance: f32,
  /// Advance of line-end whitespace inside [`Self::advance`]. Decorations, inline backgrounds
  /// and outlines do not span it (Blink skips hanging whitespace); naive for RTL, where it
  /// trims the visual right edge instead of the line-start side.
  pub trailing_whitespace: f32,
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
  /// Per-glyph cluster byte ranges into [`BuiltInlineLayout::text`], parallel
  /// to [`Self::glyphs`] (visual order). Empty when alignment failed; treat as
  /// unknown.
  pub cluster_ranges: Vec<Range<usize>>,
  /// User-space variation coordinates the run was shaped at, e.g. `[(*b"wght", 700.0)]`.
  /// A consumer that re-reads the font must apply these or it gets the default instance.
  pub variations: Vec<([u8; 4], f32)>,
  /// Stroke width in px for synthetic bold, when the face has no weight of its own to reach.
  pub synthetic_bold: Option<f32>,
  /// Synthetic oblique angle in degrees.
  pub synthetic_skew: Option<f32>,
  // Accessor, not a `pub` field: the backing `parley` blob must not leak into the public API.
  pub(super) font_data: Blob<u8>,
}

impl ShapedRun {
  /// The run `glyph_run` shapes, carrying `glyphs` and painting with `brush`.
  pub(crate) fn of(
    glyph_run: &GlyphRun<'_, InlineBrush>,
    glyphs: Vec<PositionedGlyph>,
    trailing_whitespace: f32,
    brush: InlineBrush,
    cluster_ranges: Vec<Range<usize>>,
  ) -> Self {
    let run = glyph_run.run();
    let metrics = run.metrics();
    let synthesis = run_synthesis(glyph_run);
    // The run's leaded box: the font height plus the line-height leading.
    let (above, below) = brush.line_box_contribution(
      metrics.line_height,
      metrics.ascent,
      metrics.descent,
      metrics.leading,
    );

    Self {
      glyphs,
      offset: glyph_run.offset(),
      baseline: glyph_run.baseline(),
      advance: glyph_run.advance(),
      trailing_whitespace,
      brush,
      metrics: RunMetrics {
        ascent: metrics.ascent,
        descent: metrics.descent,
        line_height: above + below,
        underline_offset: metrics.underline_offset,
        underline_size: metrics.underline_size,
      },
      font_size: run.font_size(),
      font_index: run.font().index,
      text_range: run.text_range(),
      cluster_ranges,
      variations: run_variations(glyph_run),
      synthetic_bold: synthesis.embolden,
      synthetic_skew: synthesis.skew,
      font_data: run.font().data.clone(),
    }
  }

  /// Advance that decorations span: the run without its line-end whitespace.
  pub fn decorated_advance(&self) -> f32 {
    self.advance - self.trailing_whitespace
  }

  /// Font bytes for `skrifa::FontRef::from_index`, paired with [`Self::font_index`].
  pub fn font_data(&self) -> &[u8] {
    self.font_data.as_ref()
  }

  /// The font file the run was shaped with.
  #[cfg(feature = "paint-tree")]
  pub fn font_blob(&self) -> Blob<u8> {
    self.font_data.clone()
  }

  /// Stable identifier of the backing font blob, usable as a cache key.
  pub fn font_id(&self) -> u64 {
    self.font_data.id()
  }

  /// Underline top edge relative to the run's baseline, positive downwards.
  ///
  /// Follows Blink's `TextDecorationOffset::ComputeUnderlineOffset`: `auto` leaves a gap of half
  /// the `thickness`, at least a pixel, under the baseline unless `text-underline-offset` is set,
  /// `from-font` takes the font's underline position, and `under` sits a pixel past the em box.
  pub fn underline_offset_from_baseline(&self, thickness: f32) -> f32 {
    let offset = self.brush.underline_offset.unwrap_or(0.0);

    match self.brush.underline_position {
      TextUnderlinePosition::Auto => {
        let gap = match self.brush.underline_offset {
          Some(_) => 0.0,
          None => (thickness / 2.0).ceil().max(1.0),
        };

        gap + offset.round()
      }
      TextUnderlinePosition::FromFont => -self.metrics.underline_offset + offset,
      TextUnderlinePosition::Under => self.em_box_descent() + 1.0 + offset,
    }
  }

  /// Bottom edge of the em box below the baseline. The typographic ascender and
  /// descender are normalized to sum to the font size, keeping their ratio, which is
  /// how browsers derive the em box: https://drafts.csswg.org/css-inline-3/#ascent-descent
  fn em_box_descent(&self) -> f32 {
    let (ascent, descent) = self.typographic_ascent_descent();
    let height = ascent + descent;

    if height <= 0.0 || ascent < 0.0 {
      return self.metrics.descent;
    }

    self.font_size * descent / height
  }

  fn typographic_ascent_descent(&self) -> (f32, f32) {
    FontRef::from_index(self.font_data(), self.font_index)
      .ok()
      .and_then(|font| font.os2().ok())
      .map(|os2| {
        (
          f32::from(os2.s_typo_ascender()),
          -f32::from(os2.s_typo_descender()),
        )
      })
      .filter(|(ascent, descent)| ascent + descent > 0.0)
      .unwrap_or((self.metrics.ascent, self.metrics.descent))
  }
}

/// One glyph run positioned on its line, carrying everything both backends need to paint it.
#[non_exhaustive]
pub struct PositionedInlineRun {
  /// The shaped glyph run (metrics, brush, positioned glyphs, font).
  pub glyph_run: ShapedRun,
  /// Glyphs resolved to outlines/bitmaps, keyed by glyph id.
  pub resolved_glyphs: HashMap<u32, Arc<ResolvedGlyph>>,
  /// Text-fit scale state for the line.
  pub(crate) line_scale: LineScaleState,
  /// Cumulative in-flow inline-box width before this run on the line.
  pub(crate) static_inline_prefix: f32,
  /// Baseline shift applied to glyphs on the line.
  pub baseline_shift: f32,
  /// Where the run sits among its block's runs.
  #[cfg(feature = "paint-tree")]
  pub(crate) index: usize,
}

impl PositionedInlineRun {
  /// The run's affine transform composed onto `base` (the element transform for raster, identity
  /// for vector emission).
  pub fn transform(&self, base: Affine) -> Affine {
    self.line_scale.transform(base, self.static_inline_prefix)
  }

  /// Per-glyph inline-offset origin (border/padding box top-left + baseline shift).
  pub fn glyph_offset(&self, layout: ComputedLayout) -> Point<f32> {
    let offset = layout.content_box_offset();
    Point {
      x: offset.x,
      y: offset.y + self.baseline_shift,
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
    let foreground_opacity = foreground.0[3] as f32 / 255.0;

    layers
      .iter()
      .filter_map(|layer: &ResolvedColorLayer| {
        let color = if layer.palette_index == u16::MAX {
          let alpha = (foreground_opacity * layer.alpha * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
          Color([foreground.0[0], foreground.0[1], foreground.0[2], alpha])
        } else {
          let record = palette
            .as_ref()?
            .colors()
            .get(usize::from(layer.palette_index))?;
          let alpha = ((record.alpha() as f32 / 255.0) * layer.alpha * foreground_opacity * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
          Color([record.red(), record.green(), record.blue(), alpha])
        };
        Some((color, layer.paths.as_slice()))
      })
      .collect()
  }
}

/// A positioned inline paint item shared by the backends.
#[non_exhaustive]
pub struct InlineRunLayout {
  /// Glyph runs in line/visual order.
  pub runs: Vec<PositionedInlineRun>,
  /// In-flow and out-of-flow inline boxes, positioned, sorted by id.
  pub inline_boxes: Vec<VisualInlineBox>,
  /// Outlined spans' line fragments, sorted by span then line.
  pub outline_rects: Vec<InlineOutlineRect>,
  /// Inline-span background fragments, in paint order (outer spans first).
  pub background_fragments: Vec<InlineBackgroundFragment>,
}

impl BuiltInlineLayout<'_> {
  /// Resolves every glyph run, inline box, and outline rect into backend-agnostic positioned
  /// drawables.
  pub fn resolve_runs(
    &self,
    context: &RenderContext,
    layout: ComputedLayout,
  ) -> Result<InlineRunLayout, FontError> {
    let BuiltInlineLayout {
      spans,
      positioned_floats,
      ..
    } = self;
    let mut runs = Vec::new();
    let mut decoration_coverage = DecorationAccumulator::default();
    let mut positioned_inline_boxes: HashMap<u64, VisualInlineBox> = HashMap::new();

    let content = layout.content_box_offset();

    self.walk_items(layout, |line, item| {
      let setup = &line.setup;
      let line_index = line.index;

      match item {
        PlacedItem::Run {
          glyph_run,
          static_inline_prefix,
          trailing_whitespace,
        } => {
          let run = glyph_run.run();
          // A run carrying only the direction mark paints nothing; a run the
          // mark's cluster merged into (emoji sequences) paints as the first
          // real span.
          let mut brush = glyph_run.style().brush;
          if brush.is_direction_mark {
            if glyph_run.advance() == 0.0 {
              return Ok(());
            }
            brush.is_direction_mark = false;
          }

          let font = FontRef::from_index(run.font().data.as_ref(), run.font().index)
            .map_err(|_| FontError::InvalidFontIndex)?;
          let mut glyphs: Vec<PositionedGlyph> = glyph_run
            .positioned_glyphs()
            .map(|g| PositionedGlyph {
              id: g.id,
              x: g.x,
              y: g.y,
              skips_ink: true,
            })
            .collect();
          let resolved_glyphs = context.fonts().with_context(|fonts| {
            fonts.resolve_glyphs(&glyph_run, font, glyphs.iter().map(|glyph| glyph.id))
          });

          let metrics = run.metrics();
          // The font's rounded ascent and descent, without the line-height leading, like the
          // inline box fragment `InlineBoxState::ComputeTextMetrics` sizes.
          let ascent = metrics.ascent.round();

          if let Some(span_id) = brush.source_span_id
            && let Some(ProcessedInlineSpan::Text {
              decorations: Some(chain),
              ..
            }) = spans.get(span_id as usize)
          {
            let rect = RunBox {
              x: content.x + glyph_run.offset(),
              y: content.y + glyph_run.baseline() + setup.baseline_shift - ascent,
              width: glyph_run.advance() - trailing_whitespace,
              height: ascent + metrics.descent.round(),
            }
            .scaled(setup.state, static_inline_prefix);

            decoration_coverage.cover(
              Some(chain),
              line_index,
              rect.x,
              rect.x + rect.width,
              &CoverExtent::Run {
                font_size: run.font_size(),
                top: rect.y,
                bottom: rect.y + rect.height,
                baseline: rect.y + ascent * setup.state.scale,
              },
            );
          }
          let clusters = glyph_clusters(&glyph_run, &glyphs);

          for (glyph, cluster) in glyphs.iter_mut().zip(&clusters) {
            glyph.skips_ink = !cluster.emoji
              && self
                .text
                .get(cluster.range.clone())
                .and_then(|text| text.chars().next())
                .is_none_or(skips_ink);
          }
          let cluster_ranges = clusters.into_iter().map(|cluster| cluster.range).collect();
          let shaped = ShapedRun::of(
            &glyph_run,
            glyphs,
            trailing_whitespace,
            brush,
            cluster_ranges,
          );

          runs.push(PositionedInlineRun {
            glyph_run: shaped,
            resolved_glyphs,
            line_scale: setup.state,
            static_inline_prefix,
            baseline_shift: setup.baseline_shift,
            #[cfg(feature = "paint-tree")]
            index: runs.len(),
          });
        }
        PlacedItem::Box(inline_box) => {
          // A spacer or atomic box inside a decorated span stretches the
          // span's fragment horizontally; runs set its height, like Blink's
          // box metrics ignoring atomic descendants. The line extent is the
          // last resort so padding-only coverage still paints.
          let chain = match spans.get(inline_box.id as usize) {
            Some(ProcessedInlineSpan::Box(item)) => item.decorations.as_ref(),
            Some(ProcessedInlineSpan::Spacer { decorations, .. }) => decorations.as_ref(),
            _ => None,
          };

          if chain.is_some() {
            let x0 = content.x + inline_box.x;
            let origin_y = setup.state.layout_origin.y;
            let line_y = |value: f32| origin_y + (content.y + value - origin_y) * setup.state.scale;

            decoration_coverage.cover(
              chain,
              line_index,
              x0,
              x0 + inline_box.width,
              &CoverExtent::Line {
                top: line_y(setup.resolved_metrics.resolved_line_top),
                bottom: line_y(setup.resolved_metrics.resolved_line_bottom),
                baseline: line_y(setup.resolved_metrics.resolved_baseline),
              },
            );
          }
          positioned_inline_boxes.insert(inline_box.id, inline_box);
        }
      }
      Ok(())
    })?;

    for inline_box in positioned_floats {
      let Some(inline_box) = resolve_visual_inline_box(inline_box.clone(), None, spans) else {
        continue;
      };
      positioned_inline_boxes.insert(inline_box.id, inline_box);
    }

    let mut inline_boxes: Vec<_> = positioned_inline_boxes.into_values().collect();
    inline_boxes.sort_unstable_by_key(|inline_box| inline_box.id);

    let (background_fragments, outline_rects) = decoration_coverage.into_fragments();

    Ok(InlineRunLayout {
      runs,
      inline_boxes,
      outline_rects,
      background_fragments,
    })
  }
}

/// A measured glyph run: its text (borrowed from the layout) and local bounding box, with text-fit
/// line scaling applied.
#[derive(Debug, Clone, Copy, PartialEq)]
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
  pub link: Option<&'a str>,
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

/// Extracts the source text rendered by a glyph run.
pub(super) fn measured_run_text<'a>(
  text: &'a str,
  spans: &[ProcessedInlineSpan<'_>],
  glyph_run: &GlyphRun<'_, InlineBrush>,
  span_id: Option<u64>,
) -> &'a str {
  let text_range = glyph_run.run().text_range();
  let Some(span_id) = span_id else {
    return slice_text_at_char_boundaries(text, text_range);
  };

  let Some(ProcessedInlineSpan::Text { byte_range, .. }) = spans.get(span_id as usize) else {
    return slice_text_at_char_boundaries(text, text_range);
  };

  let start = text_range.start.max(byte_range.start);
  let end = text_range.end.min(byte_range.end);
  slice_text_at_char_boundaries(text, start..end)
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
