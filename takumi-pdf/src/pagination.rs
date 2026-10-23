//! The content laid out across pages, which takumi-core's block fragmentation breaks, and the
//! page counters filled in from where it put each box.

use std::collections::HashMap;

use takumi_core::{
  geometry::Size,
  layout::{node::Node, tree::RenderNode},
  page_stream::{PageStream, Settle, Settled},
  scene::PageWindows,
};

use crate::{
  bands::Repeatable,
  counters::{
    BoxPages, FlowCounters, has_page_counters, has_target_counters, substitute_target_counters,
  },
  interactive::{AnchorTarget, PageSummary, has_internal_links, has_summary_targets},
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

/// Whether laying a tree out reads numbers the layout itself produces: a page counter, or the page
/// a link or counter names.
pub(crate) fn reads_layout(node: &Node) -> bool {
  has_page_counters(node) || has_target_counters(node) || has_internal_links(node)
}

/// Lays `root` out a page at a time and calls `visit` with the stream and each page once `settle`
/// says it is settled. Returns how many pages the layout came to.
pub(crate) fn walk_pages(
  root: &RenderNode,
  page_size: Size<f32>,
  settle: Settle<'_>,
  mut visit: impl FnMut(&PageStream<'_>, usize) -> Result<(), PdfError>,
) -> Result<usize, PdfError> {
  let mut stream = PageStream::new(root, page_size, MAX_PAGES);

  loop {
    let Settled { pages, laid } = stream.advance(settle)?;

    for index in pages {
      visit(&stream, index)?;
    }
    if !laid {
      return Ok(stream.page_count());
    }
  }
}

/// The content tree of a frame, which lays out across pages as many times as the document needs.
pub(crate) struct Prepared {
  pub(crate) root: RenderNode,
  /// The `fixed` boxes taken out of the root, which repeat on every page.
  pub(crate) repeated: Vec<Repeatable>,
  /// Whether the tree holds an anchor or a heading, which its pages are summarized for.
  pub(crate) reads_summary: bool,
}

impl Prepared {
  pub(crate) fn new(
    node: Node,
    inputs: &TreeInputs<'_>,
    frame: &PageFrame,
  ) -> Result<Self, PdfError> {
    let reads_summary = has_summary_targets(&node);
    let (root, repeated) = inputs.prepare_root(node, frame)?;

    Ok(Self {
      root,
      repeated,
      reads_summary,
    })
  }
}

/// What the passes that count a document leave for drawing it.
pub(crate) struct Counts {
  pub(crate) starts: PageStarts,
  /// Where the elements with an `id` are, when a link to one reads them.
  pub(crate) anchors: HashMap<Box<str>, AnchorTarget>,
}

/// The content once the passes that count its pages are done.
pub(crate) enum Counted {
  /// Laid out a page at a time by passes that freed each page as they went, and laid out once
  /// more from the tree the last pass built as the pages are drawn.
  Pages {
    content: Box<Prepared>,
    counts: Counts,
  },
  /// Laid out to its end, because the numbers it holds kept moving.
  Laid(Box<Paginated>),
}

impl Counted {
  /// Counts the pages `node` lays out on in `frame`, refilling the page counters it holds until
  /// the numbers stop moving, and finds the elements its links name. Each pass frees the pages
  /// `settle` says are settled. A counter inside a repeated box is left to that box, which lays
  /// out again per page with its own numbers.
  pub(crate) fn settle(
    mut node: Node,
    inputs: &TreeInputs<'_>,
    frame: &PageFrame,
    settle: Settle<'_>,
  ) -> Result<Self, PdfError> {
    let links = has_internal_links(&node);
    let collect = Collect::of(&node, links);

    let numbered = collect == Collect::Extents || has_target_counters(&node);

    if !numbered {
      return Ok(CountingPass::run(node, inputs, frame, settle, collect)?.into_counted(links));
    }

    let mut previous: Vec<String> = Vec::new();

    for _ in 0..COUNTER_PASSES {
      let pass = CountingPass::run(node.clone(), inputs, frame, settle, collect)?;
      let mut written = Vec::new();

      pass.substitute_counters(&mut node, &mut written);
      if written == previous {
        // The pass was laid out with the numbers it wrote.
        return Ok(pass.into_counted(links));
      }
      previous = written;
    }
    // The numbers are still moving, and this was the last pass allowed. They are laid out once
    // more so the page shows the numbers it was laid out with.
    Ok(Self::Laid(Box::new(Paginated::lay_out(
      node, inputs, frame,
    )?)))
  }

  pub(crate) fn pages(&self) -> usize {
    match self {
      Self::Pages { counts, .. } => counts.starts.len(),
      Self::Laid(paginated) => paginated.starts.len(),
    }
  }
}

/// What a counting pass keeps of the boxes it lays out, beyond the page count.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Collect {
  Nothing,
  /// The elements with an `id`, which a link or a target counter names.
  Anchors,
  /// Also where every box landed, which a page counter reads.
  Extents,
}

impl Collect {
  fn of(node: &Node, links: bool) -> Self {
    if has_page_counters(node) {
      Self::Extents
    } else if links || has_target_counters(node) {
      Self::Anchors
    } else {
      Self::Nothing
    }
  }
}

/// A layout run a page at a time that draws nothing, and keeps the pages and the boxes' extents
/// the counters are filled from.
struct CountingPass {
  content: Prepared,
  starts: PageStarts,
  /// Absent when nothing reads the boxes.
  summary: Option<PageSummary>,
}

impl CountingPass {
  fn run(
    node: Node,
    inputs: &TreeInputs<'_>,
    frame: &PageFrame,
    settle: Settle<'_>,
    collect: Collect,
  ) -> Result<Self, PdfError> {
    let content = Prepared::new(node, inputs, frame)?;
    let page_size = frame.page_size();
    let extents = collect == Collect::Extents;
    let mut summary = (collect != Collect::Nothing).then(|| PageSummary::new(extents));
    let pages = walk_pages(&content.root, page_size, settle, |stream, index| {
      if let Some(summary) = &mut summary {
        summary.absorb(PageSummary::of_page(
          &content.root,
          stream.view(),
          page_size,
          &stream.pages_of(index),
          extents,
        )?);
      }
      Ok(())
    })?;

    Ok(Self {
      content,
      starts: PageStarts::new(pages, frame.window_height),
      summary: summary.map(PageSummary::finished),
    })
  }

  /// What drawing the pages needs, with the anchors only when a link reads them.
  fn into_counted(self, links: bool) -> Counted {
    Counted::Pages {
      content: Box::new(self.content),
      counts: Counts {
        starts: self.starts,
        anchors: self
          .summary
          .filter(|_| links)
          .map(|summary| summary.anchors)
          .unwrap_or_default(),
      },
    }
  }

  /// Fills both kinds of counter from this pass, collecting what it wrote so the caller can see
  /// whether the numbers moved.
  fn substitute_counters(&self, node: &mut Node, written: &mut Vec<String>) {
    let Some(summary) = &self.summary else {
      return;
    };
    let pages = self.starts.len();
    let page_at = |top: &f32| self.starts.index_of(*top) + 1;
    let page_of = summary
      .extents
      .iter()
      .flatten()
      .filter_map(|(order, extent)| {
        let pages = BoxPages {
          start: page_at(&extent.top),
          next: extent.flow_bottom.as_ref().map(page_at),
        };

        // `fill_root` wraps the tree in a page-wide box, which takes preorder 0.
        Some((order.checked_sub(1)?, pages))
      })
      .collect();
    let repeated: Vec<_> = self
      .content
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

/// The content laid out across pages, with the `fixed` boxes that repeat on every page and the
/// interactive targets the pages draw from. The pages stack in the content's coordinates, each the
/// page area tall.
pub(crate) struct Paginated {
  pub(crate) content: PagedContent,
  /// Which of the content's root fragments paint on each page.
  pub(crate) windows: PageWindows,
  pub(crate) repeated: Vec<Repeatable>,
  pub(crate) starts: PageStarts,
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
  pub(crate) fn of(index: usize, window: f32) -> Self {
    let start = index as f32 * window;

    Self {
      index,
      start,
      end: start + window,
    }
  }
}

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
    })
  }

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

  pub(crate) fn summary(&self) -> Result<PageSummary, PdfError> {
    self.summarize(false)
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use takumi_core::scene::PagedScene;
  use takumi_html::{FromHtmlOptions, from_html};

  use super::*;
  use crate::{
    options::{PageMargins, PageOptions},
    page::{PageFrame, band_viewport},
    test_support::{fixture, fonts},
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

    let scene = paged.into_scene();

    PageSummary::collect(scene.view(), true).finished()
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
