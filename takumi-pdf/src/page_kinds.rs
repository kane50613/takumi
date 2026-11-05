//! A value for each kind of page Blink's `PageRuleCollector` tells apart: the first page, a left
//! page, or a right page, of each page name an `@page` rule selects, and of the pages none does.

use std::{convert::Infallible, rc::Rc};

use takumi_core::style::{Direction, PageName, page_rule::PageKind};

/// A value for the first, left, and right pages of each named page, and of the unnamed pages.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PerPage<T> {
  /// The root's `direction`, which decides which pages are left pages.
  direction: Direction,
  /// The unnamed pages' values first, then those of each name. A page whose name no entry has
  /// takes the unnamed pages' values, as no named rule selects it.
  entries: Vec<(PageName, [T; 3])>,
}

impl<T> PerPage<T> {
  /// `value` gives each kind of the unnamed pages and of the pages of each of `names`, in a
  /// document whose root has `direction`.
  pub(crate) fn of_names(
    names: Vec<PageName>,
    direction: Direction,
    mut value: impl FnMut(PageKind, &PageName) -> T,
  ) -> Self {
    let entries = [PageName::default()]
      .into_iter()
      .chain(names)
      .map(|name| {
        let values =
          [PageKind::First, PageKind::Left, PageKind::Right].map(|kind| value(kind, &name));

        (name, values)
      })
      .collect();

    Self { direction, entries }
  }

  /// The value of page `page`, counted from 0, a page `name`.
  pub(crate) fn get(&self, page: usize, name: &PageName) -> &T {
    let (_, values) = self
      .entries
      .iter()
      .find(|(entry, _)| entry == name)
      .unwrap_or(&self.entries[0]);

    &values[PageKind::of(page, self.direction) as usize]
  }

  /// The value of the first page, when it is unnamed.
  pub(crate) fn first(&self) -> &T {
    self.get(0, &PageName::default())
  }

  /// Every value.
  pub(crate) fn values(&self) -> impl Iterator<Item = &T> {
    self.entries.iter().flat_map(|(_, values)| values)
  }

  /// Each value mapped through `map`.
  pub(crate) fn map<U>(&self, mut map: impl FnMut(&T) -> U) -> PerPage<U> {
    PerPage {
      direction: self.direction,
      entries: self
        .entries
        .iter()
        .map(|(name, values)| (name.clone(), values.each_ref().map(&mut map)))
        .collect(),
    }
  }

  /// Each value mapped through `map` with the value of `other` for the same pages, or the first
  /// error `map` returns. `other` maps the same names.
  pub(crate) fn try_zip_map<U, V, E>(
    &self,
    other: &PerPage<U>,
    mut map: impl FnMut(&T, &U) -> Result<V, E>,
  ) -> Result<PerPage<V>, E> {
    let entries = self
      .entries
      .iter()
      .zip(&other.entries)
      .map(
        |((name, [first, left, right]), (_, [other_first, other_left, other_right]))| {
          Ok((
            name.clone(),
            [
              map(first, other_first)?,
              map(left, other_left)?,
              map(right, other_right)?,
            ],
          ))
        },
      )
      .collect::<Result<_, E>>()?;

    Ok(PerPage {
      direction: self.direction,
      entries,
    })
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

    self.try_zip_map(self, |value, _| {
      let key = key(value);

      if let Some((_, shared)) = made.iter().find(|(made_key, _)| *made_key == key) {
        return Ok(Rc::clone(shared));
      }

      let shared = Rc::new(map(value)?);

      made.push((key, Rc::clone(&shared)));
      Ok(shared)
    })
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use takumi_core::style::{Direction, PageName, page_rule::PageKind};

  use super::PerPage;
  use crate::test_support::per_kind;

  #[test]
  fn pages_after_the_first_alternate_left_and_right() {
    let pages = |direction| {
      let pages = PerPage::of_names(Vec::new(), direction, |kind, _| match kind {
        PageKind::First => "first",
        PageKind::Left => "left",
        PageKind::Right => "right",
      });

      (0..5)
        .map(|page| *pages.get(page, &PageName::default()))
        .collect::<Vec<_>>()
    };

    assert_eq!(
      pages(Direction::Ltr),
      ["first", "left", "right", "left", "right"]
    );
    assert_eq!(
      pages(Direction::Rtl),
      ["first", "right", "left", "right", "left"]
    );
  }

  #[test]
  fn a_page_of_a_name_without_values_takes_the_unnamed_ones() {
    let pages = PerPage::of_names(
      vec![PageName::named("cover")],
      Direction::Ltr,
      |kind, name| (kind, name.get().unwrap_or("unnamed").to_owned()),
    );

    assert_eq!(
      pages.get(0, &PageName::named("cover")),
      &(PageKind::First, "cover".to_owned())
    );
    assert_eq!(
      pages.get(3, &PageName::named("index")),
      &(PageKind::Left, "unnamed".to_owned())
    );
  }

  #[test]
  fn values_with_one_key_share_one_mapping() {
    let mut calls = 0;
    let shared = per_kind(10, 20, 10)
      .try_map_shared(
        |value| *value,
        |value| {
          calls += 1;
          Ok::<_, ()>(value * 2)
        },
      )
      .expect("maps");
    let [first, left, right] = [0, 1, 2].map(|page| shared.get(page, &PageName::default()));

    assert_eq!(calls, 2);
    assert_eq!((**first, **left), (20, 40));
    assert!(Rc::ptr_eq(first, right));
  }
}
