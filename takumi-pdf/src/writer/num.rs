//! Numerical types.

use std::{
  cmp::Ordering,
  hash::{Hash, Hasher},
};

/// An immutable, finite `f32` in a 0..=1 range.
#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(transparent)]
pub struct NormalizedF32(f32);

impl NormalizedF32 {
  /// A `NormalizedF32` value initialized with zero.
  pub const ZERO: Self = NormalizedF32(0.0);
  /// A `NormalizedF32` value initialized with one.
  pub const ONE: Self = NormalizedF32(1.0);

  /// The number, none where it is not finite or not within 0 to 1.
  pub fn new(num: f32) -> Option<Self> {
    (0.0..=1.0).contains(&num).then_some(Self(num))
  }

  /// Returns the value as a primitive type.
  #[inline]
  pub const fn get(self) -> f32 {
    self.0
  }
}

/// An immutable, finite `f32`.
#[derive(Copy, Clone, Default, PartialEq, Debug)]
#[repr(transparent)]
pub struct FiniteF32(f32);

impl FiniteF32 {
  /// The number, none where it is not finite.
  pub fn new(num: f32) -> Option<Self> {
    num.is_finite().then_some(Self(num))
  }

  /// Returns the value as a primitive type.
  #[inline]
  pub const fn get(self) -> f32 {
    self.0
  }
}

macro_rules! finite_hash {
  ($name:ident) => {
    impl Eq for $name {}

    impl Hash for $name {
      fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
      }
    }
  };
}

finite_hash!(NormalizedF32);
finite_hash!(FiniteF32);

impl Ord for NormalizedF32 {
  fn cmp(&self, other: &Self) -> Ordering {
    // Finite floats always compare.
    self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
  }
}

impl PartialOrd for NormalizedF32 {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}
