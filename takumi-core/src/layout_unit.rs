//! Blink's `FixedPoint` and the three units it makes, after
//! [`layout_unit.h`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/geometry/layout_unit.h):
//! `LayoutUnit` holds 1/64px in an `i32`, `TextRunLayoutUnit` 1/65536px in an `i32` for a run of
//! text, and `InlineLayoutUnit` 1/65536px in an `i64` for a whole line. Each saturates at the ends
//! of its range. Follows Blink under the notice in LICENSE-CHROMIUM.

use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Rem, Sub, SubAssign};

use crate::geometry::{Point, Rect, Size};

/// What every unit `FixedPoint` makes shares, so one converts to another as Blink's `To<>()`,
/// `FromFixed<>()`, and `ToCeil<>()` do.
pub(crate) trait FixedPoint: Copy {
  /// Blink's `kFractionalBits`.
  const FRACTIONAL_BITS: u32;

  /// The raw value, widened so every unit's fits.
  fn raw_value_wide(self) -> i128;

  /// Blink's `FromRawValueWithClamp`, from any unit's raw value.
  fn from_raw_value_with_clamp_wide(raw_value: i128) -> Self;

  /// The largest value, Blink's `Max()`.
  fn max_value() -> Self;

  /// The least value, Blink's `Min()`.
  fn min_value() -> Self;

  /// Blink's `FromFixed<source_fractional_bits>`: a raw value with `source_fractional_bits`
  /// fractional bits, truncated or saturated to this unit.
  fn from_fixed(source_fractional_bits: u32, value: i128) -> Self {
    if source_fractional_bits >= Self::FRACTIONAL_BITS {
      return Self::from_raw_value_with_clamp_wide(
        value >> (source_fractional_bits - Self::FRACTIONAL_BITS),
      );
    }

    let bits_diff = Self::FRACTIONAL_BITS - source_fractional_bits;

    if value >= Self::max_value().raw_value_wide() >> bits_diff {
      return Self::max_value();
    }

    if value <= Self::min_value().raw_value_wide() >> bits_diff {
      return Self::min_value();
    }

    Self::from_raw_value_with_clamp_wide(value << bits_diff)
  }

  /// Blink's `To<Target>()`: this value in another unit.
  fn to<Target: FixedPoint>(self) -> Target {
    Target::from_fixed(Self::FRACTIONAL_BITS, self.raw_value_wide())
  }

  /// Blink's `ToCeil<Target>()`: this value in a unit with fewer fractional bits, rounding the lost
  /// precision up.
  fn to_ceil<Target: FixedPoint>(self) -> Target {
    debug_assert!(Target::FRACTIONAL_BITS < Self::FRACTIONAL_BITS);

    let bits_diff = Self::FRACTIONAL_BITS - Target::FRACTIONAL_BITS;
    let raw_value = self.raw_value_wide();
    let mut ceiled = raw_value >> bits_diff;

    if raw_value & ((1 << bits_diff) - 1) != 0 {
      ceiled += 1;
    }

    Target::from_raw_value_with_clamp_wide(ceiled)
  }
}

