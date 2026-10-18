//! Blink's [`font_selection_types.h`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_selection_types.h).

use std::{
  fmt::{self, Display, Formatter},
  hash::{Hash, Hasher},
  ops::{Add, Neg, Sub},
};

/// Blink's `FontSelectionValue`: a signed fixed-point number with two fractional bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct FontSelectionValue {
  backing: i16,
}

impl FontSelectionValue {
  /// Blink's `kFractionalEntropy`.
  const FRACTIONAL_ENTROPY: f32 = 4.0;

  /// `value` rounded toward zero to a quarter, saturating at the representable range.
  pub(crate) const fn new(value: f32) -> Self {
    Self {
      backing: (value * Self::FRACTIONAL_ENTROPY) as i16,
    }
  }

  const fn from_raw(backing: i16) -> Self {
    Self { backing }
  }

  /// Blink's `operator float`.
  pub(crate) fn to_f32(self) -> f32 {
    f32::from(self.backing) / Self::FRACTIONAL_ENTROPY
  }

  /// Blink's `RawValue`.
  pub(crate) fn raw_value(self) -> i16 {
    self.backing
  }

  /// Blink's `MaximumValue`.
  pub(crate) const fn maximum_value() -> Self {
    Self::from_raw(i16::MAX)
  }

  /// Blink's `MinimumValue`.
  pub(crate) const fn minimum_value() -> Self {
    Self::from_raw(i16::MIN)
  }

  /// Blink's `ClampToObliqueRange`.
  pub(crate) fn clamp_to_oblique_range(self) -> Self {
    FontSelectionRange::new(MIN_OBLIQUE_VALUE, MAX_OBLIQUE_VALUE).clamp_to_range(self)
  }
}

// Blink narrows the `int` sum back to `int16_t`, which wraps.
impl Add for FontSelectionValue {
  type Output = Self;

  fn add(self, other: Self) -> Self {
    Self::from_raw(self.backing.wrapping_add(other.backing))
  }
}

impl Sub for FontSelectionValue {
  type Output = Self;

  fn sub(self, other: Self) -> Self {
    Self::from_raw(self.backing.wrapping_sub(other.backing))
  }
}

impl Neg for FontSelectionValue {
  type Output = Self;

  fn neg(self) -> Self {
    Self::from_raw(self.backing.wrapping_neg())
  }
}

/// Blink's `FontSelectionValue::ToString`.
impl Display for FontSelectionValue {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{:.6}", self.to_f32())
  }
}

/// Blink's `kItalicThreshold`.
pub(crate) const ITALIC_THRESHOLD: FontSelectionValue = FontSelectionValue::new(14.0);

/// Blink's `IsItalic`.
pub(crate) fn is_italic(font_style: FontSelectionValue) -> bool {
  font_style >= ITALIC_THRESHOLD
}

/// Blink's `kFontSelectionZeroValue`.
pub(crate) const FONT_SELECTION_ZERO_VALUE: FontSelectionValue = FontSelectionValue::new(0.0);
/// Blink's `kNormalSlopeValue`.
pub(crate) const NORMAL_SLOPE_VALUE: FontSelectionValue = FontSelectionValue::new(0.0);
/// Blink's `kItalicSlopeValue`.
pub(crate) const ITALIC_SLOPE_VALUE: FontSelectionValue = FontSelectionValue::new(14.0);
/// Blink's `kMaxObliqueValue`.
pub(crate) const MAX_OBLIQUE_VALUE: FontSelectionValue = FontSelectionValue::new(90.0);
/// Blink's `kMinObliqueValue`.
pub(crate) const MIN_OBLIQUE_VALUE: FontSelectionValue = FontSelectionValue::new(-90.0);

