//! Where a glyph outline crosses a decoration's band, which Chromium asks Skia's
//! `SkFont::getIntercepts` for and this answers from the outline so every backend can.

use std::{ops::RangeInclusive, sync::LazyLock};

use quick_cache::sync::Cache;
use smallvec::SmallVec;

use crate::{
  geometry::{PathCommand, Point},
  resources::glyph::ResolvedOutlineGlyph,
};

/// Segments a curve is flattened into, whose error stays under the half pixel Chromium discards.
const CURVE_STEPS: usize = 16;

/// Scanlines taken across the band.
const BAND_SAMPLES: usize = 8;

/// Disjoint x-ranges, left to right.
pub type Spans = SmallVec<[(f32, f32); 4]>;

/// The disjoint x-ranges `paths` fills between `top` and `bottom`, left to right.
fn text_intercepts(paths: &[PathCommand], top: f32, bottom: f32) -> Spans {
  let mut spans = Spans::new();

  if bottom <= top {
    return spans;
  }
  let edges = flatten(paths);

  for step in 0..=BAND_SAMPLES {
    let y = top + (bottom - top) * step as f32 / BAND_SAMPLES as f32;

    spans.extend(filled_at(&edges, y));
  }

  merge(spans)
}

/// Chromium's `kDecorationClipMaxDilation`.
const MAX_DILATION: f32 = 13.0;

/// How far Chromium insets the band, so thinner intersections are ignored.
const MIN_INTERSECTION: f32 = 0.5;

/// The x-ranges a decoration of `thickness` gives up to `glyphs` between `top` and `bottom`,
/// dilated as Blink's `TextPainter::ClipDecorationLine` dilates them.
pub fn skip_ink_ranges<'g>(
  glyphs: impl Iterator<Item = (Point<f32>, &'g ResolvedOutlineGlyph)>,
  top: f32,
  bottom: f32,
  thickness: f32,
) -> Spans {
  let mut ranges = Spans::new();

  if bottom - top <= 2.0 * MIN_INTERSECTION {
    return ranges;
  }
  let dilation = thickness.min(MAX_DILATION);

  for (origin, outline) in glyphs {
    let band_top = top + MIN_INTERSECTION - origin.y;
    let band_bottom = bottom - MIN_INTERSECTION - origin.y;

    ranges.extend(
      cached_intercepts(outline, band_top, band_bottom)
        .into_iter()
        .map(|(low, high)| (origin.x + low - dilation, origin.x + high + dilation)),
    );
  }

  merge(ranges)
}

/// Blink's `Character::CanTextDecorationSkipInk`, under the notice in LICENSE-CHROMIUM.
pub fn skips_ink(character: char) -> bool {
  let code = u32::from(character);

  !matches!(character, '/' | '\\' | '_')
    && CJK_IDEOGRAPHS_OR_SYMBOLS.binary_search(&code).is_err()
    && !CJK_IDEOGRAPH_OR_SYMBOL_RANGES
      .iter()
      .chain(NO_SKIP_INK_BLOCKS)
      .any(|range| range.contains(&code))
}

/// Blink's `kIsCjkIdeographOrSymbolArray`, sorted.
const CJK_IDEOGRAPHS_OR_SYMBOLS: &[u32] = &[
  0x2C7, 0x2CA, 0x2CB, 0x2D9, 0x2020, 0x2021, 0x2030, 0x203B, 0x203C, 0x2042, 0x2047, 0x2048,
  0x2049, 0x2051, 0x20DD, 0x20DE, 0x2100, 0x2103, 0x2105, 0x2109, 0x210A, 0x2113, 0x2116, 0x2121,
  0x212B, 0x213B, 0x2150, 0x2151, 0x2152, 0x217F, 0x2189, 0x2307, 0x2312, 0x23CE, 0x2423, 0x25A0,
  0x25A1, 0x25A2, 0x25AA, 0x25AB, 0x25B1, 0x25B2, 0x25B3, 0x25B6, 0x25B7, 0x25BC, 0x25BD, 0x25C0,
  0x25C1, 0x25C6, 0x25C7, 0x25C9, 0x25CB, 0x25CC, 0x25EF, 0x2605, 0x2606, 0x260E, 0x2616, 0x2617,
  0x26A0, 0x2713, 0x271A, 0x273F, 0x2740, 0x2756, 0x2763, 0x2B1A, 0xFE10, 0xFE11, 0xFE12, 0xFE19,
  0xFF1D, 0x1F100, 0x1F200, 0x1F237, 0x1F32C, 0x1F336, 0x1F37D, 0x1F43F, 0x1F54F, 0x1F93B, 0x1F946,
];

