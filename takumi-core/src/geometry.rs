//! Transform geometry helpers shared across backends.

use taffy::{Point, geometry::Size};

use crate::style::Affine;

/// Transforms a rect's four corners and returns the axis-aligned `(min_x, min_y,
/// max_x, max_y)` extents, or `None` if any corner is non-finite.
pub fn transformed_rect_extents(
  origin: Point<f32>,
  size: Size<f32>,
  transform: Affine,
) -> Option<(f32, f32, f32, f32)> {
  let corners = [
    transform.transform_point(origin.x, origin.y),
    transform.transform_point(origin.x + size.width, origin.y),
    transform.transform_point(origin.x, origin.y + size.height),
    transform.transform_point(origin.x + size.width, origin.y + size.height),
  ];

  let mut min_x = f32::INFINITY;
  let mut min_y = f32::INFINITY;
  let mut max_x = f32::NEG_INFINITY;
  let mut max_y = f32::NEG_INFINITY;
  for (x, y) in corners {
    min_x = min_x.min(x);
    min_y = min_y.min(y);
    max_x = max_x.max(x);
    max_y = max_y.max(y);
  }

  if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
    return None;
  }

  Some((min_x, min_y, max_x, max_y))
}
