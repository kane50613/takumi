//! The pages of a document: their geometry, and the walk that draws each.

use std::collections::HashMap;

use takumi_core::{
  context::RenderContext,
  geometry::{Point as CorePoint, Rect, Size},
  layout::{node::Node, tree::RenderNode},
  page_stream::Settle,
  scene::{SceneRef, SceneView},
  style::Color,
  viewport::{MediaTarget, Viewport},
};

use crate::{
  bands::{RepeatBounds, Repeatable, RepeatablePage},
  chunks::SceneChunks,
  emitter::{DocumentState, Emitter},
  inline::{InlineCache, TextBox},
  interactive::{AnchorTarget, PageSummary, add_link_annotations, collect_links, xyz_destination},
  krilla::{
    Document,
    destination::XyzDestination,
    geom::{Point, Size as KrillaSize, Transform},
    page::PageSettings,
    surface::Surface,
  },
  options::{PT_PER_PX, PageBand, PageOptions, PageRange, PageSelection, PdfError},
  pagination::{
    Counted, Counts, PageCounters, PageSlice, PageStarts, Paginated, Prepared, reads_layout,
    walk_pages,
  },
  paint::paint_page_background,
  tree::TreeInputs,
  window::{ContentWindow, Window},
};

/// Page geometry resolved once: everything derived from [`PageOptions`] and
/// the measured band heights.
pub(crate) struct PageFrame {
  /// Page size in px.
  pub(crate) size: Size<f32>,
  /// Page size in pt, the unit pages are written in.
  pub(crate) page_size: KrillaSize,
  pub(crate) margin: Rect<f32>,
  pub(crate) content_width: f32,
  pub(crate) window_height: f32,
  /// Full page width, unbounded height: what a band lays out against.
  pub(crate) band_viewport: Viewport,
  /// The content box, which a repeated box positions against.
  pub(crate) page_area: Viewport,
}

impl PageFrame {
  /// Resolves the geometry. Bands are measured before this so an `auto` margin
  /// can take the height they came out at; the content window is the full
  /// margin box, and a band taller than its margin overlaps content, exactly
  /// as in Chromium.
  pub(crate) fn resolve(
    page: &PageOptions,
    band_viewport: Viewport,
    header_height: Option<f32>,
    footer_height: Option<f32>,
  ) -> Result<Self, PdfError> {
    let page_size = KrillaSize::from_wh(page.width * PT_PER_PX, page.height * PT_PER_PX)
      .ok_or(PdfError::InvalidPageSize)?;
    let margin = page
      .margin
      .resolve(page.size(), header_height, footer_height);
    let Size {
      width: content_width,
      height: content_height,
    } = page.content_size(margin);

    if !(content_width.is_finite()
      && content_height.is_finite()
      && content_width > 0.0
      && content_height > 0.0)
    {
      return Err(PdfError::InvalidPageSize);
    }

    Ok(Self {
      size: page.size(),
      page_size,
      margin,
      content_width,
      window_height: content_height,
      band_viewport,
      page_area: print_viewport(
        content_width,
        Some(content_height),
        Size {
          width: content_width,
          height: content_height,
        },
      ),
    })
  }

  pub(crate) fn page_size(&self) -> Size<f32> {
    Size {
      width: self.content_width,
      height: self.window_height,
    }
  }

  /// The content column: content width, unbounded height. Viewport units
  /// take the page area, as in print media.
  pub(crate) fn column(&self) -> Viewport {
    print_viewport(
      self.content_width,
      None,
      Size {
        width: self.content_width,
        height: self.window_height,
      },
    )
  }
}

/// Full page width, unbounded height: what a band lays out against, with
/// viewport units taking the whole page.
pub(crate) fn band_viewport(page: &PageOptions) -> Viewport {
  print_viewport(page.width, None, page.size())
}

/// A print-media viewport `width` wide, `height` tall or unbounded, whose
/// viewport units resolve against `units`.
fn print_viewport(width: f32, height: Option<f32>, units: Size<f32>) -> Viewport {
  Viewport::new((width as u32, height.map(|height| height as u32)))
    .with_media_target(MediaTarget::Print)
    .with_unit_reference(units)
}

/// What a paged render takes from its options besides the content.
pub(crate) struct PageBands<'o> {
  pub(crate) header: Option<PageBand>,
  pub(crate) footer: Option<PageBand>,
  pub(crate) page_ranges: Option<&'o [PageRange]>,
}

