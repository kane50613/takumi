// Copyright 2022 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use harfrust::{Feature, Tag as HbTag, font::Variation};
use kurbo::{ParamCurve, ParamCurveArclen, ParamCurveDeriv};
use skrifa::raw::types::F2Dot14;
use strict_num::NonZeroPositiveF32;
use tiny_skia::{NonZeroRect, Transform};

use crate::font_style::ExpandedFontFamily;
use crate::fonts::font::Font as ShapingFont;
use crate::fonts::font_baseline::FontBaseline;
use crate::fonts::font_custom_platform_data::FontCustomPlatformData;
use crate::fonts::font_description::{FontDescription, FontFallbackKey, FontSelectionDescription};
use crate::fonts::font_selection_types::{
  FontSelectionRequest, FontSelectionValue, ITALIC_SLOPE_VALUE, NORMAL_SLOPE_VALUE,
};
use crate::fonts::shaping::harfbuzz_shaper::HarfBuzzShaper;
use crate::fonts::shaping::shape_result_spacing::ShapeResultSpacing;
use crate::fonts::shaping::shape_result_view::ShapeResultView;
use crate::fonts::simple_font_data::SimpleFontData;
use crate::layout::inline::decorations::{DecoratingBox, DecorationFont};
use crate::resources::font::{PrimaryFontMetrics, RunFace, RunSynthesis};
use crate::sort_key::sort_by_key;
use crate::style::{
  Direction as TextDirection, FontFamilyToken, FontFeature, FontLanguageOverride,
  FontOpticalSizing as CssFontOpticalSizing, FontSynthesic, FontVariantCaps, FontVariantLigatures,
  GenericFamilyType, SizedTextDecorationThickness, TextDecorationLines, TextUnderlinePosition,
};
use crate::svg::tree::BBox;
use crate::svg::tree::{
  AlignmentBaseline, ApproxZeroUlps, BaselineShift, Cache, DominantBaseline, Fill, FillRule, Font,
  FontFamily, FontOpticalSizing, FontStyle, GlyphId, LengthAdjust, PaintOrder, Path,
  ShapeRendering, Stroke, Text, TextAnchor, TextChunk, TextDecorationStyle, TextFlow, TextPath,
  TextSpan, WritingMode,
};
use crate::text::bidi_paragraph::{BidiParagraph, direction_from_level};

/// A glyph that has already been positioned correctly.
///
/// Note that the transform already takes the font size into consideration, so applying the
/// transform to the outline of the glyphs is all that is necessary to display it correctly.
#[derive(Clone, Debug)]
pub struct PositionedGlyph {
  /// Returns the transform of the glyph itself within the cluster. For example,
  /// for zalgo text, it contains the transform to position the glyphs above/below
  /// the main glyph.
  glyph_ts: Transform,
  /// Returns the transform of the whole cluster that the glyph is part of.
  cluster_ts: Transform,
  /// Returns the transform of the span that the glyph is a part of.
  span_ts: Transform,
  /// The ID of the glyph.
  pub id: GlyphId,
  /// The font the glyph was shaped with.
  pub(crate) font: Arc<ResolvedFont>,
}

impl PositionedGlyph {
  /// Returns the transform of the glyph's outline, which is in pixels at the font size.
  pub fn transform(&self) -> Transform {
    self
      .span_ts
      .pre_concat(self.cluster_ts)
      .pre_concat(self.glyph_ts)
  }
}

/// A span contains a number of layouted glyphs that share the same fill, stroke, paint order and
/// visibility.
#[derive(Clone, Debug)]
pub struct Span {
  /// The fill of the span.
  pub fill: Option<Fill>,
  /// The stroke of the span.
  pub stroke: Option<Stroke>,
  /// The paint order of the span.
  pub paint_order: PaintOrder,
  /// The font size of the span.
  pub font_size: NonZeroPositiveF32,
  /// The visibility of the span.
  pub visible: bool,
  /// The glyphs that make up the span.
  pub positioned_glyphs: Vec<PositionedGlyph>,
  /// An underline text decoration of the span.
  /// Needs to be rendered before all glyphs.
  pub underline: Option<Path>,
  /// An overline text decoration of the span.
  /// Needs to be rendered before all glyphs.
  pub overline: Option<Path>,
  /// A line-through text decoration of the span.
  /// Needs to be rendered after all glyphs.
  pub line_through: Option<Path>,
}

#[derive(Clone, Debug)]
struct GlyphCluster {
  byte_idx: ByteIndex,
  codepoint: char,
  width: f32,
  advance: f32,
  ascent: f32,
  descent: f32,
  has_relative_shift: bool,
  glyphs: Vec<PositionedGlyph>,
  transform: Transform,
  path_transform: Transform,
  visible: bool,
}

impl GlyphCluster {
  pub(crate) fn height(&self) -> f32 {
    self.ascent - self.descent
  }

  pub(crate) fn transform(&self) -> Transform {
    self.path_transform.post_concat(self.transform)
  }
}

