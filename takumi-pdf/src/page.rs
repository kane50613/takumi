//! One page: its resolved geometry, and the emit walk that draws it.

use std::cell::RefCell;

use takumi_core::{
  context::RenderContext,
  geometry::Rect,
  style::Color,
  viewport::{MediaTarget, Viewport},
};

use crate::{
  bands::{Repeatable, RepeatablePage},
  emitter::{FontMap, RenderIssues},
  inline::InlineMap,
  interactive::add_link_annotations,
  krilla::{
    Document,
    destination::XyzDestination,
    geom::{Point, Rect as KrillaRect, Size as KrillaSize, Transform},
    page::PageSettings,
    surface::Surface,
  },
  options::{PT_PER_PX, PageOptions, PageSelection, PdfError},
  pagination::{PageGeometry, PageSlice, Paginated, header_replays},
  paint::{fill_from_rgba, rect_path},
  tags::{TagCollector, tag_id},
  tree::TreeInputs,
  window::ContentWindow,
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
        .with_media_target(MediaTarget::Print),
    })
  }

  /// What a pagination pass lays out against.
  pub(crate) fn geometry(&self) -> PageGeometry {
    PageGeometry {
      viewport: Viewport::new((self.content_width as u32, None))
        .with_media_target(MediaTarget::Print),
      page_area: self.page_area,
      window_height: self.window_height,
    }
  }
}

/// Draws the paginated content one page at a time: background, the repeatables
/// under the content, the content window, the repeatables over it, then the
/// link annotations.
pub(crate) struct PageComposer<'c, 'g> {
  pub(crate) frame: &'c PageFrame,
  pub(crate) paginated: &'c Paginated,
  pub(crate) inputs: &'c TreeInputs<'g>,
  pub(crate) page_context: RenderContext,
  pub(crate) inline_map: &'c InlineMap<'c>,
  pub(crate) fonts: &'c mut FontMap,
  pub(crate) issues: &'c RefCell<RenderIssues>,
  pub(crate) tag_collector: Option<&'c RefCell<TagCollector>>,
  pub(crate) document_lang: Option<&'c str>,
  pub(crate) background: Option<Color>,
  /// Whether destinations name tag-tree structure elements.
  pub(crate) structural: bool,
  pub(crate) selection: &'c PageSelection,
}

impl PageComposer<'_, '_> {
  /// The destination a link or outline entry jumps to: the page `top` falls
  /// on, at its position inside that page. `None` when the page is dropped by
  /// [`crate::PdfOptions::page_ranges`].
  pub(crate) fn destination(&self, top: f32, path: &[usize]) -> Option<XyzDestination> {
    let index = self.paginated.page_index(top);
    let emitted = self.selection.emitted(index)?;
    let start = self.paginated.starts[index];
    let y = self.frame.margin.top + self.paginated.reserved_at(start) + (top - start).max(0.0);
    let dest = XyzDestination::new(
      emitted,
      Point::from_xy(self.frame.margin.left * PT_PER_PX, y * PT_PER_PX),
    );

    Some(match self.structural {
      true => dest.with_structure(tag_id(path)),
      false => dest,
    })
  }

