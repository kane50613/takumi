//! Positioning the clusters of a chunk, after Blink's `SvgTextLayoutAlgorithm`: absolute and
//! relative offsets, rotation, `textLength`, anchoring, baselines, and placement along a
//! `textPath`.

use std::rc::Rc;

use kurbo::{
  CubicBez, Line, ParamCurve, ParamCurveArclen, ParamCurveDeriv, Point as KurboPoint, QuadBez, Vec2,
};

use super::{
  DecorationStyle, Fill, LengthAdjust, SpanStyle, Stroke, Text, TextChunk, TextFlow, TextPath,
  TextPathShape,
  shape::{FontFaces, GlyphCluster, PositionedGlyph, ResolvedFont},
};
use crate::{
  geometry::{PathCommand, Point, Rect},
  style::{Affine, Angle, TextAnchor, TextDecorationLines},
  svg::layout::approx_eq::ApproxEq,
};

/// The glyphs of a span that share one paint.
pub(super) struct LaidOutSpan {
  pub(super) fill: Option<Fill>,
  pub(super) stroke: Option<Stroke>,
  pub(super) stroke_first: bool,
  pub(super) visible: bool,
  pub(super) glyphs: Vec<PositionedGlyph>,
  /// Painted before the glyphs.
  pub(super) underline: Option<DecorationLine>,
  /// Painted before the glyphs.
  pub(super) overline: Option<DecorationLine>,
  /// Painted after the glyphs.
  pub(super) line_through: Option<DecorationLine>,
}

/// A decoration line to paint with the paint of the element that declared it.
pub(super) struct DecorationLine {
  pub(super) commands: Vec<PathCommand>,
  pub(super) fill: Option<Fill>,
  pub(super) stroke: Option<Stroke>,
  pub(super) visible: bool,
}

/// The spans of a `text` and the box its characters take, measured from font metrics and not from
/// glyph outlines.
pub(super) struct LaidOutText {
  pub(super) spans: Vec<LaidOutSpan>,
  pub(super) bounds: Rect<f32>,
}

/// A horizontal stretch of a decoration line: its width and where it starts.
#[derive(Clone, Copy)]
struct DecorationSpan {
  width: f32,
  transform: Affine,
}

/// How closely the curves of a `textPath` are measured, in user units.
const ARCLEN_ACCURACY: f64 = 0.1;

/// How closely `curve` is measured: [`ARCLEN_ACCURACY`], loosened for a long curve so that its
/// cost stays bounded by the ratio rather than by its size.
fn arclen_accuracy(curve: &CubicBez) -> f64 {
  let polygon =
    curve.p0.distance(curve.p1) + curve.p1.distance(curve.p2) + curve.p2.distance(curve.p3);

  (polygon * 1e-4).max(ARCLEN_ACCURACY)
}

/// A point of a `textPath` a cluster sits on, and the direction of the path there.
struct PathNormal {
  x: f32,
  y: f32,
  angle: f32,
}

