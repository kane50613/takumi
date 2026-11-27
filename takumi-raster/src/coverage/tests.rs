use crate::style::FillRule;
use takumi_core::geometry::{Rect, Size};
use takumi_core::path::{Path, PathBuilder};

use super::{fill_path, fill_path_aliased};

fn polygon(points: &[(f32, f32)]) -> Path {
  let mut builder = PathBuilder::new();

  builder.move_to(points[0].0, points[0].1);
  for &(x, y) in &points[1..] {
    builder.line_to(x, y);
  }
  builder.close();
  builder.finish().unwrap()
}

/// Adds a circle of four cubics, the usual quarter-circle approximation.
fn push_circle(builder: &mut PathBuilder, cx: f32, cy: f32, radius: f32) {
  let k = 0.552_284_8 * radius;

  builder.move_to(cx + radius, cy);
  builder.cubic_to(cx + radius, cy + k, cx + k, cy + radius, cx, cy + radius);
  builder.cubic_to(cx - k, cy + radius, cx - radius, cy + k, cx - radius, cy);
  builder.cubic_to(cx - radius, cy - k, cx - k, cy - radius, cx, cy - radius);
  builder.cubic_to(cx + k, cy - radius, cx + radius, cy - k, cx + radius, cy);
  builder.close();
}

fn fill(path: &Path, fill_rule: FillRule, width: u32, height: u32) -> Vec<u8> {
  fill_path(path, fill_rule, Size::checked_new(width, height).unwrap())
}

#[test]
fn fractional_rect_edges_cover_their_area() {
  // Wider than `MaskAdditiveBlitter` takes, so the edges go through the run blitter.
  let path = polygon(&[(0.25, 0.5), (40.75, 0.5), (40.75, 6.25), (0.25, 6.25)]);
  let mask = fill(&path, FillRule::NonZero, 41, 7);

  assert_eq!(&mask[..3], &[96, 128, 128]);
  assert_eq!(&mask[41..44], &[191, 255, 255]);
  assert_eq!(&mask[6 * 41..6 * 41 + 3], &[48, 64, 64]);
}

#[test]
fn small_rect_takes_the_fat_rect_blit() {
  let path = Path::from_rect(Rect::from_ltrb(0.5, 0.25, 4.5, 2.0).unwrap());
  let mask = fill(&path, FillRule::NonZero, 5, 2);

  assert_eq!(mask, [95, 191, 191, 191, 95, 127, 255, 255, 255, 127]);
}

#[test]
fn diagonal_edge_covers_half_of_each_pixel() {
  let path = polygon(&[(0.0, 0.0), (40.0, 40.0), (0.0, 40.0)]);
  let mask = fill(&path, FillRule::NonZero, 40, 40);

  for y in 0..40 {
    assert_eq!(mask[y * 40 + y], 128, "row {y}");
  }
}

#[test]
fn even_odd_leaves_the_star_center_empty() {
  let points: Vec<(f32, f32)> = (0..5)
    .map(|index| {
      let angle = std::f32::consts::PI * (0.5 + 0.8 * index as f32);

      (40.0 + 38.0 * angle.cos(), 40.0 - 38.0 * angle.sin())
    })
    .collect();
  let star = polygon(&points);
  let center = 40 * 80 + 40;

  assert_eq!(fill(&star, FillRule::NonZero, 80, 80)[center], 255);
  assert_eq!(fill(&star, FillRule::EvenOdd, 80, 80)[center], 0);
}

#[test]
fn rect_fill_rounds_edges_to_256ths() {
  let mask = super::fill_rect([0.5, 0.25, 3.75, 1.0], Size::checked_new(4, 1).unwrap()).unwrap();

  // `antifilldot8` with T = 64, B = 256: a single scanline of alpha `B - T - 1`.
  assert_eq!(mask, [95, 191, 191, 143]);
}

