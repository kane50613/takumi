//! The content laid out across pages, which takumi-core's block fragmentation breaks, and the
//! page counters filled in from where it put each box.

use takumi_core::{layout::node::Node, scene::PageWindows};

use crate::{
  bands::Repeatable,
  counters::{
    BoxPages, FlowCounters, has_page_counters, has_target_counters, substitute_target_counters,
  },
  interactive::PageSummary,
  options::PdfError,
  page::PageFrame,
  tree::{PagedContent, TreeInputs},
};

/// How many pages one render may lay its content out on. A document tall enough to pass this is
/// not a document anyone reads; it is a way to spend a renderer's memory.
pub(crate) const MAX_PAGES: usize = 20_000;

/// How many times pagination reruns to settle page counters. A counter widens
/// the line it sits on, which can move a break, which can renumber the counter.
/// The second pass numbers a laid-out document, the third confirms the numbers
/// did not move, and a document still moving after that keeps the last pass's
/// numbers.
const COUNTER_PASSES: usize = 3;

/// The content laid out across pages, with the `fixed` boxes that repeat on every page and the
/// interactive targets the pages draw from. The pages stack in the content's coordinates, each the
/// page area tall.
pub(crate) struct Paginated {
  pub(crate) content: PagedContent,
  /// Which of the content's root fragments paint on each page.
  pub(crate) windows: PageWindows,
  pub(crate) repeated: Vec<Repeatable>,
  pub(crate) starts: PageStarts,
  /// Present when page counters read it.
  summary: Option<PageSummary>,
}

/// One page's window into the content.
pub(crate) struct PageSlice {
  /// 0-based; display page numbers are `index + 1`.
  pub(crate) index: usize,
  pub(crate) start: f32,
  /// Where the next page begins.
  pub(crate) end: f32,
}

impl PageSlice {
  /// The window of page `index` when each page is `window` tall.
  pub(crate) fn of(index: usize, window: f32) -> Self {
    let start = index as f32 * window;

    Self {
      index,
      start,
      end: start + window,
    }
  }
}

/// Where each page of the content begins.
pub(crate) struct PageStarts {
  starts: Vec<f32>,
  window: f32,
}

impl PageStarts {
  pub(crate) fn new(pages: usize, window: f32) -> Self {
    Self {
      starts: (0..pages)
        .map(|page| PageSlice::of(page, window).start)
        .collect(),
      window,
    }
  }

  pub(crate) fn len(&self) -> usize {
    self.starts.len()
  }

  pub(crate) fn start(&self, index: usize) -> f32 {
    self.starts[index]
  }

  /// 0-based index of the page `top` falls on.
  pub(crate) fn index_of(&self, top: f32) -> usize {
    self
      .starts
      .partition_point(|start| *start <= top)
      .saturating_sub(1)
  }

  pub(crate) fn slices(&self) -> impl Iterator<Item = PageSlice> {
    (0..self.starts.len()).map(|index| PageSlice::of(index, self.window))
  }
}

impl Paginated {
  /// Lays the content out on pages, refilling the counters it
  /// holds until the numbers stop moving. A counter inside a repeated box is
  /// left to that box, which lays out again per page with its own numbers.
  pub(crate) fn build(
    mut node: Node,
    inputs: &TreeInputs<'_>,
    frame: &PageFrame,
  ) -> Result<Self, PdfError> {
    if !has_page_counters(&node) && !has_target_counters(&node) {
      return Self::lay_out(node, inputs, frame);
    }
    let mut previous: Vec<String> = Vec::new();
    let mut passes = 0;

    loop {
      let mut paginated = Self::lay_out(node.clone(), inputs, frame)?;
      let summary = paginated.summarize(true)?;
      let mut written = Vec::new();

      passes += 1;
      paginated.substitute_counters(&summary, &mut node, &mut written);
      if written == previous {
        paginated.summary = Some(summary);
        return Ok(paginated);
      }
      previous = written;
      // The numbers are still moving, and this was the last pass allowed. They
      // are laid out once more so the page shows the numbers it was laid out with.
      if passes == COUNTER_PASSES {
        return Self::lay_out(node, inputs, frame);
      }
    }
  }

