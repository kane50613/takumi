//! Where a glyph outline crosses a horizontal band.
//!
//! `text-decoration-skip-ink` punches the glyphs out of an underline. Chromium
//! asks Skia for the x-ranges the outline occupies inside the band the
//! decoration covers (`SkFont::getIntercepts`, reached through
//! `Font::GetTextIntercepts`), then clips those ranges out of the line. This is
//! the same question, answered from the path itself so every backend can ask
//! it.

use smallvec::SmallVec;

use crate::geometry::{PathCommand, Point};

/// Segments a curve is flattened into. A glyph is small enough on screen that
/// the error from sixteen steps stays well under the half-pixel Chromium
/// already discards.
const CURVE_STEPS: usize = 16;

/// Scanlines taken across the band.
const BAND_SAMPLES: usize = 8;

/// The x-ranges `paths` fills between `top` and `bottom`, left to right and
/// never overlapping.
///
/// Ranges stay separate when the outline leaves the band between them, so the
/// gap inside a `u` stays a gap rather than being swallowed with the stems.
pub fn text_intercepts(paths: &[PathCommand], top: f32, bottom: f32) -> SmallVec<[(f32, f32); 4]> {
  let mut spans: SmallVec<[(f32, f32); 4]> = SmallVec::new();

  if bottom <= top {
    return spans;
  }
  let edges = flatten(paths);

  // The band is a decoration's thickness, a few pixels at most, so sampling it
  // costs little and catches a bowl that dips in and back out between its ends.
  for step in 0..=BAND_SAMPLES {
    let y = top + (bottom - top) * step as f32 / BAND_SAMPLES as f32;

    spans.extend(filled_at(&edges, y));
  }

  merge(spans)
}

/// Widest a skipped range grows past the ink on each side, matching Chromium's
/// `kDecorationClipMaxDilation`.
const MAX_DILATION: f32 = 13.0;

/// Intersections thinner than this are ignored, as Chromium ignores them by
/// insetting the decoration bounds before asking for intercepts.
const MIN_INTERSECTION: f32 = 0.5;

/// The x-ranges a decoration gives up to the glyphs it runs through.
///
/// `glyphs` places each outline in the decoration's own space. `top` and
/// `bottom` bound the decoration; `thickness` sets how far a skipped range
/// grows past the ink, which keeps a stroke from touching the line it
/// interrupts.
pub fn skip_ink_ranges<'g>(
  glyphs: impl Iterator<Item = (Point<f32>, &'g [PathCommand])>,
  top: f32,
  bottom: f32,
  thickness: f32,
) -> SmallVec<[(f32, f32); 4]> {
  let mut ranges: SmallVec<[(f32, f32); 4]> = SmallVec::new();

  if bottom - top <= 2.0 * MIN_INTERSECTION {
    return ranges;
  }
  let dilation = thickness.min(MAX_DILATION);

  for (origin, paths) in glyphs {
    let band_top = top + MIN_INTERSECTION - origin.y;
    let band_bottom = bottom - MIN_INTERSECTION - origin.y;

    ranges.extend(
      text_intercepts(paths, band_top, band_bottom)
        .into_iter()
        .map(|(low, high)| (origin.x + low - dilation, origin.x + high + dilation)),
    );
  }

  merge(ranges)
}

/// What is left of `start..end` once `skips` are taken out of it. `skips` must
/// be sorted and disjoint, which is what [`skip_ink_ranges`] returns.
pub fn remaining_spans(start: f32, end: f32, skips: &[(f32, f32)]) -> SmallVec<[(f32, f32); 4]> {
  let mut spans: SmallVec<[(f32, f32); 4]> = SmallVec::new();
  let mut left = start;

  for (skip_start, skip_end) in skips.iter().copied() {
    if skip_start > left {
      spans.push((left, skip_start.min(end)));
    }
    left = left.max(skip_end);
    if left >= end {
      return spans;
    }
  }
  if end > left {
    spans.push((left, end));
  }

  spans
}