/// Blink's `FixedPoint<fractional_bits, Storage>` template, instantiated as `$name`.
macro_rules! fixed_point {
  ($(#[$doc:meta])* $name:ident, $fractional_bits:expr, $storage:ty, $unsigned_storage:ty, $wide:ty) => {
    $(#[$doc])*
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    pub(crate) struct $name($storage);

    // The template's whole API, which each unit uses only part of.
    #[allow(dead_code)]
    impl $name {
      pub(crate) const FRACTIONAL_BITS: u32 = $fractional_bits;
      pub(crate) const INTEGRAL_BITS: u32 = <$storage>::BITS - Self::FRACTIONAL_BITS;
      pub(crate) const FIXED_POINT_DENOMINATOR: $storage = 1 << Self::FRACTIONAL_BITS;
      pub(crate) const RAW_VALUE_MAX: $storage = <$storage>::MAX;
      pub(crate) const RAW_VALUE_MIN: $storage = <$storage>::MIN;
      pub(crate) const INT_MAX: $storage = Self::RAW_VALUE_MAX / Self::FIXED_POINT_DENOMINATOR;
      pub(crate) const INT_MIN: $storage = Self::RAW_VALUE_MIN / Self::FIXED_POINT_DENOMINATOR;

      /// Zero, Blink's `FixedPoint()`.
      pub(crate) const ZERO: Self = Self(0);
      /// Blink's `Max()`.
      pub(crate) const MAX: Self = Self(Self::RAW_VALUE_MAX);
      /// Blink's `Min()`.
      pub(crate) const MIN: Self = Self(Self::RAW_VALUE_MIN);
      /// Blink's `NearlyMax()`, a little below `MAX` so it rounds without overflowing.
      pub(crate) const NEARLY_MAX: Self = Self(Self::RAW_VALUE_MAX - Self::FIXED_POINT_DENOMINATOR / 2);
      /// Blink's `NearlyMin()`, a little above `MIN` so it rounds without overflowing.
      pub(crate) const NEARLY_MIN: Self = Self(Self::RAW_VALUE_MIN + Self::FIXED_POINT_DENOMINATOR / 2);

      /// Blink's `ClampRawValue` from a float: truncated toward zero and saturated, where NaN gives
      /// zero, as `base::saturated_cast` and Rust's `as` both convert.
      fn clamp_raw_value_float(raw_value: f64) -> $storage {
        raw_value as $storage
      }

      /// Blink's `ClampRawValue` from a wider integer.
      fn clamp_raw_value(raw_value: $wide) -> $storage {
        raw_value.clamp(<$wide>::from(Self::RAW_VALUE_MIN), <$wide>::from(Self::RAW_VALUE_MAX))
          as $storage
      }

      /// `value` whole px, Blink's `FixedPoint(int)`, saturated by `SaturatedSet`.
      pub(crate) const fn from_int(value: i32) -> Self {
        if value as $wide > Self::INT_MAX as $wide {
          Self(Self::RAW_VALUE_MAX)
        } else if (value as $wide) < Self::INT_MIN as $wide {
          Self(Self::RAW_VALUE_MIN)
        } else {
          Self(((value as $storage as $unsigned_storage) << Self::FRACTIONAL_BITS) as $storage)
        }
      }

      /// `value` whole px, Blink's `FixedPoint(unsigned)`.
      pub(crate) const fn from_unsigned(value: u32) -> Self {
        if value as $wide >= Self::INT_MAX as $wide {
          Self(Self::RAW_VALUE_MAX)
        } else {
          Self(((value as $unsigned_storage) << Self::FRACTIONAL_BITS) as $storage)
        }
      }

      /// `value` whole px, Blink's `FixedPoint(int64_t)`, saturated by `ClampRawValue`.
      pub(crate) fn from_int64(value: i64) -> Self {
        Self(Self::clamp_raw_value(
          <$wide>::from(value).saturating_mul(<$wide>::from(Self::FIXED_POINT_DENOMINATOR)),
        ))
      }

      /// `value` px truncated toward zero to a step, Blink's `FixedPoint(float)`.
      pub(crate) fn from_float(value: f32) -> Self {
        Self(Self::clamp_raw_value_float(f64::from(value * Self::FIXED_POINT_DENOMINATOR as f32)))
      }

      /// `value` px truncated toward zero to a step, Blink's `FixedPoint(double)`.
      pub(crate) fn from_double(value: f64) -> Self {
        Self(Self::clamp_raw_value_float(value * Self::FIXED_POINT_DENOMINATOR as f64))
      }

      /// Blink's `FromFloatCeil`.
      pub(crate) fn from_float_ceil(value: f32) -> Self {
        Self(Self::clamp_raw_value_float(f64::from(
          (value * Self::FIXED_POINT_DENOMINATOR as f32).ceil(),
        )))
      }

      /// Blink's `FromFloatFloor`.
      pub(crate) fn from_float_floor(value: f32) -> Self {
        Self(Self::clamp_raw_value_float(f64::from(
          (value * Self::FIXED_POINT_DENOMINATOR as f32).floor(),
        )))
      }

      /// Blink's `FromFloatRound`: halves away from zero, as `roundf`.
      pub(crate) fn from_float_round(value: f32) -> Self {
        Self(Self::clamp_raw_value_float(f64::from(
          (value * Self::FIXED_POINT_DENOMINATOR as f32).round(),
        )))
      }

      /// Blink's `FromDoubleRound`.
      pub(crate) fn from_double_round(value: f64) -> Self {
        Self(Self::clamp_raw_value_float(
          (value * Self::FIXED_POINT_DENOMINATOR as f64).round(),
        ))
      }

      /// Blink's `FromRawValue`.
      pub(crate) const fn from_raw_value(raw_value: $storage) -> Self {
        Self(raw_value)
      }

      /// Blink's `FromRawValueWithClamp`.
      pub(crate) fn from_raw_value_with_clamp(raw_value: $wide) -> Self {
        Self(Self::clamp_raw_value(raw_value))
      }

      /// Blink's `FromFloatEncompassRound`: steps around `start_value` and `end_value` that
      /// encompass them, both floored when they are equal.
      pub(crate) fn from_float_encompass_round(start_value: f32, end_value: f32) -> (Self, Self) {
        if start_value < end_value {
          (Self::from_float_floor(start_value), Self::from_float_ceil(end_value))
        } else if start_value > end_value {
          (Self::from_float_ceil(start_value), Self::from_float_floor(end_value))
        } else {
          let floored = Self::from_float_floor(start_value);

          (floored, floored)
        }
      }

      /// Blink's `ToInt`: the whole px, truncated toward zero.
      pub(crate) const fn to_int(self) -> $storage {
        self.0 / Self::FIXED_POINT_DENOMINATOR
      }

      /// Blink's `ToFloat`.
      pub(crate) fn to_float(self) -> f32 {
        self.0 as f32 / Self::FIXED_POINT_DENOMINATOR as f32
      }

      /// Blink's `ToDouble`.
      pub(crate) fn to_double(self) -> f64 {
        self.0 as f64 / Self::FIXED_POINT_DENOMINATOR as f64
      }

      /// Blink's `RawValue`.
      pub(crate) const fn raw_value(self) -> $storage {
        self.0
      }

      /// Blink's `SetRawValue`.
      pub(crate) fn set_raw_value(&mut self, raw_value: $storage) {
        self.0 = raw_value;
      }

      /// Blink's `Abs`.
      pub(crate) const fn abs(self) -> Self {
        Self(self.0.saturating_abs())
      }

      /// Blink's `Ceil`: the whole px at or above.
      pub(crate) const fn ceil(self) -> $storage {
        if self.0 > Self::RAW_VALUE_MAX - Self::FIXED_POINT_DENOMINATOR {
          return Self::INT_MAX;
        }

        if self.0 >= 0 {
          return (self.0 + Self::FIXED_POINT_DENOMINATOR - 1) / Self::FIXED_POINT_DENOMINATOR;
        }

        self.to_int()
      }

      /// Blink's `Round`: the whole px nearest, from the truncated part and the fraction.
      pub(crate) const fn round(self) -> $storage {
        self.to_int()
          + ((self.fraction().0 + Self::FIXED_POINT_DENOMINATOR / 2) >> Self::FRACTIONAL_BITS)
      }

      /// Blink's `Floor`: the whole px at or below.
      pub(crate) const fn floor(self) -> $storage {
        if self.0 < Self::RAW_VALUE_MIN + Self::FIXED_POINT_DENOMINATOR {
          return Self::INT_MIN;
        }

        self.0 >> Self::FRACTIONAL_BITS
      }

      /// Blink's `ClampNegativeToZero`.
      pub(crate) const fn clamp_negative_to_zero(self) -> Self {
        if self.0 < 0 { Self::ZERO } else { self }
      }

      /// Blink's `ClampPositiveToZero`.
      pub(crate) const fn clamp_positive_to_zero(self) -> Self {
        if self.0 > 0 { Self::ZERO } else { self }
      }

      /// Blink's `HasFraction`.
      pub(crate) const fn has_fraction(self) -> bool {
        self.0 % Self::FIXED_POINT_DENOMINATOR != 0
      }

      /// Blink's `IsInteger`.
      pub(crate) const fn is_integer(self) -> bool {
        !self.has_fraction()
      }

      /// Blink's `Fraction`: the part below a whole px, keeping the sign.
      pub(crate) const fn fraction(self) -> Self {
        Self(self.0 % Self::FIXED_POINT_DENOMINATOR)
      }

      /// Blink's `MightBeSaturated`.
      pub(crate) const fn might_be_saturated(self) -> bool {
        self.0 == Self::RAW_VALUE_MAX || self.0 == Self::RAW_VALUE_MIN
      }

      /// Blink's `Epsilon`: the length of one step.
      pub(crate) fn epsilon() -> f32 {
        1.0 / Self::FIXED_POINT_DENOMINATOR as f32
      }

      /// Blink's `AddEpsilon`.
      pub(crate) const fn add_epsilon(self) -> Self {
        Self(if self.0 < Self::RAW_VALUE_MAX { self.0 + 1 } else { self.0 })
      }

      /// Blink's `MulDiv`: `self * m / d` as one operation, truncated toward zero.
      pub(crate) fn mul_div(self, m: Self, d: Self) -> Self {
        let n = <$wide>::from(self.0) * <$wide>::from(m.0);

        Self::from_raw_value_with_clamp(n / <$wide>::from(d.0))
      }

      /// Blink's `NullOptIf`.
      pub(crate) fn null_opt_if(self, null_value: Self) -> Option<Self> {
        (self != null_value).then_some(self)
      }
    }

    impl FixedPoint for $name {
      const FRACTIONAL_BITS: u32 = $fractional_bits;

      fn raw_value_wide(self) -> i128 {
        i128::from(self.0)
      }

      fn from_raw_value_with_clamp_wide(raw_value: i128) -> Self {
        Self(raw_value.clamp(i128::from(Self::RAW_VALUE_MIN), i128::from(Self::RAW_VALUE_MAX))
          as $storage)
      }

      fn max_value() -> Self {
        Self::MAX
      }

      fn min_value() -> Self {
        Self::MIN
      }
    }

    impl Add for $name {
      type Output = Self;

      fn add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
      }
    }

    impl AddAssign for $name {
      fn add_assign(&mut self, other: Self) {
        *self = *self + other;
      }
    }

    impl Sub for $name {
      type Output = Self;

      fn sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
      }
    }

    impl SubAssign for $name {
      fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
      }
    }

    impl Neg for $name {
      type Output = Self;

      fn neg(self) -> Self {
        Self(self.0.saturating_neg())
      }
    }

    impl Mul<i32> for $name {
      type Output = Self;

      fn mul(self, other: i32) -> Self {
        Self(self.0.saturating_mul(other.into()))
      }
    }

    impl Div<i32> for $name {
      type Output = Self;

      fn div(self, other: i32) -> Self {
        Self(self.0 / <$storage>::from(other))
      }
    }
  };
}

