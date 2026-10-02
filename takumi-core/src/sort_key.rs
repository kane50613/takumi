//! Stable sorting by keys packed into one integer, so every call site shares a single
//! instantiation of the standard library's sort and pays only for its key and a permutation.

/// A sort key that packs into at most 128 bits, in the order the key sorts.
pub(crate) trait SortKey: Copy {
  /// The bits the key takes.
  const BITS: u32;

  /// The key as an integer that orders as the key does.
  fn packed(self) -> u128;
}

impl SortKey for bool {
  const BITS: u32 = 1;

  /// Encodes a boolean as `0` for `false` and `1` for `true`.
  fn packed(self) -> u128 {
    u128::from(self)
  }
}

impl SortKey for u8 {
  const BITS: u32 = 8;

  /// Zero-extends an 8-bit unsigned integer to a 128-bit key.
  fn packed(self) -> u128 {
    u128::from(self)
  }
}

impl SortKey for u16 {
  const BITS: u32 = 16;

  /// Zero-extends a 16-bit unsigned integer to a 128-bit key.
  fn packed(self) -> u128 {
    u128::from(self)
  }
}

impl SortKey for u32 {
  const BITS: u32 = 32;

  /// Zero-extends a 32-bit unsigned integer to a 128-bit key.
  fn packed(self) -> u128 {
    u128::from(self)
  }
}

impl SortKey for u64 {
  const BITS: u32 = 64;

  /// Zero-extends a 64-bit unsigned integer to a 128-bit key.
  fn packed(self) -> u128 {
    u128::from(self)
  }
}

impl SortKey for i8 {
  const BITS: u32 = 8;

  /// Maps an 8-bit signed integer to an unsigned sort key preserving numeric order.
  fn packed(self) -> u128 {
    u128::from(self.cast_unsigned() ^ (1 << 7))
  }
}

impl SortKey for i16 {
  const BITS: u32 = 16;

  /// Maps a 16-bit signed integer to an unsigned sort key preserving numeric order.
  fn packed(self) -> u128 {
    u128::from(self.cast_unsigned() ^ (1 << 15))
  }
}

impl SortKey for i32 {
  const BITS: u32 = 32;

  /// Maps a 32-bit signed integer to an unsigned sort key preserving numeric order.
  fn packed(self) -> u128 {
    u128::from(self.cast_unsigned() ^ (1 << 31))
  }
}

impl SortKey for i64 {
  const BITS: u32 = 64;

  /// Maps a 64-bit signed integer to an unsigned sort key preserving numeric order.
  fn packed(self) -> u128 {
    u128::from(self.cast_unsigned() ^ (1 << 63))
  }
}

/// Saturates past `u32::MAX`, a count no index or source order reaches.
impl SortKey for usize {
  const BITS: u32 = 32;

  /// Saturates `usize` past `u32::MAX` to pack within 32 bits.
  fn packed(self) -> u128 {
    u128::from(u32::try_from(self).unwrap_or(u32::MAX))
  }
}

/// Orders as [`f32::total_cmp`] does.
impl SortKey for f32 {
  const BITS: u32 = 32;

  /// Encodes IEEE-754 floats into unsigned integers ordering identically to [`f32::total_cmp`].
  fn packed(self) -> u128 {
    let bits = self.to_bits();

    u128::from(if bits >> 31 == 1 {
      !bits
    } else {
      bits | (1 << 31)
    })
  }
}

impl<A: SortKey, B: SortKey> SortKey for (A, B) {
  const BITS: u32 = A::BITS + B::BITS;

  /// Packs a 2-tuple of keys side by side into a single integer.
  fn packed(self) -> u128 {
    (self.0.packed() << B::BITS) | self.1.packed()
  }
}

impl<A: SortKey, B: SortKey, C: SortKey> SortKey for (A, B, C) {
  const BITS: u32 = A::BITS + <(B, C)>::BITS;

  /// Packs a 3-tuple of keys side by side into a single integer.
  fn packed(self) -> u128 {
    (self.0, (self.1, self.2)).packed()
  }
}

impl<A: SortKey, B: SortKey, C: SortKey, D: SortKey> SortKey for (A, B, C, D) {
  const BITS: u32 = A::BITS + <(B, C, D)>::BITS;

