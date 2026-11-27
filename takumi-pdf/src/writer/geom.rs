//! The core geometry as the PDF writer needs it.

use takumi_core::{geometry::Rect, style::Affine};

/// The square from the origin to (1, 1).
pub(crate) const UNIT_RECT: Rect<f32> = Rect {
  left: 0.0,
  top: 0.0,
  right: 1.0,
  bottom: 1.0,
};

/// A rotation by `angle` degrees around `x`, `y`.
pub(crate) fn rotation_at(angle: f32, x: f32, y: f32) -> Affine {
  let radians = angle.to_radians();
  let (sin, cos) = (radians.sin(), radians.cos());
  let rotation = Affine {
    a: cos,
    b: sin,
    c: -sin,
    d: cos,
    x: 0.0,
    y: 0.0,
  };

  Affine::translation(x, y) * rotation * Affine::translation(-x, -y)
}

/// Grows `rect` to cover `other`, leaving it as it was where the union is too wide for an `f32`.
pub(crate) fn expand(rect: &mut Rect<f32>, other: &Rect<f32>) {
  if let Some(union) = Rect::from_ltrb(
    rect.left.min(other.left),
    rect.top.min(other.top),
    rect.right.max(other.right),
    rect.bottom.max(other.bottom),
  ) {
    *rect = union;
  }
}

/// `rect` as a PDF rectangle.
pub(crate) fn pdf_rect(rect: Rect<f32>) -> pdf_writer::Rect {
  pdf_writer::Rect::new(
    rect.left,
    rect.top,
    rect.left + rect.width(),
    rect.top + rect.height(),
  )
}
