#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Vector PDF output for takumi.
//!
//! [`render`] runs takumi-core layout and walks the same backend-agnostic
//! stacking-context scene as `takumi-svg`. A vendored krilla fork writes the
//! PDF. Backgrounds become filled paths. Text becomes real glyph runs with
//! embedded subset fonts: selectable, searchable, copyable.
//!
//! # Pagination
//!
//! With [`PdfOptions::page`] set, content lays out at the page's content
//! width with unbounded height, then slices into pages. Unsplittable atoms
//! (text lines, images, transformed subtrees) push cut points up, so none of
//! them is cut in half. Each page re-walks the scene through a vertical
//! window (clip + translate). Every text line lands on exactly one page.
//!
//! Pagination honors `break-before: page`, `break-after: page`,
//! `break-inside: avoid`, and the `widows` / `orphans` minimums (default 2,
//! like Chromium). Header and footer bands repeat in the page margin
//! areas, like Chromium's print templates. Nodes classed `pageNumber` /
//! `totalPages` receive the page counters.
//!
//! # CSS coverage
//!
//! - backgrounds: color, gradient and url() layers, with `background-size`,
//!   `-position`, `-repeat`, `-origin`, `-clip` (including `text`) and
//!   per-layer blend modes
//! - borders, border-radius, `outline`, and `box-shadow` (blur approximated
//!   by bands)
//! - text with decorations, `text-shadow`, and `-webkit-text-stroke`
//! - images, with `object-fit` and `object-position`
//! - `clip-path`, `mask-image`, and the color `filter` primitives
//! - opacity, blend modes, overflow clipping, and affine transforms
//!
//! `filter: blur()` and `drop-shadow()` have no PDF equivalent. [`render`]
//! stops and names the function.

use std::{cell::RefCell, mem::take, rc::Rc};

mod background;
mod bands;
mod counters;

pub use counters::counter_characters;
mod emitter;
mod filter;
mod glyph;
mod inline;
mod interactive;
mod options;
mod page;
mod pagination;
mod paint;
#[cfg(feature = "images")]
mod raster;
mod shadow;
#[cfg(all(feature = "svg", feature = "images"))]
mod svg;
mod tags;
mod tree;

#[allow(
  dead_code,
  missing_docs,
  clippy::all,
  clippy::redundant_closure_for_method_calls,
  clippy::unwrap_used,
  clippy::expect_used,
  clippy::panic
)]
mod krilla;
#[allow(
  dead_code,
  missing_docs,
  clippy::all,
  clippy::redundant_closure_for_method_calls,
  clippy::unwrap_used,
  clippy::expect_used,
  clippy::panic
)]
mod subsetter;

use takumi_core::{
  style::{Affine, Color, Lang},
  viewport::Viewport,
};

pub use crate::options::{
  Attachment, AttachmentRelationship, MeasureOptions, MeasuredSize, PageMargin, PageMargins,
  PageOptions, PdfDate, PdfError, PdfMetadata, PdfOptions, PdfStandard, Tagging, XmpProperty,
  XmpSchema,
};
use crate::{
  bands::{RepeatBounds, Repeatable, prepare_band},
  emitter::{FontMap, RenderIssues},
  glyph::uncovered_error,
  inline::{build_inline_map, collect_text_boxes},
  interactive::{add_link_annotations, build_outline, collect_interactive, destination_targets},
  krilla::{
    Document, SerializeSettings,
    configure::ConfigurationBuilder,
    destination::XyzDestination,
    embed::{EmbeddedFile, MimeType},
    geom::{Point, Rect as KrillaRect, Size as KrillaSize, Transform},
    page::PageSettings,
    surface::Surface,
  },
  options::{PT_PER_PX, build_metadata, krilla_datetime, validate_xmp_schemas},
  page::{PageComposer, PageFrame},
  pagination::{MAX_PAGES, paginate},
  paint::{fill_from_rgba, rect_path},
  tags::{TagCollector, build_tag_tree, tag_id},
  tree::{TreeInputs, prepare_tree, tree_context},
};