/// The paginated document once its pages are drawn: what the outline, the structure tree and the
/// destinations read.
pub(crate) struct PagePlan {
  pub(crate) frame: PageFrame,
  pub(crate) selection: PageSelection,
  /// Whether destinations name tag-tree structure elements.
  pub(crate) structural: bool,
  pub(crate) root: RenderNode,
  pub(crate) summary: PageSummary,
  pub(crate) starts: PageStarts,
  pub(crate) prefix: Vec<usize>,
}

impl PagePlan {
  fn destinations(&self) -> Destinations<'_> {
    Destinations {
      frame: &self.frame,
      selection: &self.selection,
      starts: &self.starts,
      structural: self.structural,
      prefix: &self.prefix,
    }
  }

  /// The destination a link or outline entry jumps to, as [`Destinations::destination`] says.
  pub(crate) fn destination(&self, top: f32, path: &[usize]) -> Option<XyzDestination> {
    self.destinations().destination(top, path)
  }

  /// Where `href="#id"` lands, or `None` when nothing carries that id or its
  /// page is dropped.
  pub(crate) fn anchor_destination(&self, id: &str) -> Option<XyzDestination> {
    self.destinations().anchor(&self.summary.anchors, id)
  }
}

/// Where a position in the content lands in the output, once the pages are counted and the ones
/// kept are known.
pub(crate) struct Destinations<'p> {
  pub(crate) frame: &'p PageFrame,
  pub(crate) selection: &'p PageSelection,
  pub(crate) starts: &'p PageStarts,
  pub(crate) structural: bool,
  /// The section index the paths sit under in the structure tree.
  pub(crate) prefix: &'p [usize],
}

impl Destinations<'_> {
  /// The page `top` falls on, at its position inside that page. `None` when the page is dropped
  /// by [`crate::PdfOptions::page_ranges`].
  pub(crate) fn destination(&self, top: f32, path: &[usize]) -> Option<XyzDestination> {
    let index = self.starts.index_of(top);
    let emitted = self.selection.emitted(index)?;
    let y = self.frame.margin.top + (top - self.starts.start(index)).max(0.0);

    Some(xyz_destination(
      emitted,
      Point::from_xy(self.frame.margin.left * PT_PER_PX, y * PT_PER_PX),
      &[self.prefix, path].concat(),
      self.structural,
    ))
  }

  fn anchor(&self, anchors: &HashMap<Box<str>, AnchorTarget>, id: &str) -> Option<XyzDestination> {
    let anchor = anchors.get(id)?;

    self.destination(anchor.top, &anchor.path)
  }
}

pub(crate) struct Solved {
  pub(crate) frame: PageFrame,
  header: Option<Repeatable>,
  footer: Option<Repeatable>,
}

impl Solved {
  /// Whether a band holds a counter, so its height can follow the page count.
  fn dynamic(&self) -> bool {
    self.header.as_ref().is_some_and(Repeatable::dynamic)
      || self.footer.as_ref().is_some_and(Repeatable::dynamic)
  }
}

/// The header and footer bands of one page geometry, and the frame they leave the content.
struct BandLayout<'b, 'g> {
  inputs: &'b TreeInputs<'g>,
  page: PageOptions,
  bands: &'b PageBands<'b>,
  viewport: Viewport,
}

impl<'b, 'g> BandLayout<'b, 'g> {
  fn new(inputs: &'b TreeInputs<'g>, page: PageOptions, bands: &'b PageBands<'b>) -> Self {
    // Bands lay out at full page width and draw inside the margin areas,
    // like Chromium's print header and footer templates.
    Self {
      inputs,
      page,
      bands,
      viewport: band_viewport(&page),
    }
  }

  fn measure(&self, pages: usize) -> Result<(Option<Repeatable>, Option<Repeatable>), PdfError> {
    let band = |template: Option<&PageBand>, bounds| {
      template
        .map(|template| Repeatable::band(self.inputs, template, self.viewport, bounds, pages))
        .transpose()
    };

    Ok((
      band(self.bands.header.as_ref(), RepeatBounds::Header)?,
      band(self.bands.footer.as_ref(), RepeatBounds::Footer)?,
    ))
  }

  fn resolve(
    &self,
    header: Option<&Repeatable>,
    footer: Option<&Repeatable>,
  ) -> Result<PageFrame, PdfError> {
    PageFrame::resolve(
      &self.page,
      self.viewport,
      header.map(Repeatable::height),
      footer.map(Repeatable::height),
    )
  }