  /// One layout of `node`.
  fn lay_out(node: Node, inputs: &TreeInputs<'_>, frame: &PageFrame) -> Result<Self, PdfError> {
    let (content, repeated) = inputs.prepare_paged(node, frame)?;
    let starts = PageStarts::new(content.page_count, frame.window_height);
    let mut windows = PageWindows::new(content.page_size);

    windows.refresh(
      &content.root,
      content.layout(),
      content.page_count,
      content.page_count,
    )?;

    Ok(Self {
      content,
      windows,
      repeated,
      starts,
      summary: None,
    })
  }

  /// What the pages' interactive targets and, when `extents` says so, box extents sum up to.
  fn summarize(&self, extents: bool) -> Result<PageSummary, PdfError> {
    let content = &self.content;

    PageSummary::of_pages(
      &content.root,
      content.layout(),
      content.page_size,
      &self.windows,
      extents,
    )
  }

  /// The summary of the pages, taken from the pass that counted them or made now.
  pub(crate) fn summary(&mut self) -> Result<PageSummary, PdfError> {
    match self.summary.take() {
      Some(summary) => Ok(summary),
      None => self.summarize(false),
    }
  }

  /// Fills both kinds of counter from this laid-out pass, collecting what it
  /// wrote so the caller can see whether the numbers moved.
  fn substitute_counters(&self, summary: &PageSummary, node: &mut Node, written: &mut Vec<String>) {
    let pages = self.starts.len();
    let page_at = |top: &f32| self.starts.index_of(*top) + 1;
    // `fill_root` wraps the tree in a page-wide box, which takes preorder 0.
    let page_of = summary
      .extents
      .iter()
      .flatten()
      .filter_map(|(order, extent)| {
        let pages = BoxPages {
          start: page_at(&extent.top),
          next: extent.flow_bottom.as_ref().map(page_at),
        };

        Some((order.checked_sub(1)?, pages))
      })
      .collect();
    let repeated: Vec<_> = self
      .repeated
      .iter()
      .filter_map(Repeatable::source_orders)
      .map(|orders| orders.start.saturating_sub(1)..orders.end.saturating_sub(1))
      .collect();

    FlowCounters::new(&page_of, pages, &repeated).substitute(node, None, written);

    let target_page = |id: &str| summary.anchors.get(id).map(|anchor| page_at(&anchor.top));

    substitute_target_counters(node, None, &target_page, written);
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use takumi_core::scene::PagedScene;
  use takumi_html::{FromHtmlOptions, from_html};

  use super::*;
  use crate::{
    interactive::Interactive,
    options::{PageMargins, PageOptions},
    page::{PageFrame, band_viewport},
    test_support::{fixture, fonts},
    tree::PreparedTree,
  };

  /// Documents with the targets a summary reads in the places a page's root fragment shares with
  /// the pages around it.
  fn synthetic_documents() -> Vec<(&'static str, String)> {
    let rows = |count: usize| {
      (1..=count)
        .map(|row| format!("<tr><td id=\"row-{row}\">Row {row}</td><td>x</td></tr>"))
        .collect::<String>()
    };
    let lines = |count: usize| {
      (1..=count)
        .map(|line| format!("<div>Line {line}</div>"))
        .collect::<String>()
    };

    vec![
      (
        "headings and ids",
        format!(
          r#"<div style="display: flex; flex-direction: column; font-size: 14px;">
            <h1 id="top">Top <span>nested</span></h1>{}
            <h2 id="two">Two</h2>{}
            <h3><b>Bold</b> three</h3>{}
            <h2 id="two">A second element with the same id</h2>{}
          </div>"#,
          lines(12),
          lines(12),
          lines(12),
          lines(12),
        ),
      ),
      (
        "a table with a repeated header",
        format!(
          r#"<table style="width: 100%; font-size: 14px;"><thead><tr><th id="head">Name</th><th>Value</th></tr></thead><tbody>{}</tbody></table>"#,
          rows(60)
        ),
      ),
      (
        "positioned boxes",
        format!(
          r#"<div style="position: relative; display: flex; flex-direction: column; font-size: 14px;">
            {}
            <div id="held" style="position: absolute; bottom: 0; left: 0; height: 30px;">Held</div>
            <div id="shifted" style="position: relative; top: 30px;"><h2>Shifted</h2></div>
            {}
          </div>"#,
          lines(30),
          lines(30),
        ),
      ),
      (
        "a positioned box in a flex container",
        format!(
          r#"<div style="display: flex; flex-direction: column; font-size: 14px;">
            <div style="position: relative; display: flex; flex-direction: column;">
              {}
              <div id="oof" style="position: absolute; top: 20px; left: 0;"><h2>Out of flow</h2>{}</div>
            </div>
            {}
          </div>"#,
          lines(25),
          lines(25),
          lines(10),
        ),
      ),
      (
        "a transform",
        format!(
          r#"<div style="font-size: 14px;">{}<h2 id="moved" style="transform: translateY(-300px);">Moved</h2>{}</div>"#,
          lines(20),
          lines(20),
        ),
      ),
    ]
  }

  /// The summary of the scene of every page of `node`, which the pages' summaries replace.
  fn whole_scene_summary(node: Node, inputs: &TreeInputs<'_>, frame: &PageFrame) -> PageSummary {
    let (root, _) = inputs.prepare_root(node, frame).expect("the tree prepares");
    let paged =
      PagedScene::lay_out(root, frame.page_size(), MAX_PAGES, true).expect("the pages lay out");

    Interactive::collect_with_extents(&PreparedTree {
      scene: paged.into_scene(),
    })
    .summary
  }

  /// A paged document's page summaries add up to the summary a scene of every page gives.
  #[test]
  fn page_summaries_add_up_to_the_summary_of_the_whole_scene() {
    let fonts = fonts();
    let inputs = TreeInputs {
      fonts: &fonts,
      stylesheet: Default::default(),
      images: Rc::default(),
      font_families: None,
      lang: None,
    };
    let mut documents = synthetic_documents();

    for name in [
      "certificate.html",
      "invoice-footer.html",
      "invoice.html",
      "report.html",
    ] {
      documents.push((name, fixture(name)));
    }

    let mut with_targets_across_pages = 0;

    for (name, source) in documents {
      let node = from_html(&source, FromHtmlOptions::default()).expect("parse the document");

      for (width, height) in [
        (523.0, 770.0),
        (400.0, 300.0),
        (300.0, 120.0),
        (250.0, 90.0),
      ] {
        let page = PageOptions {
          width,
          height,
          margin: PageMargins::uniform(12.0),
        };
        let frame = PageFrame::resolve(&page, band_viewport(&page), None, None)
          .expect("the page has an area");
        let paginated = Paginated::lay_out(node.clone(), &inputs, &frame)
          .unwrap_or_else(|_| panic!("{name} lays out on {width}x{height}"));
        let summary = paginated.summarize(true).expect("the pages summarize");
        let whole = whole_scene_summary(node.clone(), &inputs, &frame);

        assert_eq!(
          summary,
          whole,
          "{name} on {width}x{height} pages ({} pages)",
          paginated.starts.len()
        );
        if paginated.starts.len() > 1 && !summary.anchors.is_empty() && !summary.headings.is_empty()
        {
          with_targets_across_pages += 1;
        }
      }
    }
    assert!(
      with_targets_across_pages >= 12,
      "only {with_targets_across_pages} layouts had targets on several pages"
    );
  }
}
