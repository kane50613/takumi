//! One page: its resolved geometry, and the emit walk that draws it.

use takumi_core::{
  geometry::{Rect, Size},
  layout::node::Node,
  style::Color,
  viewport::{MediaTarget, Viewport},
};

use std::mem::take;

use crate::{
  bands::{Repeatable, RepeatablePage},
  emitter::DocumentState,
  inline::{InlineMap, TextBox, build_inline_map},
  interactive::add_link_annotations,
  krilla::{
    Document,
    destination::XyzDestination,
    geom::{Point, Size as KrillaSize, Transform},
    page::PageSettings,
    surface::Surface,
  },
  options::{PT_PER_PX, PageOptions, PageRange, PageSelection, PdfError},
  page_context::{PageContext, PageContexts},
  pagination::{MAX_PAGES, PageSlice, Paginated},
  paint::paint_page_background,
  tags::tag_id,
  tree::TreeInputs,
  window::{ContentWindow, Window},
};

/// Page geometry resolved once: everything derived from [`PageOptions`] and
/// the measured band heights.
pub(crate) struct PageFrame {
  /// Page size in px.
  pub(crate) size: (f32, f32),
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
      .resolve((page.width, page.height), header_height, footer_height);
    let (content_width, content_height) = page.content_size(margin);

    if !(content_width.is_finite()
      && content_height.is_finite()
      && content_width > 0.0
      && content_height > 0.0)
    {
      return Err(PdfError::InvalidPageSize);
    }

    Ok(Self {
      size: (page.width, page.height),
      page_size,
      margin,
      content_width,
      window_height: content_height,
      band_viewport,
      page_area: Viewport::new((content_width as u32, content_height as u32))
        .with_media_target(MediaTarget::Print)
        .with_unit_reference(Size {
          width: content_width,
          height: content_height,
        }),
    })
  }

  /// The content column: content width, unbounded height. Viewport units
  /// take the page area, as in print media.
  pub(crate) fn column(&self) -> Viewport {
    Viewport::new((self.content_width as u32, None))
      .with_media_target(MediaTarget::Print)
      .with_unit_reference(Size {
        width: self.content_width,
        height: self.window_height,
      })
  }
}

/// What a paged render takes from its options besides the content.
pub(crate) struct PageBands<'o> {
  pub(crate) header: Option<&'o Node>,
  pub(crate) footer: Option<&'o Node>,
  pub(crate) page_ranges: Option<&'o [PageRange]>,
}

/// The paginated document: the contexts its pages draw from, its cut content,
/// and which pages the render keeps.
pub(crate) struct PagePlan {
  pub(crate) contexts: PageContexts,
  pub(crate) paginated: Paginated,
  pub(crate) selection: PageSelection,
  /// Whether destinations name tag-tree structure elements.
  pub(crate) structural: bool,
  /// The repeated `fixed` boxes, drawn between the header and footer bands.
  pub(crate) repeated: Vec<Repeatable>,
}

impl PagePlan {
  /// The destination a link or outline entry jumps to: the page `top` falls
  /// on, at its position inside that page. `None` when the page is dropped by
  /// [`crate::PdfOptions::page_ranges`].
  pub(crate) fn destination(&self, top: f32, path: &[usize]) -> Option<XyzDestination> {
    let index = self.paginated.page_index(top);
    let emitted = self.selection.emitted(index)?;
    let start = self.paginated.starts[index];
    let frame = &self.context_of(index).frame;
    let y = frame.margin.top + self.paginated.reserved_at(start) + (top - start).max(0.0);
    let dest = XyzDestination::new(
      emitted,
      Point::from_xy(frame.margin.left * PT_PER_PX, y * PT_PER_PX),
    );

    Some(match self.structural {
      true => dest.with_structure(tag_id(path)),
      false => dest,
    })
  }

  /// The context page `index` draws on.
  pub(crate) fn context_of(&self, index: usize) -> &PageContext {
    self.contexts.get(self.paginated.page_name(index))
  }

