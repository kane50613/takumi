//! The content laid out across pages, which takumi-core's block fragmentation breaks, and the
//! page counters filled in from where it put each box.

use std::collections::HashMap;

use takumi_core::{
  layout::{
    node::Node,
    page_sizes::{PageSizer, PageSizes},
    tree::RenderNode,
  },
  page_stream::{PageStream, Settle, Settled},
  scene::PageWindows,
};

use crate::{
  bands::{Repeatable, RepeatedBoxes},
  counters::{
    BoxPages, FlowCounters, has_page_counters, has_target_counters, substitute_target_counters,
  },
  interactive::{AnchorTarget, PageSummary, has_internal_links, has_summary_targets},
  options::PdfError,
  page::PageFrames,
  tree::{PagedContent, TreeInputs},
};

/// How many pages one render may lay its content out on. A document tall enough to pass this is
/// not a document anyone reads; it is a way to spend a renderer's memory.
pub(crate) const MAX_PAGES: usize = 20_000;

/// How many passes pagination takes to settle page counters, which can move breaks.
const COUNTER_PASSES: usize = 3;

/// Whether laying a tree out reads numbers the layout itself produces: a page counter, or the page
/// a link or counter names.
pub(crate) fn reads_layout(node: &Node) -> bool {
  has_page_counters(node) || has_target_counters(node) || has_internal_links(node)
}