/// Lays out a node tree without rendering and returns its size.
///
/// With [`MeasureOptions::page`] set the tree lays out like a header/footer
/// band in [`render`]: full page width, unbounded height, `pageNumber` /
/// `totalPages` class hooks filled with three-digit counters. The returned
/// height is the band height a page margin needs to accommodate. With a
/// viewport the tree is measured as-is, counter hooks untouched.
///
/// The size is the tree's own, not the space it laid out against: a box with
/// `width: 100px` measures 100 wide on any page.
pub fn measure(options: MeasureOptions<'_>) -> Result<MeasuredSize, PdfError> {
  let inputs = TreeInputs {
    fonts: options.fonts,
    stylesheet: options.stylesheet,
    images: Rc::new(options.images),
    font_families: options.font_families,
    lang: options.lang,
  };
  let tree = match (options.page, options.viewport) {
    (Some(page), _) => prepare_band(
      &inputs,
      &options.node,
      999,
      999,
      Viewport::new((page.width as u32, None)),
    )?,
    (None, Some(viewport)) => prepare_tree(&inputs, options.node, viewport)?,
    (None, None) => return Err(PdfError::MissingViewport),
  };

  let size = tree.content_size();

  Ok(MeasuredSize {
    width: size.width,
    height: size.height,
  })
}

/// Fills the page box before the page draws anything else. An unset color
/// leaves the page empty rather than painting white, like Chromium's print
/// path.
fn paint_page_background(color: Option<Color>, size: (f32, f32), surface: &mut Surface) {
  let Some(color) = color else {
    return;
  };
  let Some(path) = KrillaRect::from_xywh(0.0, 0.0, size.0, size.1).and_then(rect_path) else {
    return;
  };

  surface.set_fill(Some(fill_from_rgba(color.0, 1.0)));
  surface.draw_path(&path);
}