/// Blink's `kBoldThreshold`.
pub(crate) const BOLD_THRESHOLD: FontSelectionValue = FontSelectionValue::new(600.0);
/// Blink's `kMinWeightValue`.
pub(crate) const MIN_WEIGHT_VALUE: FontSelectionValue = FontSelectionValue::new(1.0);
/// Blink's `kMaxWeightValue`.
pub(crate) const MAX_WEIGHT_VALUE: FontSelectionValue = FontSelectionValue::new(1000.0);
/// Blink's `kBlackWeightValue`.
pub(crate) const BLACK_WEIGHT_VALUE: FontSelectionValue = FontSelectionValue::new(900.0);
/// Blink's `kExtraBoldWeightValue`.
pub(crate) const EXTRA_BOLD_WEIGHT_VALUE: FontSelectionValue = FontSelectionValue::new(800.0);
/// Blink's `kBoldWeightValue`.
pub(crate) const BOLD_WEIGHT_VALUE: FontSelectionValue = FontSelectionValue::new(700.0);
/// Blink's `kSemiBoldWeightValue`.
pub(crate) const SEMI_BOLD_WEIGHT_VALUE: FontSelectionValue = FontSelectionValue::new(600.0);
/// Blink's `kMediumWeightValue`.
pub(crate) const MEDIUM_WEIGHT_VALUE: FontSelectionValue = FontSelectionValue::new(500.0);
/// Blink's `kNormalWeightValue`.
pub(crate) const NORMAL_WEIGHT_VALUE: FontSelectionValue = FontSelectionValue::new(400.0);
/// Blink's `kLightWeightValue`.
pub(crate) const LIGHT_WEIGHT_VALUE: FontSelectionValue = FontSelectionValue::new(300.0);
/// Blink's `kExtraLightWeightValue`.
pub(crate) const EXTRA_LIGHT_WEIGHT_VALUE: FontSelectionValue = FontSelectionValue::new(200.0);
/// Blink's `kThinWeightValue`.
pub(crate) const THIN_WEIGHT_VALUE: FontSelectionValue = FontSelectionValue::new(100.0);

/// Blink's `IsFontWeightBold`.
pub(crate) fn is_font_weight_bold(font_weight: FontSelectionValue) -> bool {
  font_weight >= BOLD_THRESHOLD
}

/// Blink's `kUpperWeightSearchThreshold`.
pub(crate) const UPPER_WEIGHT_SEARCH_THRESHOLD: FontSelectionValue = FontSelectionValue::new(500.0);
/// Blink's `kLowerWeightSearchThreshold`.
pub(crate) const LOWER_WEIGHT_SEARCH_THRESHOLD: FontSelectionValue = FontSelectionValue::new(400.0);

/// Blink's `kUltraCondensedWidthValue`.
pub(crate) const ULTRA_CONDENSED_WIDTH_VALUE: FontSelectionValue = FontSelectionValue::new(50.0);
/// Blink's `kExtraCondensedWidthValue`.
pub(crate) const EXTRA_CONDENSED_WIDTH_VALUE: FontSelectionValue = FontSelectionValue::new(62.5);
/// Blink's `kCondensedWidthValue`.
pub(crate) const CONDENSED_WIDTH_VALUE: FontSelectionValue = FontSelectionValue::new(75.0);
/// Blink's `kSemiCondensedWidthValue`.
pub(crate) const SEMI_CONDENSED_WIDTH_VALUE: FontSelectionValue = FontSelectionValue::new(87.5);
/// Blink's `kNormalWidthValue`.
pub(crate) const NORMAL_WIDTH_VALUE: FontSelectionValue = FontSelectionValue::new(100.0);
/// Blink's `kSemiExpandedWidthValue`.
pub(crate) const SEMI_EXPANDED_WIDTH_VALUE: FontSelectionValue = FontSelectionValue::new(112.5);
/// Blink's `kExpandedWidthValue`.
pub(crate) const EXPANDED_WIDTH_VALUE: FontSelectionValue = FontSelectionValue::new(125.0);
/// Blink's `kExtraExpandedWidthValue`.
pub(crate) const EXTRA_EXPANDED_WIDTH_VALUE: FontSelectionValue = FontSelectionValue::new(150.0);
/// Blink's `kUltraExpandedWidthValue`.
pub(crate) const ULTRA_EXPANDED_WIDTH_VALUE: FontSelectionValue = FontSelectionValue::new(200.0);

/// Blink's `FontSelectionRange::RangeType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RangeType {
  #[default]
  SetFromAuto,
  SetExplicitly,
}

