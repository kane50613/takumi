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

use std::{cell::RefCell, rc::Rc};

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
  style::{Affine, Lang},
  viewport::Viewport,
};

pub use crate::options::{
  Attachment, AttachmentRelationship, MeasureOptions, MeasuredSize, PageMargins, PageOptions,
  PdfDate, PdfError, PdfMetadata, PdfOptions, PdfStandard, Tagging, XmpProperty, XmpSchema,
};
use crate::{
  bands::{emit_band, measure_band, prepare_band},
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
    paint::FillRule,
  },
  options::{BAND_EDGE_PADDING, PT_PER_PX, build_metadata, krilla_datetime, validate_xmp_schemas},
  pagination::{MAX_PAGES, page_starts, resolve_target_counters},
  paint::rect_path,
  tags::{TagCollector, build_tag_tree, tag_id},
  tree::{TreeInputs, prepare_paged_tree, prepare_tree},
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
      let page_size = KrillaSize::from_wh(page.width * PT_PER_PX, page.height * PT_PER_PX)
        .ok_or(PdfError::InvalidPageSize)?;
      let (content_width, content_height) = page.content_size();
      if !(content_width.is_finite()
        && content_height.is_finite()
        && content_width > 0.0
        && content_height > 0.0)
      {
        return Err(PdfError::InvalidPageSize);
      }
      // Bands lay out at full page width and draw inside the margin areas,
      // like Chromium's print header and footer templates. The content window
      // is always the full margin box; a band taller than its margin overlaps
      // content, exactly as in Chromium.
      let band_viewport = Viewport::new((page.width as u32, None));
      let content_viewport = Viewport::new((content_width as u32, None));

      // Band heights are measured once with three-digit counters; per-page
      // emission clips to the measured band, so a wrap caused by a wider real
      // counter cannot move the band box between pages. A band without counter
      // hooks reuses the measured layout on every page.
      let header_band = options
        .header
        .as_ref()
        .map(|template| measure_band(&inputs, template, band_viewport))
        .transpose()?;
      let footer_band = options
        .footer
        .as_ref()
        .map(|template| measure_band(&inputs, template, band_viewport))
        .transpose()?;
      let header_height = header_band
        .as_ref()
        .map_or(0.0, |band| band.measured.height);
      let footer_height = footer_band
        .as_ref()
        .map_or(0.0, |band| band.measured.height);
      let window_height = content_height;

      let mut node = options.node;

      resolve_target_counters(
        &mut node,
        &inputs,
        &mut fonts,
        &issues,
        document_lang,
        content_viewport,
        window_height,
      )?;

      let page_area = Viewport::new((content_width as u32, content_height as u32));
      let (content, repeated) = prepare_paged_tree(&inputs, node, content_viewport, page_area)?;
      let repeated_links: Vec<_> = repeated
        .iter()
        .flat_map(|tree| collect_interactive(tree).links)
        .collect();
      let text_boxes = collect_text_boxes(&content);
      let inline_map = build_inline_map(&text_boxes)?;
      let mut atoms = Vec::new();
      let mut forced = Vec::new();
      let mut paragraphs = Vec::new();

      content
        .emitter(&mut fonts, Some(&inline_map), None, &issues, document_lang)
        .collect_atoms(
          0,
          Affine::IDENTITY,
          &mut atoms,
          &mut forced,
          &mut paragraphs,
        )?;
      let starts = page_starts(
        &mut atoms,
        &mut forced,
        &paragraphs,
        content.height,
        window_height,
      );

      if starts.len() >= MAX_PAGES {
        return Err(PdfError::TooManyPages(starts.len()));
      }
      let pages = starts.len();
      let interactive = collect_interactive(&content);
      let structural = options.tagged.names_structure_destinations();
      let destination = |top: f32, path: &[usize]| {
        let index = starts
          .partition_point(|start| *start <= top)
          .saturating_sub(1);
        let y = page.margin.top + (top - starts[index]).max(0.0);
        let dest = XyzDestination::new(
          index,
          Point::from_xy(page.margin.left * PT_PER_PX, y * PT_PER_PX),
        );

        match structural {
          true => dest.with_structure(tag_id(path)),
          false => dest,
        }
      };

      for (index, &y0) in starts.iter().enumerate() {
        let mut pdf_page = document.start_page_with(PageSettings::new(page_size));
        let mut surface = pdf_page.surface();

        surface.push_transform(&Transform::from_scale(PT_PER_PX, PT_PER_PX));
        if let (Some(band), Some(template)) = (&header_band, &options.header) {
          let prepared;
          let tree = if band.dynamic {
            prepared = prepare_band(&inputs, template, index + 1, pages, band_viewport)?;
            &prepared
          } else {
            &band.measured
          };

          emit_band(
            tree,
            &mut fonts,
            (0.0, BAND_EDGE_PADDING, page.width, header_height),
            tag_collector.is_some(),
            &issues,
            document_lang,
            &mut surface,
          )?;
        }

        let content_top = page.margin.top;
        // Paint stops at the next cut: the region between a raised cut and the
        // page's full height belongs to the next page and stays blank, exactly
        // like browser print fragmentation.
        let next_start = starts.get(index + 1).copied().unwrap_or(f32::INFINITY);
        let paint_height = (next_start - y0).min(window_height);

        if let Some(path) =
          KrillaRect::from_xywh(page.margin.left, content_top, content_width, paint_height)
            .and_then(rect_path)
        {
          surface.push_clip_path(&path, &FillRule::NonZero);
          surface.push_transform(&Transform::from_translate(
            page.margin.left,
            content_top - y0,
          ));
          let mut emitter = content.emitter(
            &mut fonts,
            Some(&inline_map),
            tag_collector.as_ref(),
            &issues,
            document_lang,
          );

          emitter.window = Some((y0, y0 + paint_height));
          emitter.line_window = Some((if index == 0 { f32::NEG_INFINITY } else { y0 }, next_start));
          emitter.emit_context(0, Affine::IDENTITY, &mut surface)?;
          surface.pop();
          surface.pop();
        }

        for fixed in &repeated {
          emit_band(
            fixed,
            &mut fonts,
            (page.margin.left, content_top, content_width, window_height),
            tag_collector.is_some(),
            &issues,
            document_lang,
            &mut surface,
          )?;
        }

        if let (Some(band), Some(template)) = (&footer_band, &options.footer) {
          let prepared;
          let tree = if band.dynamic {
            prepared = prepare_band(&inputs, template, index + 1, pages, band_viewport)?;
            &prepared
          } else {
            &band.measured
          };

          emit_band(
            tree,
            &mut fonts,
            (
              0.0,
              page.height - BAND_EDGE_PADDING - footer_height,
              page.width,
              footer_height,
            ),
            tag_collector.is_some(),
            &issues,
            document_lang,
            &mut surface,
          )?;
        }

        surface.pop();
        surface.finish();
        add_link_annotations(
          &mut pdf_page,
          &interactive.links,
          (y0, y0 + paint_height),
          (page.margin.left, content_top),
          tag_collector.as_ref(),
          |id| {
            interactive
              .anchors
              .get(id)
              .map(|anchor| destination(anchor.top, &anchor.path))
          },
        );
        // A repeated box sits at the same place on every page, so its links are
        // added per page against the page area rather than the content window.
        add_link_annotations(
          &mut pdf_page,
          &repeated_links,
          (0.0, window_height),
          (page.margin.left, content_top),
          tag_collector.as_ref(),
          |id| {
            interactive
              .anchors
              .get(id)
              .map(|anchor| destination(anchor.top, &anchor.path))
          },
        );
        pdf_page.finish();
      }

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
  use super::*;
  use crate::pagination::Atom;

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
    let a4 = PageOptions::A4;

    assert!((a4.width - 793.7).abs() < 0.1);
    assert!((a4.height - 1122.5).abs() < 0.1);

    let letter = PageOptions::LETTER;

    assert_eq!((letter.width, letter.height), (816.0, 1056.0));

    let landscape = PageOptions::A4.landscape();

    assert_eq!(landscape.width, a4.height);
    assert_eq!(landscape.height, a4.width);
    assert_eq!(PageOptions::A4.with_margin(0.0).margin.top, 0.0);
  }
}