impl Text {
  /// Lays the chunks out, none when no character has a box.
  pub(super) fn lay_out(&self, faces: &mut FontFaces) -> Option<LaidOutText> {
    let mut spans = Vec::new();
    let mut last_x = 0.0;
    let mut last_y = 0.0;
    let mut bounds: Option<Rect<f32>> = None;
    let mut expand = |rect: Rect<f32>| {
      bounds = Some(bounds.map_or(rect, |bounds| bounds.union(&rect)));
    };

    for chunk in &self.chunks {
      let (x, y) = match chunk.flow {
        TextFlow::Linear => (chunk.x.unwrap_or(last_x), chunk.y.unwrap_or(last_y)),
        TextFlow::Path(_) => (0.0, 0.0),
      };
      let mut clusters = chunk.clusters(faces);

      if clusters.is_empty() {
        continue;
      }

      chunk.apply_length_adjust(&mut clusters);

      let characters = ByteCharacters::new(&chunk.text);
      let current = self.resolve_cluster_positions(chunk, &characters, &mut clusters);
      let span_of = chunk.cluster_spans(&clusters);
      let mut decoration_runs = collect_decoration_spans(&clusters, &span_of, chunk.spans.len());
      let mut members = vec![Vec::new(); chunk.spans.len()];

      for (cluster, span) in clusters.iter().zip(&span_of) {
        if let (Some(span), true) = (span, cluster.visible) {
          members[*span].push(cluster);
        }
      }

      // The box of every character of the chunk, which each of its spans reports.
      let chunk_box = clusters
        .iter()
        .filter(|cluster| cluster.visible)
        .filter_map(GlyphCluster::metrics_box)
        .reduce(|union, rect| union.union(&rect));

      for (index, span) in chunk.spans.iter().enumerate() {
        let style = &span.style;
        let Some(font) = style.font.as_ref().map(|font| &font.primary) else {
          continue;
        };
        let decoration_spans = std::mem::take(&mut decoration_runs[index]);
        let mut span_transform = Affine::translation(x, y);

        if matches!(chunk.flow, TextFlow::Linear) {
          // The shift moves the transform and not the clusters, which can be rotated.
          span_transform *= Affine::translation(0.0, style.baseline_shift(font));
        }

        let mut decorate = |line, decoration: &Option<Rc<DecorationStyle>>| {
          let decoration = decoration.as_ref()?;
          let (center, thickness) = font.decoration_line(line);
          let (commands, control_bounds) =
            decoration_line((center, thickness), &decoration_spans, span_transform)?;

          expand(control_bounds);

          Some(DecorationLine {
            commands,
            fill: decoration.fill.clone(),
            stroke: decoration.stroke.clone(),
            visible: style.visible,
          })
        };
        let underline = decorate(TextDecorationLines::UNDERLINE, &style.decorations.underline);
        let overline = decorate(TextDecorationLines::OVERLINE, &style.decorations.overline);
        let line_through = decorate(
          TextDecorationLines::LINE_THROUGH,
          &style.decorations.line_through,
        );
        let mut fill = style.fill.clone();

        // The fill rule does not apply to text.
        // https://www.w3.org/TR/SVG2/text.html#TextRenderingOrder
        if let Some(fill) = fill.as_mut() {
          fill.evenodd = false;
        }

        let Some(span_box) = chunk_box
          .and_then(|rect| rect.transformed(span_transform))
          .filter(Rect::has_area)
        else {
          continue;
        };

        expand(span_box);

        let glyphs = members[index]
          .iter()
          .flat_map(|cluster| {
            cluster.glyphs.iter().map(|glyph| PositionedGlyph {
              cluster_transform: cluster.transform,
              span_transform,
              ..glyph.clone()
            })
          })
          .collect();

        spans.push(LaidOutSpan {
          fill,
          stroke: style.stroke.clone(),
          stroke_first: style.stroke_first,
          visible: style.visible,
          glyphs,
          underline,
          overline,
          line_through,
        });
      }

      last_x = x + current.0;
      last_y = y + current.1;
    }

    Some(LaidOutText {
      spans,
      bounds: bounds.filter(Rect::has_area)?,
    })
  }

  /// Sets the transforms of the clusters of a chunk, and returns where the text ends, from which
  /// the next chunk starts.
  fn resolve_cluster_positions(
    &self,
    chunk: &TextChunk,
    characters: &ByteCharacters,
    clusters: &mut [GlyphCluster],
  ) -> (f32, f32) {
    match &chunk.flow {
      TextFlow::Linear => self.resolve_linear_positions(chunk, characters, clusters),
      TextFlow::Path(path) => self.resolve_path_positions(chunk, characters, path, clusters),
    }
  }

  fn resolve_linear_positions(
    &self,
    chunk: &TextChunk,
    characters: &ByteCharacters,
    clusters: &mut [GlyphCluster],
  ) -> (f32, f32) {
    let mut x = anchor_offset(chunk.anchor, clusters_length(clusters));
    let mut y = 0.0;

    for cluster in clusters {
      let character = chunk.characters[characters.index(cluster.byte_index)];

      if let (Some(dx), Some(dy)) = (self.dx.get(character), self.dy.get(character)) {
        x += dx;
        y += dy;
        cluster.has_relative_shift = !dx.approx_eq(0.0) || !dy.approx_eq(0.0);
      }

      cluster.transform *= Affine::translation(x, y);

      if let Some(angle) = self.rotate.get(character).copied()
        && !angle.approx_eq(0.0)
      {
        cluster.transform *= Affine::rotation(Angle::new(angle));
        cluster.has_relative_shift = true;
      }

      x += cluster.advance;
    }

    (x, y)
  }

