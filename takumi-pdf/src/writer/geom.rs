//! Path geometry.

use std::hash::{Hash, Hasher};

use takumi_core::{
  path::{Path as CorePath, PathBuilder as CorePathBuilder, Rect as CoreRect},
  style::Affine,
};

/// A point.
#[allow(missing_docs)]
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Point {
  pub x: f32,
  pub y: f32,
}

impl Point {
  /// Create a new point with the given x and y coordinates.
  pub fn from_xy(x: f32, y: f32) -> Self {
    Self { x, y }
  }
}

/// A size whose width and height are finite and above zero.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Size {
  width: f32,
  height: f32,
}

impl Size {
  /// Creates a new `Size` from width and height.
  ///
  /// Returns `None` if either the width or the height is not > 0.
  pub fn from_wh(width: f32, height: f32) -> Option<Self> {
    (width > 0.0 && height > 0.0 && width.is_finite() && height.is_finite())
      .then_some(Self { width, height })
  }

  /// Returns the width of the size.
  pub fn width(&self) -> f32 {
    self.width
  }

  /// Returns the height of the size.
  pub fn height(&self) -> f32 {
    self.height
  }
}

/// A rectangle defined by left, top, right and bottom edges.
#[allow(missing_docs)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Rect(CoreRect);

impl Eq for Rect {}

impl Hash for Rect {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.left().to_bits().hash(state);
    self.0.top().to_bits().hash(state);
    self.0.right().to_bits().hash(state);
    self.0.bottom().to_bits().hash(state);
  }
}

impl Rect {
  /// The square from the origin to (1, 1).
  #[expect(clippy::unwrap_used, reason = "the unit square is a valid rectangle")]
  pub(crate) fn unit() -> Self {
    Self::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap()
  }

  /// The empty rectangle at the origin.
  #[expect(
    clippy::unwrap_used,
    reason = "a zero-size rectangle at the origin is valid"
  )]
  pub(crate) fn empty() -> Self {
    Self::from_xywh(0.0, 0.0, 0.0, 0.0).unwrap()
  }

  /// Creates new `Rect`.
  pub fn from_ltrb(left: f32, top: f32, right: f32, bottom: f32) -> Option<Self> {
    Some(Self(CoreRect::from_ltrb(left, top, right, bottom)?))
  }

  /// Apply a transform to the rect.
  pub fn transform(self, transform: Transform) -> Option<Self> {
    Some(Self(self.0.transform(transform.0)?))
  }

  /// Creates new `Rect`.
  pub fn from_xywh(x: f32, y: f32, w: f32, h: f32) -> Option<Self> {
    Some(Self(CoreRect::from_xywh(x, y, w, h)?))
  }

  /// Returns the left edge.
  pub fn left(&self) -> f32 {
    self.0.left()
  }

  /// Returns the top edge.
  pub fn top(&self) -> f32 {
    self.0.top()
  }

  /// Returns the right edge.
  pub fn right(&self) -> f32 {
    self.0.right()
  }

  /// Returns the bottom edge.
  pub fn bottom(&self) -> f32 {
    self.0.bottom()
  }

  /// Returns rect's width.
  pub fn width(&self) -> f32 {
    self.0.width()
  }

  /// Returns rect's height.
  pub fn height(&self) -> f32 {
    self.0.height()
  }

  pub(crate) fn from_core(rect: CoreRect) -> Self {
    Self(rect)
  }

  pub(crate) fn expand(&mut self, other: &Rect) {
    let left = self.left().min(other.left());
    let top = self.top().min(other.top());
    let right = self.right().max(other.right());
    let bottom = self.bottom().max(other.bottom());
    // The union of two finite rectangles can still be too wide for an `f32` width.
    if let Some(union) = Rect::from_ltrb(left, top, right, bottom) {
      *self = union;
    }
  }

  pub(crate) fn to_pdf_rect(self) -> pdf_writer::Rect {
    pdf_writer::Rect::new(
      self.left(),
      self.top(),
      self.left() + self.width(),
      self.top() + self.height(),
    )
  }
}

/// An affine transformation matrix.
///
/// Unlike other types, doesn't guarantee to be valid. This is Skia quirk.
/// Meaning Transform(0, 0, 0, 0, 0, 0) is ok, while it's technically not.
/// Non-finite values are also not an error.
#[allow(missing_docs)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Transform(Affine);

impl Default for Transform {
  fn default() -> Self {
    Self::identity()
  }
}

impl Transform {
  pub fn identity() -> Self {
    Self(Affine::IDENTITY)
  }

  pub fn from_row(sx: f32, ky: f32, kx: f32, sy: f32, tx: f32, ty: f32) -> Self {
    Self(Affine {
      a: sx,
      b: ky,
      c: kx,
      d: sy,
      x: tx,
      y: ty,
    })
  }

  pub fn from_translate(tx: f32, ty: f32) -> Self {
    Self::from_row(1.0, 0.0, 0.0, 1.0, tx, ty)
  }

  pub fn from_scale(sx: f32, sy: f32) -> Self {
    Self::from_row(sx, 0.0, 0.0, sy, 0.0, 0.0)
  }