pub(crate) fn layout_text(text_node: &Text, cache: &Cache) -> Option<(Vec<Span>, NonZeroRect)> {
  let mut fonts_cache = FontsCache::new(text_node, cache);

  let mut spans = vec![];
  let mut char_offset = 0;
  let mut last_x = 0.0;
  let mut last_y = 0.0;
  let mut bbox = BBox::default();
  for chunk in &text_node.chunks {
    let (x, y) = match chunk.text_flow {
      TextFlow::Linear => (chunk.x.unwrap_or(last_x), chunk.y.unwrap_or(last_y)),
      TextFlow::Path(_) => (0.0, 0.0),
    };

    let mut clusters = process_chunk(chunk, &mut fonts_cache);
    if clusters.is_empty() {
      char_offset += chunk.text.chars().count();
      continue;
    }

    apply_writing_mode(text_node.writing_mode, &mut clusters);

    apply_length_adjust(chunk, &mut clusters);
    let mut curr_pos = resolve_clusters_positions(
      text_node,
      chunk,
      char_offset,
      text_node.writing_mode,
      &fonts_cache,
      &mut clusters,
    );

    let mut text_ts = Transform::default();
    if text_node.writing_mode == WritingMode::TopToBottom {
      if let TextFlow::Linear = chunk.text_flow {
        text_ts = text_ts.pre_rotate_at(90.0, x, y);
      }
    }

    for span in &chunk.spans {
      let font = match fonts_cache.primary(span) {
        Some(v) => v,
        None => continue,
      };

      let decoration_spans = collect_decoration_spans(span, &clusters);

      let mut span_ts = text_ts;
      span_ts = span_ts.pre_translate(x, y);
      if let TextFlow::Linear = chunk.text_flow {
        let shift = resolve_baseline(span, font, text_node.writing_mode);

        // In case of a horizontal flow, shift transform and not clusters,
        // because clusters can be rotated and an additional shift will lead
        // to invalid results.
        span_ts = span_ts.pre_translate(0.0, shift);
      }

      let mut decorate = |line, decoration: &Option<TextDecorationStyle>, vertical_offset| {
        let decoration = decoration.clone()?;
        let (center, thickness) = font.decoration_line(line);
        let offset = match text_node.writing_mode {
          WritingMode::LeftToRight => center,
          WritingMode::TopToBottom => vertical_offset,
        };
        let path = convert_decoration(
          (offset, thickness),
          span,
          decoration,
          &decoration_spans,
          span_ts,
        )?;

        bbox = bbox.expand(path.data.bounds());
        Some(path)
      };
      // TODO: No idea what offset should be used for top-to-bottom layout.
      // There is
      // https://www.w3.org/TR/css-text-decor-3/#text-underline-position-property
      // but it doesn't go into details.
      let underline = decorate(
        TextDecorationLines::UNDERLINE,
        &span.decoration.underline,
        font.height() / 2.0,
      );
      let overline = decorate(
        TextDecorationLines::OVERLINE,
        &span.decoration.overline,
        -font.height() / 2.0,
      );
      let line_through = decorate(
        TextDecorationLines::LINE_THROUGH,
        &span.decoration.line_through,
        0.0,
      );

      let mut fill = span.fill.clone();
      if let Some(ref mut fill) = fill {
        // The `fill-rule` should be ignored.
        // https://www.w3.org/TR/SVG2/text.html#TextRenderingOrder
        //
        // 'Since the fill-rule property does not apply to SVG text elements,
        // the specific order of the subpaths within the equivalent path does not matter.'
        fill.rule = FillRule::NonZero;
      }

      if let Some((span_fragments, span_bbox)) = convert_span(span, &clusters, span_ts) {
        bbox = bbox.expand(span_bbox);

        let positioned_glyphs = span_fragments
          .into_iter()
          .flat_map(|mut gc| {
            let cluster_ts = gc.transform();
            gc.glyphs.iter_mut().for_each(|pg| {
              pg.cluster_ts = cluster_ts;
              pg.span_ts = span_ts;
            });
            gc.glyphs
          })
          .collect();

        spans.push(Span {
          fill,
          stroke: span.stroke.clone(),
          paint_order: span.paint_order,
          font_size: span.font_size,
          visible: span.visible,
          positioned_glyphs,
          underline,
          overline,
          line_through,
        });
      }
    }

    char_offset += chunk.text.chars().count();

    if text_node.writing_mode == WritingMode::TopToBottom {
      if let TextFlow::Linear = chunk.text_flow {
        std::mem::swap(&mut curr_pos.0, &mut curr_pos.1);
      }
    }

    last_x = x + curr_pos.0;
    last_y = y + curr_pos.1;
  }

  let bbox = bbox.to_non_zero_rect()?;

  Some((spans, bbox))
}

fn convert_span(
  span: &TextSpan,
  clusters: &[GlyphCluster],
  text_ts: Transform,
) -> Option<(Vec<GlyphCluster>, NonZeroRect)> {
  let mut span_clusters = vec![];
  let mut bboxes_builder = tiny_skia::PathBuilder::new();

  for cluster in clusters {
    if !cluster.visible {
      continue;
    }

    if span_contains(span, cluster.byte_idx) {
      span_clusters.push(cluster.clone());
    }

    let mut advance = cluster.advance;
    if advance <= 0.0 {
      advance = 1.0;
    }

    // We have to calculate text bbox using font metrics and not glyph shape.
    if let Some(r) = NonZeroRect::from_xywh(0.0, -cluster.ascent, advance, cluster.height()) {
      if let Some(r) = r.transform(cluster.transform()) {
        bboxes_builder.push_rect(r.to_rect());
      }
    }
  }

  let mut bboxes = bboxes_builder.finish()?;
  bboxes = bboxes.transform(text_ts)?;
  let bbox = bboxes.compute_tight_bounds()?.to_non_zero_rect()?;

  Some((span_clusters, bbox))
}