  fn resolve_path_positions(
    &self,
    chunk: &TextChunk,
    characters: &ByteCharacters,
    path: &TextPath,
    clusters: &mut [GlyphCluster],
  ) -> (f32, f32) {
    let mut last_x = 0.0;
    let mut last_y = 0.0;
    let mut dy = 0.0;
    // In the path mode, the `x` of a chunk is an additional offset along the path.
    let start_offset = chunk.x.unwrap_or(0.0)
      + path.start_offset
      + anchor_offset(chunk.anchor, clusters_length(clusters));
    let normals = self.collect_normals(chunk, characters, clusters, path, start_offset);

    for (cluster, normal) in clusters.iter_mut().zip(normals) {
      let Some(normal) = normal else {
        // A cluster outside the path is hidden.
        cluster.visible = false;
        continue;
      };

      // A decoration line breaks at each cluster along a path.
      cluster.has_relative_shift = true;

      let original = cluster.transform;
      // A cluster rotates around the middle of its baseline.
      let half_width = cluster.width / 2.0;

      cluster.transform = Affine::translation(normal.x - half_width, normal.y)
        * Affine::translation(half_width, 0.0)
        * Affine::rotation(Angle::new(normal.angle))
        * Affine::translation(-half_width, 0.0);

      let character = chunk.characters[characters.index(cluster.byte_index)];

      dy += self.dy.get(character).copied().unwrap_or(0.0);

      let baseline_shift = chunk
        .span_at(cluster.byte_index)
        .and_then(|span| {
          span
            .style
            .font
            .as_ref()
            .map(|font| -span.style.baseline_shift(&font.primary))
        })
        .unwrap_or(0.0);

      // The shift is by `dy` alone, as `dx` is in the offset along the path.
      if !dy.approx_eq(0.0) || !baseline_shift.approx_eq(0.0) {
        cluster.transform *= Affine::translation(0.0, dy - baseline_shift);
      }

      if let Some(angle) = self.rotate.get(character).copied()
        && !angle.approx_eq(0.0)
      {
        cluster.transform *= Affine::rotation(Angle::new(angle));
      }

      // A `lengthAdjust` transform goes on after the placement along the path.
      cluster.transform *= original;

      last_x = normal.x + cluster.advance;
      last_y = normal.y;
    }

    (last_x, last_y)
  }

  /// The point and direction of the path under the middle of each cluster, none for one off its
  /// ends. Each cluster is placed by its own offset along the path.
  fn collect_normals(
    &self,
    chunk: &TextChunk,
    characters: &ByteCharacters,
    clusters: &[GlyphCluster],
    path: &TextPath,
    offset: f32,
  ) -> Vec<Option<PathNormal>> {
    let mut advance = offset;

    clusters
      .iter()
      .map(|cluster| {
        // A cluster is placed by the middle of its baseline.
        let character = chunk.characters[characters.index(cluster.byte_index)];

        advance += self.dx.get(character).copied().unwrap_or(0.0);

        let at = f64::from(advance + cluster.width / 2.0);

        advance += cluster.advance;
        path.path.normal_at(at)
      })
      .collect()
  }
}

impl TextPathShape {
  /// The path with the curves its commands make and where each ends along it.
  pub(super) fn new(commands: Vec<PathCommand>) -> Self {
    let curves = path_curves(&commands);
    let mut length = 0.0;
    let ends = curves
      .iter()
      .map(|curve| {
        length += curve.arclen(arclen_accuracy(curve));
        length
      })
      .collect();

    Self {
      commands,
      curves,
      ends,
    }
  }

  /// How long the path is.
  pub(super) fn length(&self) -> f64 {
    self.ends.last().copied().unwrap_or(0.0)
  }

