//! Page contexts: the geometry each page name resolves to, with the header
//! and footer bands measured at that page's width (css-page-3 §8.1).

use std::sync::Arc;

use takumi_core::layout::node::Node;

use crate::{
  bands::{RepeatBounds, Repeatable},
  options::{PageOptions, PdfError},
  page::{PageBands, PageFrame},
  tree::TreeInputs,
};

/// One page name's frame and the bands drawn on its pages.
pub(crate) struct PageContext {
  pub(crate) frame: PageFrame,
  pub(crate) header: Option<Repeatable>,
  pub(crate) footer: Option<Repeatable>,
}

impl PageContext {
  fn resolve(
    inputs: &TreeInputs<'_>,
    page: PageOptions,
    bands: &PageBands<'_>,
    pages: usize,
  ) -> Result<Self, PdfError> {
    let band_viewport = page.band_viewport();
    let band = |template: Option<&Node>, bounds| {
      template
        .map(|template| Repeatable::band(inputs, template, band_viewport, bounds, pages))
        .transpose()
    };
    let header = band(bands.header, RepeatBounds::Header)?;
    let footer = band(bands.footer, RepeatBounds::Footer)?;
    let frame = PageFrame::resolve(
      &page,
      band_viewport,
      header.as_ref().map(Repeatable::height),
      footer.as_ref().map(Repeatable::height),
    )?;

    Ok(Self {
      frame,
      header,
      footer,
    })
  }

  fn band_heights(&self) -> (Option<f32>, Option<f32>) {
    (
      self.header.as_ref().map(Repeatable::height),
      self.footer.as_ref().map(Repeatable::height),
    )
  }
}

/// The contexts a document's pages draw from: the unnamed page first, then
/// every named page the content asks for.
pub(crate) struct PageContexts {
  page: PageOptions,
  header: Option<Node>,
  footer: Option<Node>,
  /// The page count the bands are measured with.
  pages: usize,
  contexts: Vec<(Option<Arc<str>>, PageContext)>,
}

impl PageContexts {
  /// Resolves the unnamed page, with its bands laid out for `pages` pages.
  pub(crate) fn resolve(
    inputs: &TreeInputs<'_>,
    page: PageOptions,
    bands: &PageBands<'_>,
    pages: usize,
  ) -> Result<Self, PdfError> {
    let mut contexts = Self {
      page,
      header: bands.header.cloned(),
      footer: bands.footer.cloned(),
      pages,
      contexts: Vec::new(),
    };

    contexts.ensure(inputs, None)?;
    Ok(contexts)
  }

  /// Resolves `name` if no context holds it yet.
  pub(crate) fn ensure(
    &mut self,
    inputs: &TreeInputs<'_>,
    name: Option<&Arc<str>>,
  ) -> Result<(), PdfError> {
    if self.position(name.map(Arc::as_ref)).is_some() {
      return Ok(());
    }
    let page = self
      .page
      .for_page(&inputs.stylesheet, name.map(Arc::as_ref));
    let bands = PageBands {
      header: self.header.as_ref(),
      footer: self.footer.as_ref(),
      page_ranges: None,
    };
    let context = PageContext::resolve(inputs, page, &bands, self.pages)?;

    self.contexts.push((name.cloned(), context));
    Ok(())
  }

  /// The context for `name`, or the unnamed page's when none was resolved.
  pub(crate) fn get(&self, name: Option<&str>) -> &PageContext {
    let index = self.position(name).unwrap_or(0);

    &self.contexts[index].1
  }

  pub(crate) fn unnamed(&self) -> &PageContext {
    &self.contexts[0].1
  }

  /// Whether any band holds a page counter and lays out again per page.
  pub(crate) fn dynamic(&self) -> bool {
    self.contexts.iter().any(|(_, context)| {
      context.header.as_ref().is_some_and(Repeatable::dynamic)
        || context.footer.as_ref().is_some_and(Repeatable::dynamic)
    })
  }

  /// Whether every context's bands came out the same height as in `other`,
  /// which is what a re-measure with a new page count checks for.
  pub(crate) fn same_band_heights(&self, other: &Self) -> bool {
    self.contexts.len() == other.contexts.len()
      && self.contexts.iter().all(|(name, context)| {
        other
          .position(name.as_deref())
          .is_some_and(|index| other.contexts[index].1.band_heights() == context.band_heights())
      })
  }

  pub(crate) fn names(&self) -> impl Iterator<Item = Option<&Arc<str>>> {
    self.contexts.iter().map(|(name, _)| name.as_ref())
  }

  fn position(&self, name: Option<&str>) -> Option<usize> {
    self
      .contexts
      .iter()
      .position(|(candidate, _)| candidate.as_deref() == name)
  }
}