fn collect_decoration_spans(span: &TextSpan, clusters: &[GlyphCluster]) -> Vec<DecorationSpan> {
  let mut spans = Vec::new();

  let mut started = false;
  let mut width = 0.0;
  let mut transform = Transform::default();

  for cluster in clusters {
    if span_contains(span, cluster.byte_idx) {
      if started && cluster.has_relative_shift {
        started = false;
        spans.push(DecorationSpan { width, transform });
      }

      if !started {
        width = cluster.advance;
        started = true;
        transform = cluster.transform;
      } else {
        width += cluster.advance;
      }
    } else if started {
      spans.push(DecorationSpan { width, transform });
      started = false;
    }
  }

  if started {
    spans.push(DecorationSpan { width, transform });
  }

  spans
}

/// The decoration `thickness` thick, centered `dy` below the baseline, under `decoration_spans`.
pub(crate) fn convert_decoration(
  (dy, thickness): (f32, f32),
  span: &TextSpan,
  mut decoration: TextDecorationStyle,
  decoration_spans: &[DecorationSpan],
  transform: Transform,
) -> Option<Path> {
  debug_assert!(!decoration_spans.is_empty());

  let mut builder = tiny_skia::PathBuilder::new();
  for dec_span in decoration_spans {
    let rect = match NonZeroRect::from_xywh(0.0, -thickness / 2.0, dec_span.width, thickness) {
      Some(v) => v,
      None => {
        log::warn!("a decoration span has a malformed bbox");
        continue;
      }
    };

    let ts = dec_span.transform.pre_translate(0.0, dy);

    let mut path = tiny_skia::PathBuilder::from_rect(rect.to_rect());
    path = match path.transform(ts) {
      Some(v) => v,
      None => continue,
    };

    builder.push_path(&path);
  }

  let mut path_data = builder.finish()?;
  path_data = path_data.transform(transform)?;

  Path::new(
    String::new(),
    span.visible,
    decoration.fill.take(),
    decoration.stroke.take(),
    PaintOrder::default(),
    ShapeRendering::default(),
    Arc::new(path_data),
    Transform::default(),
  )
}

/// A text decoration span.
///
/// Basically a horizontal line, that will be used for underline, overline and line-through.
/// It doesn't have a height, since it depends on the Font metrics.
#[derive(Clone, Copy)]
pub(crate) struct DecorationSpan {
  pub(crate) width: f32,
  pub(crate) transform: Transform,
}

/// Resolves clusters positions.
///
/// Mainly sets the `transform` property.
///
/// Returns the last text position. The next text chunk should start from that position.
fn resolve_clusters_positions(
  text: &Text,
  chunk: &TextChunk,
  char_offset: usize,
  writing_mode: WritingMode,
  fonts_cache: &FontsCache,
  clusters: &mut [GlyphCluster],
) -> (f32, f32) {
  match chunk.text_flow {
    TextFlow::Linear => {
      resolve_clusters_positions_horizontal(text, chunk, char_offset, writing_mode, clusters)
    }
    TextFlow::Path(ref path) => resolve_clusters_positions_path(
      text,
      chunk,
      char_offset,
      path,
      writing_mode,
      fonts_cache,
      clusters,
    ),
  }
}

fn clusters_length(clusters: &[GlyphCluster]) -> f32 {
  clusters.iter().fold(0.0, |w, cluster| w + cluster.advance)
}

fn resolve_clusters_positions_horizontal(
  text: &Text,
  chunk: &TextChunk,
  offset: usize,
  writing_mode: WritingMode,
  clusters: &mut [GlyphCluster],
) -> (f32, f32) {
  let mut x = process_anchor(chunk.anchor, clusters_length(clusters));
  let mut y = 0.0;

  for cluster in clusters {
    let cp = offset + cluster.byte_idx.code_point_at(&chunk.text);
    if let (Some(dx), Some(dy)) = (text.dx.get(cp), text.dy.get(cp)) {
      if writing_mode == WritingMode::LeftToRight {
        x += dx;
        y += dy;
      } else {
        y -= dx;
        x += dy;
      }
      cluster.has_relative_shift = !dx.approx_zero_ulps(4) || !dy.approx_zero_ulps(4);
    }

    cluster.transform = cluster.transform.pre_translate(x, y);

    if let Some(angle) = text.rotate.get(cp).cloned() {
      if !angle.approx_zero_ulps(4) {
        cluster.transform = cluster.transform.pre_rotate(angle);
        cluster.has_relative_shift = true;
      }
    }

    x += cluster.advance;
  }

  (x, y)
}

// Baseline resolving in SVG is a mess.
// Not only it's poorly documented, but as soon as you start mixing
// `dominant-baseline` and `alignment-baseline` each application/browser will produce
// different results.
//
// For now, resvg simply tries to match Chrome's output and not the mythical SVG spec output.
//
// See `alignment_baseline_shift` method comment for more details.
pub(crate) fn resolve_baseline(
  span: &TextSpan,
  font: &ResolvedFont,
  writing_mode: WritingMode,
) -> f32 {
  let mut shift = -resolve_baseline_shift(&span.baseline_shift, font);

  // TODO: support vertical layout as well
  if writing_mode == WritingMode::LeftToRight {
    let baseline = span
      .alignment_baseline
      .font_baseline()
      .unwrap_or_else(|| span.dominant_baseline.font_baseline());

    shift += font.baseline_shift(baseline);
  }

  shift
}

fn resolve_baseline_shift(baselines: &[BaselineShift], font: &ResolvedFont) -> f32 {
  let mut shift = 0.0;
  for baseline in baselines.iter().rev() {
    match baseline {
      BaselineShift::Baseline => {}
      BaselineShift::Subscript => shift -= font.height() / 2.0,
      BaselineShift::Superscript => shift += font.height() / 2.0,
      BaselineShift::Number(n) => shift += n,
    }
  }

  shift
}

