//! Blink's `LayoutUnit`: a layout length in 1/64px fixed point that saturates at the ends of its
//! `i32` range, after
//! [`layout_unit.h`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/geometry/layout_unit.h).
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

const FRACTIONAL_BITS: u32 = 6;
const DENOMINATOR: i32 = 1 << FRACTIONAL_BITS;
const INT_MAX: i32 = i32::MAX / DENOMINATOR;
const INT_MIN: i32 = i32::MIN / DENOMINATOR;

/// A length in 1/64px.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct LayoutUnit(i32);

impl LayoutUnit {
  /// Zero.
  pub const ZERO: Self = Self(0);

  /// The unit whose raw fixed-point value is `raw`.
  pub const fn from_raw(raw: i32) -> Self {
    Self(raw)
  }

  /// `raw`, saturated to the `i32` range, where NaN gives zero, as `base::saturated_cast` does.
  fn saturated(raw: f64) -> Self {
    if raw.is_nan() {
      return Self::ZERO;
    }

    Self(raw.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32)
  }

  /// The raw fixed-point value.
  pub const fn raw(self) -> i32 {
    self.0
  }

  /// `value` px, saturated to the whole pixels the unit can hold.
  pub const fn from_int(value: i32) -> Self {
    if value > INT_MAX {
      Self(i32::MAX)
    } else if value < INT_MIN {
      Self(i32::MIN)
    } else {
      Self(value << FRACTIONAL_BITS)
    }
  }

  /// `value` px truncated toward zero to a 1/64 step, as `LayoutUnit(float)` constructs it.
  pub fn from_f32(value: f32) -> Self {
    Self::saturated(f64::from(value * DENOMINATOR as f32).trunc())
  }

  /// `value` px rounded down to a 1/64 step.
  pub fn from_f32_floor(value: f32) -> Self {
    Self::saturated(f64::from((value * DENOMINATOR as f32).floor()))
  }

  /// `value` px rounded up to a 1/64 step.
  pub fn from_f32_ceil(value: f32) -> Self {
    Self::saturated(f64::from((value * DENOMINATOR as f32).ceil()))
  }

  /// `value` px rounded to the nearest 1/64 step, halves away from zero.
  pub fn from_f32_round(value: f32) -> Self {
    Self::saturated(f64::from((value * DENOMINATOR as f32).round()))
  }

  /// The length in px.
  pub fn to_f32(self) -> f32 {
    self.0 as f32 / DENOMINATOR as f32
  }

  /// The whole px, truncated toward zero.
  pub const fn to_int(self) -> i32 {
    self.0 / DENOMINATOR
  }

  /// The whole px nearest, as Blink's `Round()` computes it from the truncated part and the
  /// fraction.
  pub const fn round(self) -> i32 {
    self.to_int() + ((self.fraction().0 + DENOMINATOR / 2) >> FRACTIONAL_BITS)
  }

  /// The whole px at or below.
  pub const fn floor(self) -> i32 {
    if self.0 < i32::MIN + DENOMINATOR {
      return INT_MIN;
    }

    self.0 >> FRACTIONAL_BITS
  }

  /// The whole px at or above.
  pub const fn ceil(self) -> i32 {
    if self.0 > i32::MAX - DENOMINATOR {
      return INT_MAX;
    }
    if self.0 >= 0 {
      return (self.0 + DENOMINATOR - 1) / DENOMINATOR;
    }

    self.to_int()
  }

  /// The part below a whole px, keeping the sign.
  pub const fn fraction(self) -> Self {
    Self(self.0 % DENOMINATOR)
  }

  /// The length without its sign.
  pub const fn abs(self) -> Self {
    Self(self.0.saturating_abs())
  }

  /// Zero in place of a negative length.
  pub const fn clamp_negative_to_zero(self) -> Self {
    if self.0 < 0 { Self::ZERO } else { self }
  }

  /// Blink's `IntMod`: the remainder after a division with a whole result, so that
  /// `a = (a / b).to_int() * b + a.int_mod(b)`.
  pub const fn int_mod(self, divisor: Self) -> Self {
    Self(self.0 % divisor.0)
  }
}

impl Add for LayoutUnit {
  type Output = Self;

  fn add(self, other: Self) -> Self {
    Self(self.0.saturating_add(other.0))
  }
}

impl AddAssign for LayoutUnit {
  fn add_assign(&mut self, other: Self) {
    *self = *self + other;
  }
}