/// Blink's `FontSelectionRange`; equality and hashing ignore `range_type`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FontSelectionRange {
  pub(crate) minimum: FontSelectionValue,
  pub(crate) maximum: FontSelectionValue,
  pub(crate) range_type: RangeType,
}

impl Default for FontSelectionRange {
  fn default() -> Self {
    Self::new(FontSelectionValue::new(1.0), FontSelectionValue::new(0.0))
  }
}

impl PartialEq for FontSelectionRange {
  fn eq(&self, other: &Self) -> bool {
    self.minimum == other.minimum && self.maximum == other.maximum
  }
}

impl Eq for FontSelectionRange {}

impl Hash for FontSelectionRange {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.unique_value().hash(state);
  }
}

impl FontSelectionRange {
  pub(crate) const fn new(minimum: FontSelectionValue, maximum: FontSelectionValue) -> Self {
    Self::with_type(minimum, maximum, RangeType::SetFromAuto)
  }

  /// Blink's single-value constructor.
  pub(crate) const fn single(value: FontSelectionValue) -> Self {
    Self::new(value, value)
  }

  pub(crate) const fn with_type(
    minimum: FontSelectionValue,
    maximum: FontSelectionValue,
    range_type: RangeType,
  ) -> Self {
    Self {
      minimum,
      maximum,
      range_type,
    }
  }

  /// Blink's `IsValid`.
  pub(crate) fn is_valid(&self) -> bool {
    self.minimum <= self.maximum
  }

  /// Blink's `IsRange`.
  pub(crate) fn is_range(&self) -> bool {
    self.maximum > self.minimum
  }

  /// Blink's `IsRangeSetFromAuto`.
  pub(crate) fn is_range_set_from_auto(&self) -> bool {
    self.range_type == RangeType::SetFromAuto
  }

  /// Blink's `Expand`.
  pub(crate) fn expand(&mut self, other: &Self) {
    debug_assert!(other.is_valid());

    if !self.is_valid() {
      *self = *other;
    } else {
      self.minimum = self.minimum.min(other.minimum);
      self.maximum = self.maximum.max(other.maximum);
    }

    debug_assert!(self.is_valid());
  }

  /// Blink's `Includes`.
  pub(crate) fn includes(&self, target: FontSelectionValue) -> bool {
    target >= self.minimum && target <= self.maximum
  }

  /// Blink's `UniqueValue`.
  pub(crate) fn unique_value(&self) -> u32 {
    (u32::from(self.minimum.raw_value() as u16) << 16) | u32::from(self.maximum.raw_value() as u16)
  }

  /// Blink's `ClampToRange`, as `std::clamp` orders its comparisons.
  pub(crate) fn clamp_to_range(&self, selection_value: FontSelectionValue) -> FontSelectionValue {
    if selection_value < self.minimum {
      self.minimum
    } else if self.maximum < selection_value {
      self.maximum
    } else {
      selection_value
    }
  }
}

/// Blink's `FontSelectionRequest`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct FontSelectionRequest {
  pub(crate) weight: FontSelectionValue,
  pub(crate) width: FontSelectionValue,
  pub(crate) slope: FontSelectionValue,
}

impl FontSelectionRequest {
  pub(crate) fn new(
    weight: FontSelectionValue,
    width: FontSelectionValue,
    slope: FontSelectionValue,
  ) -> Self {
    Self {
      weight,
      width,
      slope,
    }
  }
}

/// Blink's `FontSelectionRequest::ToString`.
impl Display for FontSelectionRequest {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "weight={}, width={}, slope={}",
      self.weight, self.width, self.slope
    )
  }
}

/// Blink's `FontSelectionCapabilities`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FontSelectionCapabilities {
  pub(crate) width: FontSelectionRange,
  pub(crate) slope: FontSelectionRange,
  pub(crate) weight: FontSelectionRange,
}

impl Default for FontSelectionCapabilities {
  fn default() -> Self {
    let zero = FontSelectionRange::single(FONT_SELECTION_ZERO_VALUE);

    Self::new(zero, zero, zero)
  }
}

