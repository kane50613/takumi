//! Skia's `SkRect` and `SkIRect`, finite and ordered, and a non-empty integer size.

use super::Point;
use crate::style::Affine;

/// A finite rect whose left is at most its right and top at most its bottom, `SkRect`.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Rect {
  left: f32,
  top: f32,
  right: f32,
  bottom: f32,
}

impl Rect {
  /// The rect between the edges, none where one is not finite, they are out of order, or the
  /// width or height overflows.
  pub fn from_ltrb(left: f32, top: f32, right: f32, bottom: f32) -> Option<Self> {
    let finite = [left, top, right, bottom]
      .iter()
      .all(|edge| edge.is_finite());

    (finite
      && left <= right
      && top <= bottom
      && (right - left).is_finite()
      && (bottom - top).is_finite())
    .then_some(Self {
      left,
      top,
      right,
      bottom,
    })
  }

  /// The rect at `x`, `y` of `width` by `height`.
  pub fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Option<Self> {
    Self::from_ltrb(x, y, width + x, height + y)
  }

  /// The bounds of `points`, none where there are none or one is not finite.
  pub fn from_points(points: &[Point]) -> Option<Self> {
    match points {
      [] => None,
      [point] => Self::from_xywh(point.x, point.y, 0.0, 0.0),
      [first, second] => {
        let (left, right) = if first.x < second.x {
          (first.x, second.x)
        } else {
          (second.x, first.x)
        };
        let (top, bottom) = if first.y < second.y {
          (first.y, second.y)
        } else {
          (second.y, first.y)
        };

        Self::from_ltrb(left, top, right, bottom)
      }
      [first, rest @ ..] => {
        if !points.iter().all(|point| point.is_finite()) {
          return None;
        }

        let (left, top, right, bottom) = rest.iter().fold(
          (first.x, first.y, first.x, first.y),
          |(left, top, right, bottom), point| {
            (
              left.min(point.x),
              top.min(point.y),
              right.max(point.x),
              bottom.max(point.y),
            )
          },
        );

        Self::from_ltrb(left, top, right, bottom)
      }
    }
  }

  /// The left edge.
  pub fn left(&self) -> f32 {
    self.left
  }

  /// The top edge.
  pub fn top(&self) -> f32 {
    self.top
  }

  /// The right edge.
  pub fn right(&self) -> f32 {
    self.right
  }

  /// The bottom edge.
  pub fn bottom(&self) -> f32 {
    self.bottom
  }

  /// The left edge.
  pub fn x(&self) -> f32 {
    self.left
  }

  /// The top edge.
  pub fn y(&self) -> f32 {
    self.top
  }

  /// The width.
  pub fn width(&self) -> f32 {
    self.right - self.left
  }

  /// The height.
  pub fn height(&self) -> f32 {
    self.bottom - self.top
  }

  /// `SkRect::roundOut`: the pixels the rect touches, at least one each way.
  pub fn round_out(&self) -> Option<IntRect> {
    let left = self.left.floor() as i32;
    let top = self.top.floor() as i32;
    let right = self.right.ceil() as i32;
    let bottom = self.bottom.ceil() as i32;

    IntRect::from_xywh(
      left,
      top,
      right.saturating_sub(left).max(1) as u32,
      bottom.saturating_sub(top).max(1) as u32,
    )
  }

  /// The rect grown by `dx` on the left and right and `dy` on the top and bottom.
  pub fn outset(&self, dx: f32, dy: f32) -> Option<Self> {
    Self::from_ltrb(
      self.left - dx,
      self.top - dy,
      self.right + dx,
      self.bottom + dy,
    )
  }

  /// Whether the rect has no width or no height.
  pub fn is_empty(&self) -> bool {
    self.left == self.right || self.top == self.bottom
  }