  /// The bands laid out with `pages` as the page count, and the frame around them.
  fn solve(&self, pages: usize) -> Result<Solved, PdfError> {
    let (header, footer) = self.measure(pages)?;
    let frame = self.resolve(header.as_ref(), footer.as_ref())?;

    Ok(Solved {
      frame,
      header,
      footer,
    })
  }

  /// Counts the pages `node` lays out on in the solved frame. A band with a counter and the page
  /// count depend on each other: an `auto` margin takes the band's height, the band lays out with
  /// the real page numbers, and the page count is only known once the content is laid out. The
  /// band is re-measured with the count each layout produced until its height stops moving, like
  /// the content counters in `Counted::settle`.
  fn settle(
    &self,
    solved: Solved,
    node: Node,
    settle: Settle<'_>,
    counters: PageCounters<'_>,
  ) -> Result<(Solved, Counted), PdfError> {
    let dynamic = solved.dynamic();
    let Solved {
      mut frame,
      mut header,
      mut footer,
    } = solved;
    // Only a margin that follows a band can move the frame, which is when the content lays out
    // again from the source.
    let source = (dynamic && self.page.margin.follow_bands()).then(|| node.clone());
    let mut counted = Counted::settle(node, self.inputs, &frame, settle, counters)?;

    if dynamic {
      const BAND_PASSES: usize = 3;

      for _ in 0..BAND_PASSES {
        let (next_header, next_footer) = self.measure(counters.total.unwrap_or(counted.pages()))?;
        let stable = next_header.as_ref().map(Repeatable::height)
          == header.as_ref().map(Repeatable::height)
          && next_footer.as_ref().map(Repeatable::height)
            == footer.as_ref().map(Repeatable::height);

        header = next_header;
        footer = next_footer;
        if stable {
          break;
        }

        let next_frame = self.resolve(header.as_ref(), footer.as_ref())?;

        // The content lays out the same in a frame whose margins did not move.
        if next_frame.margin == frame.margin {
          break;
        }
        let Some(source) = &source else {
          break;
        };

        frame = next_frame;
        counted = Counted::settle(source.clone(), self.inputs, &frame, settle, counters)?;
      }
    }

    Ok((
      Solved {
        frame,
        header,
        footer,
      },
      counted,
    ))
  }
}

/// What the passes that solved the frame left of the content.
enum Content {
  /// No pass was needed, so its pages draw as they lay out.
  Stream(Box<Prepared>),
  /// The passes that settled the frame and the counters.
  Counted(Counted),
}

pub(crate) struct PageDrawing<'d, 'g> {
  pub(crate) pdf: &'d mut Document,
  pub(crate) inputs: &'d TreeInputs<'g>,
  pub(crate) state: &'d DocumentState<'d>,
  pub(crate) background: Option<Color>,
  /// Where these pages sit among the document's.
  pub(crate) counters: PageCounters<'d>,
  /// Where `href="#id"` lands across every section, set when the document has sections.
  pub(crate) destinations: Option<&'d HashMap<Box<str>, XyzDestination>>,
}

impl PageDrawing<'_, '_> {
  /// Lays the content out on pages and draws the ones `bands` keeps, each as soon as `settle` says
  /// no later page paints on it. A document that reads the page count, where a box landed or where
  /// an element sits is first laid out by passes that only count, so only a counter that never
  /// settles makes it lay out to its end before the first page is drawn.
  pub(crate) fn render(
    &mut self,
    page: PageOptions,
    bands: PageBands<'_>,
    node: Node,
    structural: bool,
    settle: Settle<'_>,
  ) -> Result<PagePlan, PdfError> {
    let selection = PageSelection::new(bands.page_ranges)?;
    let (solved, content) = Self::solve(self.inputs, page, &bands, node, settle)?;

    match content {
      Content::Stream(content) => self.stream(solved, content, None, selection, structural, settle),
      Content::Counted(counted) => {
        let selection = selection.with_pages(counted.pages())?;

        self.draw(solved, counted, selection, structural, settle)
      }
    }
  }

  /// Draws the pages of content its counting passes laid out, keeping the ones `selection` does.
  pub(crate) fn draw(
    &mut self,
    solved: Solved,
    counted: Counted,
    selection: PageSelection,
    structural: bool,
    settle: Settle<'_>,
  ) -> Result<PagePlan, PdfError> {
    match counted {
      Counted::Pages { content, counts } => {
        self.stream(solved, content, Some(counts), selection, structural, settle)
      }
      Counted::Laid(paginated) => self.compose(solved, paginated, selection, structural),
    }
  }