  /// Packs a 4-tuple of keys side by side into a single integer.
  fn packed(self) -> u128 {
    (self.0, (self.1, self.2, self.3)).packed()
  }
}

/// Stable-sorts `items` by `key`.
pub(crate) fn sort_by_key<T, K: SortKey>(items: &mut [T], mut key: impl FnMut(&T) -> K) {
  const { assert!(K::BITS <= 128) };

  if items.len() < 2 {
    return;
  }

  if items.len() == 2 {
    if key(&items[0]).packed() > key(&items[1]).packed() {
      items.swap(0, 1);
    }
    return;
  }

  let mut keyed: Vec<(u128, u32)> = items
    .iter()
    .zip(0..)
    .map(|(item, index)| (key(item).packed(), index))
    .collect();

  keyed.sort_unstable();
  permute(items, &mut keyed);
}

/// Moves `items[order[i].1]` to `items[i]`, walking each cycle of the permutation once.
fn permute<T>(items: &mut [T], order: &mut [(u128, u32)]) {
  const PLACED: u32 = u32::MAX;

  for start in 0..order.len() {
    let mut at = start;

    while order[at].1 != PLACED {
      let from = order[at].1 as usize;

      order[at].1 = PLACED;
      if from == start {
        break;
      }
      items.swap(at, from);
      at = from;
    }
  }
}

#[cfg(test)]
mod tests {
  use super::sort_by_key;

  /// Verifies that elements with equal keys maintain their relative original input order.
  #[test]
  fn ties_keep_their_order() {
    let mut items = [(2, 'a'), (1, 'b'), (2, 'c'), (1, 'd'), (0, 'e')];

    sort_by_key(&mut items, |&(key, _)| key as u32);
    assert_eq!(items, [(0, 'e'), (1, 'b'), (1, 'd'), (2, 'a'), (2, 'c')]);
  }

  /// Verifies that two-element slices are sorted correctly and maintain stability on ties.
  #[test]
  fn two_items_sort_and_keep_ties() {
    let mut items = [(2, 'a'), (1, 'b')];
    sort_by_key(&mut items, |&(key, _)| key as u32);
    assert_eq!(items, [(1, 'b'), (2, 'a')]);

    let mut items = [(1, 'a'), (2, 'b')];
    sort_by_key(&mut items, |&(key, _)| key as u32);
    assert_eq!(items, [(1, 'a'), (2, 'b')]);

    let mut items = [(1, 'a'), (1, 'b')];
    sort_by_key(&mut items, |&(key, _)| key as u32);
    assert_eq!(items, [(1, 'a'), (1, 'b')]);
  }

  /// Verifies that signed integer keys (i8, i16, i64) sort according to standard numerical ordering.
  #[test]
  fn signed_integers_sort_in_order() {
    let mut items = [10i8, -5, 0, i8::MIN, i8::MAX, -128, -1];
    let mut expected = items;
    expected.sort();
    sort_by_key(&mut items, |&item| item);
    assert_eq!(items, expected);

    let mut items = [1000i16, -500, 0, i16::MIN, i16::MAX, -1];
    let mut expected = items;
    expected.sort();
    sort_by_key(&mut items, |&item| item);
    assert_eq!(items, expected);

    let mut items = [1000i64, -500, 0, i64::MIN, i64::MAX, -1];
    let mut expected = items;
    expected.sort();
    sort_by_key(&mut items, |&item| item);
    assert_eq!(items, expected);
  }

  /// Verifies that floating point values sort matching `f32::total_cmp`.
  #[test]
  fn floats_sort_as_total_cmp_does() {
    let mut items = [
      3.0f32,
      -0.0,
      f32::NEG_INFINITY,
      0.0,
      -2.5,
      f32::INFINITY,
      1.0,
    ];
    let mut expected = items;

    expected.sort_by(f32::total_cmp);
    sort_by_key(&mut items, |&value| value);
    assert_eq!(items.map(f32::to_bits), expected.map(f32::to_bits));
  }

  /// Verifies that tuple keys sort lexicographically field by field.
  #[test]
  fn tuples_sort_field_by_field() {
    let mut items = [(1i32, 5usize), (-3, 9), (1, 2), (-3, 1)];
    let mut expected = items;

    expected.sort();
    sort_by_key(&mut items, |&item| item);
    assert_eq!(items, expected);
  }
}
