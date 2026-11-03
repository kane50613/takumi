//! The page area of each page of a paged layout. As Blink's `PageRuleCollector` tells pages apart,
//! the first page, the left pages, and the right pages can each take their own size. Approximate:
//! the pages progress left to right whatever the root's `direction`, where Blink makes the first
//! page a left page under `direction: rtl`.

use crate::{
  geometry::{Point, Size},
  layout_unit::LayoutUnit,
};

/// A value for the first page, one for the left pages, and one for the right pages after it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerPage<T> {
  first: T,
  left: T,
  right: T,
}

impl<T: Clone> PerPage<T> {
  /// `value` for every page.
  pub fn uniform(value: T) -> Self {
    Self::new(value.clone(), value.clone(), value)
  }
}

impl<T> PerPage<T> {
  /// `first` for the first page, and `left` and `right` for the left and right pages after it.
  pub fn new(first: T, left: T, right: T) -> Self {
    Self { first, left, right }
  }

  /// The value of page `page`, counted from 0.
  pub fn get(&self, page: usize) -> &T {
    if page == 0 {
      &self.first
    } else if page % 2 == 1 {
      &self.left
    } else {
      &self.right
    }
  }

  /// Each value mapped through `map`.
  pub fn map<U>(&self, mut map: impl FnMut(&T) -> U) -> PerPage<U> {
    PerPage::new(map(&self.first), map(&self.left), map(&self.right))
  }

  /// The first page's value, the left pages', and the right pages'.
  pub fn values(&self) -> [&T; 3] {
    [&self.first, &self.left, &self.right]
  }

  /// How many left and right pages come after the first and before page `page`. Page 1 is the
  /// first left page, as Blink's `PageRuleCollector::IsLeftPage` says.
  fn sides_before(&self, page: usize) -> (usize, usize) {
    let lefts = page / 2;

    (lefts, page.saturating_sub(1) - lefts)
  }
}

/// The size of each page's page area. The pages stack in the block direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSizes(PerPage<Size<f32>>);

impl From<PerPage<Size<f32>>> for PageSizes {
  fn from(sizes: PerPage<Size<f32>>) -> Self {
    Self(sizes)
  }
}

impl PageSizes {
  /// Every page `size`.
  pub fn uniform(size: Size<f32>) -> Self {
    Self(PerPage::uniform(size))
  }

  /// The page area of page `page`, counted from 0.
  pub fn size(&self, page: usize) -> Size<f32> {
    *self.0.get(page)
  }

  /// Where page `page` starts in the scene of every page.
  pub fn origin(&self, page: usize) -> Point<f32> {
    Point {
      x: 0.0,
      y: self.top(page),
    }
  }

  /// How far down page `page` starts: the heights of the pages before it.
  pub fn top(&self, page: usize) -> f32 {
    let [first, left, right] = self.0.values().map(|size| size.height);

    if first == left && left == right {
      // Exact multiples of the height, which a running sum of heights would round away from.
      return first * page as f32;
    }

    let (lefts, rights) = self.0.sides_before(page);

    first * page.min(1) as f32 + left * lefts as f32 + right * rights as f32
  }

  /// [`Self::top`] in layout units, summed from each page's rounded height.
  pub(crate) fn block_offset(&self, page: usize) -> LayoutUnit {
    let [first, left, right] = self
      .0
      .values()
      .map(|size| LayoutUnit::from_float(size.height));
    let (lefts, rights) = self.0.sides_before(page);

    first * page.min(1) as i32 + left * lefts as i32 + right * rights as i32
  }

  /// The shortest page height, which a distance up the pages takes the most pages to cover.
  pub(crate) fn min_height(&self) -> f32 {
    let [first, left, right] = self.0.values().map(|size| size.height);

    first.min(left).min(right)
  }
}

#[cfg(test)]
mod tests {
  use super::{PageSizes, PerPage};
  use crate::{geometry::Size, layout_unit::LayoutUnit};

  fn height(height: f32) -> Size<f32> {
    Size {
      width: 300.0,
      height,
    }
  }

  #[test]
  fn pages_start_below_the_first_page_and_the_alternating_sides() {
    let sizes = PageSizes::from(PerPage::new(height(100.0), height(50.0), height(70.0)));

    assert_eq!(
      (0..5).map(|page| sizes.top(page)).collect::<Vec<_>>(),
      [0.0, 100.0, 150.0, 220.0, 270.0]
    );
    assert_eq!(sizes.block_offset(4), LayoutUnit::from_float(270.0));
    assert_eq!(sizes.size(3).height, 50.0);
    assert_eq!(sizes.size(4).height, 70.0);
    assert_eq!(sizes.min_height(), 50.0);
  }

  #[test]
  fn uniform_pages_start_a_page_height_apart() {
    let sizes = PageSizes::uniform(Size {
      width: 300.0,
      height: 123.4,
    });

    for page in 0..6 {
      assert_eq!(sizes.top(page), 123.4 * page as f32);
      assert_eq!(sizes.size(page).height, 123.4);
    }
  }
}