  /// Lays the bands out, resolves the page frame around them, and, when the page count or
  /// the boxes' pages are needed first, lays the content out on pages.
  fn solve(
    inputs: &TreeInputs<'_>,
    page: PageOptions,
    bands: &PageBands<'_>,
    node: Node,
    settle: Settle<'_>,
  ) -> Result<(Solved, Content), PdfError> {
    let layout = BandLayout::new(inputs, page, bands);
    let solved = layout.solve(1)?;

    if !solved.dynamic() && !reads_layout(&node) {
      let content = Prepared::new(node, inputs, &solved.frame)?;

      return Ok((solved, Content::Stream(Box::new(content))));
    }

    let (solved, counted) = layout.settle(solved, node, settle, PageCounters::default())?;

    Ok((solved, Content::Counted(counted)))
  }

  /// Lays the bands out and counts the pages `node` lays out on, numbering them as `counters`
  /// says.
  pub(crate) fn count(
    inputs: &TreeInputs<'_>,
    page: PageOptions,
    bands: &PageBands<'_>,
    node: Node,
    settle: Settle<'_>,
    counters: PageCounters<'_>,
  ) -> Result<(Solved, Counted), PdfError> {
    let layout = BandLayout::new(inputs, page, bands);
    let solved = layout.solve(counters.total.unwrap_or(1))?;

    layout.settle(solved, node, settle, counters)
  }

  /// Lays `content` out a page at a time and draws each page, in order, once `settle` says it is
  /// settled. `counts` is what the passes that counted the pages left, when any ran.
  fn stream(
    &mut self,
    solved: Solved,
    content: Box<Prepared>,
    counts: Option<Counts>,
    mut selection: PageSelection,
    structural: bool,
    settle: Settle<'_>,
  ) -> Result<PagePlan, PdfError> {
    let Solved {
      frame,
      header,
      footer,
    } = solved;
    let Prepared {
      root,
      repeated,
      reads_summary,
    } = *content;
    let repeatables: Vec<_> = header.into_iter().chain(repeated).chain(footer).collect();
    let composer = PageComposer::new(self, &frame, &repeatables);
    let page_size = frame.page_size();
    let mut summary = PageSummary::new(false);
    let counted_pages = counts.as_ref().map(|counts| counts.starts.len());
    let destinations = self.destinations;

    let pages = walk_pages(&root, page_size, settle, |stream, index| {
      let kept = match counted_pages {
        Some(_) => selection.keeps(index),
        None => selection.push_page().is_some(),
      };

      if reads_summary {
        summary.absorb(PageSummary::of_page(
          &root,
          stream.view(),
          page_size,
          &stream.pages_of(index),
          false,
        )?);
      }
      if kept {
        composer.compose(
          self.pdf,
          &PageSlice::of(index, frame.window_height),
          counted_pages.unwrap_or_default(),
          stream.window(index)?.view(),
          |id| {
            if let Some(destinations) = destinations {
              return destinations.get(id).cloned();
            }
            let counts = counts.as_ref()?;

            Destinations {
              frame: &frame,
              selection: &selection,
              starts: &counts.starts,
              structural,
              prefix: &[],
            }
            .anchor(&counts.anchors, id)
          },
        )?;
      }
      Ok(())
    })?;
    let window = frame.window_height;

    selection.finish()?;
    Ok(PagePlan {
      frame,
      selection,
      structural,
      root,
      summary: summary.finished(),
      starts: PageStarts::new(pages, window),
      prefix: Vec::new(),
    })
  }

  fn compose(
    &mut self,
    solved: Solved,
    paginated: Box<Paginated>,
    selection: PageSelection,
    structural: bool,
  ) -> Result<PagePlan, PdfError> {
    let Solved {
      frame,
      header,
      footer,
    } = solved;
    let summary = paginated.summary()?;
    let Paginated {
      content,
      windows,
      repeated,
      starts,
      ..
    } = *paginated;
    let (page_size, pages) = (content.page_size, content.page_count);
    let (root, results) = content.into_parts();
    let repeatables: Vec<_> = header.into_iter().chain(repeated).chain(footer).collect();
    let plan = PagePlan {
      frame,
      selection,
      structural,
      root,
      summary,
      starts,
      prefix: Vec::new(),
    };
    let composer = PageComposer::new(self, &plan.frame, &repeatables);
    let destinations = self.destinations;

    for slice in plan.starts.slices() {
      if !plan.selection.keeps(slice.index) {
        continue;
      }
      let scene = SceneRef::window(
        &plan.root,
        results.view(),
        page_size,
        &windows.pages_reaching(slice.index),
        true,
      )?;

      composer.compose(
        self.pdf,
        &slice,
        pages,
        scene.view(),
        |id| match destinations {
          Some(destinations) => destinations.get(id).cloned(),
          None => plan.anchor_destination(id),
        },
      )?;
    }
    Ok(plan)
  }
}

