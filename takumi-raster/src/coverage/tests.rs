use crate::path::Fill;
use tiny_skia_path::{IntSize, Path, PathBuilder, Rect};

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

fn fill(path: &Path, fill_rule: Fill, width: u32, height: u32) -> Vec<u8> {
  fill_path(path, fill_rule, IntSize::from_wh(width, height).unwrap()).unwrap()
}

#[test]
fn fractional_rect_edges_cover_their_area() {
  // Wider than `MaskAdditiveBlitter` takes, so the edges go through the run blitter.
  let path = polygon(&[(0.25, 0.5), (40.75, 0.5), (40.75, 6.25), (0.25, 6.25)]);
  let mask = fill(&path, Fill::NonZero, 41, 7);

  assert_eq!(&mask[..3], &[96, 128, 128]);
  assert_eq!(&mask[41..44], &[191, 255, 255]);
  assert_eq!(&mask[6 * 41..6 * 41 + 3], &[48, 64, 64]);
}

#[test]
fn small_rect_takes_the_fat_rect_blit() {
  let path = PathBuilder::from_rect(Rect::from_ltrb(0.5, 0.25, 4.5, 2.0).unwrap());
  let mask = fill(&path, Fill::NonZero, 5, 2);

  assert_eq!(mask, [95, 191, 191, 191, 95, 127, 255, 255, 255, 127]);
}

#[test]
fn diagonal_edge_covers_half_of_each_pixel() {
  let path = polygon(&[(0.0, 0.0), (40.0, 40.0), (0.0, 40.0)]);
  let mask = fill(&path, Fill::NonZero, 40, 40);

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

  assert_eq!(fill(&star, Fill::NonZero, 80, 80)[center], 255);
  assert_eq!(fill(&star, Fill::EvenOdd, 80, 80)[center], 0);
}

#[test]
fn rect_fill_rounds_edges_to_256ths() {
  let mask = super::fill_rect([0.5, 0.25, 3.75, 1.0], IntSize::from_wh(4, 1).unwrap()).unwrap();

  // `antifilldot8` with T = 64, B = 256: a single scanline of alpha `B - T - 1`.
  assert_eq!(mask, [95, 191, 191, 143]);
}

/// Clipping chops curves at the clip, which changes how `SkAnalyticEdge` subdivides them, so only
/// edge pixels may move, by under a quarter.
#[test]
fn clipped_path_matches_its_unclipped_crop() {
  let circle = |cx: f32| PathBuilder::from_circle(cx, 40.0, 37.3).unwrap();
  let clipped = fill(&circle(10.5), Fill::NonZero, 40, 80);
  let whole = fill(&circle(50.5), Fill::NonZero, 90, 80);
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
    let mask = fill(&triangle, Fill::NonZero, 10, 10);

    assert_eq!(mask.len(), 100, "extent {extent}");
    assert!(mask.iter().all(|alpha| *alpha == 255), "extent {extent}");
  }
}

#[test]
fn path_far_outside_the_clip_leaves_the_mask_empty() {
  let far = polygon(&[(1e9, 1e9), (1e9 + 50.0, 1e9), (1e9, 1e9 + 50.0)]);

  assert!(
    fill(&far, Fill::NonZero, 10, 10)
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
    let mask = fill(&path, Fill::NonZero, width, width);
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

/// tiny-skia's aliased fill, a port of an older `SkScan::FillPath`, as the oracle.
fn tiny_skia_aliased(path: &Path, fill_rule: Fill, width: u32, height: u32) -> Vec<u8> {
  let mut mask = tiny_skia::Mask::new(width, height).unwrap();
  let fill_rule = match fill_rule {
    Fill::NonZero => tiny_skia::FillRule::Winding,
    Fill::EvenOdd => tiny_skia::FillRule::EvenOdd,
  };

  mask.fill_path(
    path,
    fill_rule,
    false,
    tiny_skia_path::Transform::identity(),
  );
  mask.data().to_vec()
}

#[test]
fn aliased_polygons_match_skias_fill() {
  let mut seed = 0x2545_f491_u32;
  let mut next = |range: f32| {
    seed ^= seed << 13;
    seed ^= seed >> 17;
    seed ^= seed << 5;
    (seed % 10_000) as f32 / 10_000.0 * range - 8.0
  };

  for case in 0..200 {
    let points: Vec<(f32, f32)> = (0..3 + case % 6)
      .map(|_| (next(80.0), next(60.0)))
      .collect();
    let path = polygon(&points);

    for fill_rule in [Fill::NonZero, Fill::EvenOdd] {
      assert_eq!(
        fill_path_aliased(&path, fill_rule, IntSize::from_wh(64, 48).unwrap()).unwrap(),
        tiny_skia_aliased(&path, fill_rule, 64, 48),
        "case {case} {fill_rule:?} {points:?}",
      );
    }
  }
}

#[test]
fn aliased_curves_match_skias_fill() {
  let mut builder = PathBuilder::new();

  builder.push_circle(30.3, 25.7, 19.6);
  builder.move_to(2.0, 40.0);
  builder.quad_to(30.0, -10.0, 60.0, 44.5);
  builder.close();

  let path = builder.finish().unwrap();

  for fill_rule in [Fill::NonZero, Fill::EvenOdd] {
    assert_eq!(
      fill_path_aliased(&path, fill_rule, IntSize::from_wh(64, 48).unwrap()).unwrap(),
      tiny_skia_aliased(&path, fill_rule, 64, 48),
      "{fill_rule:?}",
    );
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
  let mask = fill_path_aliased(&path, Fill::NonZero, IntSize::from_wh(width, 2).unwrap()).unwrap();

  let row = width as usize;

  // `clip_to_limit` leaves the pixels past 16383 unfilled, as in Skia.
  assert_eq!([mask[0], mask[row], mask[row + 16_382]], [255; 3]);
  assert_eq!([mask[16_383], mask[row + 16_383]], [0; 2]);
}
