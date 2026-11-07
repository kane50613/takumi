//! The page area of each page of a paged layout, which Blink's `PaginatedRootLayoutAlgorithm` asks
//! `StyleForPage` for as it reaches the page, by its index and the named page its content is on.

use std::sync::Arc;

use crate::{
  geometry::{Point, Rect, Size},
  layout_unit::LayoutUnit,
  style::{
    Length, PageName, StyleSheet,
    page_rule::{PageContext, PageKind},
  },
  viewport::Viewport,
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

impl From<PageRules> for PageSizer {
  fn from(rules: PageRules) -> Self {
    Self::new(move |page, name| rules.page_box(page, name).area())
  }
}

impl From<Size<f32>> for PageSizer {
  fn from(size: Size<f32>) -> Self {
    Self::uniform(size)
  }
}

/// A page's size and the margins around its page area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageBox {
  /// The page's size.
  pub size: Size<f32>,
  /// The margins between the page's edges and its page area.
  pub margin: Rect<f32>,
}

impl PageBox {
  /// The page area inside the margins.
  pub fn area(&self) -> Size<f32> {
    self.size.inset(self.margin)
  }
}

/// The `@page` rules of a stylesheet laid over a default page box.
pub struct PageRules {
  stylesheet: Arc<StyleSheet>,
  base: PageBox,
  viewport: Viewport,
  context: PageContext,
}

impl PageRules {
  /// The `@page` rules of `stylesheet` over `base`, the page box no rule changes, for pages whose
  /// style inherits `context`.
  pub fn new(stylesheet: Arc<StyleSheet>, base: PageBox, context: PageContext) -> Self {
    Self {
      stylesheet,
      base,
      viewport: Viewport::print_page(base.size),
      context,
    }
  }

  /// Page `page`'s box as a page `name`, as Blink's `StyleForPage` and `ResolvePageBoxGeometry`
  /// give it, a margin percentage taking the page's size on its own axis. Lengths resolve with
  /// the base page as the viewport, as Blink's `ViewportSizeChangeScopeForPrinting` sizes it. A
  /// rule that leaves less than a pixel of page area on either axis is ignored, as Blink's
  /// `LayoutPageContainer` ignores one that leaves none.
  pub fn page_box(&self, page: usize, name: &PageName) -> PageBox {
    let kind = PageKind::of(page, self.context.direction());
    let descriptors = self
      .stylesheet
      .page_descriptors(self.viewport, kind, name, &self.context);
    let sizing = self.context.sizing(self.viewport);
    let size = descriptors
      .size
      .map_or(self.base.size, |size| size.resolve(self.base.size, &sizing));
    // Blink's `ResolveAutoMargins` leaves a page box that fills its page no `auto` margin.
    let side = |base: f32, length: Option<Length>, basis: f32| match length {
      None => base,
      Some(Length::Auto) => 0.0,
      Some(length) => length.to_px(&sizing, basis),
    };
    let margin = self.base.margin;
    let page_box = PageBox {
      size,
      margin: Rect {
        top: side(margin.top, descriptors.margin.top, size.height),
        right: side(margin.right, descriptors.margin.right, size.width),
        bottom: side(margin.bottom, descriptors.margin.bottom, size.height),
        left: side(margin.left, descriptors.margin.left, size.width),
      },
    };
    let area = page_box.area();

    if area.width >= 1.0 && area.height >= 1.0 {
      page_box
    } else {
      self.base
    }
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