/// The operators Blink defines only for an `int32_t` storage.
macro_rules! fixed_point_int32_operators {
  ($name:ident) => {
    /// Blink's `BoundedMultiply`.
    impl Mul for $name {
      type Output = Self;

      fn mul(self, other: Self) -> Self {
        let result =
          i64::from(self.0) * i64::from(other.0) / i64::from(Self::FIXED_POINT_DENOMINATOR);
        let high = (result >> 32) as i32;
        let low = result as i32;
        let saturated = ((self.0 ^ other.0) as u32 >> 31).wrapping_add(Self::RAW_VALUE_MAX as u32);

        if high != low >> 31 {
          return Self(saturated as i32);
        }

        Self(result as i32)
      }
    }

    /// Division rounds toward zero, as Blink's does.
    impl Div for $name {
      type Output = Self;

      fn div(self, other: Self) -> Self {
        Self::from_raw_value_with_clamp(
          i64::from(Self::FIXED_POINT_DENOMINATOR) * i64::from(self.0) / i64::from(other.0),
        )
      }
    }

    impl Rem for $name {
      type Output = Self;

      fn rem(self, other: Self) -> Self {
        Self::from_raw_value_with_clamp(i64::from(self.0 % other.0))
      }
    }
  };
}

fixed_point!(
  /// A layout length in 1/64px.
  LayoutUnit, 6, i32, u32, i64
);
fixed_point!(
  /// A length within a run of text in 1/65536px, with 16 bits of whole px.
  TextRunLayoutUnit, 16, i32, u32, i64
);
fixed_point!(
  /// A length on a line in 1/65536px, with 48 bits of whole px.
  InlineLayoutUnit, 16, i64, u64, i128
);
fixed_point_int32_operators!(LayoutUnit);
fixed_point_int32_operators!(TextRunLayoutUnit);