fn resolve_clusters_positions_path(
  text: &Text,
  chunk: &TextChunk,
  char_offset: usize,
  path: &TextPath,
  writing_mode: WritingMode,
  fonts_cache: &FontsCache,
  clusters: &mut [GlyphCluster],
) -> (f32, f32) {
  let mut last_x = 0.0;
  let mut last_y = 0.0;

  let mut dy = 0.0;

  // In the text path mode, chunk's x/y coordinates provide an additional offset along the path.
  // The X coordinate is used in a horizontal mode, and Y in vertical.
  let chunk_offset = match writing_mode {
    WritingMode::LeftToRight => chunk.x.unwrap_or(0.0),
    WritingMode::TopToBottom => chunk.y.unwrap_or(0.0),
  };

  let start_offset =
    chunk_offset + path.start_offset + process_anchor(chunk.anchor, clusters_length(clusters));

  let normals = collect_normals(text, chunk, clusters, &path.path, char_offset, start_offset);
  for (cluster, normal) in clusters.iter_mut().zip(normals) {
    let (x, y, angle) = match normal {
      Some(normal) => (normal.x, normal.y, normal.angle),
      None => {
        // Hide clusters that are outside the text path.
        cluster.visible = false;
        continue;
      }
    };

    // We have to break a decoration line for each cluster during text-on-path.
    cluster.has_relative_shift = true;

    let orig_ts = cluster.transform;

    // Clusters should be rotated by the x-midpoint x baseline position.
    let half_width = cluster.width / 2.0;
    cluster.transform = Transform::default();
    cluster.transform = cluster.transform.pre_translate(x - half_width, y);
    cluster.transform = cluster.transform.pre_rotate_at(angle, half_width, 0.0);

    let cp = char_offset + cluster.byte_idx.code_point_at(&chunk.text);
    dy += text.dy.get(cp).cloned().unwrap_or(0.0);

    let baseline_shift = chunk_span_at(chunk, cluster.byte_idx)
      .map(|span| {
        let font = match fonts_cache.primary(span) {
          Some(v) => v,
          None => return 0.0,
        };
        -resolve_baseline(span, font, writing_mode)
      })
      .unwrap_or(0.0);

    // Shift only by `dy` since we already applied `dx`
    // during offset along the path calculation.
    if !dy.approx_zero_ulps(4) || !baseline_shift.approx_zero_ulps(4) {
      let shift = kurbo::Vec2::new(0.0, (dy - baseline_shift) as f64);
      cluster.transform = cluster
        .transform
        .pre_translate(shift.x as f32, shift.y as f32);
    }

    if let Some(angle) = text.rotate.get(cp).cloned() {
      if !angle.approx_zero_ulps(4) {
        cluster.transform = cluster.transform.pre_rotate(angle);
      }
    }

    // The possible `lengthAdjust` transform should be applied after text-on-path positioning.
    cluster.transform = cluster.transform.pre_concat(orig_ts);

    last_x = x + cluster.advance;
    last_y = y;
  }

  (last_x, last_y)
}

pub(crate) fn process_anchor(a: TextAnchor, text_width: f32) -> f32 {
  match a {
    TextAnchor::Start => 0.0, // Nothing.
    TextAnchor::Middle => -text_width / 2.0,
    TextAnchor::End => -text_width,
  }
}

pub(crate) struct PathNormal {
  pub(crate) x: f32,
  pub(crate) y: f32,
  pub(crate) angle: f32,
}

