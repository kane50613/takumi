//! The rectangle arithmetic SVG layout does on [`Rect`], after tiny-skia's `Rect` and
//! `NonZeroRect`, whose rounding the goldens keep.

use crate::{
  geometry::{Rect, Size},
  style::Affine,
};

impl Rect<f32> {
  /// The rect between the edges, none unless it is finite and not inverted.
  pub(in crate::svg) fn from_ltrb(left: f32, top: f32, right: f32, bottom: f32) -> Option<Self> {
    let rect = Self {
      left,
      right,
      top,
      bottom,
    };

    (rect.is_valid() && left <= right && top <= bottom).then_some(rect)
  }

  /// The rect at `x`, `y` of `width` by `height`, none unless it is finite and not inverted.
  pub(in crate::svg) fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Option<Self> {
    Self::from_ltrb(x, y, width + x, height + y)
  }

  /// [`Self::from_xywh`], but with both dimensions above zero.
  pub(in crate::svg) fn positive_from_xywh(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
  ) -> Option<Self> {
    Self::from_xywh(x, y, width, height).filter(Self::has_area)
  }

  fn is_valid(&self) -> bool {
    [self.left, self.right, self.top, self.bottom]
      .iter()
      .all(|edge| edge.is_finite())
      && (self.right - self.left).is_finite()
      && (self.bottom - self.top).is_finite()
  }

  pub(in crate::svg) fn x(&self) -> f32 {
    self.left
  }

  pub(in crate::svg) fn y(&self) -> f32 {
    self.top
  }

  pub(in crate::svg) fn width(&self) -> f32 {
    self.right - self.left
  }

  pub(in crate::svg) fn height(&self) -> f32 {
    self.bottom - self.top
  }

  /// Whether the rect has both dimensions above zero.
  pub(in crate::svg) fn has_area(&self) -> bool {
    self.left < self.right && self.top < self.bottom
  }

  /// The smallest rect holding both.
  pub(in crate::svg) fn union(&self, other: &Self) -> Self {
    Self {
      left: self.left.min(other.left),
      right: self.right.max(other.right),
      top: self.top.min(other.top),
      bottom: self.bottom.max(other.bottom),
    }
  }

  /// The bounds of the rect under `transform`, none when they are not finite.
  pub(in crate::svg) fn transformed(&self, transform: Affine) -> Option<Self> {
    if transform.to_cols_array() == Affine::IDENTITY.to_cols_array() {
      return Some(*self);
    }

    let corners = [
      (self.left, self.top),
      (self.right, self.top),
      (self.right, self.bottom),
      (self.left, self.bottom),
    ]
    .map(|(x, y)| transform.transform_point(x, y));
    let (mut left, mut top) = corners[0];
    let (mut right, mut bottom) = corners[0];

    for (x, y) in corners {
      left = left.min(x);
      right = right.max(x);
      top = top.min(y);
      bottom = bottom.max(y);
    }

    Self::from_ltrb(left, top, right, bottom)
  }

  /// The rect with its coordinates read as fractions of `bbox`.
  pub(in crate::svg) fn in_bounding_box(&self, bbox: &Self) -> Option<Self> {
    Self::positive_from_xywh(
      self.x() * bbox.width() + bbox.x(),
      self.y() * bbox.height() + bbox.y(),
      self.width() * bbox.width(),
      self.height() * bbox.height(),
    )
  }

  /// The transform that maps the unit square onto the rect.
  pub(in crate::svg) fn unit_square_transform(&self) -> Affine {
    Affine {
      a: self.width(),
      b: 0.0,
      c: 0.0,
      d: self.height(),
      x: self.x(),
      y: self.y(),
    }
  }
}

impl Size<f32> {
  /// The size scaled to fit inside `target` keeping its ratio, or to cover it when `expand`, after
  /// tiny-skia's `Size::scale_to` and `Size::expand_to`.
  pub(in crate::svg) fn fitted_to(self, target: Self, expand: bool) -> Self {
    let width = target.height * self.width / self.height;
    let fits_height = if expand {
      width <= target.width
    } else {
      width >= target.width
    };

    if fits_height {
      Self::new(target.width, target.width * self.height / self.width)
    } else {
      Self::new(width, target.height)
    }
  }
}

/// strict-num's `approx_eq_ulps` with four ulps, which usvg compares floats by.
pub(in crate::svg) trait ApproxEq {
  fn approx_eq(self, other: Self) -> bool;
}

impl ApproxEq for f32 {
  fn approx_eq(self, other: Self) -> bool {
    self == other
      || (self.is_sign_negative() == other.is_sign_negative()
        && self.to_bits().abs_diff(other.to_bits()) <= 4)
  }
}