  pub fn from_skew(kx: f32, ky: f32) -> Self {
    Self::from_row(1.0, ky, kx, 1.0, 0.0, 0.0)
  }

  /// A rotation by `angle` degrees.
  pub fn from_rotate(angle: f32) -> Self {
    let radians = angle.to_radians();
    let (sin, cos) = (radians.sin(), radians.cos());

    Self::from_row(cos, sin, -sin, cos, 0.0, 0.0)
  }

  /// A rotation by `angle` degrees around `tx`, `ty`.
  pub fn from_rotate_at(angle: f32, tx: f32, ty: f32) -> Self {
    Self::from_translate(tx, ty)
      .pre_concat(Self::from_rotate(angle))
      .pre_concat(Self::from_translate(-tx, -ty))
  }

  pub fn sx(&self) -> f32 {
    self.0.a
  }

  pub fn sy(&self) -> f32 {
    self.0.d
  }

  pub fn kx(&self) -> f32 {
    self.0.c
  }

  pub fn ky(&self) -> f32 {
    self.0.b
  }

  pub fn tx(&self) -> f32 {
    self.0.x
  }

  pub fn ty(&self) -> f32 {
    self.0.y
  }

  /// `SkMatrix::invertNonIdentity`: the inverse through doubles, none where the determinant is
  /// nearly zero or the inverse leaves the finite range.
  pub fn invert(&self) -> Option<Self> {
    let Affine { a, b, c, d, x, y } = self.0;

    if self.0 == Affine::IDENTITY {
      return Some(*self);
    }

    if b == 0.0 && c == 0.0 {
      if a != 1.0 || d != 1.0 {
        let (inv_x, inv_y) = (1.0 / a, 1.0 / d);

        return Some(Self::from_row(
          inv_x,
          0.0,
          0.0,
          inv_y,
          -x * inv_x,
          -y * inv_y,
        ));
      }

      return Some(Self::from_translate(-x, -y));
    }

    let det = f64::from(a) * f64::from(d) - f64::from(c) * f64::from(b);
    let nearly_zero = 1.0 / 4096.0_f32;

    if (det as f32).abs() <= nearly_zero * nearly_zero * nearly_zero {
      return None;
    }

    let inv_det = 1.0 / det;
    let cross_scale = |p: f32, q: f32, r: f32, s: f32| {
      ((f64::from(p) * f64::from(q) - f64::from(r) * f64::from(s)) * inv_det) as f32
    };
    let inverse = Self::from_row(
      (f64::from(d) * inv_det) as f32,
      (-f64::from(b) * inv_det) as f32,
      (-f64::from(c) * inv_det) as f32,
      (f64::from(a) * inv_det) as f32,
      cross_scale(c, y, d, x),
      cross_scale(b, x, a, y),
    );
    let Affine { a, b, c, d, x, y } = inverse.0;

    [a, b, c, d, x, y]
      .iter()
      .all(|value| value.is_finite())
      .then_some(inverse)
  }

  pub(crate) fn pre_concat(&self, other: Self) -> Self {
    Self(self.0 * other.0)
  }

  pub(crate) fn post_concat(&self, other: Self) -> Self {
    Self(other.0 * self.0)
  }

  pub(crate) fn to_affine(self) -> Affine {
    self.0
  }

  /// The writer's copy of a core transform.
  pub(crate) fn from_affine(affine: Affine) -> Self {
    Self(affine)
  }

  pub(crate) fn to_pdf_transform(self) -> [f32; 6] {
    let Affine { a, b, c, d, x, y } = self.0;

    [a, b, c, d, x, y]
  }
}

impl Hash for Transform {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.x.to_bits().hash(state);
    self.0.y.to_bits().hash(state);
    self.0.a.to_bits().hash(state);
    self.0.d.to_bits().hash(state);
    self.0.c.to_bits().hash(state);
    self.0.b.to_bits().hash(state);
  }
}

/// A path.
pub struct Path(pub(crate) CorePath);

#[derive(Default)]
pub struct PathBuilder(CorePathBuilder);

impl PathBuilder {
  /// Create a new path.
  pub fn new() -> Self {
    Self::default()
  }

  /// Adds beginning of a contour.
  pub fn move_to(&mut self, x: f32, y: f32) {
    self.0.move_to(x, y)
  }

  /// Adds a line from the last point.
  pub fn line_to(&mut self, x: f32, y: f32) {
    self.0.line_to(x, y)
  }

  /// Adds a quad curve from the last point to `x`, `y`.
  pub fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
    self.0.quad_to(x1, y1, x, y)
  }
  /// Adds a cubic curve from the last point to `x`, `y`.
  pub fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
    self.0.cubic_to(x1, y1, x2, y2, x, y)
  }

  /// Close the current contour.
  pub fn close(&mut self) {
    self.0.close()
  }

  /// Push a rectangle to the path.
  pub fn push_rect(&mut self, rect: Rect) {
    self.0.move_to(rect.left(), rect.top());
    self.0.line_to(rect.right(), rect.top());
    self.0.line_to(rect.right(), rect.bottom());
    self.0.line_to(rect.left(), rect.bottom());
    self.0.close();
  }

  /// Finish the current path.
  pub fn finish(self) -> Option<Path> {
    Some(Path(self.0.finish()?))
  }
}