fn collect_normals(
  text: &Text,
  chunk: &TextChunk,
  clusters: &[GlyphCluster],
  path: &tiny_skia::Path,
  char_offset: usize,
  offset: f32,
) -> Vec<Option<PathNormal>> {
  let mut offsets = Vec::with_capacity(clusters.len());
  let mut normals = Vec::with_capacity(clusters.len());
  {
    let mut advance = offset;
    for cluster in clusters {
      // Clusters should be rotated by the x-midpoint x baseline position.
      let half_width = cluster.width / 2.0;

      // Include relative position.
      let cp = char_offset + cluster.byte_idx.code_point_at(&chunk.text);
      advance += text.dx.get(cp).cloned().unwrap_or(0.0);

      let offset = advance + half_width;

      // Clusters outside the path have no normals.
      if offset < 0.0 {
        normals.push(None);
      }

      offsets.push(offset as f64);
      advance += cluster.advance;
    }
  }

  let mut prev_mx = path.points()[0].x;
  let mut prev_my = path.points()[0].y;
  let mut prev_x = prev_mx;
  let mut prev_y = prev_my;

  fn create_curve_from_line(px: f32, py: f32, x: f32, y: f32) -> kurbo::CubicBez {
    let line = kurbo::Line::new(
      kurbo::Point::new(px as f64, py as f64),
      kurbo::Point::new(x as f64, y as f64),
    );
    let p1 = line.eval(0.33);
    let p2 = line.eval(0.66);
    kurbo::CubicBez {
      p0: line.p0,
      p1,
      p2,
      p3: line.p1,
    }
  }

  let mut length: f64 = 0.0;
  for seg in path.segments() {
    let curve = match seg {
      tiny_skia::PathSegment::MoveTo(p) => {
        prev_mx = p.x;
        prev_my = p.y;
        prev_x = p.x;
        prev_y = p.y;
        continue;
      }
      tiny_skia::PathSegment::LineTo(p) => create_curve_from_line(prev_x, prev_y, p.x, p.y),
      tiny_skia::PathSegment::QuadTo(p1, p) => kurbo::QuadBez {
        p0: kurbo::Point::new(prev_x as f64, prev_y as f64),
        p1: kurbo::Point::new(p1.x as f64, p1.y as f64),
        p2: kurbo::Point::new(p.x as f64, p.y as f64),
      }
      .raise(),
      tiny_skia::PathSegment::CubicTo(p1, p2, p) => kurbo::CubicBez {
        p0: kurbo::Point::new(prev_x as f64, prev_y as f64),
        p1: kurbo::Point::new(p1.x as f64, p1.y as f64),
        p2: kurbo::Point::new(p2.x as f64, p2.y as f64),
        p3: kurbo::Point::new(p.x as f64, p.y as f64),
      },
      tiny_skia::PathSegment::Close => create_curve_from_line(prev_x, prev_y, prev_mx, prev_my),
    };

    let arclen_accuracy = {
      let base_arclen_accuracy = 0.5;
      // Accuracy depends on a current scale.
      // When we have a tiny path scaled by a large value,
      // we have to increase out accuracy accordingly.
      let (sx, sy) = text.abs_transform.get_scale();
      // 1.0 acts as a threshold to prevent division by 0 and/or low accuracy.
      base_arclen_accuracy / (sx * sy).sqrt().max(1.0)
    };

    let curve_len = curve.arclen(arclen_accuracy as f64);

    for offset in &offsets[normals.len()..] {
      if *offset >= length && *offset <= length + curve_len {
        let mut offset = curve.inv_arclen(offset - length, arclen_accuracy as f64);
        // some rounding error may occur, so we give offset a little tolerance
        debug_assert!((-1.0e-3..=1.0 + 1.0e-3).contains(&offset));
        offset = offset.clamp(0.0, 1.0);

        let pos = curve.eval(offset);
        let d = curve.deriv().eval(offset);
        let d = kurbo::Vec2::new(-d.y, d.x); // tangent
        let angle = d.atan2().to_degrees() - 90.0;

        normals.push(Some(PathNormal {
          x: pos.x as f32,
          y: pos.y as f32,
          angle: angle as f32,
        }));

        if normals.len() == offsets.len() {
          break;
        }
      }
    }

    length += curve_len;
    prev_x = curve.p3.x as f32;
    prev_y = curve.p3.y as f32;
  }

  // If path ended and we still have unresolved normals - set them to `None`.
  for _ in 0..(offsets.len() - normals.len()) {
    normals.push(None);
  }

  normals
}

/// Shapes a text chunk into clusters at the 0x0 position, in visual order.
fn process_chunk(chunk: &TextChunk, fonts_cache: &mut FontsCache) -> Vec<GlyphCluster> {
  let glyphs = shape_chunk(chunk, fonts_cache);
  let mut clusters = Vec::new();

  for (range, byte_idx) in GlyphClusters::new(&glyphs) {
    if chunk_span_at(chunk, byte_idx).is_some() {
      clusters.push(form_glyph_clusters(&glyphs[range], &chunk.text));
    }
  }

  clusters
}

/// Shapes `chunk` through `HarfBuzzShaper` and spaces it through `ShapeResultSpacing`, in visual
/// order. Each span shapes with its own font and spacing, as Blink's `InlineNode::ShapeText`
/// shapes each item, so a bidi run is shaped piece by piece at span boundaries.
fn shape_chunk(chunk: &TextChunk, fonts_cache: &mut FontsCache) -> Vec<Glyph> {
  let bidi = BidiParagraph::new(&chunk.text, Some(TextDirection::Ltr));
  let shaper = HarfBuzzShaper::new(&chunk.text);
  let mut spacing = ShapeResultSpacing::new(&chunk.text, true);
  let mut glyphs = Vec::new();

  for run in bidi.get_visual_runs() {
    let direction = direction_from_level(run.level);
    let mut pieces: Vec<(&TextSpan, usize, usize)> = chunk
      .spans
      .iter()
      .filter_map(|span| {
        let start = span.start.max(run.start);
        let end = span.end.min(run.end);

        (start < end).then_some((span, start, end))
      })
      .collect();

    if direction == TextDirection::Rtl {
      pieces.reverse();
    }

    for (span, start, end) in pieces {
      let Some(font) = fonts_cache.shaping_font(span) else {
        continue;
      };
      let mut result = shaper.shape(&font, direction, start, end);

      if spacing.set_spacing_from(
        font.description.letter_spacing,
        font.description.word_spacing,
      ) {
        result.apply_spacing(&mut spacing, 0);
      }

      let mut shaped = Vec::new();
      let total_advance = ShapeResultView::create_from_result(&Rc::new(result)).for_each_glyph(
        0.0,
        |character_index, glyph, offset, advance, font_data| {
          shaped.push((character_index, glyph, offset, advance, font_data.cloned()));
        },
      );

      for (index, (character_index, glyph, offset, advance, font_data)) in shaped.iter().enumerate()
      {
        let next_advance = shaped
          .get(index + 1)
          .map_or(total_advance, |&(_, _, _, next_advance, _)| next_advance);
        let Some(font) = font_data
          .as_ref()
          .and_then(|font_data| fonts_cache.resolved(font_data))
        else {
          continue;
        };

        glyphs.push(Glyph {
          id: GlyphId(u32::from(*glyph)),
          byte_idx: ByteIndex::new(*character_index as usize),
          dx: offset.x,
          dy: offset.y,
          width: next_advance - advance,
          font,
        });
      }
    }
  }

  glyphs
}

