//! A process-wide table of values a `Copy` handle names, so a public `Copy` type can hold a value
//! of any size. A value is never freed. The table holds at most [`MAX_VALUES`] values of at most
//! [`MAX_PARTS`] parts each, so a stream of distinct stylesheets cannot grow it without bound.

use std::{
  collections::HashMap,
  sync::{PoisonError, RwLock},
};

/// How many distinct values one table keeps. A value past it does not intern.
const MAX_VALUES: usize = 1 << 16;

/// How many parts, such as a tree's nodes or a curve's points, one value may have.
pub(crate) const MAX_PARTS: usize = 1 << 10;

/// Values, each at the index its serialization keys.
pub(crate) struct Interner<T> {
  values: Vec<T>,
  indices: HashMap<String, u32>,
}

impl<T> Default for Interner<T> {
  fn default() -> Self {
    Self {
      values: Vec::new(),
      indices: HashMap::new(),
    }
  }
}

/// Interns `value`, which serializes to `key`, in `table`: the index it already holds, a new one,
/// or `None` once the table is full.
pub(crate) fn intern<T>(table: &RwLock<Interner<T>>, key: String, value: T) -> Option<u32> {
  if let Some(&index) = table.read().ok()?.indices.get(&key) {
    return Some(index);
  }

  let mut interner = table.write().ok()?;

  if let Some(&index) = interner.indices.get(&key) {
    return Some(index);
  }
  if interner.values.len() >= MAX_VALUES {
    return None;
  }

  let index = u32::try_from(interner.values.len()).ok()?;

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
