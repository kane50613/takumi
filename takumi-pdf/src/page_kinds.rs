//! A value for each kind of page Blink's `PageRuleCollector` tells apart: the first page, the left
//! pages, and the right pages. Approximate: the pages progress left to right whatever the root's
//! `direction`, where Blink makes the first page a left page under `direction: rtl`.

use std::{convert::Infallible, rc::Rc};

/// A value for the first page, one for the left pages, and one for the right pages after it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PerPage<T> {
  first: T,
  left: T,
  right: T,
}

impl<T: Clone> PerPage<T> {
  /// `value` for every page.
  pub(crate) fn uniform(value: T) -> Self {
    Self::new(value.clone(), value.clone(), value)
  }
}

impl<T> PerPage<T> {
  /// `first` for the first page, and `left` and `right` for the left and right pages after it.
  pub(crate) fn new(first: T, left: T, right: T) -> Self {
    Self { first, left, right }
  }

  /// The value of page `page`, counted from 0. Page 1 is the first left page, as Blink's
  /// `PageRuleCollector::IsLeftPage` says.
  pub(crate) fn get(&self, page: usize) -> &T {
    if page == 0 {
      &self.first
    } else if page % 2 == 1 {
      &self.left
    } else {
      &self.right
    }
  }

  /// The first page's value, the left pages', and the right pages'.
  pub(crate) fn values(&self) -> [&T; 3] {
    [&self.first, &self.left, &self.right]
  }

  /// Each value mapped through `map`.
  pub(crate) fn map<U>(&self, mut map: impl FnMut(&T) -> U) -> PerPage<U> {
    PerPage::new(map(&self.first), map(&self.left), map(&self.right))
  }

  /// Each value mapped through `map` with the value of `other` for the same pages, or the first
  /// error `map` returns.
  pub(crate) fn try_zip_map<U, V, E>(
    &self,
    other: &PerPage<U>,
    mut map: impl FnMut(&T, &U) -> Result<V, E>,
  ) -> Result<PerPage<V>, E> {
    Ok(PerPage::new(
      map(&self.first, &other.first)?,
      map(&self.left, &other.left)?,
      map(&self.right, &other.right)?,
    ))
  }

  /// [`Self::try_map_shared`] with a `map` that cannot fail.
  pub(crate) fn map_shared<K: PartialEq, U>(
    &self,
    key: impl Fn(&T) -> K,
    mut map: impl FnMut(&T) -> U,
  ) -> PerPage<Rc<U>> {
    match self.try_map_shared(key, |value| Ok::<_, Infallible>(map(value))) {
      Ok(shared) => shared,
    }
  }

  /// Each value mapped through `map` once per distinct `key`, the values with the same key
  /// sharing what it returned, or the first error it returns.
  pub(crate) fn try_map_shared<K: PartialEq, U, E>(
    &self,
    key: impl Fn(&T) -> K,
    mut map: impl FnMut(&T) -> Result<U, E>,
  ) -> Result<PerPage<Rc<U>>, E> {
    let mut made: Vec<(K, Rc<U>)> = Vec::new();
    let mut share = |value: &T| {
      let key = key(value);

      if let Some((_, shared)) = made.iter().find(|(made_key, _)| *made_key == key) {
        return Ok(Rc::clone(shared));
      }

      let shared = Rc::new(map(value)?);

      made.push((key, Rc::clone(&shared)));
      Ok(shared)
    };

    Ok(PerPage::new(
      share(&self.first)?,
      share(&self.left)?,
      share(&self.right)?,
    ))
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::PerPage;

  #[test]
  fn pages_after_the_first_alternate_left_and_right() {
    let pages = PerPage::new("first", "left", "right");

    assert_eq!(
      (0..5).map(|page| *pages.get(page)).collect::<Vec<_>>(),
      ["first", "left", "right", "left", "right"]
    );
  }

  #[test]
  fn values_with_one_key_share_one_mapping() {
    let mut calls = 0;
    let shared = PerPage::new(10, 20, 10)
      .try_map_shared(
        |value| *value,
        |value| {
          calls += 1;
          Ok::<_, ()>(value * 2)
        },
      )
      .expect("maps");
    let [first, left, right] = shared.values();

    assert_eq!(calls, 2);
    assert_eq!((**first, **left), (20, 40));
    assert!(Rc::ptr_eq(first, right));
  }
}