/// Renders a node tree to a PDF: single-page at the viewport size, or paged
/// when [`PdfOptions::page`] is set.
pub fn render(options: PdfOptions<'_>) -> Result<Vec<u8>, PdfError> {
  let inputs = TreeInputs {
    fonts: options.fonts,
    stylesheet: options.stylesheet,
    images: Rc::new(options.images),
    font_families: options.font_families,
    lang: options.lang,
  };
  let mut fonts = FontMap::new();
  let issues = RefCell::new(RenderIssues::default());
  let document_lang = inputs.lang.as_ref().map(Lang::as_str);
  let mut document = {
    let mut builder = ConfigurationBuilder::new();
    let mut validated = false;

    if let Some(archival) = options.standard.archival() {
      builder = builder.with_archival_validator(archival);
      validated = true;
    }
    if let Some(accessibility) = options.tagged.accessibility() {
      builder = builder.with_accessibility_validator(accessibility);
      validated = true;
    }
    if validated {
      let configuration = builder.finish().map_err(|_| PdfError::InvalidStandard)?;

      Document::new_with(SerializeSettings {
        configuration,
        ..SerializeSettings::default()
      })
    } else {
      Document::new()
    }
  };
  let tag_collector = (options.tagged != Tagging::Off || options.standard.requires_tagging())
    .then(|| RefCell::new(TagCollector::default()));

  if let Some(metadata) = &options.metadata {
    validate_xmp_schemas(&metadata.xmp)?;
    document.set_metadata(build_metadata(metadata, inputs.lang));
  } else if tag_collector.is_some() && inputs.lang.is_some() {
    // Tagged standards check the document language even without metadata.
    document.set_metadata(build_metadata(&PdfMetadata::default(), inputs.lang));
  }

  let fallback_date = options.metadata.as_ref().and_then(|m| m.creation_date);

  for attachment in options.attachments {
    let mime_type = match attachment.mime_type {
      Some(mime) => Some(MimeType::new(&mime).ok_or(PdfError::InvalidMimeType(mime))?),
      None => None,
    };
    let file = EmbeddedFile {
      path: attachment.name.clone(),
      mime_type,
      description: attachment.description,
      association_kind: attachment.relationship.association_kind(),
      data: attachment.data.into(),
      modification_date: attachment
        .modification_date
        .or(fallback_date)
        .map(krilla_datetime),
      compress: None,
      location: None,
    };

    document
      .embed_file(file)
      .ok_or(PdfError::DuplicateAttachment(attachment.name))?;
  }
  match options.page {
    Some(page) => {
      // Bands lay out at full page width and draw inside the margin areas,
      // like Chromium's print header and footer templates.
      let band_viewport = Viewport::new((page.width as u32, None));
      let header = options
        .header
        .as_ref()
        .map(|template| Repeatable::band(&inputs, template, band_viewport, RepeatBounds::Header))
        .transpose()?;
      let footer = options
        .footer
        .as_ref()
        .map(|template| Repeatable::band(&inputs, template, band_viewport, RepeatBounds::Footer))
        .transpose()?;
      let frame = PageFrame::resolve(
        &page,
        band_viewport,
        header.as_ref().map(Repeatable::height),
        footer.as_ref().map(Repeatable::height),
      )?;
      let mut paginated = paginate(
        options.node,
        &inputs,
        &mut fonts,
        &issues,
        document_lang,
        &frame.geometry(),
      )?;

      if paginated.starts.len() >= MAX_PAGES {
        return Err(PdfError::TooManyPages(paginated.starts.len()));
      }
      let repeatables: Vec<Repeatable> = header
        .into_iter()
        .chain(
          take(&mut paginated.repeated)
            .into_iter()
            .map(Repeatable::fixed),
        )
        .chain(footer)
        .collect();
      let text_boxes = collect_text_boxes(&paginated.content);
      let inline_map = build_inline_map(&text_boxes)?;
      let mut composer = PageComposer {
        frame: &frame,
        paginated: &paginated,
        inputs: &inputs,
        page_context: tree_context(&inputs, frame.page_area),
        inline_map: &inline_map,
        fonts: &mut fonts,
        issues: &issues,
        tag_collector: tag_collector.as_ref(),
        document_lang,
        background: options.background_color,
        structural: options.tagged.names_structure_destinations(),
      };

      for slice in paginated.pages() {
        composer.compose(&mut document, &repeatables, &slice)?;
      }

      let interactive = &paginated.interactive;

      if (options.outline || options.tagged.requires_outline()) && !interactive.headings.is_empty()
      {
        document.set_outline(build_outline(&interactive.headings, |heading| {
          composer.destination(heading.top, &heading.path)
        }));
      }
      if let Some(collector) = &tag_collector {
        document.set_tag_tree(build_tag_tree(
          &paginated.content.root,
          inputs.lang.as_ref().map(Lang::as_str),
          &mut collector.borrow_mut(),
          &destination_targets(interactive),
        ));
      }
    }
    None => {
      let viewport = options.viewport.ok_or(PdfError::MissingViewport)?;
      let content = prepare_tree(&inputs, options.node, viewport)?;
      let page_size = KrillaSize::from_wh(content.width * PT_PER_PX, content.height * PT_PER_PX)
        .ok_or(PdfError::InvalidPageSize)?;
      let text_boxes = collect_text_boxes(&content);
      let inline_map = build_inline_map(&text_boxes)?;
      let mut page = document.start_page_with(PageSettings::new(page_size));
      let mut surface = page.surface();

      surface.push_transform(&Transform::from_scale(PT_PER_PX, PT_PER_PX));
      paint_page_background(
        options.background_color,
        (content.width, content.height),
        &mut surface,
      );

      let mut emitter = content.emitter(
        &mut fonts,
        Some(&inline_map),
        tag_collector.as_ref(),
        &issues,
        document_lang,
      );

      emitter.emit_context(0, Affine::IDENTITY, &mut surface)?;
      surface.pop();
      surface.finish();
      let interactive = collect_interactive(&content);
      let structural = options.tagged.names_structure_destinations();
      let destination = |top: f32, path: &[usize]| {
        let dest = XyzDestination::new(0, Point::from_xy(0.0, top.max(0.0) * PT_PER_PX));

        match structural {
          true => dest.with_structure(tag_id(path)),
          false => dest,
        }
      };

      add_link_annotations(
        &mut page,
        &interactive.links,
        (0.0, content.height),
        (0.0, 0.0),
        tag_collector.as_ref(),
        |id| {
          interactive
            .anchors
            .get(id)
            .map(|anchor| destination(anchor.top, &anchor.path))
        },
      );
      page.finish();
      if (options.outline || options.tagged.requires_outline()) && !interactive.headings.is_empty()
      {
        document.set_outline(build_outline(&interactive.headings, |heading| {
          destination(heading.top, &heading.path)
        }));
      }
      if let Some(collector) = &tag_collector {
        document.set_tag_tree(build_tag_tree(
          &content.root,
          inputs.lang.as_ref().map(Lang::as_str),
          &mut collector.borrow_mut(),
          &destination_targets(&interactive),
        ));
      }
    }
  }

  let issues = issues.into_inner();

  if let Some(error) = issues
    .failure
    .or_else(|| uncovered_error(&issues.uncovered))
  {
    return Err(error);
  }

  document.finish().map_err(PdfError::Krilla)
}

