//! Sections laid out at independent widths in one PDF document.

use std::collections::HashMap;

use takumi_core::page_stream::Settle;

use crate::{
  Band, PageOptions, PageRules, PdfError, PdfSection,
  counters::{has_page_counters, has_target_counters},
  interactive::{PageSummary, has_internal_links},
  krilla::destination::XyzDestination,
  options::{PageBand, PageRange, PageSelection},
  page::{Destinations, PageBands, PageDrawing, PagePlan, Solved},
  pagination::{Counted, MAX_PAGES, PageCounters},
};

/// How many times the sections lay out to settle the document's page numbers.
const SECTION_PASSES: usize = 8;

pub(crate) struct SectionPlan {
  pub(crate) sections: Vec<PagePlan>,
  pub(crate) summary: PageSummary,
}

impl SectionPlan {
  /// Counts the pages of every section until the document's page numbers stop moving, then draws
  /// each section's pages in order.
  pub(crate) fn render(
    drawing: &mut PageDrawing<'_, '_>,
    sources: &[PdfSection],
    page: PageOptions,
    rules: &PageRules,
    ranges: Option<&[PageRange]>,
    structural: bool,
    settle: Settle<'_>,
  ) -> Result<Self, PdfError> {
    if sources.len() >= MAX_PAGES {
      return Err(PdfError::TooManyPages(sources.len()));
    }
    let linked = sources
      .iter()
      .any(|source| has_target_counters(&source.node) || has_internal_links(&source.node));
    let mut counts = vec![1; sources.len()];
    let mut targets = HashMap::new();

    for _ in 0..SECTION_PASSES {
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
        let (solved, counted) = PageDrawing::count(
          drawing.inputs,
          source.page.unwrap_or(page),
          &bands,
          source.node.clone(),
          settle,
          PageCounters {
            offset,
            total: Some(total),
            targets: linked.then_some(&targets),
          },
        )?;

        next_total += counted.pages();
        if next_total >= MAX_PAGES {
          return Err(PdfError::TooManyPages(next_total));
        }
        if linked {
          for (id, anchor) in counted.anchors()? {
            let page = offset + counted.starts().index_of(anchor.top) + 1;

            next_targets.entry(id).or_insert(page);
          }
        }
        offset += counts[index];
        sections.push((solved, counted));
      }
      let next_counts: Vec<_> = sections
        .iter()
        .map(|(_, counted)| counted.pages())
        .collect();

      if !dynamic || (next_counts == counts && next_targets == targets) {
        return Self::draw(drawing, sections, ranges, structural, settle, linked);
      }
      counts = next_counts;
      targets = next_targets;
    }
    Err(PdfError::UnstableSections)
  }

  /// Draws the counted sections in order, numbering their pages across the document.
  fn draw(
    drawing: &mut PageDrawing<'_, '_>,
    sections: Vec<(Solved, Counted)>,
    ranges: Option<&[PageRange]>,
    structural: bool,
    settle: Settle<'_>,
    linked: bool,
  ) -> Result<Self, PdfError> {
    let total = sections.iter().map(|(_, counted)| counted.pages()).sum();
    let selection = PageSelection::resolve(ranges, total)?;
    let mut selections = Vec::with_capacity(sections.len());
    let mut destinations = HashMap::new();
    let mut offset = 0;

    for (index, (solved, counted)) in sections.iter().enumerate() {
      let section = selection.section(offset, counted.pages());

      if linked {
        let place = Destinations {
          frame: &solved.frame,
          selection: &section,
          starts: counted.starts(),
          structural,
          prefix: &[index],
        };

        for (id, anchor) in counted.anchors()? {
          if let Some(destination) = place.destination(anchor.top, &anchor.path) {
            destinations.entry(id).or_insert(destination);
          }
        }
      }
      offset += counted.pages();
      selections.push(section);
    }

    let mut plans = Vec::with_capacity(sections.len());
    let mut summary = PageSummary::new(false);
    let mut offset = 0;

    for (index, ((solved, counted), selection)) in sections.into_iter().zip(selections).enumerate()
    {
      let pages = counted.pages();
      let mut plan = PageDrawing {
        pdf: &mut *drawing.pdf,
        inputs: drawing.inputs,
        state: drawing.state,
        background: drawing.background,
        counters: PageCounters {
          offset,
          total: Some(total),
          targets: None,
        },
        destinations: Some(&destinations),
      }
      .draw(solved, counted, selection, structural, settle)?;

      plan.prefix = vec![index];
      summary.append_section(&plan.summary, index);
      offset += pages;
      plans.push(plan);
    }
    Ok(Self {
      sections: plans,
      summary,
    })
  }

  pub(crate) fn destination(&self, top: f32, path: &[usize]) -> Option<XyzDestination> {
    let (&index, path) = path.split_first()?;

    self.sections.get(index)?.destination(top, path)
  }
}