fn apply_length_adjust(chunk: &TextChunk, clusters: &mut [GlyphCluster]) {
  let is_horizontal = matches!(chunk.text_flow, TextFlow::Linear);

  for span in &chunk.spans {
    let target_width = match span.text_length {
      Some(v) => v,
      None => continue,
    };

    let mut width = 0.0;
    let mut cluster_indexes = Vec::new();
    for i in span.start..span.end {
      if let Some(index) = clusters.iter().position(|c| c.byte_idx.value() == i) {
        cluster_indexes.push(index);
      }
    }
    // Complex scripts can have multi-codepoint clusters therefore we have to remove duplicates.
    sort_by_key(&mut cluster_indexes, |&index| index);
    cluster_indexes.dedup();

    for i in &cluster_indexes {
      // Use the original cluster `width` and not `advance`.
      width += clusters[*i].width;
    }

    if cluster_indexes.is_empty() {
      continue;
    }

    if span.length_adjust == LengthAdjust::Spacing {
      let factor = if cluster_indexes.len() > 1 {
        (target_width - width) / (cluster_indexes.len() - 1) as f32
      } else {
        0.0
      };

      for i in cluster_indexes {
        clusters[i].advance = clusters[i].width + factor;
      }
    } else {
      let factor = target_width / width;
      // Prevent multiplying by zero.
      if factor < 0.001 {
        continue;
      }

      for i in cluster_indexes {
        clusters[i].transform = clusters[i].transform.pre_scale(factor, 1.0);

        // Technically just a hack to support the current text-on-path algorithm.
        if !is_horizontal {
          clusters[i].advance *= factor;
          clusters[i].width *= factor;
        }
      }
    }
  }
}

/// Rotates clusters according to
/// [Unicode Vertical_Orientation Property](https://www.unicode.org/reports/tr50/tr50-19.html).
fn apply_writing_mode(writing_mode: WritingMode, _clusters: &mut [GlyphCluster]) {
  // Vertical writing modes are unsupported (the parser never emits
  // `TopToBottom`), so upstream's Vertical_Orientation rotation is stripped.
  debug_assert!(writing_mode != WritingMode::TopToBottom);
}

fn form_glyph_clusters(glyphs: &[Glyph], text: &str) -> GlyphCluster {
  debug_assert!(!glyphs.is_empty());

  let mut x = 0.0;
  let mut width = 0.0;

  let mut positioned_glyphs = vec![];

  for glyph in glyphs {
    // The first glyph in the cluster will have an offset from 0x0,
    // but the later one will have an offset from the "current position".
    // So we have to keep an advance.
    positioned_glyphs.push(PositionedGlyph {
      glyph_ts: Transform::from_translate(x + glyph.dx, glyph.dy),
      // Will be set later.
      cluster_ts: Transform::default(),
      // Will be set later.
      span_ts: Transform::default(),
      id: glyph.id,
      font: Arc::clone(&glyph.font),
    });

    x += glyph.width;

    if glyph.width > width {
      width = glyph.width;
    }
  }

  let byte_idx = glyphs[0].byte_idx;
  let font = &glyphs[0].font;
  GlyphCluster {
    byte_idx,
    codepoint: byte_idx.char_from(text),
    width,
    advance: x,
    ascent: font.ascent(),
    descent: font.descent(),
    has_relative_shift: false,
    transform: Transform::default(),
    path_transform: Transform::default(),
    glyphs: positioned_glyphs,
    visible: true,
  }
}

/// An iterator over glyph clusters.
///
/// Input:  0 2 2 2 3 4 4 5 5
/// Result: 0 1     4 5   7
pub(crate) struct GlyphClusters<'a> {
  data: &'a [Glyph],
  idx: usize,
}

impl<'a> GlyphClusters<'a> {
  pub(crate) fn new(data: &'a [Glyph]) -> Self {
    GlyphClusters { data, idx: 0 }
  }
}

impl Iterator for GlyphClusters<'_> {
  type Item = (std::ops::Range<usize>, ByteIndex);

  fn next(&mut self) -> Option<Self::Item> {
    if self.idx == self.data.len() {
      return None;
    }

    let start = self.idx;
    let cluster = self.data[self.idx].byte_idx;
    for g in &self.data[self.idx..] {
      if g.byte_idx != cluster {
        break;
      }

      self.idx += 1;
    }

    Some((start..self.idx, cluster))
  }
}

/// A glyph.
///
/// Basically, a glyph ID and it's metrics.
#[derive(Clone)]
pub(crate) struct Glyph {
  /// The glyph ID in the font.
  pub(crate) id: GlyphId,

  /// Position in bytes in the original string.
  ///
  /// We use it to match a glyph with a character in the text chunk and therefore with the style.
  pub(crate) byte_idx: ByteIndex,

  /// The glyph offset in pixels.
  pub(crate) dx: f32,

  /// The glyph offset in pixels, downward.
  pub(crate) dy: f32,

  /// The glyph width / X-advance in pixels.
  pub(crate) width: f32,

  /// Reference to the source font.
  ///
  /// Each glyph can have it's own source font.
  pub(crate) font: Arc<ResolvedFont>,
}

/// A face at the size and variation coordinates a glyph was shaped with, and its metrics.
#[derive(Debug)]
pub(crate) struct ResolvedFont {
  pub(crate) typeface: Arc<FontCustomPlatformData>,
  /// The normalized variation coordinates the face was shaped at.
  normalized_coords: Vec<F2Dot14>,
  size: f32,
  synthesis: RunSynthesis,
  metrics: PrimaryFontMetrics,
}