  /// The point and direction of the path at `offset` along it, none beyond its ends.
  fn normal_at(&self, offset: f64) -> Option<PathNormal> {
    if offset < 0.0 {
      return None;
    }

    let index = self.ends.partition_point(|end| *end < offset);
    let curve = self.curves.get(index)?;
    let start = index
      .checked_sub(1)
      .map_or(0.0, |previous| self.ends[previous]);
    let position = curve
      .inv_arclen(offset - start, arclen_accuracy(curve))
      .clamp(0.0, 1.0);
    let point = curve.eval(position);
    let derivative = curve.deriv().eval(position);
    let tangent = Vec2::new(-derivative.y, derivative.x);

    Some(PathNormal {
      x: point.x as f32,
      y: point.y as f32,
      angle: (tangent.atan2().to_degrees() - 90.0) as f32,
    })
  }
}

impl TextChunk {
  /// Stretches the spans that have a `textLength`.
  pub(super) fn apply_length_adjust(&self, clusters: &mut [GlyphCluster]) {
    if self
      .spans
      .iter()
      .all(|span| span.style.text_length.is_none())
    {
      return;
    }

    let is_horizontal = matches!(self.flow, TextFlow::Linear);
    let mut by_byte: Vec<(usize, usize)> = clusters
      .iter()
      .enumerate()
      .map(|(index, cluster)| (cluster.byte_index, index))
      .collect();

    by_byte.sort_unstable();

    for span in &self.spans {
      let Some(target_width) = span.style.text_length else {
        continue;
      };
      let first = by_byte.partition_point(|&(byte, _)| byte < span.start);
      let last = by_byte.partition_point(|&(byte, _)| byte < span.end);
      let indexes: Vec<usize> = by_byte[first..last]
        .iter()
        .map(|&(_, index)| index)
        .collect();

      if indexes.is_empty() {
        continue;
      }

      // The original width of a cluster, not its advance.
      let width: f32 = indexes.iter().map(|&index| clusters[index].width).sum();

      if span.style.length_adjust == LengthAdjust::Spacing {
        let factor = if indexes.len() > 1 {
          (target_width - width) / (indexes.len() - 1) as f32
        } else {
          0.0
        };

        for index in indexes {
          clusters[index].advance = clusters[index].width + factor;
        }
      } else {
        let factor = target_width / width;

        if factor < 0.001 {
          continue;
        }

        for index in indexes {
          clusters[index].transform *= Affine::scale(factor, 1.0);

          // The text-on-path placement reads the width and the advance of the cluster.
          if !is_horizontal {
            clusters[index].advance *= factor;
            clusters[index].width *= factor;
          }
        }
      }
    }
  }

  /// The span each cluster belongs to, by index.
  fn cluster_spans(&self, clusters: &[GlyphCluster]) -> Vec<Option<usize>> {
    clusters
      .iter()
      .map(|cluster| self.span_index_at(cluster.byte_index))
      .collect()
  }
}

impl SpanStyle {
  /// How far the span moves down to sit on its baseline, as Blink resolves the baselines of SVG
  /// text, which usvg follows: the `baseline-shift` of the span and its elements, and the baseline
  /// the `dominant-baseline` or the `alignment-baseline` names.
  pub(super) fn baseline_shift(&self, font: &ResolvedFont) -> f32 {
    let shift =
      self.baseline_offset.length + self.baseline_offset.superscripts as f32 * font.height() / 2.0;
    let baseline = self
      .alignment_baseline
      .font_baseline()
      .unwrap_or_else(|| self.dominant_baseline.font_baseline());

    -shift + font.baseline_shift(baseline)
  }
}

fn anchor_offset(anchor: TextAnchor, text_width: f32) -> f32 {
  match anchor {
    TextAnchor::Start => 0.0,
    TextAnchor::Middle => -text_width / 2.0,
    TextAnchor::End => -text_width,
  }
}

fn clusters_length(clusters: &[GlyphCluster]) -> f32 {
  clusters
    .iter()
    .fold(0.0, |width, cluster| width + cluster.advance)
}

/// The byte offsets at which the characters of a text start, to turn a byte into a character.
struct ByteCharacters(Vec<usize>);

impl ByteCharacters {
  fn new(text: &str) -> Self {
    Self(text.char_indices().map(|(byte, _)| byte).collect())
  }

