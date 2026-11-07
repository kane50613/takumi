//! A process-wide table of values a `Copy` handle names, so a public `Copy` type can hold a value
//! of any size. A value is never freed, so a table holds at most [`MAX_TOTAL_PARTS`] parts, such
//! as a tree's nodes or a curve's points, across all its values, and a stream of distinct
//! stylesheets cannot grow it past a few megabytes.

use std::{
  collections::HashMap,
  sync::{PoisonError, RwLock},
};

/// How many parts one value may have.
pub(crate) const MAX_PARTS: usize = 1 << 10;

/// How many parts one table keeps across its values. A value past it does not intern.
const MAX_TOTAL_PARTS: usize = 1 << 20;

/// Values, each at the index its serialization keys.
pub(crate) struct Interner<T> {
  values: Vec<T>,
  indices: HashMap<String, u32>,
  /// The parts of every value held.
  parts: usize,
}

impl<T> Default for Interner<T> {
  fn default() -> Self {
    Self {
      values: Vec::new(),
      indices: HashMap::new(),
      parts: 0,
    }
  }
}

/// Interns `value`, which serializes to `key` and has `parts` parts, in `table`: the index it
/// already holds, a new one, or `None` when it is too large or the table is full.
pub(crate) fn intern<T>(
  table: &RwLock<Interner<T>>,
  key: String,
  value: T,
  parts: usize,
) -> Option<u32> {
  if parts > MAX_PARTS {
    return None;
  }
  if let Some(&index) = table.read().ok()?.indices.get(&key) {
    return Some(index);
  }

  let mut interner = table.write().ok()?;

  if let Some(&index) = interner.indices.get(&key) {
    return Some(index);
  }
  if interner.parts + parts > MAX_TOTAL_PARTS {
    return None;
  }

  let index = u32::try_from(interner.values.len()).ok()?;

  interner.parts += parts;
  interner.values.push(value);
  interner.indices.insert(key, index);
  Some(index)
}

/// What `read` makes of the value at `index` in `table`. Interning takes the lock this holds, so
/// `read` must not intern.
pub(crate) fn with<T, R>(table: &RwLock<Interner<T>>, index: u32, read: impl FnOnce(&T) -> R) -> R {
  let interner = table.read().unwrap_or_else(PoisonError::into_inner);

  read(&interner.values[index as usize])
}
