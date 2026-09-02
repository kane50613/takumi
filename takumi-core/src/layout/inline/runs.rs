//! Shaped glyph runs resolved for painting and measuring.

use crate::{
  context::RenderContext,
  geometry::{ComputedLayout, PathCommand, Point},
  resources::{
    font::{FontError, run_synthesis, run_variations},
    glyph::{ResolvedColorLayer, ResolvedGlyph, ResolvedOutlineGlyph},
  },
  style::{Affine, Color, TextUnderlinePosition},
};
use parley::{GlyphRun, InlineBoxKind, PositionedLayoutItem};
use skrifa::{FontRef, MetadataProvider, raw::TableProvider};
use std::{collections::HashMap, ops::Range, sync::Arc};

use super::{
  BuiltInlineLayout, InlineBrush, LineSetup,
  background::{CoverExtent, DecorationAccumulator, InlineBackgroundFragment},
  breaking::distribute_trailing_whitespace,
  items::ProcessedInlineSpan,
  metrics::{VisualInlineBox, resolve_visual_inline_box},
  outline::{InlineOutlineRect, scale_outline_rect},
  text_fit::{LineScaleState, scale_text_fit_x},
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
}

/// Vertical font metrics for a shaped run, in px.
#[derive(Clone, Copy, Debug)]
pub struct RunMetrics {
  /// Typographic ascent.
  pub ascent: f32,
  /// Typographic descent.
  pub descent: f32,
  /// Underline offset from the baseline.
  pub underline_offset: f32,
  /// Underline stroke thickness.
  pub underline_size: f32,
  /// Strikethrough offset from the baseline.
  pub strikethrough_offset: f32,
  /// Strikethrough stroke thickness.
  pub strikethrough_size: f32,
}

/// Per-glyph cluster text ranges for a [`GlyphRun`], aligned to its positioned glyphs.
fn glyph_cluster_ranges(
  glyph_run: &GlyphRun<'_, InlineBrush>,
  positioned: &[PositionedGlyph],
) -> Vec<Range<usize>> {
  let mut full: Vec<(u32, Range<usize>)> = Vec::new();

  for cluster in glyph_run.run().visual_clusters() {
    let range = cluster.text_range();
    let before = full.len();

    for glyph in cluster.glyphs() {
      full.push((glyph.id, range.clone()));
    }
    // A glyph-less cluster is a ligature continuation: fold its text into the
    // carrying glyph so the ligature maps to its full source text.
    if full.len() == before
      && let Some((_, last)) = full.last_mut()
    {
      last.start = last.start.min(range.start);
      last.end = last.end.max(range.end);
    }
  }
  let count = positioned.len();

  let matches_at = |start: usize| {
    full[start..start + count]
      .iter()
      .zip(positioned)
      .all(|((id, _), glyph)| *id == glyph.id)
  };
  let Some(start) = (0..=full.len().saturating_sub(count)).find(|&s| matches_at(s)) else {
    return Vec::new();
  };
  full[start..start + count]
    .iter()
    .map(|(_, range)| range.clone())
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
  /// Advance of line-end whitespace inside [`Self::advance`]. Decorations do
  /// not span it (Blink skips hanging whitespace); naive for RTL, where it
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
  pub(super) font_data: parley::fontique::Blob<u8>,
}

impl ShapedRun {
  /// Advance that decorations span: the run without its line-end whitespace.
  pub fn decorated_advance(&self) -> f32 {
    self.advance - self.trailing_whitespace
  }

  /// Font bytes for `skrifa::FontRef::from_index`, paired with [`Self::font_index`].
  pub fn font_data(&self) -> &[u8] {
    self.font_data.as_ref()
  }

  /// Stable identifier of the backing font blob, usable as a cache key.
  pub fn font_id(&self) -> u64 {
    self.font_data.id()
  }