  /// Lays the bands out, resolves the page contexts around them, and cuts the
  /// content into pages.
  pub(crate) fn solve(
    inputs: &TreeInputs<'_>,
    page: PageOptions,
    bands: PageBands<'_>,
    node: Node,
    structural: bool,
  ) -> Result<Self, PdfError> {
    let mut contexts = PageContexts::resolve(inputs, page, &bands, 1)?;
    // A band with a counter and the cut list depend on each other: an `auto`
    // margin takes the band's height, the band lays out with the real page
    // numbers, and the page count is only known once the content is cut. The
    // bands are re-measured with the count each cut produced until their
    // heights stop moving, like the content counters in `Paginated::build`.
    let source = contexts.dynamic().then(|| node.clone());
    let mut paginated = Paginated::build(node, inputs, &mut contexts)?;

    if let Some(source) = source {
      const BAND_PASSES: usize = 3;

      for _ in 0..BAND_PASSES {
        let mut next = PageContexts::resolve(inputs, page, &bands, paginated.starts.len())?;

        for name in contexts.names().flatten() {
          next.ensure(inputs, Some(name))?;
        }
        let stable = next.same_band_heights(&contexts);

        contexts = next;
        if stable {
          break;
        }
        paginated = Paginated::build(source.clone(), inputs, &mut contexts)?;
      }
    }

    if paginated.starts.len() >= MAX_PAGES {
      return Err(PdfError::TooManyPages(paginated.starts.len()));
    }
    let selection = PageSelection::resolve(bands.page_ranges, paginated.starts.len())?;
    let repeated = take(&mut paginated.repeated);

    Ok(Self {
      contexts,
      paginated,
      selection,
      structural,
      repeated,
    })
  }

  /// Draws every kept page.
  pub(crate) fn compose(
    &self,
    pdf: &mut Document,
    inputs: &TreeInputs<'_>,
    state: &DocumentState<'_>,
    background: Option<Color>,
  ) -> Result<(), PdfError> {
    let text_boxes = TextBox::collect(&self.paginated.content);
    let inline_map = build_inline_map(&text_boxes)?;
    let composer = PageComposer {
      plan: self,
      inputs,
      inline_map: &inline_map,
      state,
      background,
    };

    for slice in self.paginated.pages() {
      if !self.selection.keeps(slice.index) {
        continue;
      }
      composer.compose(pdf, &slice)?;
    }
    Ok(())
  }

  /// Where `href="#id"` lands, or `None` when nothing carries that id or its
  /// page is dropped.
  pub(crate) fn anchor_destination(&self, id: &str) -> Option<XyzDestination> {
    let anchor = self.paginated.interactive.anchors.get(id)?;

    self.destination(anchor.top, &anchor.path)
  }
}

/// Draws the paginated content one page at a time: background, the repeatables
/// under the content, the content window, the repeatables over it, then the
/// link annotations.
pub(crate) struct PageComposer<'c, 'g> {
  pub(crate) plan: &'c PagePlan,
  pub(crate) inputs: &'c TreeInputs<'g>,
  pub(crate) inline_map: &'c InlineMap<'c>,
  pub(crate) state: &'c DocumentState<'c>,
  pub(crate) background: Option<Color>,
}