/// Draws the paginated content one page at a time: background, the repeatables
/// under the content, the content window, the repeatables over it, then the
/// link annotations.
struct PageComposer<'c, 'g> {
  frame: &'c PageFrame,
  /// In draw order: the header band, the repeated boxes, the footer band.
  repeatables: &'c [Repeatable],
  inputs: &'c TreeInputs<'g>,
  page_context: RenderContext,
  state: &'c DocumentState<'c>,
  background: Option<Color>,
  /// How many of the document's pages come before these.
  offset: usize,
  /// The document's page count, when it has pages besides these.
  total: Option<usize>,
}

impl<'c, 'g> PageComposer<'c, 'g> {
  fn new(
    drawing: &PageDrawing<'c, 'g>,
    frame: &'c PageFrame,
    repeatables: &'c [Repeatable],
  ) -> Self {
    Self {
      frame,
      repeatables,
      inputs: drawing.inputs,
      page_context: drawing.inputs.context(frame.page_area),
      state: drawing.state,
      background: drawing.background,
      offset: drawing.counters.offset,
      total: drawing.counters.total,
    }
  }

  /// Draws one page of `pages`, the content it shows being `scene`. `anchor` says where
  /// `href="#id"` lands.
  fn compose(
    &self,
    pdf: &mut Document,
    slice: &PageSlice,
    pages: usize,
    scene: SceneView<'_>,
    anchor: impl Fn(&str) -> Option<XyzDestination>,
  ) -> Result<(), PdfError> {
    let frame = self.frame;
    let resolved = self
      .repeatables
      .iter()
      .map(|repeatable| {
        repeatable.for_page(
          self.inputs,
          &self.page_context,
          frame,
          self.offset + slice.index + 1,
          self.total.unwrap_or(pages),
        )
      })
      .collect::<Result<Vec<_>, _>>()?;
    let links = collect_links(scene);
    let text_boxes = TextBox::collect(scene);
    let inline = InlineCache::new(&text_boxes);
    let chunks = SceneChunks::new(scene);
    let mut pdf_page = pdf.start_page_with(PageSettings::new(frame.page_size));
    let mut surface = pdf_page.surface();

    surface.push_transform(&Transform::from_scale(PT_PER_PX, PT_PER_PX));
    paint_page_background(self.background, frame.size, &mut surface);
    for repeatable in resolved.iter().filter(|page| page.draws_before_content()) {
      repeatable.emit(frame, self.state, &mut surface)?;
    }
    self.emit_content(
      Emitter::new(scene, self.state, Some(&inline), Some(&chunks), true),
      slice,
      &mut surface,
    )?;
    for repeatable in resolved.iter().filter(|page| !page.draws_before_content()) {
      repeatable.emit(frame, self.state, &mut surface)?;
    }
    surface.pop();
    surface.finish();
    add_link_annotations(
      &mut pdf_page,
      &links,
      (slice.start, slice.end),
      CorePoint {
        x: frame.margin.left,
        y: frame.margin.top,
      },
      self.state.tags.as_ref(),
      &anchor,
    );
    // A repeated box sits at the same place on every page, so its links are
    // added per page against the page area rather than the content window.
    // The box itself is an artifact, and its paths do not exist in the tag
    // tree, so the annotations stay out of the structure too.
    add_link_annotations(
      &mut pdf_page,
      resolved.iter().flat_map(RepeatablePage::links),
      (0.0, frame.window_height),
      CorePoint {
        x: frame.margin.left,
        y: frame.margin.top,
      },
      None,
      &anchor,
    );
    pdf_page.finish();
    Ok(())
  }

  /// Emits the content through this page's window: clipped to the page area and translated so
  /// the page lands at the content origin.
  fn emit_content(
    &self,
    emitter: Emitter<'_>,
    slice: &PageSlice,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    let frame = self.frame;

    ContentWindow {
      clip: (
        frame.margin.left,
        frame.margin.top,
        frame.content_width,
        frame.window_height,
      ),
      translate: CorePoint {
        x: frame.margin.left,
        y: frame.margin.top - slice.start,
      },
      window: Window {
        y: Some((slice.start, slice.end)),
        lines: Some((slice.start, slice.end)),
      },
      artifact: false,
    }
    .emit(emitter, surface)
  }
}