/// Lays `root` out a page at a time and calls `visit` with the stream and each page once `settle`
/// says it is settled. Returns the pages the layout came to.
pub(crate) fn walk_pages(
  root: &RenderNode,
  sizer: PageSizer,
  settle: Settle<'_>,
  mut visit: impl FnMut(&PageStream<'_>, usize) -> Result<(), PdfError>,
) -> Result<PageSizes, PdfError> {
  let mut stream = PageStream::new(root, sizer, MAX_PAGES);

  loop {
    let Settled { pages, laid } = stream.advance(settle)?;

    for index in pages {
      visit(&stream, index)?;
    }
    if !laid {
      return Ok(stream.sizes().clone());
    }
  }
}

/// The content tree of a frame, which lays out across pages as many times as the document needs.
pub(crate) struct Prepared {
  pub(crate) root: RenderNode,
  /// The `fixed` boxes taken out of the root, which repeat on every page.
  pub(crate) repeated: RepeatedBoxes,
  /// Whether the tree holds an anchor or a heading, which its pages are summarized for.
  pub(crate) reads_summary: bool,
}

impl Prepared {
  pub(crate) fn new(
    node: Node,
    inputs: &TreeInputs<'_>,
    frames: &PageFrames,
  ) -> Result<Self, PdfError> {
    let reads_summary = has_summary_targets(&node);
    let (root, repeated) = inputs.prepare_root(node, frames)?;

    Ok(Self {
      root,
      repeated,
      reads_summary,
    })
  }
}

/// What the passes that count a document leave for drawing it.
pub(crate) struct Counts {
  pub(crate) sizes: PageSizes,
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
  /// Counts `node`'s pages until its counters settle, and finds the elements its links name.
  pub(crate) fn settle(
    mut node: Node,
    inputs: &TreeInputs<'_>,
    frames: &PageFrames,
    settle: Settle<'_>,
  ) -> Result<Self, PdfError> {
    let links = has_internal_links(&node);
    let collect = Collect::of(&node, links);

    let numbered = collect == Collect::Extents || has_target_counters(&node);

    if !numbered {
      return Ok(CountingPass::run(node, inputs, frames, settle, collect)?.into_counted(links));
    }

    let mut previous: Vec<String> = Vec::new();

    for _ in 0..COUNTER_PASSES {
      let pass = CountingPass::run(node.clone(), inputs, frames, settle, collect)?;
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
      node, inputs, frames,
    )?)))
  }

  pub(crate) fn pages(&self) -> usize {
    match self {
      Self::Pages { counts, .. } => counts.sizes.len(),
      Self::Laid(paginated) => paginated.content.sizes.len(),
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
  sizes: PageSizes,
  /// Absent when nothing reads the boxes.
  summary: Option<PageSummary>,
}

impl CountingPass {
  fn run(
    node: Node,
    inputs: &TreeInputs<'_>,
    frames: &PageFrames,
    settle: Settle<'_>,
    collect: Collect,
  ) -> Result<Self, PdfError> {
    let content = Prepared::new(node, inputs, frames)?;
    let extents = collect == Collect::Extents;
    let mut summary = (collect != Collect::Nothing).then(|| PageSummary::new(extents));
    let sizes = walk_pages(
      &content.root,
      frames.page_sizer(),
      settle,
      |stream, index| {
        if let Some(summary) = &mut summary {
          summary.absorb(PageSummary::of_page(
            &content.root,
            stream.view(),
            stream.sizes(),
            &stream.pages_of(index),
            extents,
          )?);
        }
        Ok(())
      },
    )?;

    Ok(Self {
      content,
      sizes,
      summary: summary.map(PageSummary::finished),
    })
  }

  /// What drawing the pages needs, with the anchors only when a link reads them.
  fn into_counted(self, links: bool) -> Counted {
    Counted::Pages {
      content: Box::new(self.content),
      counts: Counts {
        sizes: self.sizes,
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
    let pages = self.sizes.len();
    let page_at = |top: &f32| self.sizes.page_at(*top) + 1;
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
      .get(0)
      .iter()
      .filter_map(Repeatable::source_orders)
      .map(|orders| orders.start.saturating_sub(1)..orders.end.saturating_sub(1))
      .collect();

    FlowCounters::new(&page_of, pages, &repeated).substitute(node, None, written);

    let target_page = |id: &str| summary.anchors.get(id).map(|anchor| page_at(&anchor.top));

    substitute_target_counters(node, None, &target_page, written);
  }
}

/// The content laid out across pages, with its repeated `fixed` boxes and link targets.
pub(crate) struct Paginated {
  pub(crate) content: PagedContent,
  /// Which of the content's root fragments paint on each page.
  pub(crate) windows: PageWindows,
  pub(crate) repeated: RepeatedBoxes,
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
  pub(crate) fn of(index: usize, sizes: &PageSizes) -> Self {
    let start = sizes.top(index);

    Self {
      index,
      start,
      end: start + sizes.size(index).height,
    }
  }

  /// Every page of `sizes`, in order.
  pub(crate) fn all(sizes: &PageSizes) -> impl Iterator<Item = Self> {
    (0..sizes.len()).map(|index| Self::of(index, sizes))
  }
}

impl Paginated {
  fn lay_out(node: Node, inputs: &TreeInputs<'_>, frames: &PageFrames) -> Result<Self, PdfError> {
    let (content, repeated) = inputs.prepare_paged(node, frames)?;
    let mut windows = PageWindows::default();

    windows.refresh(
      &content.root,
      content.layout(),
      &content.sizes,
      content.sizes.len(),
    )?;

    Ok(Self {
      content,
      windows,
      repeated,
    })
  }

  fn summarize(&self, extents: bool) -> Result<PageSummary, PdfError> {
    let content = &self.content;

    PageSummary::of_pages(
      &content.root,
      content.layout(),
      &content.sizes,
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

  use takumi_core::html::{FromHtmlOptions, from_html};
  use takumi_core::scene::PagedScene;

  use super::*;
  use crate::{
    options::{PageMargins, PageOptions},
    page::{PageFrame, PageFrames},
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
  fn whole_scene_summary(node: Node, inputs: &TreeInputs<'_>, frames: &PageFrames) -> PageSummary {
    let (root, _) = inputs
      .prepare_root(node, frames)
      .expect("the tree prepares");
    let paged =
      PagedScene::lay_out(root, frames.page_sizer(), MAX_PAGES, true).expect("the pages lay out");

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
      resource_cache: Default::default(),
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
        let frames = PageFrames::uniform(
          PageFrame::resolve(&page, (None, None)).expect("the page has an area"),
        );
        let paginated = Paginated::lay_out(node.clone(), &inputs, &frames)
          .unwrap_or_else(|_| panic!("{name} lays out on {width}x{height}"));
        let summary = paginated.summarize(true).expect("the pages summarize");
        let whole = whole_scene_summary(node.clone(), &inputs, &frames);

        assert_eq!(
          summary,
          whole,
          "{name} on {width}x{height} pages ({} pages)",
          paginated.content.sizes.len()
        );
        if paginated.content.sizes.len() > 1
          && !summary.anchors.is_empty()
          && !summary.headings.is_empty()
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
  /// The first, left, and right pages each take the height of their own frame, and the pages'
  /// summaries still add up to the summary of the whole scene.
  #[test]
  fn first_left_and_right_frames_size_their_own_pages() {
    let fonts = fonts();
    let inputs = TreeInputs {
      fonts: &fonts,
      stylesheet: Default::default(),
      images: Rc::default(),
      font_families: None,
      lang: None,
    };
    let frame = |height| {
      PageFrame::resolve(
        &PageOptions {
          width: 300.0,
          height,
          margin: PageMargins::uniform(10.0),
        },
        (None, None),
      )
      .expect("the page has an area")
    };
    let frames = PageFrames::new(frame(220.0), frame(140.0), frame(180.0));
    let source = (1..=40)
      .map(|line| format!(r#"<div id="line-{line}" style="height: 20px">Line {line}</div>"#))
      .collect::<String>();
    let node = from_html(&source, FromHtmlOptions::default()).expect("parse the document");
    let paginated = Paginated::lay_out(node.clone(), &inputs, &frames).expect("the pages lay out");
    let slices: Vec<_> = PageSlice::all(&paginated.content.sizes)
      .map(|slice| (slice.start, slice.end))
      .collect();

    assert_eq!(
      slices[..5],
      [
        (0.0, 200.0),
        (200.0, 320.0),
        (320.0, 480.0),
        (480.0, 600.0),
        (600.0, 760.0)
      ]
    );
    assert_eq!(
      paginated.summarize(true).expect("the pages summarize"),
      whole_scene_summary(node, &inputs, &frames)
    );
  }
}