#[cfg(test)]
mod tests {
  use takumi_core::units::ONE_IN_PX;

  use super::*;
  use crate::{
    options::BAND_EDGE_PADDING,
    pagination::{Atom, page_starts},
  };

  const A4: (f32, f32) = (PageOptions::A4.width, PageOptions::A4.height);

  #[test]
  fn content_taller_than_a_render_allows_stops_counting() {
    let starts = page_starts(&mut [], &mut Vec::new(), &[], 2_000_000.0, 10.0);

    assert_eq!(starts.len(), MAX_PAGES, "the page count runs unbounded");
  }

  #[test]
  fn page_starts_without_atoms_cuts_at_window() {
    assert_eq!(
      page_starts(&mut [], &mut Vec::new(), &[], 250.0, 100.0),
      vec![0.0, 100.0, 200.0]
    );
  }

  #[test]
  fn page_starts_pushes_straddling_atom_to_next_page() {
    let mut atoms = [(90.0, 110.0)];

    assert_eq!(
      page_starts(&mut atoms, &mut Vec::new(), &[], 250.0, 100.0),
      vec![0.0, 90.0, 190.0]
    );
  }

  #[test]
  fn page_starts_hard_cuts_atom_taller_than_window() {
    let mut atoms = [(0.0, 300.0)];

    assert_eq!(
      page_starts(&mut atoms, &mut Vec::new(), &[], 300.0, 100.0),
      vec![0.0, 100.0, 200.0]
    );
  }

  /// Six 10pt lines from y=50: the window cut at 100 would leave a lone line
  /// on the next page, so the solver moves it up to keep two.
  #[test]
  fn page_starts_keeps_widows_together() {
    let lines: Vec<Atom> = (0..6)
      .map(|i| (50.0 + i as f32 * 10.0, 60.0 + i as f32 * 10.0))
      .collect();
    let mut atoms = lines.clone();
    let paragraphs = [pagination::Paragraph {
      lines,
      before: 2,
      after: 2,
    }];

    assert_eq!(
      page_starts(&mut atoms, &mut Vec::new(), &paragraphs, 110.0, 100.0),
      vec![0.0, 90.0],
      "the cut moves from 100 to 90 so two lines reach the next page"
    );
  }

  /// The cut at 100 would strand one line at the bottom of the page, so the
  /// whole paragraph moves over.
  #[test]
  fn page_starts_keeps_orphans_together() {
    let lines: Vec<Atom> = (0..4)
      .map(|i| (90.0 + i as f32 * 10.0, 100.0 + i as f32 * 10.0))
      .collect();
    let mut atoms = lines.clone();
    let paragraphs = [pagination::Paragraph {
      lines,
      before: 2,
      after: 2,
    }];

    assert_eq!(
      page_starts(&mut atoms, &mut Vec::new(), &paragraphs, 140.0, 100.0),
      vec![0.0, 90.0],
      "one line before the cut violates orphans, so the paragraph starts the next page"
    );
  }

  /// Three lines with 2/2 minimums cannot satisfy both sides. Blink keeps the
  /// orphans and accepts the widow violation, and so does the solver.
  #[test]
  fn page_starts_prefers_orphans_over_widows() {
    let lines: Vec<Atom> = (0..3)
      .map(|i| (80.0 + i as f32 * 10.0, 90.0 + i as f32 * 10.0))
      .collect();
    let mut atoms = lines.clone();
    let paragraphs = [pagination::Paragraph {
      lines,
      before: 2,
      after: 2,
    }];

    assert_eq!(
      page_starts(&mut atoms, &mut Vec::new(), &paragraphs, 110.0, 100.0),
      vec![0.0, 100.0],
      "backing up past the orphans floor is worse than a lone widow"
    );
  }