/// Blink's lossless `FixedPoint(FixedPoint<source>)` constructor.
impl From<LayoutUnit> for InlineLayoutUnit {
  fn from(source: LayoutUnit) -> Self {
    Self(i64::from(source.0) << (Self::FRACTIONAL_BITS - LayoutUnit::FRACTIONAL_BITS))
  }
}

/// Blink's lossless `FixedPoint(FixedPoint<source>)` constructor.
impl From<TextRunLayoutUnit> for InlineLayoutUnit {
  fn from(source: TextRunLayoutUnit) -> Self {
    Self(i64::from(source.0))
  }
}

/// Blink's `operator*=(LayoutUnit&, float)`, truncating the product.
impl MulAssign<f32> for LayoutUnit {
  fn mul_assign(&mut self, other: f32) {
    *self = Self::from_float(self.to_float() * other);
  }
}

impl Mul<LayoutUnit> for i32 {
  type Output = LayoutUnit;

  fn mul(self, other: LayoutUnit) -> LayoutUnit {
    other * self
  }
}

impl LayoutUnit {
  /// Blink's `IntMod`: the remainder after a division with a whole result, so that
  /// `a = (a / b).to_int() * b + a.int_mod(b)`.
  pub(crate) const fn int_mod(self, divisor: Self) -> Self {
    Self(self.0 % divisor.0)
  }
}

