use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::ops::Add;

use rustc_hash::FxHashMap;

/// A structure that allows to remap numeric types to new
/// numbers so that they form a contiguous sequence of numbers.
#[derive(Debug, Clone)]
pub struct Remapper<C, T> {
  /// The counter that keeps track of the next number to be assigned.
  /// Should always start with 0.
  counter: C,
  /// The map that maps numbers from their old value to their new value.
  forward: FxHashMap<T, T>,
  /// The vector that stores the "reverse" mapping, i.e. given a new number,
  /// it allows to map back to the old one.
  backward: Vec<T>,
}

impl<C, T: Hash> Hash for Remapper<C, T> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.backward.hash(state);
  }
}

impl<C, T: PartialEq + Hash> PartialEq for Remapper<C, T> {
  fn eq(&self, other: &Self) -> bool {
    self.backward == other.backward
  }
}

impl<C, T: PartialEq + Hash> Eq for Remapper<C, T> {}

/// A wrapper trait around `checked_add` so we can require it for the remapper.
pub trait CheckedAdd: Sized + Add<Self, Output = Self> {
  fn checked_add(&self, v: &Self) -> Option<Self>;
}

impl CheckedAdd for u8 {
  fn checked_add(&self, v: &Self) -> Option<Self> {
    u8::checked_add(*self, *v)
  }
}

impl CheckedAdd for u16 {
  fn checked_add(&self, v: &Self) -> Option<Self> {
    u16::checked_add(*self, *v)
  }
}

impl CheckedAdd for u32 {
  fn checked_add(&self, v: &Self) -> Option<Self> {
    u32::checked_add(*self, *v)
  }
}

impl<C, T> Remapper<C, T>
where
  C: CheckedAdd + Copy + From<u8>,
  T: From<C> + Copy + From<u8> + Eq + Hash,
{
  /// Create a new instance of a remapper.
  pub fn new() -> Self
  where
    C: Default,
  {
    Self {
      counter: C::default(),
      forward: FxHashMap::default(),
      backward: Vec::new(),
    }
  }

  /// Get the new mapping of a value that has been remapped before.
  /// Returns `None` if it has not been remapped.
  pub fn get(&self, old: T) -> Option<T> {
    self.forward.get(&old).copied()
  }

  /// Remap a new value, either returning the previously assigned number
  /// if it already has been remapped, and assigning a new number if it
  /// has not been remapped.
  pub fn remap(&mut self, old: T) -> T {
    *self.forward.entry(old).or_insert_with(|| {
      let value = self.counter;
      self.backward.push(old);
      self.counter = self
        .counter
        .checked_add(&C::from(1))
        .expect("remapper was overflowed");
      value.into()
    })
  }

  /// Get the number of elements that have been remapped. Assumes that
  /// the remapper was constructed with a type where `C::default` yields 0.
  pub fn len(&self) -> C {
    self.counter
  }

  /// Returns an iterator over the old values, in ascending order that is defined
  /// by the remapping.
  pub fn sorted_iter(&self) -> impl Iterator<Item = T> + '_ {
    self.backward.iter().copied()
  }
}

/// A remapper that allows to assign a new ordering to a subset of glyphs.
///
/// For example, let's say that we want to subset a font that only contains the
/// glyphs 4, 9 and 16. In this case, the remapper could yield a remapping
/// that assigns the following glyph IDs:
/// - 0 -> 0 (The .notdef glyph will always be included)
/// - 4 -> 1
/// - 9 -> 2
/// - 16 -> 3
///
/// This is necessary because a font needs to have a contiguous sequence of
/// glyph IDs that start from 0, so we cannot just reuse the old ones, but we
/// need to define a mapping.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct GlyphRemapper(Remapper<u16, u16>);

impl Default for GlyphRemapper {
  fn default() -> Self {
    let mut remapper = Remapper::new();
    // .notdef is always a part of a subset.
    remapper.remap(0);
    Self(remapper)
  }
}

impl GlyphRemapper {
  /// Create a new instance of a glyph remapper. `.notdef` will always be a member
  /// of the subset.
  pub fn new() -> Self {
    Self::default()
  }

  /// Create a remapper from an existing set of glyphs
  pub fn new_from_glyphs(glyphs: &[u16]) -> Self {
    let mut map = Self::new();

    for glyph in glyphs {
      map.remap(*glyph);
    }

    map
  }

  /// Create a remapper from an existing set of glyphs. The method
  /// will ensure that the mapping is monotonically increasing.
  pub fn new_from_glyphs_sorted(glyphs: &[u16]) -> Self {
    let sorted = BTreeSet::from_iter(glyphs)
      .iter()
      .map(|g| **g)
      .collect::<Vec<_>>();

    GlyphRemapper::new_from_glyphs(&sorted)
  }

  /// Get the number of gids that have been remapped.
  pub fn num_gids(&self) -> u16 {
    self.0.len()
  }

  /// Remap a glyph ID, or return the existing mapping if the
  /// glyph ID has already been remapped before.
  #[inline]
  pub fn remap(&mut self, old: u16) -> u16 {
    self.0.remap(old)
  }

  /// Get the mapping of a glyph ID, if it has been remapped before.
  #[inline]
  pub fn get(&self, old: u16) -> Option<u16> {
    self.0.get(old)
  }

  /// Return an iterator that yields the old glyphs, in ascending order that
  /// is defined by the remapping. For example, if we perform the following remappings:
  /// 3, 39, 8, 3, 10, 2
  ///
  /// Then the iterator will yield the following items in the order below. The order
  /// also implicitly defines the glyph IDs in the new mapping:
  ///
  /// 0 (0), 3 (1), 39 (2), 8 (3), 10 (4), 2 (5)
  pub fn remapped_gids(&self) -> impl Iterator<Item = u16> + '_ {
    self.0.backward.iter().copied()
  }
}