  /// The rect covering both, or whichever is not empty.
  pub fn join(&self, other: &Self) -> Option<Self> {
    if other.is_empty() {
      return Some(*self);
    }

    if self.is_empty() {
      return Some(*other);
    }

    Self::from_ltrb(
      self.left.min(other.left),
      self.top.min(other.top),
      self.right.max(other.right),
      self.bottom.max(other.bottom),
    )
  }

  /// The overlap with `other`, none where it is not a rect.
  pub fn intersect(&self, other: &Self) -> Option<Self> {
    Self::from_ltrb(
      self.left.max(other.left),
      self.top.max(other.top),
      self.right.min(other.right),
      self.bottom.min(other.bottom),
    )
  }

  /// The bounds of the rect's corners mapped by `transform`.
  pub fn transform(&self, transform: Affine) -> Option<Self> {
    if transform.is_identity() {
      return Some(*self);
    }

    let corners = [
      (self.left, self.top),
      (self.right, self.top),
      (self.right, self.bottom),
      (self.left, self.bottom),
    ]
    .map(|(x, y)| {
      let (x, y) = transform.transform_point(x, y);

      Point::from_xy(x, y)
    });

    Self::from_points(&corners)
  }
}

/// A non-empty integer rect, `SkIRect`, whose right and bottom fit `i32`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct IntRect {
  x: i32,
  y: i32,
  width: u32,
  height: u32,
}

impl IntRect {
  /// The rect at `x`, `y` of `width` by `height`, none where it is empty or its far edges
  /// overflow.
  pub fn from_xywh(x: i32, y: i32, width: u32, height: u32) -> Option<Self> {
    x.checked_add(i32::try_from(width).ok()?)?;
    y.checked_add(i32::try_from(height).ok()?)?;

    (width > 0 && height > 0).then_some(Self {
      x,
      y,
      width,
      height,
    })
  }

  /// The rect between the edges, none where it is empty.
  pub fn from_ltrb(left: i32, top: i32, right: i32, bottom: i32) -> Option<Self> {
    let width = u32::try_from(right.checked_sub(left)?).ok()?;
    let height = u32::try_from(bottom.checked_sub(top)?).ok()?;

    Self::from_xywh(left, top, width, height)
  }

  /// The left edge.
  pub fn x(&self) -> i32 {
    self.x
  }

  /// The top edge.
  pub fn y(&self) -> i32 {
    self.y
  }

  /// The width.
  pub fn width(&self) -> u32 {
    self.width
  }

  /// The height.
  pub fn height(&self) -> u32 {
    self.height
  }

  /// The left edge.
  pub fn left(&self) -> i32 {
    self.x
  }

  /// The top edge.
  pub fn top(&self) -> i32 {
    self.y
  }

  /// The right edge.
  pub fn right(&self) -> i32 {
    self.x + self.width as i32
  }

  /// The bottom edge.
  pub fn bottom(&self) -> i32 {
    self.y + self.height as i32
  }

  /// The size.
  pub fn size(&self) -> IntSize {
    IntSize {
      width: self.width,
      height: self.height,
    }
  }

  /// The rect moved by `dx` and `dy`.
  pub fn translate(&self, dx: i32, dy: i32) -> Option<Self> {
    Self::from_xywh(self.x + dx, self.y + dy, self.width, self.height)
  }

  /// The overlap with `other`, none where they do not overlap.
  pub fn intersect(&self, other: &Self) -> Option<Self> {
    Self::from_ltrb(
      self.x.max(other.x),
      self.y.max(other.y),
      self.right().min(other.right()),
      self.bottom().min(other.bottom()),
    )
  }
}

/// A non-empty integer size.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct IntSize {
  width: u32,
  height: u32,
}

impl IntSize {
  /// The size `width` by `height`, none where either is zero.
  pub fn from_wh(width: u32, height: u32) -> Option<Self> {
    (width > 0 && height > 0).then_some(Self { width, height })
  }

  /// The width.
  pub fn width(&self) -> u32 {
    self.width
  }

  /// The height.
  pub fn height(&self) -> u32 {
    self.height
  }
}
