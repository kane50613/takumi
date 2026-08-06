//! Numerical types.

/// An immutable, finite `f32` in a 0..=1 range.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[repr(transparent)]
pub struct NormalizedF32(tiny_skia_path::NormalizedF32);

impl NormalizedF32 {
  /// A `NormalizedF32` value initialized with zero.
  pub const ZERO: Self = NormalizedF32(tiny_skia_path::NormalizedF32::ZERO);
  /// A `NormalizedF32` value initialized with one.
  pub const ONE: Self = NormalizedF32(tiny_skia_path::NormalizedF32::ONE);

  /// Create a new normalized f32.
  ///
  /// Panics if the number is not normalized.
  pub fn new(num: f32) -> Option<Self> {
    Some(Self(tiny_skia_path::NormalizedF32::new(num)?))
  }

  /// Returns the value as a primitive type.
  #[inline]
  pub const fn get(self) -> f32 {
    self.0.get()
  }
}