/// Clipping chops curves at the clip, which changes how `SkAnalyticEdge` subdivides them, so only
/// edge pixels may move, by under a quarter.
#[test]
fn clipped_path_matches_its_unclipped_crop() {
  let circle = |cx: f32| {
    let mut builder = PathBuilder::new();

    push_circle(&mut builder, cx, 40.0, 37.3);
    builder.finish().unwrap()
  };
  let clipped = fill(&circle(10.5), FillRule::NonZero, 40, 80);
  let whole = fill(&circle(50.5), FillRule::NonZero, 90, 80);
  let mut total = 0u32;

  for y in 0..80 {
    for x in 0..40 {
      let (a, b) = (clipped[y * 40 + x], whole[y * 90 + x + 40]);

      assert!(a.abs_diff(b) < 64, "({x}, {y}): {a} vs {b}");
      total += u32::from(a.abs_diff(b));
    }
  }
  assert!(total < 40 * 80, "mean difference {}", total as f32 / 3200.0);
}

#[test]
fn huge_coordinates_stay_in_range() {
  for extent in [1e9, 2.1e9, 3e9, i32::MAX as f32, 1e30] {
    let triangle = polygon(&[(-extent, -extent), (extent, -extent), (0.0, extent)]);
    let mask = fill(&triangle, FillRule::NonZero, 10, 10);

    assert_eq!(mask.len(), 100, "extent {extent}");
    assert!(mask.iter().all(|alpha| *alpha == 255), "extent {extent}");
  }
}

#[test]
fn path_far_outside_the_clip_leaves_the_mask_empty() {
  let far = polygon(&[(1e9, 1e9), (1e9 + 50.0, 1e9), (1e9, 1e9 + 50.0)]);

  assert!(
    fill(&far, FillRule::NonZero, 10, 10)
      .iter()
      .all(|alpha| *alpha == 0)
  );
}

#[test]
fn non_finite_points_make_no_path() {
  let mut builder = PathBuilder::new();

  builder.move_to(0.0, 0.0);
  builder.line_to(f32::NAN, 5.0);
  builder.line_to(f32::INFINITY, 5.0);
  builder.close();

  assert!(builder.finish().is_none());
}

#[test]
fn whole_pixel_square_covers_its_edges_fully() {
  for size in [8.0, 50.0, 200.0] {
    let path = polygon(&[
      (2.0, 2.0),
      (2.0 + size, 2.0),
      (2.0 + size, 2.0 + size),
      (2.0, 2.0 + size),
    ]);
    let width = size as u32 + 4;
    let mask = fill(&path, FillRule::NonZero, width, width);
    let at = |x: u32, y: u32| mask[(y * width + x) as usize];

    assert_eq!(
      [
        at(2, 2),
        at(2, 5),
        at(1 + size as u32, 5),
        at(5, 1 + size as u32)
      ],
      [255; 4],
      "{size}"
    );
    assert_eq!([at(1, 5), at(5, 1)], [0; 2], "{size}");
  }
}

/// The aliased fill of the polygon `points` by its winding at each row center, with the vertices
/// on Skia's 1/64 grid and each span from the rounded crossings: an oracle independent of the edge
/// walk. `SkEdge::setLine` starts its x on the 1/64 grid and steps it in 16.16, so a crossing
/// within 1/32 of a half pixel can round either way, and the pixels beside one come back as
/// ambiguous.
fn aliased_reference(
  points: &[(f32, f32)],
  fill_rule: FillRule,
  width: u32,
  height: u32,
) -> (Vec<u8>, Vec<bool>) {
  let grid = |value: f32| f64::from((value * 64.0) as i32) / 64.0;
  let vertices: Vec<(f64, f64)> = points.iter().map(|&(x, y)| (grid(x), grid(y))).collect();
  let mask_bits = if fill_rule == FillRule::EvenOdd {
    1
  } else {
    -1
  };
  let (mut mask, mut ambiguous) = (
    vec![0; (width * height) as usize],
    vec![false; (width * height) as usize],
  );

  for row in 0..height as i32 {
    let center = f64::from(row) + 0.5;
    let mut crossings: Vec<(f64, i32)> = Vec::new();

    for (index, &(x0, y0)) in vertices.iter().enumerate() {
      let (x1, y1) = vertices[(index + 1) % vertices.len()];
      let ((top_x, top), (bottom_x, bottom), winding) = if y0 <= y1 {
        ((x0, y0), (x1, y1), 1)
      } else {
        ((x1, y1), (x0, y0), -1)
      };

      // `SkEdge` covers the rows whose centers its rounded ends enclose.
      if f64::from(row) >= (top + 0.5).floor() && f64::from(row) < (bottom + 0.5).floor() {
        crossings.push((
          top_x + (center - top) * (bottom_x - top_x) / (bottom - top),
          winding,
        ));
      }
    }

    crossings.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut winding = 0;
    let mut left = 0.0;

    for (x, edge_winding) in crossings {
      if winding & mask_bits == 0 {
        left = x;
      }

      winding += edge_winding;

      if winding & mask_bits == 0 {
        let (from, to) = ((left + 0.5).floor() as i32, (x + 0.5).floor() as i32);
        let row_start = (row as u32 * width) as usize;

        for column in from.max(0)..to.min(width as i32) {
          mask[row_start + column as usize] = 255;
        }

        for edge in [left, x] {
          if ((edge + 0.5).fract() - 0.5).abs() > 0.5 - 1.0 / 32.0 {
            for column in [(edge + 0.5).floor() as i32 - 1, (edge + 0.5).floor() as i32] {
              if (0..width as i32).contains(&column) {
                ambiguous[row_start + column as usize] = true;
              }
            }
          }
        }
      }
    }
  }

  (mask, ambiguous)
}

