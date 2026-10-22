//! Sections laid out at independent widths in one PDF document.

use std::collections::HashMap;

use takumi_core::style::Color;

use crate::{
  Band, PageOptions, PageRules, PdfError, PdfSection,
  counters::{has_page_counters, has_target_counters},
  emitter::DocumentState,
  interactive::Interactive,
  krilla::{Document, destination::XyzDestination},
  options::{PageBand, PageRange, PageSelection},
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
    if sources.len() >= MAX_PAGES {
      return Err(PdfError::TooManyPages(sources.len()));
    }
    let mut counts = vec![1; sources.len()];
    let mut targets = HashMap::new();

    for _ in 0..8 {
      let total = counts.iter().sum();
      let mut offset = 0;
      let mut next_total = 0;
      let mut dynamic = false;
      let mut sections = Vec::with_capacity(sources.len());
      let mut next_targets = HashMap::new();

      for (index, source) in sources.iter().enumerate() {
        let mut rules = rules.clone();

        if index > 0 {
          rules.first = Default::default();
        }
        if index + 1 < sources.len() {
          rules.last = Default::default();
        }
        // The first source owns the document's default bands.
        let [header, footer] = [
          source.header.as_ref().or(sources[0].header.as_ref()),
          source.footer.as_ref().or(sources[0].footer.as_ref()),
        ]
        .map(|band| match band {
          Some(Band::Node(node)) => Some(node.as_ref()),
          _ => None,
        });
        let bands = PageBands {
          header: rules.header_band(header),
          footer: rules.footer_band(footer),
          page_ranges: None,
        };

        dynamic |= [&source.node]
          .into_iter()
          .chain(
            bands
              .header
              .iter()
              .chain(&bands.footer)
              .flat_map(PageBand::nodes),
          )
          .any(|node| has_page_counters(node) || has_target_counters(node));
        let mut plan = PagePlan::solve(
          inputs,
          source.page.unwrap_or(page),
          bands,
          source.node.clone(),
          structural,
          PageCounters {
            offset,
            total: Some(total),
            targets: Some(&targets),
          },
        )?;

        next_total += plan.paginated.starts.len();
        if next_total >= MAX_PAGES {
          return Err(PdfError::TooManyPages(next_total));
        }
        plan.prefix = vec![index];
        for (id, anchor) in &plan.paginated.interactive.anchors {
          next_targets
            .entry(id.clone())
            .or_insert(offset + plan.paginated.page_index(anchor.top) + 1);
        }
        offset += counts[index];
        sections.push(plan);
      }
      let next_counts: Vec<_> = sections
        .iter()
        .map(|plan| plan.paginated.starts.len())
        .collect();

      if !dynamic || (next_counts == counts && next_targets == targets) {
        let selection = PageSelection::resolve(ranges, next_total)?;
        let mut interactive = Interactive::default();
        let mut offset = 0;

        for (index, plan) in sections.iter_mut().enumerate() {
          plan.offset = offset;
          plan.total = next_total;
          plan.selection = selection.section(offset, plan.paginated.starts.len());
          interactive.append_section(&plan.paginated.interactive, index);
          offset += plan.paginated.starts.len();
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