/// What of a span selects and shapes its font.
#[derive(Clone, PartialEq, Eq, Hash)]
struct SpanFontKey {
  font: Font,
  font_size: u32,
  small_caps: bool,
  apply_kerning: bool,
  font_optical_sizing: FontOpticalSizing,
  letter_spacing: u32,
  word_spacing: u32,
}

impl SpanFontKey {
  fn of(span: &TextSpan) -> Self {
    Self {
      font: span.font.clone(),
      font_size: span.font_size.get().to_bits(),
      small_caps: span.small_caps,
      apply_kerning: span.apply_kerning,
      font_optical_sizing: span.font_optical_sizing,
      letter_spacing: span.letter_spacing.to_bits(),
      word_spacing: span.word_spacing.to_bits(),
    }
  }

  /// The span's presentation attributes as Blink's `FontBuilder` would resolve them. Approximate:
  /// Blink shapes SVG text at its scaled size (`LayoutSVGInlineText::ScalingFactor`), where this
  /// shapes at the size in user units.
  fn description(&self) -> FontDescription {
    let font_size = f32::from_bits(self.font_size);
    let families = self
      .font
      .families
      .iter()
      .map(|family| match family {
        FontFamily::Serif => FontFamilyToken::Generic(GenericFamilyType::Serif),
        FontFamily::SansSerif => FontFamilyToken::Generic(GenericFamilyType::SansSerif),
        FontFamily::Cursive => FontFamilyToken::Generic(GenericFamilyType::Cursive),
        FontFamily::Fantasy => FontFamilyToken::Generic(GenericFamilyType::Fantasy),
        FontFamily::Monospace => FontFamilyToken::Generic(GenericFamilyType::Monospace),
        FontFamily::Named(name) => FontFamilyToken::Owned(name.clone()),
      })
      .collect();
    let slope = match self.font.style {
      FontStyle::Normal => NORMAL_SLOPE_VALUE,
      FontStyle::Italic | FontStyle::Oblique => ITALIC_SLOPE_VALUE,
    };
    let letter_spacing = f32::from_bits(self.letter_spacing);
    let mut font_features = Vec::new();

    FontVariantLigatures::default().append_features(letter_spacing != 0.0, &mut font_features);

    let mut features: Vec<Feature> = font_features
      .into_iter()
      .map(FontFeature::into_harfrust)
      .collect();

    if !self.apply_kerning {
      features.push(Feature::new(HbTag::new(b"kern"), 0, ..));
    }

    FontDescription {
      fallback: FontFallbackKey {
        family: Rc::new(ExpandedFontFamily::new(families)),
        selection: FontSelectionDescription {
          font_selection_request: FontSelectionRequest::new(
            FontSelectionValue::new(f32::from(self.font.weight)),
            FontSelectionValue::new(self.font.stretch.percentage()),
            slope,
          ),
          is_italic_keyword: self.font.style == FontStyle::Italic,
          synthetic_bold_allowed: true,
          synthetic_italic_allowed: true,
          is_synthetic_bold: false,
          is_synthetic_italic: false,
          effective_font_size: font_size,
          adjusted_specified_size: font_size,
          font_optical_sizing: match self.font_optical_sizing {
            FontOpticalSizing::Auto => CssFontOpticalSizing::Auto,
            FontOpticalSizing::None => CssFontOpticalSizing::None,
          },
          variation_settings: self
            .font
            .variations
            .iter()
            .map(|variation| Variation {
              tag: HbTag::from_be_bytes(variation.tag),
              value: variation.value,
            })
            .collect(),
        },
        computed_size: font_size,
        specified_size: font_size,
      },
      features,
      locale: None,
      language_override: FontLanguageOverride::Normal,
      variant_caps: if self.small_caps {
        FontVariantCaps::SmallCaps
      } else {
        FontVariantCaps::Normal
      },
      font_synthesis_small_caps: FontSynthesic::Auto,
      letter_spacing,
      word_spacing: f32::from_bits(self.word_spacing),
      is_letter_spacing_fixed: true,
      is_word_spacing_fixed: true,
    }
  }
}

/// A span's shaping font, and the metrics of its primary font.
struct SpanFont {
  font: Rc<ShapingFont>,
  primary: Arc<ResolvedFont>,
}

/// The fonts a `<text>`'s spans shape with, and the faces shaping picked.
pub(crate) struct FontsCache {
  spans: HashMap<SpanFontKey, SpanFont>,
  faces: Vec<(Rc<SimpleFontData>, Arc<ResolvedFont>)>,
}

impl FontsCache {
  fn new(text_node: &Text, cache: &Cache) -> Self {
    let mut fonts_cache = Self {
      spans: HashMap::new(),
      faces: Vec::new(),
    };

    for span in text_node.chunks.iter().flat_map(|chunk| &chunk.spans) {
      let key = SpanFontKey::of(span);

      if fonts_cache.spans.contains_key(&key) {
        continue;
      }

      let description = key.description();
      let font = ShapingFont {
        fallback_list: cache
          .font_fallback_map
          .get(&cache.fonts, &description.fallback),
        description,
      };
      let Some(primary) = font
        .primary_font()
        .and_then(|font_data| fonts_cache.resolved(&font_data))
      else {
        continue;
      };

      fonts_cache.spans.insert(
        key,
        SpanFont {
          font: Rc::new(font),
          primary,
        },
      );
    }

    fonts_cache
  }

  /// The metrics of `span`'s primary font.
  fn primary(&self, span: &TextSpan) -> Option<&Arc<ResolvedFont>> {
    self
      .spans
      .get(&SpanFontKey::of(span))
      .map(|span_font| &span_font.primary)
  }