  /// Minimums that cannot fit the current page are dropped rather than looping.
  #[test]
  fn page_starts_drops_unsatisfiable_minimums() {
    let lines: Vec<Atom> = (0..30)
      .map(|i| (i as f32 * 10.0, 10.0 + i as f32 * 10.0))
      .collect();
    let mut atoms = lines.clone();
    let paragraphs = [pagination::Paragraph {
      lines,
      before: 20,
      after: 20,
    }];

    assert_eq!(
      page_starts(&mut atoms, &mut Vec::new(), &paragraphs, 300.0, 100.0),
      vec![0.0, 100.0, 200.0],
      "a paragraph that can never satisfy 20/20 still paginates at the window"
    );
  }

  #[test]
  fn page_starts_honors_forced_cuts() {
    let mut forced = vec![40.0, 150.0];

    assert_eq!(
      page_starts(&mut [], &mut forced, &[], 250.0, 100.0),
      vec![0.0, 40.0, 140.0, 150.0]
    );
  }

  #[test]
  fn presets_match_css_page_keywords() {
    // px at 96 dpi, from the millimetres and inches CSS Paged Media lists.
    for (page, width, height) in [
      (PageOptions::A3, 1122.52, 1587.40),
      (PageOptions::A4, 793.70, 1122.52),
      (PageOptions::A5, 559.37, 793.70),
      (PageOptions::B4, 944.88, 1334.17),
      (PageOptions::B5, 665.20, 944.88),
      (PageOptions::JIS_B4, 971.34, 1375.75),
      (PageOptions::JIS_B5, 687.87, 971.34),
      (PageOptions::LEDGER, 1056.0, 1632.0),
      (PageOptions::LEGAL, 816.0, 1344.0),
      (PageOptions::LETTER, 816.0, 1056.0),
    ] {
      assert!(
        (page.width - width).abs() < 0.1,
        "width of {width}x{height}"
      );
      assert!(
        (page.height - height).abs() < 0.1,
        "height of {width}x{height}"
      );
      assert!(page.width < page.height, "presets are portrait");
    }

    let a4 = PageOptions::A4;

    let landscape = PageOptions::A4.landscape();

    assert_eq!(landscape.width, a4.height);
    assert_eq!(landscape.height, a4.width);
    assert_eq!(
      PageOptions::A4
        .with_margin(0.0)
        .margin
        .resolve(A4, None, None)
        .top,
      0.0
    );
  }

  #[test]
  fn auto_margin_takes_the_band_it_holds() {
    let margins = PageOptions::A4.margin;

    assert_eq!(
      margins.resolve(A4, None, None).top,
      PageOptions::DEFAULT_MARGIN,
      "a side with no band sits at the default"
    );
    assert_eq!(
      margins.resolve(A4, Some(4.0), None).top,
      PageOptions::DEFAULT_MARGIN,
      "a band that fits inside the default does not shrink the page"
    );
    assert_eq!(
      margins.resolve(A4, Some(80.0), None).top,
      80.0 + BAND_EDGE_PADDING,
      "a taller band takes its height plus the inset it draws at"
    );
    assert_eq!(
      margins.resolve(A4, None, Some(80.0)).bottom,
      80.0 + BAND_EDGE_PADDING
    );
    assert_eq!(
      margins.resolve(A4, None, Some(80.0)).left,
      PageOptions::DEFAULT_MARGIN,
      "the sides hold no band"
    );
  }

  /// Chromium drops the default margin on an axis that does not clear an inch
  /// rather than leave the page with nothing to print on. Its test is
  /// `axis > one inch`, so an axis of exactly an inch keeps no margin either.
  #[test]
  fn a_page_under_an_inch_keeps_no_margin_on_that_axis() {
    for width in [ONE_IN_PX, ONE_IN_PX - 1.0] {
      let margins = PageMargins::AUTO.resolve((width, 4.0 * ONE_IN_PX), None, None);

      assert_eq!(margins.left, 0.0, "width of {width}");
      assert_eq!(margins.right, 0.0, "width of {width}");
      assert_eq!(margins.top, PageOptions::DEFAULT_MARGIN);
      assert_eq!(margins.bottom, PageOptions::DEFAULT_MARGIN);
    }
    assert_eq!(
      PageMargins::AUTO
        .resolve((ONE_IN_PX + 1.0, 4.0 * ONE_IN_PX), None, None)
        .left,
      PageOptions::DEFAULT_MARGIN,
      "an axis past the inch keeps its margin"
    );
  }
}