/// Blink's `kIsCjkIdeographOrSymbolRanges`.
const CJK_IDEOGRAPH_OR_SYMBOL_RANGES: &[RangeInclusive<u32>] = &[
  0x2E80..=0x2FDF,
  0x31C0..=0x31EF,
  0x3400..=0x4DBF,
  0x4E00..=0x9FFF,
  0xF900..=0xFAFF,
  0x20000..=0x2FFFF,
  0x2156..=0x215A,
  0x2160..=0x216B,
  0x2170..=0x217B,
  0x23BE..=0x23CC,
  0x2460..=0x2492,
  0x249C..=0x24FF,
  0x25CE..=0x25D3,
  0x25E2..=0x25E6,
  0x2600..=0x2603,
  0x2660..=0x266F,
  0x2672..=0x267D,
  0x2776..=0x277F,
  0x2FF0..=0x302D,
  0x3031..=0x312F,
  0x3190..=0x31BF,
  0x3200..=0x33FF,
  0x4DC0..=0x4DFF,
  0xF860..=0xF862,
  0xFE30..=0xFE6F,
  0xFF00..=0xFF0C,
  0xFF0E..=0xFF1A,
  0xFF1F..=0xFFEF,
  0x16FE0..=0x16FFF,
  0x17000..=0x187FF,
  0x18800..=0x18AFF,
  0x1B000..=0x1B0FF,
  0x1B100..=0x1B12F,
  0x1B170..=0x1B2FF,
  0x1F110..=0x1F129,
  0x1F130..=0x1F149,
  0x1F150..=0x1F169,
  0x1F170..=0x1F189,
  0x1F202..=0x1F219,
  0x1F21B..=0x1F22E,
  0x1F230..=0x1F231,
  0x1F23B..=0x1F24F,
  0x1F252..=0x1F2FF,
  0x1F321..=0x1F32A,
  0x1F394..=0x1F39F,
  0x1F3CD..=0x1F3CE,
  0x1F3D4..=0x1F3DF,
  0x1F3F1..=0x1F3F2,
  0x1F3F5..=0x1F3F7,
  0x1F4FD..=0x1F4FE,
  0x1F53E..=0x1F54A,
  0x1F568..=0x1F573,
  0x1F576..=0x1F579,
  0x1F57B..=0x1F58F,
  0x1F591..=0x1F594,
  0x1F597..=0x1F5A3,
  0x1F5A5..=0x1F5E7,
  0x1F5E9..=0x1F5FA,
  0x1F650..=0x1F67F,
  0x1F6C6..=0x1F6CB,
  0x1F6CD..=0x1F6CF,
  0x1F6D3..=0x1F6D4,
  0x1F6D9..=0x1F6DB,
  0x1F6E0..=0x1F6EA,
  0x1F6ED..=0x1F6F3,
  0x1F6FD..=0x1F6FF,
  0x1F900..=0x1F90B,
  0x1FAC9..=0x1FACC,
];

/// The Hangul and Linear B blocks `CanTextDecorationSkipInk` adds.
const NO_SKIP_INK_BLOCKS: &[RangeInclusive<u32>] = &[
  0x1100..=0x11FF,
  0x3130..=0x318F,
  0xA960..=0xA97F,
  0xAC00..=0xD7AF,
  0xD7B0..=0xD7FF,
  0x10080..=0x100FF,
];

/// Intercepts keyed by outline signature and band, shared across runs and renders.
const INTERCEPT_CACHE_ITEMS: usize = 8192;

static INTERCEPTS: LazyLock<Cache<(u64, u32, u32), Spans>> =
  LazyLock::new(|| Cache::new(INTERCEPT_CACHE_ITEMS));

fn cached_intercepts(outline: &ResolvedOutlineGlyph, top: f32, bottom: f32) -> Spans {
  let key = (outline.cache_signature(), top.to_bits(), bottom.to_bits());
  if let Some(spans) = INTERCEPTS.get(&key) {
    return spans;
  }
  let paths = outline.paths();
  let spans = if reaches_band(paths, top, bottom) {
    text_intercepts(paths, top, bottom)
  } else {
    Spans::new()
  };
  INTERCEPTS.insert(key, spans.clone());
  spans
}