  fn shaping_font(&self, span: &TextSpan) -> Option<Rc<ShapingFont>> {
    self
      .spans
      .get(&SpanFontKey::of(span))
      .map(|span_font| Rc::clone(&span_font.font))
  }

  fn resolved(&mut self, font_data: &Rc<SimpleFontData>) -> Option<Arc<ResolvedFont>> {
    if let Some((_, resolved)) = self
      .faces
      .iter()
      .find(|(cached, _)| Rc::ptr_eq(cached, font_data))
    {
      return Some(Arc::clone(resolved));
    }

    let resolved = Arc::new(ResolvedFont::new(font_data)?);

    self
      .faces
      .push((Rc::clone(font_data), Arc::clone(&resolved)));
    Some(resolved)
  }
}

pub(crate) fn chunk_span_at(chunk: &TextChunk, byte_offset: ByteIndex) -> Option<&TextSpan> {
  chunk
    .spans
    .iter()
    .find(|&span| span_contains(span, byte_offset))
}

pub(crate) fn span_contains(span: &TextSpan, byte_offset: ByteIndex) -> bool {
  byte_offset.value() >= span.start && byte_offset.value() < span.end
}

/// Checks that the selected character is a word separator.
///
/// According to: https://www.w3.org/TR/css-text-3/#word-separator
impl ResolvedFont {
  /// `font_data`'s face and metrics; none for a face without metrics.
  fn new(font_data: &SimpleFontData) -> Option<Self> {
    Some(Self {
      typeface: Arc::clone(&font_data.face.typeface),
      normalized_coords: font_data.normalized_coords(),
      size: font_data.size,
      synthesis: font_data.synthesis(),
      metrics: font_data.font_metrics()?,
    })
  }

  /// The face its glyphs resolve against.
  pub(crate) fn face(&self) -> RunFace<'_> {
    RunFace {
      font_id: self.typeface.id,
      font_index: self.typeface.index,
      font_size: self.size,
      normalized_coords: &self.normalized_coords,
      synthesis: self.synthesis,
    }
  }

  pub(crate) fn ascent(&self) -> f32 {
    self.metrics.ascent
  }

  /// Negative, below the baseline.
  pub(crate) fn descent(&self) -> f32 {
    -self.metrics.descent
  }

  pub(crate) fn height(&self) -> f32 {
    self.metrics.float_height()
  }

  /// How far the text moves down to put `baseline` where its alphabetic baseline was, as Blink
  /// aligns SVG text.
  fn baseline_shift(&self, baseline: FontBaseline) -> f32 {
    self.metrics.ascent - self.metrics.float_ascent(baseline)
  }

  /// The center of a `line` decoration below the baseline, and its thickness, as Blink's
  /// `TextDecorationInfo` places it.
  fn decoration_line(&self, line: TextDecorationLines) -> (f32, f32) {
    let font = DecorationFont::new(self.metrics, self.size);
    let thickness = font.thickness(SizedTextDecorationThickness::Auto);
    let top = DecoratingBox::new(font, 0.0).line_top(
      line,
      (TextUnderlinePosition::Auto, None),
      -self.ascent(),
      thickness,
    );

    (top + thickness / 2.0, thickness)
  }
}

impl DominantBaseline {
  /// Blink's `ComputedStyle::GetFontBaseline`.
  fn font_baseline(self) -> FontBaseline {
    match self {
      Self::Auto | Self::UseScript | Self::NoChange | Self::ResetSize | Self::Alphabetic => {
        FontBaseline::Alphabetic
      }
      Self::Ideographic => FontBaseline::IdeographicUnder,
      Self::Hanging => FontBaseline::Hanging,
      Self::Mathematical => FontBaseline::Math,
      Self::Central => FontBaseline::Central,
      Self::Middle => FontBaseline::XMiddle,
      Self::TextAfterEdge => FontBaseline::TextUnder,
      Self::TextBeforeEdge => FontBaseline::TextOver,
    }
  }
}

impl AlignmentBaseline {
  /// Blink's `InlineBoxState::ResetStyle` for SVG text; none where it takes the dominant baseline.
  fn font_baseline(self) -> Option<FontBaseline> {
    match self {
      Self::Auto | Self::Baseline => None,
      Self::BeforeEdge | Self::TextBeforeEdge => Some(FontBaseline::TextOver),
      Self::Middle => Some(FontBaseline::XMiddle),
      Self::Central => Some(FontBaseline::Central),
      Self::AfterEdge | Self::TextAfterEdge => Some(FontBaseline::TextUnder),
      Self::Ideographic => Some(FontBaseline::IdeographicUnder),
      Self::Alphabetic => Some(FontBaseline::Alphabetic),
      Self::Hanging => Some(FontBaseline::Hanging),
      Self::Mathematical => Some(FontBaseline::Math),
    }
  }
}

/// A read-only text index in bytes.
///
/// Guarantee to be on a char boundary and in text bounds.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct ByteIndex(usize);

impl ByteIndex {
  fn new(i: usize) -> Self {
    ByteIndex(i)
  }

  pub(crate) fn value(&self) -> usize {
    self.0
  }

  /// Converts byte position into a code point position.
  pub(crate) fn code_point_at(&self, text: &str) -> usize {
    text
      .char_indices()
      .take_while(|(i, _)| *i != self.0)
      .count()
  }

  /// Converts byte position into a character.
  pub(crate) fn char_from(&self, text: &str) -> char {
    text[self.0..].chars().next().unwrap()
  }
}