  /// Underline top edge relative to the run's baseline, positive downwards.
  pub fn underline_offset_from_baseline(&self) -> f32 {
    let from_metrics = match self.brush.underline_position {
      TextUnderlinePosition::Auto | TextUnderlinePosition::FromFont => {
        -self.metrics.underline_offset
      }
      TextUnderlinePosition::Under => self.em_box_descent(),
    };

    from_metrics + self.brush.underline_offset
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
  /// Text-outline rects (unmerged), in collection order.
  pub outline_rects: Vec<InlineOutlineRect>,
  /// Inline-span background fragments, in paint order (outer spans first).
  pub background_fragments: Vec<InlineBackgroundFragment>,
}

/// Walks `built` once, resolving every glyph run, inline box, and outline rect into
/// backend-agnostic positioned drawables.
pub fn resolve_inline_runs(
  built: &BuiltInlineLayout<'_>,
  context: &RenderContext,
  layout: ComputedLayout,
) -> Result<InlineRunLayout, FontError> {
  let BuiltInlineLayout {
    layout: inline_layout,
    spans,
    positioned_floats,
    line_scales,
    ..
  } = built;
  let line_vertical_metrics = built.line_metrics();
  let line_states = built.line_states();

  let need_outline = spans.iter().any(|span| match span {
    ProcessedInlineSpan::Text { style, .. } => {
      style.outline_width > 0.0 && style.outline_style.is_rendered()
    }
    ProcessedInlineSpan::DirectionMark { .. }
    | ProcessedInlineSpan::Box(_)
    | ProcessedInlineSpan::Spacer { .. } => false,
  });

  let mut runs = Vec::new();
  let mut outline_rects = Vec::new();
  let mut decoration_coverage = DecorationAccumulator::default();
  let mut positioned_inline_boxes: HashMap<u64, VisualInlineBox> = HashMap::new();

  for (line_index, line) in inline_layout.lines().enumerate() {
    let Some(setup) = LineSetup::new(
      &line,
      layout,
      &line_vertical_metrics,
      line_scales,
      line_index,
    ) else {
      continue;
    };
    let mut static_inline_prefix = 0.0_f32;

    let items: Vec<_> = line.items().collect();
    let run_trailing_whitespace = distribute_trailing_whitespace(&items, &line);

    for (item_index, item) in items.into_iter().enumerate() {
      match item {
        PositionedLayoutItem::GlyphRun(glyph_run) => {
          let run = glyph_run.run();
          // A run carrying only the direction mark paints nothing; a run the
          // mark's cluster merged into (emoji sequences) paints as the first
          // real span.
          let mut brush = glyph_run.style().brush;
          if brush.is_direction_mark {
            if glyph_run.advance() == 0.0 {
              continue;
            }
            brush.is_direction_mark = false;
          }

          let font = FontRef::from_index(run.font().data.as_ref(), run.font().index)
            .map_err(|_| FontError::InvalidFontIndex)?;
          let glyph_ids = glyph_run.positioned_glyphs().map(|glyph| glyph.id);
          let resolved_glyphs = context
            .fonts()
            .with_context(|fonts| fonts.resolve_glyphs(&glyph_run, font, glyph_ids));

          if need_outline && let Some(span_id) = brush.source_span_id {
            outline_rects.push(scale_outline_rect(
              InlineOutlineRect {
                span_id,
                line_index,
                x: layout.border.left + layout.padding.left + glyph_run.offset(),
                y: layout.border.top
                  + layout.padding.top
                  + glyph_run.baseline()
                  + setup.baseline_shift
                  - setup.resolved_metrics.resolved_ascent,
                width: glyph_run.advance(),
                height: setup.resolved_metrics.resolved_line_height,
              },
              setup.state,
              static_inline_prefix,
            ));
          }

          let metrics = run.metrics();

          if let Some(span_id) = brush.source_span_id
            && let Some(ProcessedInlineSpan::Text {
              decorations: Some(chain),
              ..
            }) = spans.get(span_id as usize)
          {
            // The run's leaded box, like Blink's inline box fragment
            // (`InlineBoxState::ComputeTextMetrics` adds the line-height
            // leading to the font height).
            let (above, below) =
              brush.line_box_contribution(metrics.line_height, metrics.ascent, metrics.descent);
            let rect = scale_outline_rect(
              InlineOutlineRect {
                span_id,
                line_index,
                x: layout.border.left + layout.padding.left + glyph_run.offset(),
                y: layout.border.top
                  + layout.padding.top
                  + glyph_run.baseline()
                  + setup.baseline_shift
                  - above,
                width: glyph_run.advance(),
                height: above + below,
              },
              setup.state,
              static_inline_prefix,
            );

            decoration_coverage.cover(
              Some(chain),
              line_index,
              rect.x,
              rect.x + rect.width,
              &CoverExtent::Run {
                font_size: run.font_size(),
                top: rect.y,
                bottom: rect.y + rect.height,
                baseline: rect.y + above * setup.state.scale,
              },
            );
          }
          let glyphs: Vec<PositionedGlyph> = glyph_run
            .positioned_glyphs()
            .map(|g| PositionedGlyph {
              id: g.id,
              x: g.x,
              y: g.y,
            })
            .collect();
          let cluster_ranges = glyph_cluster_ranges(&glyph_run, &glyphs);
          let synthesis = run_synthesis(&glyph_run);
          let shaped = ShapedRun {
            glyphs,
            offset: glyph_run.offset(),
            baseline: glyph_run.baseline(),
            advance: glyph_run.advance(),
            trailing_whitespace: run_trailing_whitespace[item_index],
            brush,
            metrics: RunMetrics {
              ascent: metrics.ascent,
              descent: metrics.descent,
              underline_offset: metrics.underline_offset,
              underline_size: metrics.underline_size,
              strikethrough_offset: metrics.strikethrough_offset,
              strikethrough_size: metrics.strikethrough_size,
            },
            font_size: run.font_size(),
            font_index: run.font().index,
            text_range: run.text_range(),
            cluster_ranges,
            variations: run_variations(&glyph_run),
            synthetic_bold: synthesis.embolden,
            synthetic_skew: synthesis.skew,
            font_data: run.font().data.clone(),
          };

          runs.push(PositionedInlineRun {
            glyph_run: shaped,
            resolved_glyphs,
            line_scale: setup.state,
            static_inline_prefix,
            baseline_shift: setup.baseline_shift,
          });
        }
        PositionedLayoutItem::InlineBox(inline_box) => {
          if inline_box.kind != InlineBoxKind::InFlow {
            continue;
          }
          let Some(inline_box) =
            resolve_visual_inline_box(inline_box, Some(line_states[line_index]), spans)
          else {
            continue;
          };
          let inline_box = VisualInlineBox {
            x: scale_text_fit_x(
              inline_box.layout_x,
              setup.line_scale_origin_x,
              setup.state.scale,
              static_inline_prefix,
              setup.state.alignment_correction,
            ),
            ..inline_box
          };
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
            let x0 = layout.border.left + layout.padding.left + inline_box.x;
            let origin_y = setup.state.layout_origin.y;
            let content_top = layout.border.top + layout.padding.top;
            let line_y =
              |value: f32| origin_y + (content_top + value - origin_y) * setup.state.scale;

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
          static_inline_prefix += inline_box.layout_advance;
        }
      }
    }
  }

  for inline_box in positioned_floats {
    let Some(inline_box) = resolve_visual_inline_box(inline_box.clone(), None, spans) else {
      continue;
    };
    positioned_inline_boxes.insert(inline_box.id, inline_box);
  }

  let mut inline_boxes: Vec<_> = positioned_inline_boxes.into_values().collect();
  inline_boxes.sort_by_key(|inline_box| inline_box.id);

  Ok(InlineRunLayout {
    runs,
    inline_boxes,
    outline_rects,
    background_fragments: decoration_coverage.into_fragments(),
  })
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