/// The x-ranges the outline encloses along the line `y`, by nonzero winding.
fn filled_at(edges: &[(Point<f32>, Point<f32>)], y: f32) -> SmallVec<[(f32, f32); 4]> {
  let mut crossings: SmallVec<[(f32, i32); 8]> = SmallVec::new();

  for (a, b) in edges.iter().copied() {
    // Half-open in y so a vertex shared by two segments counts once.
    let winding = if a.y <= y && b.y > y {
      1
    } else if b.y <= y && a.y > y {
      -1
    } else {
      continue;
    };

    crossings.push((a.x + (b.x - a.x) * (y - a.y) / (b.y - a.y), winding));
  }
  crossings.sort_by(|a, b| a.0.total_cmp(&b.0));

  let mut spans: SmallVec<[(f32, f32); 4]> = SmallVec::new();
  let mut winding = 0;
  let mut entered = 0.0;

  for (x, direction) in crossings {
    let was_inside = winding != 0;

    winding += direction;
    match (was_inside, winding != 0) {
      (false, true) => entered = x,
      (true, false) => spans.push((entered, x)),
      _ => {}
    }
  }

  spans
}

/// The path as straight segments, curves flattened and contours closed.
fn flatten(paths: &[PathCommand]) -> SmallVec<[(Point<f32>, Point<f32>); 32]> {
  let mut edges: SmallVec<[(Point<f32>, Point<f32>); 32]> = SmallVec::new();
  let mut start = Point::ZERO;
  let mut current = Point::ZERO;
  let mut flat: SmallVec<[Point<f32>; CURVE_STEPS]> = SmallVec::new();

  for command in paths {
    match command {
      PathCommand::MoveTo(point) => {
        // A contour bounds ink whether or not it was closed explicitly.
        if current != start {
          edges.push((current, start));
        }
        start = *point;
        current = *point;
      }
      PathCommand::LineTo(point) => {
        edges.push((current, *point));
        current = *point;
      }
      PathCommand::QuadTo(control, end) => {
        flat.clear();
        flatten_quad(current, *control, *end, &mut flat);
        for point in flat.iter().copied() {
          edges.push((current, point));
          current = point;
        }
      }
      PathCommand::CubicTo(first, second, end) => {
        flat.clear();
        flatten_cubic(current, *first, *second, *end, &mut flat);
        for point in flat.iter().copied() {
          edges.push((current, point));
          current = point;
        }
      }
      PathCommand::Close => {
        edges.push((current, start));
        current = start;
      }
    }
  }
  if current != start {
    edges.push((current, start));
  }

  edges
}

/// Sorts the spans and unions the ones that touch.
fn merge(mut spans: SmallVec<[(f32, f32); 4]>) -> SmallVec<[(f32, f32); 4]> {
  if spans.len() < 2 {
    return spans;
  }
  spans.sort_by(|a, b| a.0.total_cmp(&b.0));

  let mut merged: SmallVec<[(f32, f32); 4]> = SmallVec::new();

  for (low, high) in spans {
    match merged.last_mut() {
      Some(last) if low <= last.1 => last.1 = last.1.max(high),
      _ => merged.push((low, high)),
    }
  }

  merged
}

fn flatten_quad(
  from: Point<f32>,
  control: Point<f32>,
  to: Point<f32>,
  out: &mut SmallVec<[Point<f32>; CURVE_STEPS]>,
) {
  for step in 1..=CURVE_STEPS {
    let t = step as f32 / CURVE_STEPS as f32;
    let inv = 1.0 - t;

    out.push(Point {
      x: inv * inv * from.x + 2.0 * inv * t * control.x + t * t * to.x,
      y: inv * inv * from.y + 2.0 * inv * t * control.y + t * t * to.y,
    });
  }
}

fn flatten_cubic(
  from: Point<f32>,
  first: Point<f32>,
  second: Point<f32>,
  to: Point<f32>,
  out: &mut SmallVec<[Point<f32>; CURVE_STEPS]>,
) {
  for step in 1..=CURVE_STEPS {
    let t = step as f32 / CURVE_STEPS as f32;
    let inv = 1.0 - t;
    let (a, b, c, d) = (
      inv * inv * inv,
      3.0 * inv * inv * t,
      3.0 * inv * t * t,
      t * t * t,
    );

    out.push(Point {
      x: a * from.x + b * first.x + c * second.x + d * to.x,
      y: a * from.y + b * first.y + c * second.y + d * to.y,
    });
  }
}

#[cfg(test)]
mod tests {
  use super::{remaining_spans, skip_ink_ranges, text_intercepts};
  use crate::geometry::{PathCommand, Point};

  fn point(x: f32, y: f32) -> Point<f32> {
    Point { x, y }
  }

  fn rect(left: f32, top: f32, right: f32, bottom: f32) -> Vec<PathCommand> {
    vec![
      PathCommand::MoveTo(point(left, top)),
      PathCommand::LineTo(point(right, top)),
      PathCommand::LineTo(point(right, bottom)),
      PathCommand::LineTo(point(left, bottom)),
      PathCommand::Close,
    ]
  }

