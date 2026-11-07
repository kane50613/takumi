//! The pages of a document: their geometry, and the walk that draws each.

use std::{collections::HashMap, rc::Rc};

use takumi_core::{
  context::RenderContext,
  geometry::{Point as CorePoint, Rect, Size},
  layout::{
    node::Node,
    page_sizes::{PageSizer, PageSizes},
    tree::RenderNode,
  },
  page_stream::Settle,
  scene::{SceneRef, SceneView},
  style::{Color, page_rule::PageContext},
  viewport::{MediaTarget, Viewport},
};

use crate::{
  bands::{Bands, RepeatablePage, RepeatedBoxes},
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
  page_kinds::PerPage,
  pagination::{Counted, Counts, PageSlice, Paginated, Prepared, reads_layout, walk_pages},
  paint::paint_page_background,
  tree::TreeInputs,
  window::{ContentWindow, Window},
};

/// One page's geometry: everything derived from [`PageOptions`] and the measured band heights.
#[derive(Clone)]
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
  /// Resolves the geometry, a band taller than its margin overlapping content as in Chromium.
  pub(crate) fn resolve(
    page: &PageOptions,
    (header_height, footer_height): (Option<f32>, Option<f32>),
  ) -> Result<Self, PdfError> {
    let page_size = KrillaSize::from_wh(page.width * PT_PER_PX, page.height * PT_PER_PX)
      .ok_or(PdfError::InvalidPageSize)?;
    let margin = page
      .margin
      .resolve(page.size(), header_height, footer_height);
    let content = page.content_size(margin);

    if !content.has_area() {
      return Err(PdfError::InvalidPageSize);
    }

    Ok(Self {
      size: page.size(),
      page_size,
      margin,
      content_width: content.width,
      window_height: content.height,
      band_viewport: band_viewport(page),
      page_area: print_viewport(content.width, Some(content.height), content),
    })
  }

  /// The page area, which the content lays out on.
  fn area(&self) -> Size<f32> {
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

/// Each kind of page's geometry.
pub(crate) type PageFrames = PerPage<PageFrame>;

impl PageFrames {
  /// Resolves each page's geometry around the bands measured for it, as [`PageFrame::resolve`]
  /// does.
  pub(crate) fn resolve(
    pages: &PerPage<PageOptions>,
    bands: &PerPage<Rc<Bands>>,
  ) -> Result<Self, PdfError> {
    pages.try_zip_map(bands, |page, bands| {
      PageFrame::resolve(page, bands.heights())
    })
  }

  /// Each page's area, which the content lays out on.
  pub(crate) fn page_sizer(&self) -> PageSizer {
    let areas = self.map(PageFrame::area);

    PageSizer::new(move |page, name| *areas.get(page, name))
  }

  /// Whether every page has the margins it has in `other`, so the content lays out the same.
  fn has_margins_of(&self, other: &Self) -> bool {
    self.map(|frame| frame.margin) == other.map(|frame| frame.margin)
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
  pub(crate) frames: PageFrames,
  pub(crate) selection: PageSelection,
  /// Whether destinations name tag-tree structure elements.
  pub(crate) structural: bool,
  pub(crate) root: RenderNode,
  pub(crate) summary: PageSummary,
  pub(crate) sizes: PageSizes,
}

impl PagePlan {
  fn destinations(&self) -> Destinations<'_> {
    Destinations {
      frames: &self.frames,
      selection: &self.selection,
      sizes: &self.sizes,
      structural: self.structural,
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
struct Destinations<'p> {
  frames: &'p PageFrames,
  selection: &'p PageSelection,
  sizes: &'p PageSizes,
  structural: bool,
}

impl Destinations<'_> {
  /// The page `top` falls on, at its position inside that page. `None` when the page is dropped
  /// by [`crate::PdfOptions::page_ranges`].
  fn destination(&self, top: f32, path: &[usize]) -> Option<XyzDestination> {
    let index = self.sizes.page_at(top);
    let emitted = self.selection.emitted(index)?;
    let margin = self.frames.get(index, self.sizes.name(index)).margin;
    let y = margin.top + (top - self.sizes.top(index)).max(0.0);

    Some(xyz_destination(
      emitted,
      Point::from_xy(margin.left * PT_PER_PX, y * PT_PER_PX),
      path,
      self.structural,
    ))
  }

  fn anchor(&self, anchors: &HashMap<Box<str>, AnchorTarget>, id: &str) -> Option<XyzDestination> {
    let anchor = anchors.get(id)?;

    self.destination(anchor.top, &anchor.path)
  }
}

struct Solved {
  frames: PageFrames,
  /// The bands of each page, measured at its width.
  bands: PerPage<Rc<Bands>>,
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
}

impl PageDrawing<'_, '_> {
  /// Lays the content out on pages and draws each kept page once no later page paints on it.
  pub(crate) fn render(
    &mut self,
    page: PageOptions,
    bands: PageBands<'_>,
    node: Node,
    structural: bool,
    settle: Settle<'_>,
  ) -> Result<PagePlan, PdfError> {
    let selection = PageSelection::new(bands.page_ranges)?;
    let root =
      PageContext::from_initial_containing_block(&self.inputs.context(page.viewport()), &node);
    let pages = page.with_page_rules(&self.inputs.stylesheet, &root);
    let (solved, content) = Self::solve(self.inputs, &pages, &bands, node, settle)?;

    match content {
      Content::Stream(content) => self.stream(solved, content, None, selection, structural, settle),
      Content::Counted(Counted::Pages { content, counts }) => {
        self.stream(solved, content, Some(counts), selection, structural, settle)
      }
      Content::Counted(Counted::Laid(paginated)) => {
        let selection = selection.with_pages(paginated.content.sizes.len())?;

        self.compose(solved, paginated, selection, structural)
      }
    }
  }

  /// Lays the bands out, resolves the page frames around them, and, when the page count or
  /// the boxes' pages are needed first, lays the content out on pages.
  fn solve(
    inputs: &TreeInputs<'_>,
    pages: &PerPage<PageOptions>,
    templates: &PageBands<'_>,
    node: Node,
    settle: Settle<'_>,
  ) -> Result<(Solved, Content), PdfError> {
    // Bands lay out at full page width and draw inside the margin areas,
    // like Chromium's print header and footer templates.
    let measure_bands = |count: usize| {
      pages.try_map_shared(band_viewport, |page| {
        Bands::measure(inputs, templates, band_viewport(page), count)
      })
    };
    let mut bands = measure_bands(1)?;
    let mut frames = PageFrames::resolve(pages, &bands)?;
    // A counter band and the page count depend on each other, so the band is measured again until
    // its height settles.
    let dynamic = bands.values().any(|bands| bands.dynamic());

    if !dynamic && !reads_layout(&node) {
      let content = Prepared::new(node, inputs, &frames)?;

      return Ok((Solved { frames, bands }, Content::Stream(Box::new(content))));
    }

    // Only a margin that follows a band can move the frame, which is when the content lays out
    // again from the source.
    let follows_bands = pages.values().any(|page| page.margin.follow_bands());
    let source = (dynamic && follows_bands).then(|| node.clone());
    let mut counted = Counted::settle(node, inputs, &frames, settle)?;

    if dynamic {
      const BAND_PASSES: usize = 3;

      for _ in 0..BAND_PASSES {
        let next_bands = measure_bands(counted.pages())?;
        let stable = next_bands.map(|bands| bands.heights()) == bands.map(|bands| bands.heights());

        bands = next_bands;
        if stable {
          break;
        }

        let next_frames = PageFrames::resolve(pages, &bands)?;

        // The content lays out the same in frames whose margins did not move.
        if next_frames.has_margins_of(&frames) {
          break;
        }
        let Some(source) = &source else {
          break;
        };

        frames = next_frames;
        counted = Counted::settle(source.clone(), inputs, &frames, settle)?;
      }
    }

    Ok((Solved { frames, bands }, Content::Counted(counted)))
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
    let Solved { frames, bands } = solved;
    let Prepared {
      root,
      repeated,
      reads_summary,
    } = *content;
    let composer = PageComposer::new(self, &frames, &bands, &repeated);
    let mut summary = PageSummary::new(false);
    let counted_pages = counts.as_ref().map(|counts| counts.sizes.len());

    if let Some(pages) = counted_pages {
      selection = selection.with_pages(pages)?;
    }

    let sizes = walk_pages(&root, frames.page_sizer(), settle, |stream, index| {
      let kept = match counted_pages {
        Some(_) => selection.keeps(index),
        None => selection.push_page().is_some(),
      };

      let mut scene = None;

      if reads_summary {
        let pages = stream.pages_of(index);

        // The summary ignores paint bounds, so a drawing scene of the same pages serves it too.
        if kept && stream.pages_reaching(index) == pages {
          let window = scene.insert(stream.window(index)?);

          summary.absorb(PageSummary::collect(window.view(), false));
        } else {
          summary.absorb(PageSummary::of_page(
            &root,
            stream.view(),
            stream.sizes(),
            &pages,
            false,
          )?);
        }
      }
      if kept {
        let scene = match scene {
          Some(scene) => scene,
          None => stream.window(index)?,
        };

        composer.compose(
          self.pdf,
          &PageSlice::of(index, stream.sizes()),
          counted_pages.unwrap_or_default(),
          scene.view(),
          |id| {
            let counts = counts.as_ref()?;

            Destinations {
              frames: &frames,
              selection: &selection,
              sizes: &counts.sizes,
              structural,
            }
            .anchor(&counts.anchors, id)
          },
        )?;
      }
      Ok(())
    })?;

    selection.finish()?;
    Ok(PagePlan {
      frames,
      selection,
      structural,
      root,
      summary: summary.finished(),
      sizes,
    })
  }

  fn compose(
    &mut self,
    solved: Solved,
    paginated: Box<Paginated>,
    selection: PageSelection,
    structural: bool,
  ) -> Result<PagePlan, PdfError> {
    let Solved { frames, bands } = solved;
    let summary = paginated.summary()?;
    let Paginated {
      content,
      windows,
      repeated,
      ..
    } = *paginated;
    let (root, results, sizes) = content.into_parts();
    let plan = PagePlan {
      frames,
      selection,
      structural,
      root,
      summary,
      sizes,
    };
    let composer = PageComposer::new(self, &plan.frames, &bands, &repeated);

    for slice in PageSlice::all(&plan.sizes) {
      if !plan.selection.keeps(slice.index) {
        continue;
      }
      let scene = SceneRef::window(
        &plan.root,
        results.view(),
        &plan.sizes,
        &windows.pages_reaching(&plan.sizes, slice.index),
        true,
      )?;

      composer.compose(self.pdf, &slice, plan.sizes.len(), scene.view(), |id| {
        plan.anchor_destination(id)
      })?;
    }
    Ok(plan)
  }
}

/// Draws the paginated content one page at a time.
struct PageComposer<'c, 'g> {
  frames: &'c PageFrames,
  bands: &'c PerPage<Rc<Bands>>,
  repeated: &'c RepeatedBoxes,
  inputs: &'c TreeInputs<'g>,
  /// Each page's context, which a repeated box lays out in against the page area.
  page_contexts: PerPage<Rc<RenderContext>>,
  state: &'c DocumentState<'c>,
  background: Option<Color>,
}

impl<'c, 'g> PageComposer<'c, 'g> {
  fn new(
    drawing: &PageDrawing<'c, 'g>,
    frames: &'c PageFrames,
    bands: &'c PerPage<Rc<Bands>>,
    repeated: &'c RepeatedBoxes,
  ) -> Self {
    let page_contexts = frames.map_shared(
      |frame| frame.page_area,
      |frame| drawing.inputs.context(frame.page_area),
    );

    Self {
      frames,
      bands,
      repeated,
      inputs: drawing.inputs,
      page_contexts,
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
    let frame = self.frames.get(slice.index, &slice.name);
    let bands = self.bands.get(slice.index, &slice.name);
    let page_context = self.page_contexts.get(slice.index, &slice.name);
    // In draw order: the header band, the repeated boxes, the footer band.
    let resolved = bands
      .header
      .iter()
      .chain(self.repeated.get(slice.index, &slice.name).iter())
      .chain(&bands.footer)
      .map(|repeatable| {
        repeatable.for_page(self.inputs, page_context, frame, slice.index + 1, pages)
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
      frame,
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
    // A repeated box's links go on every page against the page area, outside the structure.
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
    frame: &PageFrame,
    slice: &PageSlice,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
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
