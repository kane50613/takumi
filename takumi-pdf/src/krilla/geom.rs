//! Path geometry.

use std::hash::{Hash, Hasher};

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

  pub(crate) fn to_tsp(self) -> tiny_skia_path::Point {
    tiny_skia_path::Point::from_xy(self.x, self.y)
  }
}

/// A size.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Size(tiny_skia_path::Size);

impl Size {
  /// Creates a new `Size` from width and height.
  ///
  /// Returns `None` if either the width or the height is not > 0.
  pub fn from_wh(width: f32, height: f32) -> Option<Self> {
    Some(Self(tiny_skia_path::Size::from_wh(width, height)?))
  }

  /// Returns the width of the size.
  pub fn width(&self) -> f32 {
    self.0.width()
  }

  /// Returns the height of the size.
  pub fn height(&self) -> f32 {
    self.0.height()
  }
}

/// A rectangle defined by left, top, right and bottom edges.
#[allow(missing_docs)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Rect(tiny_skia_path::Rect);

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
  /// Creates new `Rect`.
  pub fn from_ltrb(left: f32, top: f32, right: f32, bottom: f32) -> Option<Self> {
    Some(Self(tiny_skia_path::Rect::from_ltrb(
      left, top, right, bottom,
    )?))
  }

  /// Apply a transform to the rect.
  pub fn transform(self, transform: Transform) -> Option<Self> {
    Some(Self(self.0.transform(transform.to_tsp())?))
  }

  /// Creates new `Rect`.
  pub fn from_xywh(x: f32, y: f32, w: f32, h: f32) -> Option<Self> {
    Some(Self(tiny_skia_path::Rect::from_xywh(x, y, w, h)?))
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

  pub(crate) fn to_tsp(self) -> tiny_skia_path::Rect {
    self.0
  }

  pub(crate) fn from_tsp(rect: tiny_skia_path::Rect) -> Self {
    Self(rect)
  }

  pub(crate) fn expand(&mut self, other: &Rect) {
    let left = self.left().min(other.left());
    let top = self.top().min(other.top());
    let right = self.right().max(other.right());
    let bottom = self.bottom().max(other.bottom());
    *self = Rect::from_ltrb(left, top, right, bottom).unwrap();
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

/// A quadrilateral.
///
/// The points should be given in the following order:
/// 1. bottom-left
/// 2. bottom-right
/// 3. top-right
/// 4. top-left
///
/// Where the coordinate system is Y-down:
/// ```md
///      |
///      |
/// -----+---->  X
///      |
///      |
///      v
///
///      Y
/// ```
#[derive(Clone, Copy, PartialEq)]
pub struct Quadrilateral(pub [Point; 4]);

impl From<Rect> for Quadrilateral {
  fn from(r: Rect) -> Self {
    Self([
      Point::from_xy(r.left(), r.bottom()),
      Point::from_xy(r.right(), r.bottom()),
      Point::from_xy(r.right(), r.top()),
      Point::from_xy(r.left(), r.top()),
    ])
  }
}

/// An affine transformation matrix.
///
/// Unlike other types, doesn't guarantee to be valid. This is Skia quirk.
/// Meaning Transform(0, 0, 0, 0, 0, 0) is ok, while it's technically not.
/// Non-finite values are also not an error.
#[allow(missing_docs)]
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct Transform(tiny_skia_path::Transform);

impl Transform {
  /// Creates an identity transform.
  pub fn identity() -> Self {
    Self(tiny_skia_path::Transform::default())
  }

  /// Creates a new `Transform`.
  pub fn from_row(sx: f32, ky: f32, kx: f32, sy: f32, tx: f32, ty: f32) -> Self {
    Self(tiny_skia_path::Transform::from_row(sx, ky, kx, sy, tx, ty))
  }

  /// Creates a new translating `Transform`.
  pub fn from_translate(tx: f32, ty: f32) -> Self {
    Self(tiny_skia_path::Transform::from_translate(tx, ty))
  }

  /// Creates a new scaling `Transform`.
  pub fn from_scale(sx: f32, sy: f32) -> Self {
    Self(tiny_skia_path::Transform::from_scale(sx, sy))
  }

  /// Creates a new skewing `Transform`.
  pub fn from_skew(kx: f32, ky: f32) -> Self {
    Self(tiny_skia_path::Transform::from_skew(kx, ky))
  }

  /// Creates a new rotating `Transform`.
  ///
  /// `angle` in degrees.
  pub fn from_rotate(angle: f32) -> Self {
    Self(tiny_skia_path::Transform::from_rotate(angle))
  }

  /// Creates a new rotating `Transform` at the specified position.
  ///
  /// `angle` in degrees.
  pub fn from_rotate_at(angle: f32, tx: f32, ty: f32) -> Self {
    Self(tiny_skia_path::Transform::from_rotate_at(angle, tx, ty))
  }

  /// Return the `sx` component
  pub fn sx(&self) -> f32 {
    self.0.sx
  }

  /// Return the `sy` component
  pub fn sy(&self) -> f32 {
    self.0.sy
  }

  /// Return the `kx` component
  pub fn kx(&self) -> f32 {
    self.0.kx
  }

  /// Return the `kx` component
  pub fn ky(&self) -> f32 {
    self.0.ky
  }

  /// Return the `tx` component
  pub fn tx(&self) -> f32 {
    self.0.tx
  }

  /// Return the `ty` component
  pub fn ty(&self) -> f32 {
    self.0.ty
  }

  /// Returns the inverted transform.
  pub fn invert(&self) -> Option<Self> {
    Some(Self(self.0.invert()?))
  }

  pub(crate) fn pre_concat(&self, other: Self) -> Self {
    Self(self.0.pre_concat(other.0))
  }

  pub(crate) fn post_concat(&self, other: Self) -> Self {
    Self(self.0.post_concat(other.0))
  }

  pub(crate) fn to_tsp(self) -> tiny_skia_path::Transform {
    self.0
  }

  pub(crate) fn from_tsp(ts: tiny_skia_path::Transform) -> Self {
    Self(ts)
  }

  pub(crate) fn to_pdf_transform(self) -> [f32; 6] {
    [
      self.0.sx, self.0.ky, self.0.kx, self.0.sy, self.0.tx, self.0.ty,
    ]
  }
}

impl Hash for Transform {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.tx.to_bits().hash(state);
    self.0.ty.to_bits().hash(state);
    self.0.sx.to_bits().hash(state);
    self.0.sy.to_bits().hash(state);
    self.0.kx.to_bits().hash(state);
    self.0.ky.to_bits().hash(state);
  }
}

/// A path.
pub struct Path(pub(crate) tiny_skia_path::Path);

impl Path {
  /// Apply a transformation to the path.
  pub fn transform(self, transform: Transform) -> Option<Self> {
    Some(Self(self.0.transform(transform.to_tsp())?))
  }
}

/// A path builder.
#[derive(Default)]
pub struct PathBuilder(tiny_skia_path::PathBuilder);

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
    self.0.push_rect(rect.to_tsp())
  }

  /// Finish the current path.
  pub fn finish(self) -> Option<Path> {
    Some(Path(self.0.finish()?))
  }
}