/// Whether any point, control points included, lies in the band, which a flattened edge must.
fn reaches_band(paths: &[PathCommand], top: f32, bottom: f32) -> bool {
  let (mut min_y, mut max_y) = (f32::INFINITY, f32::NEG_INFINITY);
  let mut span = |point: &Point<f32>| {
    min_y = min_y.min(point.y);
    max_y = max_y.max(point.y);
  };
  for command in paths {
    match command {
      PathCommand::MoveTo(point) | PathCommand::LineTo(point) => span(point),
      PathCommand::QuadTo(control, end) => {
        span(control);
        span(end);
      }
      PathCommand::CubicTo(first, second, end) => {
        span(first);
        span(second);
        span(end);
      }
      PathCommand::Close => {}
    }
  }
  max_y >= top && min_y <= bottom
}

/// What is left of `start..end` once the sorted `skips` are taken out.
pub fn remaining_spans(start: f32, end: f32, skips: &[(f32, f32)]) -> Spans {
  let mut spans = Spans::new();
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
fn filled_at(edges: &[(Point<f32>, Point<f32>)], y: f32) -> Spans {
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

  let mut spans = Spans::new();
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
fn merge(mut spans: Spans) -> Spans {
  if spans.len() < 2 {
    return spans;
  }
  spans.sort_by(|a, b| a.0.total_cmp(&b.0));

  let mut merged = Spans::new();

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
  use crate::{
    geometry::{PathCommand, Point},
    resources::glyph::ResolvedOutlineGlyph,
  };

  fn outline(paths: Vec<PathCommand>, signature: u64) -> ResolvedOutlineGlyph {
    ResolvedOutlineGlyph::Plain {
      paths,
      embolden: None,
      cache_signature: signature,
    }
  }

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
    let glyph = outline(rect(2.0, -10.0, 5.0, 10.0), 1);
    let ranges = skip_ink_ranges([(point(0.0, 0.0), &glyph)].into_iter(), -2.0, 2.0, 2.0);

    assert_eq!(ranges.as_slice(), [(0.0, 7.0)]);
  }

  #[test]
  fn a_line_thinner_than_the_ignored_slice_skips_nothing() {
    let glyph = outline(rect(2.0, -10.0, 5.0, 10.0), 2);

    assert!(skip_ink_ranges([(point(0.0, 0.0), &glyph)].into_iter(), -0.5, 0.5, 1.0).is_empty());
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

  #[test]
  fn a_curve_is_flattened_before_it_is_measured() {
    // A circle of radius 5 at the origin, drawn as four cubics. Across a band
    // hugging its middle it is at its widest, so the span is the diameter.
    const K: f32 = 5.0 * 0.552_284_7;
    let paths = vec![
      PathCommand::MoveTo(point(5.0, 0.0)),
      PathCommand::CubicTo(point(5.0, K), point(K, 5.0), point(0.0, 5.0)),
      PathCommand::CubicTo(point(-K, 5.0), point(-5.0, K), point(-5.0, 0.0)),
      PathCommand::CubicTo(point(-5.0, -K), point(-K, -5.0), point(0.0, -5.0)),
      PathCommand::CubicTo(point(K, -5.0), point(5.0, -K), point(5.0, 0.0)),
      PathCommand::Close,
    ];
    let spans = text_intercepts(&paths, -0.5, 0.5);

    assert_eq!(spans.len(), 1, "{spans:?}");
    assert!((spans[0].0 + 5.0).abs() < 0.05, "{spans:?}");
    assert!((spans[0].1 - 5.0).abs() < 0.05, "{spans:?}");
  }

  #[test]
  fn a_counter_is_not_reported_as_ink() {
    // An `o`: an outer contour with an inner one wound the other way. The band
    // sees the two strokes, not the hole between them.
    let mut paths = rect(0.0, -10.0, 10.0, 10.0);

    paths.extend([
      PathCommand::MoveTo(point(3.0, -10.0)),
      PathCommand::LineTo(point(3.0, 10.0)),
      PathCommand::LineTo(point(7.0, 10.0)),
      PathCommand::LineTo(point(7.0, -10.0)),
      PathCommand::Close,
    ]);

    assert_eq!(
      text_intercepts(&paths, -1.0, 1.0).as_slice(),
      [(0.0, 3.0), (7.0, 10.0)]
    );
  }
}