#[test]
fn aliased_polygons_match_their_winding_at_pixel_centers() {
  let mut seed = 0x2545_f491_u32;
  let mut next = |range: f32| {
    seed ^= seed << 13;
    seed ^= seed >> 17;
    seed ^= seed << 5;
    (seed % 10_000) as f32 / 10_000.0 * range
  };

  for case in 0..200 {
    let points: Vec<(f32, f32)> = (0..3 + case % 6)
      .map(|_| (next(64.0), next(48.0)))
      .collect();
    let path = polygon(&points);

    for fill_rule in [FillRule::NonZero, FillRule::EvenOdd] {
      let ours = fill_path_aliased(&path, fill_rule, Size::checked_new(64, 48).unwrap());
      let (reference, ambiguous) = aliased_reference(&points, fill_rule, 64, 48);

      assert!(ambiguous.iter().filter(|&&ambiguous| ambiguous).count() < 64 * 48 / 20);

      for (index, ((ours, reference), ambiguous)) in
        ours.iter().zip(&reference).zip(&ambiguous).enumerate()
      {
        assert!(
          *ambiguous || ours == reference,
          "case {case} {fill_rule:?} pixel {index}: {ours} against {reference}, {points:?}",
        );
      }
    }
  }
}

#[test]
fn aliased_curves_agree_with_their_antialiased_coverage() {
  let mut builder = PathBuilder::new();

  push_circle(&mut builder, 30.3, 25.7, 19.6);
  builder.move_to(2.0, 40.0);
  builder.quad_to(30.0, -10.0, 60.0, 44.5);
  builder.close();

  let path = builder.finish().unwrap();
  let size = Size::checked_new(64, 48).unwrap();

  for fill_rule in [FillRule::NonZero, FillRule::EvenOdd] {
    let aliased = fill_path_aliased(&path, fill_rule, size);
    let antialiased = fill_path(&path, fill_rule, size);

    assert!(
      aliased.iter().filter(|&&alpha| alpha == 255).count() > 500,
      "{fill_rule:?}"
    );

    // A pixel the analytic scan covers whole is filled, and one it leaves empty is not.
    for (index, (&aliased, &coverage)) in aliased.iter().zip(&antialiased).enumerate() {
      assert!(
        (coverage != 255 || aliased == 255) && (coverage != 0 || aliased == 0),
        "{fill_rule:?} pixel {index}: {aliased} against coverage {coverage}",
      );
    }
  }
}

#[test]
fn an_aliased_fill_wider_than_the_clip_limit_keeps_its_rows() {
  let width = 20_000;
  let path = polygon(&[
    (0.0, 0.0),
    (width as f32, 0.0),
    (width as f32, 2.0),
    (0.0, 2.0),
  ]);
  let mask = fill_path_aliased(
    &path,
    FillRule::NonZero,
    Size::checked_new(width, 2).unwrap(),
  );

  let row = width as usize;

  // `clip_to_limit` leaves the pixels past 16383 unfilled, as in Skia.
  assert_eq!([mask[0], mask[row], mask[row + 16_382]], [255; 3]);
  assert_eq!([mask[16_383], mask[row + 16_383]], [0; 2]);
}
