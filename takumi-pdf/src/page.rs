//! The pages of a document: their geometry, and the walk that draws each.

use takumi_core::{
  context::RenderContext,
  geometry::{Point as CorePoint, Rect, Size},
  layout::{node::Node, tree::RenderNode},
  page_stream::{PageStream, Settle, Settled},
  scene::{SceneRef, SceneView},
  style::Color,
  viewport::{MediaTarget, Viewport},
};

use crate::{
  bands::{RepeatBounds, Repeatable, RepeatablePage},
  chunks::SceneChunks,
  counters::{has_page_counters, has_target_counters},
  emitter::{DocumentState, Emitter},
  inline::{InlineCache, TextBox},
  interactive::{
    PageSummary, add_link_annotations, collect_links, has_internal_links, has_summary_targets,
    xyz_destination,
  },
  krilla::{
    Document,
    destination::XyzDestination,
    geom::{Point, Size as KrillaSize, Transform},
    page::PageSettings,
    surface::Surface,
  },
  options::{PT_PER_PX, PageBand, PageOptions, PageRange, PageSelection, PdfError},
  pagination::{MAX_PAGES, PageSlice, PageStarts, Paginated},
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
}

impl PagePlan {
  /// The destination a link or outline entry jumps to: the page `top` falls
  /// on, at its position inside that page. `None` when the page is dropped by
  /// [`crate::PdfOptions::page_ranges`].
  pub(crate) fn destination(&self, top: f32, path: &[usize]) -> Option<XyzDestination> {
    let index = self.starts.index_of(top);
    let emitted = self.selection.emitted(index)?;
    let y = self.frame.margin.top + (top - self.starts.start(index)).max(0.0);

    Some(xyz_destination(
      emitted,
      Point::from_xy(self.frame.margin.left * PT_PER_PX, y * PT_PER_PX),
      path,
      self.structural,
    ))
  }

  /// Where `href="#id"` lands, or `None` when nothing carries that id or its
  /// page is dropped.
  pub(crate) fn anchor_destination(&self, id: &str) -> Option<XyzDestination> {
    let anchor = self.summary.anchors.get(id)?;

    self.destination(anchor.top, &anchor.path)
  }
}

struct Solved {
  frame: PageFrame,
  header: Option<Repeatable>,
  footer: Option<Repeatable>,
}

/// What the passes that solved the frame left of the content.
enum Content {
  /// Nothing is laid out: no pass was needed, so its pages draw as they lay out.
  Stream(Node),
  /// The pass that settled the frame and the counters, laid out to its end.
  Laid(Box<Paginated>),
}

pub(crate) struct PageDrawing<'d, 'g> {
  pub(crate) pdf: &'d mut Document,
  pub(crate) inputs: &'d TreeInputs<'g>,
  pub(crate) state: &'d DocumentState<'d>,
  pub(crate) background: Option<Color>,
}

impl PageDrawing<'_, '_> {
  /// Lays the content out on pages and draws the ones `bands` keeps. A document that reads neither
  /// the page count nor where a box landed draws each page as soon as `settle` says no later page
  /// paints on it.
  pub(crate) fn render(
    &mut self,
    page: PageOptions,
    bands: PageBands<'_>,
    node: Node,
    structural: bool,
    settle: Settle<'_>,
  ) -> Result<PagePlan, PdfError> {
    let ranges = bands.page_ranges;
    let (solved, content) = Self::solve(self.inputs, page, &bands, node)?;

    match content {
      Content::Stream(node) => self.stream(solved, node, ranges, structural, settle),
      Content::Laid(paginated) => self.compose(solved, paginated, ranges, structural),
    }
  }

