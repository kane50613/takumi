//! The page area of each page of a paged layout, which Blink's `PaginatedRootLayoutAlgorithm` asks
//! `StyleForPage` for as it reaches the page, by its index and the named page its content is on.

use crate::{
  geometry::{Point, Size},
  layout_unit::LayoutUnit,
  style::PageName,
};

/// A page's index and name to the size of its page area.
type SizeOf = dyn Fn(usize, &PageName) -> Size<f32>;

/// What gives each page its page area, as Blink's `StyleForPage` does.
pub struct PageSizer(Box<SizeOf>);

impl PageSizer {
  /// `size_of` gives page `page`, counted from 0, its page area as a page `name`, which every name
  /// must have: a page lays out first under the name of the page before it.
  pub fn new(size_of: impl Fn(usize, &PageName) -> Size<f32> + 'static) -> Self {
    Self(Box::new(size_of))
  }

  /// Every page `size`.
  pub fn uniform(size: Size<f32>) -> Self {
    Self::new(move |_, _| size)
  }

  pub(crate) fn size(&self, page: usize, name: &PageName) -> Size<f32> {
    (self.0)(page, name)
  }
}

impl From<Size<f32>> for PageSizer {
  fn from(size: Size<f32>) -> Self {
    Self::uniform(size)
  }
}

/// The page area and the page name of each page laid out so far. The pages stack in the block
/// direction.
#[derive(Debug, Clone, PartialEq)]
pub struct PageSizes {
  sizes: Vec<Size<f32>>,
  names: Vec<PageName>,
  /// Where each page starts, and after them where the next would.
  tops: Vec<f32>,
  /// The tops in layout units, summed from each page's rounded height.
  block_offsets: Vec<LayoutUnit>,
  /// The height every page has, while they all have the same.
  uniform_height: Option<f32>,
}

impl Default for PageSizes {
  fn default() -> Self {
    Self {
      sizes: Vec::new(),
      names: Vec::new(),
      tops: vec![0.0],
      block_offsets: vec![LayoutUnit::ZERO],
      uniform_height: None,
    }
  }
}

impl PageSizes {
  /// `pages` pages of `size`.
  pub fn uniform(size: Size<f32>, pages: usize) -> Self {
    let mut sizes = Self::default();

    for _ in 0..pages {
      sizes.push(size, PageName::default());
    }
    sizes
  }

  /// Adds the next page, a page `name`.
  pub(crate) fn push(&mut self, size: Size<f32>, name: PageName) {
    let page = self.sizes.len();
    let uniform_height = match self.uniform_height {
      None if page == 0 => Some(size.height),
      Some(height) if height == size.height => Some(height),
      _ => None,
    };
    // Exact multiples of the height, which a running sum of heights would round away from.
    let next_top = match uniform_height {
      Some(height) => height * (page + 1) as f32,
      None => self.tops[page] + size.height,
    };

    self.sizes.push(size);
    self.names.push(name);
    self.tops.push(next_top);
    self
      .block_offsets
      .push(self.block_offsets[page] + LayoutUnit::from_float(size.height));
    self.uniform_height = uniform_height;
  }

  /// How many pages there are.
  pub fn len(&self) -> usize {
    self.sizes.len()
  }

  /// Whether there are no pages.
  pub fn is_empty(&self) -> bool {
    self.sizes.is_empty()
  }

  /// The page area of page `page`, counted from 0.
  pub fn size(&self, page: usize) -> Size<f32> {
    self.sizes[page]
  }

  /// The named page page `page` is, as its content's `page` says.
  pub fn name(&self, page: usize) -> &PageName {
    &self.names[page]
  }

  /// Where page `page` starts in the scene of every page.
  pub fn origin(&self, page: usize) -> Point<f32> {
    Point {
      x: 0.0,
      y: self.top(page),
    }
  }

  /// How far down page `page` starts: the heights of the pages before it. Page [`Self::len`] is
  /// where the next page would start.
  pub fn top(&self, page: usize) -> f32 {
    self.tops[page]
  }

  /// The page `top` falls on, counted from 0: the last page that starts at or above it.
  pub fn page_at(&self, top: f32) -> usize {
    self.tops[..self.len()]
      .partition_point(|&start| start <= top)
      .saturating_sub(1)
  }

  /// How many of the first pages end at or above `top`.
  pub(crate) fn pages_above(&self, top: f32) -> usize {
    self.tops[1..].partition_point(|&bottom| bottom <= top)
  }

  /// [`Self::top`] in layout units.
  pub(crate) fn block_offset(&self, page: usize) -> LayoutUnit {
    self.block_offsets[page]
  }
}

#[cfg(test)]
mod tests {
  use super::PageSizes;
  use crate::{geometry::Size, layout_unit::LayoutUnit, style::PageName};

  fn height(height: f32) -> Size<f32> {
    Size {
      width: 300.0,
      height,
    }
  }

  #[test]
  fn pages_start_below_the_pages_before_them() {
    let mut sizes = PageSizes::default();

    for page in [100.0, 50.0, 70.0, 50.0] {
      sizes.push(height(page), PageName::default());
    }

    assert_eq!(
      (0..5).map(|page| sizes.top(page)).collect::<Vec<_>>(),
      [0.0, 100.0, 150.0, 220.0, 270.0]
    );
    assert_eq!(sizes.block_offset(4), LayoutUnit::from_float(270.0));
    assert_eq!(sizes.size(3).height, 50.0);
    assert_eq!(sizes.page_at(149.0), 1);
    assert_eq!(sizes.page_at(1000.0), 3);
    assert_eq!(sizes.pages_above(220.0), 3);
    assert_eq!(sizes.pages_above(219.0), 2);
  }

  #[test]
  fn uniform_pages_start_a_page_height_apart() {
    let sizes = PageSizes::uniform(height(123.4), 6);

    for page in 0..6 {
      assert_eq!(sizes.top(page), 123.4 * page as f32);
      assert_eq!(sizes.size(page).height, 123.4);
    }
  }
}