  #[test]
  fn a_bar_crossing_the_band_reports_its_width() {
    let spans = text_intercepts(&rect(2.0, -10.0, 5.0, 10.0), -1.0, 1.0);

    assert_eq!(spans.as_slice(), [(2.0, 5.0)]);
  }

  #[test]
  fn a_shape_clear_of_the_band_reports_nothing() {
    assert!(text_intercepts(&rect(2.0, -10.0, 5.0, -6.0), -1.0, 1.0).is_empty());
  }

  #[test]
  fn two_stems_stay_two_ranges() {
    let mut paths = rect(0.0, -10.0, 2.0, 10.0);

    paths.extend(rect(6.0, -10.0, 8.0, 10.0));

    let spans = text_intercepts(&paths, -1.0, 1.0);

    assert_eq!(spans.as_slice(), [(0.0, 2.0), (6.0, 8.0)]);
  }

  #[test]
  fn touching_ranges_become_one() {
    let mut paths = rect(0.0, -10.0, 4.0, 10.0);

    paths.extend(rect(4.0, -10.0, 9.0, 10.0));

    assert_eq!(text_intercepts(&paths, -1.0, 1.0).as_slice(), [(0.0, 9.0)]);
  }

  #[test]
  fn a_slanted_bar_reports_the_slice_the_band_sees() {
    // A bar two wide leaning right: at y = -1 it sits at x 4.5..6.5, at y = 1
    // at 5.5..7.5, so the band covers 4.5..7.5.
    let paths = vec![
      PathCommand::MoveTo(point(0.0, -10.0)),
      PathCommand::LineTo(point(2.0, -10.0)),
      PathCommand::LineTo(point(12.0, 10.0)),
      PathCommand::LineTo(point(10.0, 10.0)),
      PathCommand::Close,
    ];
    let spans = text_intercepts(&paths, -1.0, 1.0);

    assert_eq!(spans.len(), 1, "{spans:?}");
    assert!((spans[0].0 - 4.5).abs() < 1e-4, "{spans:?}");
    assert!((spans[0].1 - 7.5).abs() < 1e-4, "{spans:?}");
  }

  #[test]
  fn an_open_contour_still_bounds_ink() {
    // Three sides of a box, left unclosed: the fill is the same as closing it.
    let paths = vec![
      PathCommand::MoveTo(point(2.0, -10.0)),
      PathCommand::LineTo(point(5.0, -10.0)),
      PathCommand::LineTo(point(5.0, 10.0)),
      PathCommand::LineTo(point(2.0, 10.0)),
    ];

    assert_eq!(text_intercepts(&paths, -1.0, 1.0).as_slice(), [(2.0, 5.0)]);
  }

  #[test]
  fn an_empty_band_reports_nothing() {
    assert!(text_intercepts(&rect(0.0, -10.0, 5.0, 10.0), 1.0, 1.0).is_empty());
  }

  #[test]
  fn a_glyph_gives_up_a_dilated_range() {
    // A 3-wide bar at x 2..5 under a 2px-thick line: the range grows 2 each
    // side, so the line loses 0..7.
    let glyph = rect(2.0, -10.0, 5.0, 10.0);
    let ranges = skip_ink_ranges(
      [(point(0.0, 0.0), glyph.as_slice())].into_iter(),
      -2.0,
      2.0,
      2.0,
    );

    assert_eq!(ranges.as_slice(), [(0.0, 7.0)]);
  }

  #[test]
  fn a_line_thinner_than_the_ignored_slice_skips_nothing() {
    let glyph = rect(2.0, -10.0, 5.0, 10.0);

    assert!(
      skip_ink_ranges(
        [(point(0.0, 0.0), glyph.as_slice())].into_iter(),
        -0.5,
        0.5,
        1.0
      )
      .is_empty()
    );
  }

  #[test]
  fn what_is_left_of_a_line_skips_the_ink() {
    assert_eq!(
      remaining_spans(0.0, 20.0, &[(4.0, 7.0), (12.0, 15.0)]).as_slice(),
      [(0.0, 4.0), (7.0, 12.0), (15.0, 20.0)]
    );
  }

  #[test]
  fn a_skip_covering_the_line_leaves_nothing() {
    assert!(remaining_spans(2.0, 8.0, &[(0.0, 10.0)]).is_empty());
  }
}