/// Blink's `SnapSizeToPixel`: the whole px `size` covers from `location`, never snapping a size
/// of more than 4/64 px to zero.
pub(crate) fn snap_size_to_pixel(size: LayoutUnit, location: LayoutUnit) -> i32 {
  let result = snap_size_to_pixel_allowing_zero(size, location);

  if result == 0 && (size.0 > 4 || size.0 < -4) {
    return if size > LayoutUnit::ZERO { 1 } else { -1 };
  }

  result
}

/// Blink's `SnapSizeToPixelAllowingZero`.
pub(crate) fn snap_size_to_pixel_allowing_zero(size: LayoutUnit, location: LayoutUnit) -> i32 {
  let fraction = location.fraction();

  (fraction + size).round() - fraction.round()
}

/// Blink's `PhysicalBoxStrut`: a length on each side of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct BoxStrut {
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
  pub(crate) const ZERO: Self = Self {
    top: LayoutUnit::ZERO,
    right: LayoutUnit::ZERO,
    bottom: LayoutUnit::ZERO,
    left: LayoutUnit::ZERO,
  };

  pub(crate) fn horizontal_sum(self) -> LayoutUnit {
    self.left + self.right
  }

  pub(crate) fn vertical_sum(self) -> LayoutUnit {
    self.top + self.bottom
  }

  /// Whether every side is zero.
  pub fn is_zero(self) -> bool {
    self == Self::default()
  }

  /// The offset the top and left sides move a box's origin by.
  pub fn offset(self) -> UnitOffset {
    UnitOffset {
      left: self.left,
      top: self.top,
    }
  }
}

impl Neg for BoxStrut {
  type Output = Self;

  fn neg(self) -> Self {
    Self {
      top: -self.top,
      right: -self.right,
      bottom: -self.bottom,
      left: -self.left,
    }
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

/// Blink's `PhysicalOffset`: a point in layout units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct UnitOffset {
  /// The distance from the left.
  pub left: LayoutUnit,
  /// The distance from the top.
  pub top: LayoutUnit,
}

impl UnitOffset {
  /// The offset in px.
  pub(crate) fn to_point(self) -> Point<f32> {
    Point {
      x: self.left.to_float(),
      y: self.top.to_float(),
    }
  }
}

impl Add for UnitOffset {
  type Output = Self;

  fn add(self, other: Self) -> Self {
    Self {
      left: self.left + other.left,
      top: self.top + other.top,
    }
  }
}

impl Sub for UnitOffset {
  type Output = Self;

  fn sub(self, other: Self) -> Self {
    Self {
      left: self.left - other.left,
      top: self.top - other.top,
    }
  }
}

/// Blink's `PhysicalSize`: a size in layout units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct UnitSize {
  /// The width.
  pub width: LayoutUnit,
  /// The height.
  pub height: LayoutUnit,
}

impl UnitSize {
  /// Blink's `FromSizeFFloor`.
  pub(crate) fn from_size_floor(size: Size<f32>) -> Self {
    Self {
      width: LayoutUnit::from_float_floor(size.width),
      height: LayoutUnit::from_float_floor(size.height),
    }
  }

  /// Blink's `IsEmpty`: whether either side is zero.
  pub fn is_empty(self) -> bool {
    self.width == LayoutUnit::ZERO || self.height == LayoutUnit::ZERO
  }

  /// The size with a negative side set to zero.
  pub(crate) fn clamp_negative_to_zero(self) -> Self {
    Self {
      width: self.width.clamp_negative_to_zero(),
      height: self.height.clamp_negative_to_zero(),
    }
  }

  /// Blink's `FitToAspectRatio`: the size scaled on one side to `aspect_ratio`, growing to cover
  /// or shrinking to fit.
  pub(crate) fn fit_to_aspect_ratio(self, aspect_ratio: Self, grow: bool) -> Self {
    let constrained_height = self.width.mul_div(aspect_ratio.height, aspect_ratio.width);

    if (grow && constrained_height < self.height) || (!grow && constrained_height > self.height) {
      return Self {
        width: self.height.mul_div(aspect_ratio.width, aspect_ratio.height),
        height: self.height,
      };
    }

    Self {
      width: self.width,
      height: constrained_height,
    }
  }