impl Sub for LayoutUnit {
  type Output = Self;

  fn sub(self, other: Self) -> Self {
    Self(self.0.saturating_sub(other.0))
  }
}

impl SubAssign for LayoutUnit {
  fn sub_assign(&mut self, other: Self) {
    *self = *self - other;
  }
}

impl Neg for LayoutUnit {
  type Output = Self;

  fn neg(self) -> Self {
    Self(self.0.saturating_neg())
  }
}

/// Blink's `BoundedMultiply`.
impl Mul for LayoutUnit {
  type Output = Self;

  fn mul(self, other: Self) -> Self {
    Self::saturated((i64::from(self.0) * i64::from(other.0) / i64::from(DENOMINATOR)) as f64)
  }
}

impl Mul<i32> for LayoutUnit {
  type Output = Self;

  fn mul(self, other: i32) -> Self {
    Self(self.0.saturating_mul(other))
  }
}

impl Mul<LayoutUnit> for i32 {
  type Output = LayoutUnit;

  fn mul(self, other: LayoutUnit) -> LayoutUnit {
    other * self
  }
}

/// Division rounds toward zero, as Blink's does.
impl Div for LayoutUnit {
  type Output = Self;

  fn div(self, other: Self) -> Self {
    Self::saturated((i64::from(DENOMINATOR) * i64::from(self.0) / i64::from(other.0)) as f64)
  }
}

impl Div<i32> for LayoutUnit {
  type Output = Self;

  fn div(self, other: i32) -> Self {
    Self(self.0 / other)
  }
}

/// Blink's `SnapSizeToPixel`: the whole px `size` covers from `location`, never snapping a size
/// of more than 4/64 px to zero.
pub fn snap_size_to_pixel(size: LayoutUnit, location: LayoutUnit) -> i32 {
  let result = snap_size_to_pixel_allowing_zero(size, location);

  if result == 0 && (size.0 > 4 || size.0 < -4) {
    return if size > LayoutUnit::ZERO { 1 } else { -1 };
  }

  result
}

/// Blink's `SnapSizeToPixelAllowingZero`.
pub fn snap_size_to_pixel_allowing_zero(size: LayoutUnit, location: LayoutUnit) -> i32 {
  let fraction = location.fraction();

  (fraction + size).round() - fraction.round()
}

/// Blink's `PhysicalBoxStrut`: a length on each side of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BoxStrut {
  /// The top side.
  pub top: LayoutUnit,
  /// The right side.
  pub right: LayoutUnit,
  /// The bottom side.
  pub bottom: LayoutUnit,
  /// The left side.
  pub left: LayoutUnit,
}

impl BoxStrut {
  /// Whether every side is zero.
  pub fn is_zero(self) -> bool {
    self == Self::default()
  }

  /// The offset the top and left sides move a box's origin by.
  pub fn offset(self) -> (LayoutUnit, LayoutUnit) {
    (self.left, self.top)
  }
}

impl Add for BoxStrut {
  type Output = Self;

  fn add(self, other: Self) -> Self {
    Self {
      top: self.top + other.top,
      right: self.right + other.right,
      bottom: self.bottom + other.bottom,
      left: self.left + other.left,
    }
  }
}

/// Blink's `PhysicalRect`: a rectangle in layout units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnitRect {
  /// The left edge.
  pub x: LayoutUnit,
  /// The top edge.
  pub y: LayoutUnit,
  /// The width.
  pub width: LayoutUnit,
  /// The height.
  pub height: LayoutUnit,
}

impl UnitRect {
  /// The right edge.
  pub fn right(self) -> LayoutUnit {
    self.x + self.width
  }

  /// The bottom edge.
  pub fn bottom(self) -> LayoutUnit {
    self.y + self.height
  }

  /// Whether the rectangle encloses no area.
  pub fn is_empty(self) -> bool {
    self.width <= LayoutUnit::ZERO || self.height <= LayoutUnit::ZERO
  }

  /// Blink's `Expand`: the rectangle grown by `strut` on each side.
  pub fn expand(self, strut: BoxStrut) -> Self {
    Self {
      x: self.x - strut.left,
      y: self.y - strut.top,
      width: self.width + strut.left + strut.right,
      height: self.height + strut.top + strut.bottom,
    }
  }

