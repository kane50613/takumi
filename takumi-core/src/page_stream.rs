//! A render tree laid out a page at a time, with the pages no later page can paint on handed out.

use std::ops::Range;

use crate::{
  Error, Result,
  geometry::Size,
  layout::{layout_view::LayoutView, paged_layout::PagedLayout, tree::RenderNode},
  scene::{PageWindows, SceneRef},
};

/// How many of the first pages are safe to paint, as [`PagedLayout::settled_pages`] says.
pub type Settle<'s> = &'s dyn Fn(&PagedLayout<'_>) -> usize;

/// [`PagedLayout::settled_pages`], as a closure because the method's layout lifetime is early-bound.
#[allow(clippy::redundant_closure_for_method_calls)]
pub const SETTLED_PAGES: Settle<'static> = &|layout| layout.settled_pages();

/// What one [`PageStream::advance`] settled.
pub struct Settled {
  /// The pages that became safe to paint, in order.
  pub pages: Range<usize>,
  /// Whether a page was laid out, so the layout may go on.
  pub laid: bool,
}

/// A tree laid out a page at a time, with the windows to paint the settled pages from.
pub struct PageStream<'r> {
  root: &'r RenderNode,
  layout: PagedLayout<'r>,
  windows: PageWindows,
  page_size: Size<f32>,
  handed_over: usize,
  released: usize,
}

impl<'r> PageStream<'r> {
  /// Starts laying `root` out on at most `max_pages` pages `page_size`.
  pub fn new(root: &'r RenderNode, page_size: Size<f32>, max_pages: usize) -> Self {
    Self {
      root,
      layout: PagedLayout::new(root, page_size, max_pages).with_release(),
      windows: PageWindows::new(page_size),
      page_size,
      handed_over: 0,
      released: 0,
    }
  }

  /// Lays out the next page and returns the pages `settle` newly says are safe to paint. The caller
  /// has painted the pages an earlier call returned by now, so this one frees what they held.
  pub fn advance(&mut self, settle: Settle<'_>) -> Result<Settled> {
    self.release();

    let laid = self.layout.next_page()?;
    let settled = settle(&self.layout).min(self.layout.page_count());
    let pages = self.handed_over..settled.max(self.handed_over);

    if !pages.is_empty() {
      self.windows.refresh(
        self.root,
        self.layout.view(),
        self.layout.page_count(),
        self.layout.finalized_pages(),
      )?;
    }
    self.handed_over = pages.end;
    Ok(Settled { pages, laid })
  }

  fn release(&mut self) {
    let layout = &self.layout;
    let handed_over = self.handed_over;
    let pages = self
      .windows
      .releasable_pages(self.released, handed_over, |node| {
        layout.has_ended_by(node, handed_over)
      });

    self.layout.release(pages);
    self.released = self.released.max(pages);
  }

  /// The layout so far.
  pub fn view(&self) -> LayoutView<'_> {
    self.layout.view()
  }

  /// How many pages are laid out.
  pub fn page_count(&self) -> usize {
    self.layout.page_count()
  }

  /// The pages whose root fragments a scene needs to paint the root fragment of `page`.
  pub fn pages_of(&self, page: usize) -> Vec<usize> {
    self.windows.pages_of(page)
  }

  /// The pages whose root fragments the scene that paints `page` draws from.
  pub fn pages_reaching(&self, page: usize) -> Vec<usize> {
    self.windows.pages_reaching(page)
  }

  /// The scene that paints the settled page `page`. Fails with [`Error::PageReleased`] when it
  /// needs a page that was released.
  pub fn window(&self, page: usize) -> Result<SceneRef<'_>> {
    let pages = self.pages_reaching(page);

    if let Some(&released) = pages.iter().find(|&&reached| reached < self.released) {
      return Err(Error::PageReleased(released));
    }

    SceneRef::window(self.root, self.layout.view(), self.page_size, &pages, true)
  }
}