  /// The size in px.
  pub(crate) fn to_size(self) -> Size<f32> {
    Size {
      width: self.width.to_float(),
      height: self.height.to_float(),
    }
  }
}

impl Add for UnitSize {
  type Output = Self;

  fn add(self, other: Self) -> Self {
    Self {
      width: self.width + other.width,
      height: self.height + other.height,
    }
  }
}

impl Sub for UnitSize {
  type Output = Self;

  fn sub(self, other: Self) -> Self {
    Self {
      width: self.width - other.width,
      height: self.height - other.height,
    }
  }
}

/// Blink's `PhysicalRect`: a rectangle in layout units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct UnitRect {
  /// The top-left corner.
  pub offset: UnitOffset,
  /// The size.
  pub size: UnitSize,
}

impl UnitRect {
  /// The rectangle of `size` px at `offset`, each in the nearest layout units.
  pub fn nearest(offset: Point<f32>, size: Size<f32>) -> Self {
    Self {
      offset: UnitOffset {
        left: LayoutUnit::from_float_round(offset.x),
        top: LayoutUnit::from_float_round(offset.y),
      },
      size: UnitSize {
        width: LayoutUnit::from_float_round(size.width),
        height: LayoutUnit::from_float_round(size.height),
      },
    }
  }

  /// The left edge.
  pub fn x(self) -> LayoutUnit {
    self.offset.left
  }

  /// The top edge.
  pub fn y(self) -> LayoutUnit {
    self.offset.top
  }

  /// The width.
  pub fn width(self) -> LayoutUnit {
    self.size.width
  }

  /// The height.
  pub fn height(self) -> LayoutUnit {
    self.size.height
  }

  /// The right edge.
  pub fn right(self) -> LayoutUnit {
    self.x() + self.width()
  }

  /// The bottom edge.
  pub fn bottom(self) -> LayoutUnit {
    self.y() + self.height()
  }

  /// Whether the rectangle encloses no area.
  pub fn is_empty(self) -> bool {
    self.size.is_empty()
  }

  /// Blink's `Expand`: the rectangle grown by `strut` on each side.
  pub fn expand(self, strut: BoxStrut) -> Self {
    Self {
      offset: UnitOffset {
        left: self.x() - strut.left,
        top: self.y() - strut.top,
      },
      size: UnitSize {
        width: self.width() + strut.left + strut.right,
        height: self.height() + strut.top + strut.bottom,
      },
    }
  }

  /// Blink's `Contract`: the rectangle shrunk by `strut` on each side.
  pub fn contract(self, strut: BoxStrut) -> Self {
    self.expand(-strut)
  }

  /// The rectangle with a negative width or height set to zero.
  pub(crate) fn clamp_negative_size_to_zero(self) -> Self {
    Self {
      offset: self.offset,
      size: self.size.clamp_negative_to_zero(),
    }
  }

  /// Blink's `ToPixelSnappedRect`, back in layout units, a negative size clamped to zero as
  /// `gfx::Rect` clamps it.
  pub(crate) fn pixel_snapped(self) -> Self {
    Self {
      offset: UnitOffset {
        left: LayoutUnit::from_int(self.x().round()),
        top: LayoutUnit::from_int(self.y().round()),
      },
      size: UnitSize {
        width: LayoutUnit::from_int(snap_size_to_pixel(self.width(), self.x()).max(0)),
        height: LayoutUnit::from_int(snap_size_to_pixel(self.height(), self.y()).max(0)),
      },
    }
  }

  /// Blink's `Intersect`: the overlap with `other`, or an empty rectangle at the origin.
  pub fn intersect(self, other: Self) -> Self {
    let left = self.x().max(other.x());
    let top = self.y().max(other.y());
    let right = self.right().min(other.right());
    let bottom = self.bottom().min(other.bottom());

    if left >= right || top >= bottom {
      return Self::default();
    }

    Self {
      offset: UnitOffset { left, top },
      size: UnitSize {
        width: right - left,
        height: bottom - top,
      },
    }
  }

  /// Whether `other` lies wholly inside.
  pub fn contains(self, other: Self) -> bool {
    self.x() <= other.x()
      && self.y() <= other.y()
      && self.right() >= other.right()
      && self.bottom() >= other.bottom()
  }

