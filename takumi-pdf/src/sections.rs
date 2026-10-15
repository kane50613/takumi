//! Sections laid out at independent widths in one PDF document.

use std::collections::HashMap;

use takumi_core::style::Color;

use crate::{
  Band, PageOptions, PageRules, PdfError, PdfSection,
  emitter::DocumentState,
  interactive::Interactive,
  krilla::{Document, destination::XyzDestination},
  options::{PageRange, PageSelection},
  page::{PageBands, PagePlan},
  pagination::{MAX_PAGES, PageCounters},
  tree::TreeInputs,
};

pub(crate) struct SectionPlan {
  pub(crate) sections: Vec<PagePlan>,
  pub(crate) interactive: Interactive,
}

impl SectionPlan {
  pub(crate) fn solve(
    inputs: &TreeInputs<'_>,
    sources: &[PdfSection],
    page: PageOptions,
    rules: &PageRules,
    ranges: Option<&[PageRange]>,
    structural: bool,
  ) -> Result<Self, PdfError> {
    let mut counts = vec![1; sources.len()];
    let mut targets = HashMap::new();

    for _ in 0..8 {
      let total = counts.iter().sum();
      let mut offset = 0;
      let mut sections = Vec::with_capacity(sources.len());
      let mut next_targets = HashMap::new();
      let mut interactive = Interactive::default();

      for (index, source) in sources.iter().enumerate() {
        let header = source.header.clone().and_then(Band::node);
        let footer = source.footer.clone().and_then(Band::node);
        let mut plan = PagePlan::solve(
          inputs,
          source.page.unwrap_or(page),
          PageBands {
            header: rules.header_band(header.as_ref()),
            footer: rules.footer_band(footer.as_ref()),
            page_ranges: None,
          },
          source.node.clone(),
          structural,
          PageCounters {
            offset,
            total: Some(total),
            targets: Some(&targets),
          },
        )?;

        plan.prefix = vec![index];
        for (id, anchor) in &plan.paginated.interactive.anchors {
          next_targets
            .entry(id.clone())
            .or_insert(offset + plan.paginated.page_index(anchor.top) + 1);
        }
        interactive.append_section(&plan.paginated.interactive, index);
        offset += counts[index];
        sections.push(plan);
      }
      let next_counts: Vec<_> = sections
        .iter()
        .map(|plan| plan.paginated.starts.len())
        .collect();
      let next_total = next_counts.iter().sum();

      if next_total >= MAX_PAGES {
        return Err(PdfError::TooManyPages(next_total));
      }
      if next_counts == counts && next_targets == targets {
        let selection = PageSelection::resolve(ranges, total)?;

        for plan in &mut sections {
          plan.selection = selection.section(plan.offset, plan.paginated.starts.len());
        }
        return Ok(Self {
          sections,
          interactive,
        });
      }
      counts = next_counts;
      targets = next_targets;
    }
    Err(PdfError::UnstableSections)
  }

  pub(crate) fn destination(&self, top: f32, path: &[usize]) -> Option<XyzDestination> {
    let (&index, path) = path.split_first()?;

    self.sections.get(index)?.destination(top, path)
  }

  pub(crate) fn compose(
    &self,
    pdf: &mut Document,
    inputs: &TreeInputs<'_>,
    state: &DocumentState<'_>,
    background: Option<Color>,
  ) -> Result<(), PdfError> {
    let destinations = self
      .interactive
      .anchors
      .iter()
      .filter_map(|(id, anchor)| {
        self
          .destination(anchor.top, &anchor.path)
          .map(|destination| (id.clone(), destination))
      })
      .collect();

    for plan in &self.sections {
      if let Some(tags) = &state.tags {
        tags.borrow_mut().prefix.clone_from(&plan.prefix);
      }
      plan.compose(pdf, inputs, state, background, Some(&destinations))?;
    }
    Ok(())
  }
}