  /// Lays the bands out, resolves the page frame around them, and, when the page count or
  /// the boxes' pages are needed first, lays the content out on pages.
  fn solve(
    inputs: &TreeInputs<'_>,
    page: PageOptions,
    bands: &PageBands<'_>,
    node: Node,
  ) -> Result<(Solved, Content), PdfError> {
    // Bands lay out at full page width and draw inside the margin areas,
    // like Chromium's print header and footer templates.
    let band_viewport = band_viewport(&page);
    let measure_bands =
      |pages: usize| -> Result<(Option<Repeatable>, Option<Repeatable>), PdfError> {
        let band = |template: Option<&PageBand>, bounds| {
          template
            .map(|template| Repeatable::band(inputs, template, band_viewport, bounds, pages))
            .transpose()
        };

        Ok((
          band(bands.header.as_ref(), RepeatBounds::Header)?,
          band(bands.footer.as_ref(), RepeatBounds::Footer)?,
        ))
      };
    let resolve = |header: Option<&Repeatable>, footer: Option<&Repeatable>| {
      PageFrame::resolve(
        &page,
        band_viewport,
        header.map(Repeatable::height),
        footer.map(Repeatable::height),
      )
    };
    let (mut header, mut footer) = measure_bands(1)?;
    let mut frame = resolve(header.as_ref(), footer.as_ref())?;
    // A band with a counter and the page count depend on each other: an `auto`
    // margin takes the band's height, the band lays out with the real page
    // numbers, and the page count is only known once the content is laid out.
    // The band is re-measured with the count each layout produced until its height
    // stops moving, like the content counters in `Paginated::build`.
    let dynamic = header.as_ref().is_some_and(Repeatable::dynamic)
      || footer.as_ref().is_some_and(Repeatable::dynamic);

    if !dynamic
      && !has_page_counters(&node)
      && !has_target_counters(&node)
      && !has_internal_links(&node)
    {
      return Ok((
        Solved {
          frame,
          header,
          footer,
        },
        Content::Stream(node),
      ));
    }

    let source = dynamic.then(|| node.clone());
    let mut paginated = Paginated::build(node, inputs, &frame)?;

    if let Some(source) = source {
      const BAND_PASSES: usize = 3;

      for _ in 0..BAND_PASSES {
        let (next_header, next_footer) = measure_bands(paginated.starts.len())?;
        let stable = next_header.as_ref().map(Repeatable::height)
          == header.as_ref().map(Repeatable::height)
          && next_footer.as_ref().map(Repeatable::height)
            == footer.as_ref().map(Repeatable::height);

        header = next_header;
        footer = next_footer;
        if stable {
          break;
        }
        frame = resolve(header.as_ref(), footer.as_ref())?;
        paginated = Paginated::build(source.clone(), inputs, &frame)?;
      }
    }

    Ok((
      Solved {
        frame,
        header,
        footer,
      },
      Content::Laid(Box::new(paginated)),
    ))
  }

  /// Lays `node` out a page at a time and draws each page, in order, once `settle` says it is
  /// settled. A document with a link to an element inside it, or a counter, is never streamed.
  fn stream(
    &mut self,
    solved: Solved,
    node: Node,
    ranges: Option<&[PageRange]>,
    structural: bool,
    settle: Settle<'_>,
  ) -> Result<PagePlan, PdfError> {
    let Solved {
      frame,
      header,
      footer,
    } = solved;
    let reads_summary = has_summary_targets(&node);
    let (root, repeated) = self.inputs.prepare_root(node, &frame)?;
    let repeatables: Vec<_> = header.into_iter().chain(repeated).chain(footer).collect();
    let composer = PageComposer::new(self, &frame, &repeatables);
    let page_size = frame.page_size();
    let mut stream = PageStream::new(&root, page_size, MAX_PAGES);
    let mut selection = PageSelection::new(ranges)?;
    let mut summary = PageSummary::new(false);

    loop {
      let Settled { pages, laid } = stream.advance(settle)?;

      for index in pages {
        let kept = selection.push_page().is_some();

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
            0,
            stream.window(index)?.view(),
            |_| None,
          )?;
        }
      }
      if !laid {
        break;
      }
    }

    let pages = stream.page_count();
    let window = frame.window_height;

    drop(stream);
    selection.finish()?;
    Ok(PagePlan {
      frame,
      selection,
      structural,
      root,
      summary: summary.finished(),
      starts: PageStarts::new(pages, window),
    })
  }

  fn compose(
    &mut self,
    solved: Solved,
    mut paginated: Box<Paginated>,
    ranges: Option<&[PageRange]>,
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
    let selection = PageSelection::resolve(ranges, content.page_count)?;
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
    };
    let composer = PageComposer::new(self, &plan.frame, &repeatables);

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

      composer.compose(self.pdf, &slice, pages, scene.view(), |id| {
        plan.anchor_destination(id)
      })?;
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
          slice.index + 1,
          pages,
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