  /// The rectangle in px.
  pub(crate) fn to_rect(self) -> Rect<f32> {
    Rect {
      left: self.x().to_float(),
      top: self.y().to_float(),
      right: self.right().to_float(),
      bottom: self.bottom().to_float(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{
    FixedPoint, InlineLayoutUnit, LayoutUnit, TextRunLayoutUnit, UnitOffset, UnitRect, UnitSize,
  };

  #[test]
  fn floats_convert_as_blink_rounds_them() {
    assert_eq!(LayoutUnit::from_float(1.999).raw_value(), 127);
    assert_eq!(LayoutUnit::from_float(-1.999).raw_value(), -127);
    assert_eq!(LayoutUnit::from_float_floor(-1.999).raw_value(), -128);
    assert_eq!(LayoutUnit::from_float_ceil(1.001).raw_value(), 65);
    assert_eq!(LayoutUnit::from_float_round(0.5 / 64.0).raw_value(), 1);
    assert_eq!(LayoutUnit::from_float(f32::NAN), LayoutUnit::ZERO);
    assert_eq!(LayoutUnit::from_float(1e12).raw_value(), i32::MAX);
  }

  #[test]
  fn whole_pixels_round_as_blink_rounds_them() {
    let px = |raw| LayoutUnit::from_raw_value(raw);

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

    assert_eq!((px(7) / px(2)).raw_value(), 224);
    assert_eq!((px(-7) / px(2)).raw_value(), -224);
    assert_eq!((px(3) * px(2)), px(6));
    assert_eq!(px(7).int_mod(px(2)), px(1));
    assert_eq!(px(-7).int_mod(px(2)), px(-1));
    assert_eq!(LayoutUnit::from_int(i32::MAX).raw_value(), i32::MAX);
    assert_eq!(
      (LayoutUnit::from_raw_value(i32::MAX) + px(1)).raw_value(),
      i32::MAX
    );
    assert_eq!(
      (-LayoutUnit::from_raw_value(i32::MIN)).raw_value(),
      i32::MAX
    );
  }

  #[test]
  fn pixel_snapping_follows_the_location_fraction() {
    let rect = UnitRect {
      offset: UnitOffset {
        left: LayoutUnit::from_float(10.5),
        top: LayoutUnit::from_float(0.25),
      },
      size: UnitSize {
        width: LayoutUnit::from_float(20.25),
        height: LayoutUnit::from_float(0.1),
      },
    }
    .pixel_snapped();

    assert_eq!(rect.x(), LayoutUnit::from_int(11));
    assert_eq!(rect.width(), LayoutUnit::from_int(20));
    assert_eq!(rect.height(), LayoutUnit::from_int(1));
  }

  // The tests below follow `LayoutUnitTest` in Blink's `layout_unit_test.cc`.

  #[test]
  fn fixed_consts() {
    assert_eq!(LayoutUnit::FRACTIONAL_BITS, 6);
    assert_eq!(LayoutUnit::INTEGRAL_BITS, 26);
    assert_eq!(TextRunLayoutUnit::FRACTIONAL_BITS, 16);
    assert_eq!(TextRunLayoutUnit::INTEGRAL_BITS, 16);
    assert_eq!(InlineLayoutUnit::FRACTIONAL_BITS, 16);
    assert_eq!(InlineLayoutUnit::INTEGRAL_BITS, 48);
  }

  #[test]
  fn fixed() {
    let raw_value16 = 0x1234_5678;
    let raw_value6 = raw_value16 >> 10;
    let value16 = TextRunLayoutUnit::from_raw_value(raw_value16);
    let value6 = LayoutUnit::from_raw_value(raw_value6);

    assert_eq!(value16.to::<LayoutUnit>(), value6);
  }

  #[test]
  fn raw64_from_int32() {
    let int32_max_plus = LayoutUnit::INT_MAX + 10;

    assert_ne!(
      LayoutUnit::from_int(int32_max_plus).to_int(),
      int32_max_plus
    );
    assert_eq!(
      InlineLayoutUnit::from_int(int32_max_plus).to_int(),
      i64::from(int32_max_plus)
    );

    let int32_min_minus = LayoutUnit::INT_MIN - 10;

    assert_ne!(
      LayoutUnit::from_int(int32_min_minus).to_int(),
      int32_min_minus
    );
    assert_eq!(
      InlineLayoutUnit::from_int(int32_min_minus).to_int(),
      i64::from(int32_min_minus)
    );

    let raw32_max_plus = i64::from(LayoutUnit::RAW_VALUE_MAX) + 10;

    assert_ne!(
      i64::from(LayoutUnit::from_int64(raw32_max_plus).to_int()),
      raw32_max_plus
    );
    assert_eq!(
      InlineLayoutUnit::from_int64(raw32_max_plus).to_int(),
      raw32_max_plus
    );

    let raw32_min_minus = i64::from(LayoutUnit::RAW_VALUE_MIN) - 10;

    assert_ne!(
      i64::from(LayoutUnit::from_int64(raw32_min_minus).to_int()),
      raw32_min_minus
    );
    assert_eq!(
      InlineLayoutUnit::from_int64(raw32_min_minus).to_int(),
      raw32_min_minus
    );
  }

  #[test]
  fn raw64_from_raw32() {
    let value = 1.0 + LayoutUnit::epsilon() * 234.0;

    assert_eq!(
      InlineLayoutUnit::from(LayoutUnit::from_float(value)),
      InlineLayoutUnit::from_float(value)
    );
    assert_eq!(
      InlineLayoutUnit::from(TextRunLayoutUnit::from_float(value)),
      InlineLayoutUnit::from_float(value)
    );
  }

  #[test]
  fn to() {
    for value in [1.0, 1.5, -1.0] {
      assert_eq!(
        LayoutUnit::from_float(value),
        TextRunLayoutUnit::from_float(value).to::<LayoutUnit>()
      );
      assert_eq!(
        TextRunLayoutUnit::from_float(value),
        LayoutUnit::from_float(value).to::<TextRunLayoutUnit>()
      );
      assert_eq!(
        LayoutUnit::from_float(value),
        InlineLayoutUnit::from_float(value).to::<LayoutUnit>()
      );
      assert_eq!(
        InlineLayoutUnit::from_float(value),
        LayoutUnit::from_float(value).to::<InlineLayoutUnit>()
      );
      assert_eq!(
        TextRunLayoutUnit::from_float(value),
        InlineLayoutUnit::from_float(value).to::<TextRunLayoutUnit>()
      );
      assert_eq!(
        InlineLayoutUnit::from_float(value),
        TextRunLayoutUnit::from_float(value).to::<InlineLayoutUnit>()
      );
    }
  }

  #[test]
  fn to_clamp_same_fractional64_to32() {
    assert_eq!(
      TextRunLayoutUnit::MAX,
      InlineLayoutUnit::from_int(TextRunLayoutUnit::INT_MAX + 1).to::<TextRunLayoutUnit>()
    );
    assert_eq!(
      TextRunLayoutUnit::MIN,
      InlineLayoutUnit::from_int(TextRunLayoutUnit::INT_MIN - 1).to::<TextRunLayoutUnit>()
    );
  }

  #[test]
  fn to_clamp_less_fractional64_to32() {
    assert_eq!(
      LayoutUnit::MAX,
      InlineLayoutUnit::from_int(LayoutUnit::INT_MAX + 1).to::<LayoutUnit>()
    );
    assert_eq!(
      LayoutUnit::MIN,
      InlineLayoutUnit::from_int(LayoutUnit::INT_MIN - 1).to::<LayoutUnit>()
    );
  }

  #[test]
  fn to_clamp_more_fractional() {
    assert_eq!(
      TextRunLayoutUnit::MAX,
      LayoutUnit::from_int(TextRunLayoutUnit::INT_MAX + 1).to::<TextRunLayoutUnit>()
    );
    assert_eq!(
      TextRunLayoutUnit::MIN,
      LayoutUnit::from_int(TextRunLayoutUnit::INT_MIN - 1).to::<TextRunLayoutUnit>()
    );
  }

  #[test]
  fn raw64_ceil() {
    let layout = LayoutUnit::from_double(1.234);
    let inline_value = InlineLayoutUnit::from(layout);

    assert_eq!(layout, inline_value.to_ceil::<LayoutUnit>());

    let inline_value = inline_value.add_epsilon();

    assert_ne!(layout, inline_value.to_ceil::<LayoutUnit>());
    assert_eq!(layout.add_epsilon(), inline_value.to_ceil::<LayoutUnit>());
  }

  #[test]
  fn multiplication_saturates_by_sign() {
    let max = LayoutUnit::from_int(LayoutUnit::INT_MAX);

    assert_eq!(max * LayoutUnit::from_int(2), LayoutUnit::MAX);
    assert_eq!(max * LayoutUnit::from_int(-2), LayoutUnit::MIN);
  }
}