  /// Draws one page. `repeatables` are in draw order: the header band, the
  /// repeated boxes, the footer band.
  pub(crate) fn compose(
    &mut self,
    document: &mut Document,
    repeatables: &[Repeatable],
    slice: &PageSlice,
  ) -> Result<(), PdfError> {
    let pages = self.paginated.starts.len();
    let resolved = repeatables
      .iter()
      .map(|repeatable| {
        repeatable.for_page(
          self.inputs,
          &self.page_context,
          self.frame,
          slice.index + 1,
          pages,
        )
      })
      .collect::<Result<Vec<_>, _>>()?;
    let mut pdf_page = document.start_page_with(PageSettings::new(self.frame.page_size));
    let mut surface = pdf_page.surface();

    surface.push_transform(&Transform::from_scale(PT_PER_PX, PT_PER_PX));
    self.paint_background(&mut surface);
    for repeatable in resolved.iter().filter(|page| page.draws_before_content()) {
      repeatable.emit(
        self.frame,
        self.fonts,
        self.tag_collector.is_some(),
        self.issues,
        self.document_lang,
        &mut surface,
      )?;
    }
    self.emit_content(slice, &mut surface)?;
    for repeatable in resolved.iter().filter(|page| !page.draws_before_content()) {
      repeatable.emit(
        self.frame,
        self.fonts,
        self.tag_collector.is_some(),
        self.issues,
        self.document_lang,
        &mut surface,
      )?;
    }
    surface.pop();
    surface.finish();
    add_link_annotations(
      &mut pdf_page,
      &self.paginated.interactive.links,
      (slice.start, slice.start + slice.paint_height),
      None,
      (
        self.frame.margin.left,
        self.frame.margin.top + slice.reserved,
      ),
      self.tag_collector,
      |id| {
        self
          .paginated
          .interactive
          .anchors
          .get(id)
          .and_then(|anchor| self.destination(anchor.top, &anchor.path))
      },
    );
    // A repeated box sits at the same place on every page, so its links are
    // added per page against the page area rather than the content window.
    // The box itself is an artifact, and its paths do not exist in the tag
    // tree, so the annotations stay out of the structure too.
    add_link_annotations(
      &mut pdf_page,
      resolved.iter().flat_map(RepeatablePage::links),
      (0.0, self.frame.window_height),
      None,
      (self.frame.margin.left, self.frame.margin.top),
      None,
      |id| {
        self
          .paginated
          .interactive
          .anchors
          .get(id)
          .and_then(|anchor| self.destination(anchor.top, &anchor.path))
      },
    );
    // A replayed table header is an artifact like a repeated box, so its
    // links annotate per page and stay out of the structure.
    for (offset, index) in self.replays(slice) {
      let band = &self.paginated.headers[index];

      add_link_annotations(
        &mut pdf_page,
        &self.paginated.interactive.links,
        (band.top, band.bottom),
        Some((band.left, band.right)),
        (self.frame.margin.left, self.frame.margin.top + offset),
        None,
        |id| {
          self
            .paginated
            .interactive
            .anchors
            .get(id)
            .and_then(|anchor| self.destination(anchor.top, &anchor.path))
        },
      );
    }
    pdf_page.finish();
    Ok(())
  }

  /// The header bands this page replays, with each band's window offset.
  fn replays(&self, slice: &PageSlice) -> Vec<(f32, usize)> {
    header_replays(
      &self.paginated.headers,
      slice.start,
      self.frame.window_height,
    )
    .1
  }

  /// Fills the page box before anything else draws. An unset color leaves the
  /// page empty rather than painting white, like Chromium's print path.
  fn paint_background(&self, surface: &mut Surface) {
    let Some(color) = self.background else {
      return;
    };
    let Some(path) =
      KrillaRect::from_xywh(0.0, 0.0, self.frame.size.0, self.frame.size.1).and_then(rect_path)
    else {
      return;
    };

    surface.set_fill(Some(fill_from_rgba(color.0, 1.0)));
    surface.draw_path(&path);
  }

  /// Emits the content column through this page's window: clipped to the paint
  /// height and translated so the slice lands at the content origin, below any
  /// repeated table headers.
  fn emit_content(&mut self, slice: &PageSlice, surface: &mut Surface) -> Result<(), PdfError> {
    // Paint stops at the next cut: the region between a raised cut and the
    // page's full height belongs to the next page and stays blank, exactly
    // like browser print fragmentation.
    ContentWindow {
      clip: (
        self.frame.margin.left,
        self.frame.margin.top + slice.reserved,
        self.frame.content_width,
        slice.paint_height,
      ),
      translate: (
        self.frame.margin.left,
        self.frame.margin.top + slice.reserved - slice.start,
      ),
      window: Some((slice.start, slice.start + slice.paint_height)),
      x_window: None,
      line_window: Some((
        if slice.index == 0 {
          f32::NEG_INFINITY
        } else {
          slice.start
        },
        slice.end,
      )),
      artifact: false,
    }
    .emit(
      self.paginated.content.emitter(
        self.fonts,
        Some(self.inline_map),
        self.tag_collector,
        self.issues,
        self.document_lang,
      ),
      surface,
    )?;
    if slice.reserved > 0.0 {
      self.emit_repeated_headers(slice, surface)?;
    }
    Ok(())
  }

  /// Replays each repeating table header band at the top of the window. The
  /// first occurrence carried the tags, so a replay is an artifact.
  fn emit_repeated_headers(
    &mut self,
    slice: &PageSlice,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    for (offset, index) in self.replays(slice) {
      let band = &self.paginated.headers[index];

      // Clipping to the table keeps content beside it out of the replay.
      ContentWindow {
        clip: (
          self.frame.margin.left + band.left,
          self.frame.margin.top + offset,
          band.right - band.left,
          band.height(),
        ),
        translate: (
          self.frame.margin.left,
          self.frame.margin.top + offset - band.top,
        ),
        window: Some((band.top, band.bottom)),
        x_window: Some((band.left, band.right)),
        line_window: Some((f32::NEG_INFINITY, band.bottom)),
        artifact: true,
      }
      .emit(
        self.paginated.content.emitter(
          self.fonts,
          Some(self.inline_map),
          None,
          self.issues,
          self.document_lang,
        ),
        surface,
      )?;
    }
    Ok(())
  }
}