  /// The index, in code points, of the character that starts at the byte.
  fn index(&self, byte_index: usize) -> usize {
    self.0.partition_point(|&start| start < byte_index)
  }
}

impl GlyphCluster {
  /// The box the font gives the cluster, in the space of its chunk.
  fn metrics_box(&self) -> Option<Rect<f32>> {
    let advance = if self.advance <= 0.0 {
      1.0
    } else {
      self.advance
    };

    Rect::positive_from_xywh(0.0, -self.ascent, advance, self.height())
      .and_then(|rect| rect.transformed(self.transform))
  }
}

/// The runs of consecutive clusters of each span, which a decoration line is drawn under. A run
/// ends at a cluster of another span and at one that is shifted relative to the one before it.
fn collect_decoration_spans(
  clusters: &[GlyphCluster],
  span_of: &[Option<usize>],
  span_count: usize,
) -> Vec<Vec<DecorationSpan>> {
  let mut runs = vec![Vec::new(); span_count];
  let mut current: Option<(usize, DecorationSpan)> = None;

  for (cluster, span) in clusters.iter().zip(span_of) {
    if let Some((owner, run)) = current
      && (Some(owner) != *span || cluster.has_relative_shift)
    {
      runs[owner].push(run);
      current = None;
    }

    let Some(span) = span else {
      continue;
    };

    current = Some(match current {
      Some((owner, mut run)) => {
        run.width += cluster.advance;

        (owner, run)
      }
      None => (
        *span,
        DecorationSpan {
          width: cluster.advance,
          transform: cluster.transform,
        },
      ),
    });
  }

  if let Some((owner, run)) = current {
    runs[owner].push(run);
  }

  runs
}

/// The outline of a decoration `thickness` thick, centered `dy` below the baseline, under the
/// spans, and the bounds of its points.
fn decoration_line(
  (dy, thickness): (f32, f32),
  spans: &[DecorationSpan],
  transform: Affine,
) -> Option<(Vec<PathCommand>, Rect<f32>)> {
  let map = |affine: Affine| {
    move |point: Point<f32>| {
      let (x, y) = affine.transform_point(point.x, point.y);

      Point::new(x, y)
    }
  };
  let mut commands = Vec::new();

  for span in spans {
    if Rect::positive_from_xywh(0.0, -thickness / 2.0, span.width, thickness).is_none() {
      continue;
    }

    let placement = span.transform * Affine::translation(0.0, dy);

    commands.extend(
      PathCommand::rectangle(0.0, -thickness / 2.0, span.width, thickness)
        .into_iter()
        .map(|command| command.map_points(map(placement))),
    );
  }

  let placed: Vec<PathCommand> = commands
    .into_iter()
    .map(|command| command.map_points(map(transform)))
    .collect();
  let outline = PathCommand::normalize(&placed)?;

  Some((outline.commands, outline.control_bounds))
}

/// The curves of a path, with its lines and quadratics raised to cubics.
fn path_curves(commands: &[PathCommand]) -> Vec<CubicBez> {
  let point = |point: Point<f32>| KurboPoint::new(f64::from(point.x), f64::from(point.y));
  let line = |from: KurboPoint, to: KurboPoint| {
    let line = Line::new(from, to);

    CubicBez::new(line.p0, line.eval(0.33), line.eval(0.66), line.p1)
  };
  let first = commands.iter().find_map(|command| match command {
    PathCommand::MoveTo(to) => Some(point(*to)),
    _ => None,
  });
  let Some(first) = first else {
    return Vec::new();
  };
  let mut curves = Vec::new();
  let (mut subpath_start, mut current) = (first, first);

  for command in commands {
    let curve = match *command {
      PathCommand::MoveTo(to) => {
        subpath_start = point(to);
        current = subpath_start;
        continue;
      }
      PathCommand::LineTo(to) => line(current, point(to)),
      PathCommand::QuadTo(control, to) => QuadBez::new(current, point(control), point(to)).raise(),
      PathCommand::CubicTo(first, second, to) => {
        CubicBez::new(current, point(first), point(second), point(to))
      }
      PathCommand::Close => line(current, subpath_start),
    };

    current = curve.p3;
    curves.push(curve);
  }

  curves
}