impl PageComposer<'_, '_> {
  /// Draws one page: its context's header band, the repeated boxes, and its
  /// footer band around the content.
  pub(crate) fn compose(&self, pdf: &mut Document, slice: &PageSlice) -> Result<(), PdfError> {
    let context = self.plan.context_of(slice.index);
    let frame = &context.frame;
    let paginated = &self.plan.paginated;
    let pages = paginated.starts.len();
    let page_context = self.inputs.context(frame.page_area);
    let resolved = context
      .header
      .iter()
      .chain(&self.plan.repeated)
      .chain(&context.footer)
      .map(|repeatable| {
        repeatable.for_page(self.inputs, &page_context, frame, slice.index + 1, pages)
      })
      .collect::<Result<Vec<_>, _>>()?;
    let mut pdf_page = pdf.start_page_with(PageSettings::new(frame.page_size));
    let mut surface = pdf_page.surface();

    surface.push_transform(&Transform::from_scale(PT_PER_PX, PT_PER_PX));
    paint_page_background(self.background, frame.size, &mut surface);
    for repeatable in resolved.iter().filter(|page| page.draws_before_content()) {
      repeatable.emit(frame, self.state, &mut surface)?;
    }
    self.emit_content(slice, &mut surface)?;
    for repeatable in resolved.iter().filter(|page| !page.draws_before_content()) {
      repeatable.emit(frame, self.state, &mut surface)?;
    }
    surface.pop();
    surface.finish();
    let anchor = |id: &str| self.plan.anchor_destination(id);

    let column_left = frame.margin.left - slice.left;

    add_link_annotations(
      &mut pdf_page,
      &paginated.interactive.links,
      Window {
        y: Some((slice.start, slice.start + slice.paint_height)),
        x: Some((slice.left, slice.left + frame.content_width)),
        lines: None,
      },
      (column_left, frame.margin.top + slice.reserved),
      self.state.tags.as_ref(),
      anchor,
    );
    // A repeated box sits at the same place on every page, so its links are
    // added per page against the page area rather than the content window.
    // The box itself is an artifact, and its paths do not exist in the tag
    // tree, so the annotations stay out of the structure too.
    add_link_annotations(
      &mut pdf_page,
      resolved.iter().flat_map(RepeatablePage::links),
      Window {
        y: Some((0.0, frame.window_height)),
        ..Window::default()
      },
      (frame.margin.left, frame.margin.top),
      None,
      anchor,
    );
    // A replayed table header is an artifact like a repeated box, so its
    // links annotate per page and stay out of the structure.
    for &(offset, index) in &slice.replays {
      let band = &paginated.headers[index];

      add_link_annotations(
        &mut pdf_page,
        &paginated.interactive.links,
        band.window(),
        (column_left, frame.margin.top + offset),
        None,
        anchor,
      );
    }
    pdf_page.finish();
    Ok(())
  }

  /// Emits the content column through this page's window: clipped to the paint
  /// height and translated so the slice lands at the content origin, below any
  /// repeated table headers.
  fn emit_content(&self, slice: &PageSlice, surface: &mut Surface) -> Result<(), PdfError> {
    let frame = &self.plan.context_of(slice.index).frame;
    let paginated = &self.plan.paginated;
    let column_left = frame.margin.left - slice.left;

    // Paint stops at the next cut: the region between a raised cut and the
    // page's full height belongs to the next page and stays blank, exactly
    // like browser print fragmentation.
    ContentWindow {
      clip: (
        frame.margin.left,
        frame.margin.top + slice.reserved,
        frame.content_width,
        slice.paint_height,
      ),
      translate: (column_left, frame.margin.top + slice.reserved - slice.start),
      window: Window {
        y: Some((slice.start, slice.start + slice.paint_height)),
        x: None,
        lines: Some((
          if slice.index == 0 {
            f32::NEG_INFINITY
          } else {
            slice.start
          },
          slice.end,
        )),
      },
      artifact: false,
    }
    .emit(
      paginated
        .content
        .emitter(self.state, Some(self.inline_map), true),
      surface,
    )?;
    // Each repeating table header band replays at the top of the window. The
    // first occurrence carried the tags, so a replay is an artifact. Clipping
    // to the table keeps content beside it out of the replay.
    for &(offset, index) in &slice.replays {
      let band = &paginated.headers[index];

      ContentWindow {
        clip: (
          column_left + band.left,
          frame.margin.top + offset,
          band.right - band.left,
          band.height(),
        ),
        translate: (column_left, frame.margin.top + offset - band.top),
        window: Window {
          lines: Some((f32::NEG_INFINITY, band.bottom)),
          ..band.window()
        },
        artifact: true,
      }
      .emit(
        paginated
          .content
          .emitter(self.state, Some(self.inline_map), false),
        surface,
      )?;
    }
    Ok(())
  }
}