  /// Blink's `Contract`: the rectangle shrunk by `strut` on each side.
  pub fn contract(self, strut: BoxStrut) -> Self {
    self.expand(BoxStrut {
      top: -strut.top,
      right: -strut.right,
      bottom: -strut.bottom,
      left: -strut.left,
    })
  }

  /// The rectangle with a negative width or height set to zero.
  pub fn clamp_negative_size_to_zero(self) -> Self {
    Self {
      width: self.width.clamp_negative_to_zero(),
      height: self.height.clamp_negative_to_zero(),
      ..self
    }
  }

  /// Blink's `ToPixelSnappedRect`, back in layout units.
  pub fn pixel_snapped(self) -> Self {
    Self {
      x: LayoutUnit::from_int(self.x.round()),
      y: LayoutUnit::from_int(self.y.round()),
      width: LayoutUnit::from_int(snap_size_to_pixel(self.width, self.x)),
      height: LayoutUnit::from_int(snap_size_to_pixel(self.height, self.y)),
    }
  }

  /// Blink's `Intersect`: the overlap with `other`, or an empty rectangle at the origin.
  pub fn intersect(self, other: Self) -> Self {
    let x = self.x.max(other.x);
    let y = self.y.max(other.y);
    let right = self.right().min(other.right());
    let bottom = self.bottom().min(other.bottom());

    if x >= right || y >= bottom {
      return Self::default();
    }

    Self {
      x,
      y,
      width: right - x,
      height: bottom - y,
    }
  }

  /// Whether `other` lies wholly inside.
  pub fn contains(self, other: Self) -> bool {
    self.x <= other.x
      && self.y <= other.y
      && self.right() >= other.right()
      && self.bottom() >= other.bottom()
  }
}

#[cfg(test)]
mod tests {
  use super::{LayoutUnit, UnitRect};

  #[test]
  fn floats_convert_as_blink_rounds_them() {
    assert_eq!(LayoutUnit::from_f32(1.999).raw(), 127);
    assert_eq!(LayoutUnit::from_f32(-1.999).raw(), -127);
    assert_eq!(LayoutUnit::from_f32_floor(-1.999).raw(), -128);
    assert_eq!(LayoutUnit::from_f32_ceil(1.001).raw(), 65);
    assert_eq!(LayoutUnit::from_f32_round(0.5 / 64.0).raw(), 1);
    assert_eq!(LayoutUnit::from_f32(f32::NAN), LayoutUnit::ZERO);
    assert_eq!(LayoutUnit::from_f32(1e12).raw(), i32::MAX);
  }

  #[test]
  fn whole_pixels_round_as_blink_rounds_them() {
    let px = |raw| LayoutUnit::from_raw(raw);

    assert_eq!(px(96).round(), 2);
    assert_eq!(px(95).round(), 1);
    assert_eq!(px(-96).round(), -1);
    assert_eq!(px(-97).round(), -2);
    assert_eq!(px(-1).floor(), -1);
    assert_eq!(px(-1).ceil(), 0);
    assert_eq!(px(65).ceil(), 2);
    assert_eq!(px(-65).to_int(), -1);
  }

  #[test]
  fn arithmetic_saturates_and_truncates() {
    let px = LayoutUnit::from_int;

    assert_eq!((px(7) / px(2)).raw(), 224);
    assert_eq!((px(-7) / px(2)).raw(), -224);
    assert_eq!((px(3) * px(2)), px(6));
    assert_eq!(px(7).int_mod(px(2)), px(1));
    assert_eq!(px(-7).int_mod(px(2)), px(-1));
    assert_eq!(LayoutUnit::from_int(i32::MAX).raw(), i32::MAX);
    assert_eq!((LayoutUnit::from_raw(i32::MAX) + px(1)).raw(), i32::MAX);
    assert_eq!((-LayoutUnit::from_raw(i32::MIN)).raw(), i32::MAX);
  }

  #[test]
  fn pixel_snapping_follows_the_location_fraction() {
    let rect = UnitRect {
      x: LayoutUnit::from_f32(10.5),
      y: LayoutUnit::from_f32(0.25),
      width: LayoutUnit::from_f32(20.25),
      height: LayoutUnit::from_f32(0.1),
    }
    .pixel_snapped();

    assert_eq!(rect.x, LayoutUnit::from_int(11));
    assert_eq!(rect.width, LayoutUnit::from_int(20));
    assert_eq!(rect.height, LayoutUnit::from_int(1));
  }
}
