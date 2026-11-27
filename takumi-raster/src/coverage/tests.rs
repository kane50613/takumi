use tiny_skia::{FillRule, IntSize, Path, PathBuilder, Rect};

use super::fill_path;

fn polygon(points: &[(f32, f32)]) -> Path {
  let mut builder = PathBuilder::new();

  builder.move_to(points[0].0, points[0].1);
  for &(x, y) in &points[1..] {
    builder.line_to(x, y);
  }
  builder.close();
  builder.finish().unwrap()
}

fn fill(path: &Path, fill_rule: FillRule, width: u32, height: u32) -> Vec<u8> {
  fill_path(path, fill_rule, IntSize::from_wh(width, height).unwrap()).unwrap()
}

#[test]
fn fractional_rect_edges_cover_their_area() {
  // Wider than `MaskAdditiveBlitter` takes, so the edges go through the run blitter.
  let path = polygon(&[(0.25, 0.5), (40.75, 0.5), (40.75, 6.25), (0.25, 6.25)]);
  let mask = fill(&path, FillRule::Winding, 41, 7);

  assert_eq!(&mask[..3], &[96, 128, 128]);
  assert_eq!(&mask[41..44], &[191, 255, 255]);
  assert_eq!(&mask[6 * 41..6 * 41 + 3], &[48, 64, 64]);
}

#[test]
fn small_rect_takes_the_fat_rect_blit() {
  let path = PathBuilder::from_rect(Rect::from_ltrb(0.5, 0.25, 4.5, 2.0).unwrap());
  let mask = fill(&path, FillRule::Winding, 5, 2);

  assert_eq!(mask, [95, 191, 191, 191, 95, 127, 255, 255, 255, 127]);
}

#[test]
fn diagonal_edge_covers_half_of_each_pixel() {
  let path = polygon(&[(0.0, 0.0), (40.0, 40.0), (0.0, 40.0)]);
  let mask = fill(&path, FillRule::Winding, 40, 40);

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

  assert_eq!(fill(&star, FillRule::Winding, 80, 80)[center], 255);
  assert_eq!(fill(&star, FillRule::EvenOdd, 80, 80)[center], 0);
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
  let clipped = fill(&circle(10.5), FillRule::Winding, 40, 80);
  let whole = fill(&circle(50.5), FillRule::Winding, 90, 80);
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