impl FontSelectionCapabilities {
  pub(crate) fn new(
    width: FontSelectionRange,
    slope: FontSelectionRange,
    weight: FontSelectionRange,
  ) -> Self {
    Self {
      width,
      slope,
      weight,
    }
  }

  /// Blink's `Expand`.
  pub(crate) fn expand(&mut self, capabilities: &Self) {
    self.width.expand(&capabilities.width);
    self.slope.expand(&capabilities.slope);
    self.weight.expand(&capabilities.weight);
  }

  /// Blink's `IsValid`.
  pub(crate) fn is_valid(&self) -> bool {
    self.width.is_valid() && self.slope.is_valid() && self.weight.is_valid()
  }

  /// Blink's `HasRange`.
  pub(crate) fn has_range(&self) -> bool {
    self.width.is_range() || self.slope.is_range() || self.weight.is_range()
  }
}

#[cfg(test)]
mod tests {
  use std::{
    collections::HashSet,
    hash::{BuildHasher, RandomState},
  };

  use super::*;

  #[test]
  fn hash_collisions() {
    let weights = [
      100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0,
    ];
    let slopes = [
      -90.0, -67.5, -30.0, -20.0, -10.0, 0.0, 10.0, 20.0, 30.0, 67.5, 90.0,
    ];
    let widths = [50.0, 67.5, 75.0, 100.0, 125.0, 150.0, 167.5, 175.0, 200.0];
    let state = RandomState::new();
    let mut hashes = HashSet::new();

    for weight in weights {
      for slope in slopes {
        for width in widths {
          let request = FontSelectionRequest::new(
            FontSelectionValue::new(weight),
            FontSelectionValue::new(width),
            FontSelectionValue::new(slope),
          );

          assert!(hashes.insert(state.hash_one(request)));
        }
      }
    }

    assert_eq!(hashes.len(), weights.len() * slopes.len() * widths.len());
  }

  #[test]
  fn value_to_string() {
    assert_eq!(FontSelectionValue::new(42.0).to_string(), "42.000000");
    assert_eq!(FontSelectionValue::new(42.81).to_string(), "42.750000");
    assert_eq!(FontSelectionValue::new(42.923_46).to_string(), "42.750000");
  }

  #[test]
  fn request_to_string() {
    let request = FontSelectionRequest::new(
      FontSelectionValue::new(42.0),
      FontSelectionValue::new(43.0),
      FontSelectionValue::new(44.0),
    );

    assert_eq!(
      request.to_string(),
      "weight=42.000000, width=43.000000, slope=44.000000"
    );
  }

  #[test]
  fn values_saturate_and_truncate_toward_zero() {
    assert_eq!(
      FontSelectionValue::new(1e9),
      FontSelectionValue::maximum_value()
    );
    assert_eq!(
      FontSelectionValue::new(-1e9),
      FontSelectionValue::minimum_value()
    );
    assert_eq!(FontSelectionValue::new(-0.3).raw_value(), -1);
    assert_eq!(FontSelectionValue::new(f32::NAN).raw_value(), 0);
  }

  #[test]
  fn range_equality_ignores_how_it_was_set() {
    let auto = FontSelectionRange::new(THIN_WEIGHT_VALUE, BLACK_WEIGHT_VALUE);
    let explicit = FontSelectionRange::with_type(
      THIN_WEIGHT_VALUE,
      BLACK_WEIGHT_VALUE,
      RangeType::SetExplicitly,
    );

    assert_eq!(auto, explicit);
    assert!(auto.is_range_set_from_auto());
    assert!(!explicit.is_range_set_from_auto());
  }

  #[test]
  fn an_invalid_range_takes_the_first_expansion() {
    let mut range = FontSelectionRange::default();

    assert!(!range.is_valid());
    range.expand(&FontSelectionRange::single(BOLD_WEIGHT_VALUE));
    assert_eq!(range, FontSelectionRange::single(BOLD_WEIGHT_VALUE));
    range.expand(&FontSelectionRange::single(LIGHT_WEIGHT_VALUE));
    assert_eq!(
      range,
      FontSelectionRange::new(LIGHT_WEIGHT_VALUE, BOLD_WEIGHT_VALUE)
    );
  }
}
