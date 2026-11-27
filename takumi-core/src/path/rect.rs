//! The parts of Skia's `SkRect` and `SkIRect` that paths and pixmaps use, on the core rects.

use crate::math::hash_f32;
use std::hash::{Hash, Hasher};

use crate::geometry::{Point, Rect, Size};

impl Rect<f32> {
  /// `SkRect::setBounds`: the bounds of `points`, none where there are none or one is not finite.
  pub(crate) fn from_points(points: &[Point<f32>]) -> Option<Self> {
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

  /// `SkRect::roundOut`: the pixels the rect touches, at least one each way.
  pub fn round_out(&self) -> Option<Rect<i32>> {
    let left = self.left.floor() as i32;
    let top = self.top.floor() as i32;
    let right = self.right.ceil() as i32;
    let bottom = self.bottom.ceil() as i32;

    Rect::checked_from_size(
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

  /// `SkRect::join`: the rect covering both, or whichever is not empty.
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
}

impl Rect<i32> {
  /// [`Self::from_size`], none where it is empty or its far edges overflow.
  pub fn checked_from_size(x: i32, y: i32, width: u32, height: u32) -> Option<Self> {
    let right = x.checked_add(i32::try_from(width).ok()?)?;
    let bottom = y.checked_add(i32::try_from(height).ok()?)?;

    (width > 0 && height > 0).then_some(Self {
      left: x,
      top: y,
      right,
      bottom,
    })
  }

  /// The rect between the edges, none where it is empty.
  pub fn checked_from_edges(left: i32, top: i32, right: i32, bottom: i32) -> Option<Self> {
    let width = u32::try_from(right.checked_sub(left)?).ok()?;
    let height = u32::try_from(bottom.checked_sub(top)?).ok()?;

    Self::checked_from_size(left, top, width, height)
  }

  /// [`Self::translated`], none where an edge overflows.
  pub fn checked_translated(&self, dx: i32, dy: i32) -> Option<Self> {
    Self::checked_from_size(
      self.left.checked_add(dx)?,
      self.top.checked_add(dy)?,
      self.width(),
      self.height(),
    )
  }

  /// The width and height.
  pub fn size(&self) -> Size<u32> {
    Size::new(self.width(), self.height())
  }
}

impl Eq for Rect<f32> {}

impl Hash for Rect<f32> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    for edge in [self.left, self.top, self.right, self.bottom] {
      hash_f32(edge, state);
    }
  }
}

impl Size<f32> {
  /// The size, none where either side is not finite and above zero.
  pub fn positive(width: f32, height: f32) -> Option<Self> {
    (width > 0.0 && height > 0.0 && width.is_finite() && height.is_finite())
      .then_some(Self::new(width, height))
  }
}

impl Size<u32> {
  /// The size, none where either side is zero.
  pub fn checked_new(width: u32, height: u32) -> Option<Self> {
    (width > 0 && height > 0).then_some(Self::new(width, height))
  }
}
