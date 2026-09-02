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
//!   per-layer blend modes; an inline span fills `background-color` per line
//!   fragment, `box-decoration-break: slice` style (the drifts are listed on
//!   [`takumi_core::layout::inline::InlineBackgroundFragment`])
//! - borders, border-radius, `outline`, and `box-shadow` (blur approximated
//!   by bands)
//! - text with decorations, `text-shadow`, and `-webkit-text-stroke`
//! - images, with `object-fit` and `object-position`
//! - `clip-path`, `mask-image`, and the color `filter` primitives
//! - opacity, blend modes, overflow clipping, and affine transforms
//!
//! `filter: blur()` and `drop-shadow()` have no PDF equivalent. [`render`]
//! stops and names the function.

use std::{mem::take, rc::Rc};

mod atoms;
mod background;
mod bands;
mod counters;

pub use counters::counter_characters;
mod emitter;
mod filter;
mod form;
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
mod window;

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
  layout::tree::RenderNode,
  style::{Affine, Color, Lang},
  viewport::{MediaTarget, Viewport},
};

/// Written as `/Producer` and `pdf:Producer` in every document takumi renders.
pub const PRODUCER: &str = concat!("takumi-pdf ", env!("CARGO_PKG_VERSION"));

pub use crate::options::{
  Attachment, AttachmentRelationship, MeasureOptions, MeasuredSize, PageMargin, PageMargins,
  PageOptions, PageRange, PdfDate, PdfError, PdfMetadata, PdfOptions, PdfStandard, Tagging,
  XmpProperty, XmpSchema,
};
use crate::{
  emitter::DocumentState,
  form::add_field_annotations,
  inline::{TextBox, build_inline_map},
  interactive::{Interactive, add_link_annotations},
  krilla::{
    Document, SerializeSettings,
    configure::ConfigurationBuilder,
    destination::XyzDestination,
    embed::{EmbeddedFile, MimeType},
    geom::{Point, Size as KrillaSize, Transform},
    page::PageSettings,
  },
  options::{PT_PER_PX, PageSelection, build_metadata, krilla_datetime, validate_xmp_schemas},
  page::{PageBands, PagePlan},
  paint::paint_page_background,
  tags::tag_id,
  tree::{PreparedTree, TreeInputs},
  window::Window,
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
    form: false,
  };
  let tree = match (options.page, options.viewport) {
    (Some(page), _) => inputs.prepare_band(
      &options.node,
      999,
      999,
      Viewport::new((page.width as u32, None)).with_media_target(MediaTarget::Print),
    )?,
    (None, Some(viewport)) => {
      inputs.prepare(options.node, viewport.with_media_target(MediaTarget::Print))?
    }
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
pub fn render(mut options: PdfOptions<'_>) -> Result<Vec<u8>, PdfError> {
  let attachments = take(&mut options.attachments);
  let mut pdf = open_document(&options, attachments)?;
  let inputs = TreeInputs {
    fonts: options.fonts,
    stylesheet: options.stylesheet,
    images: Rc::new(options.images),
    font_families: options.font_families,
    lang: options.lang,
    form: options.form,
  };
  let tagged = options.tagged != Tagging::Off || options.standard.requires_tagging();
  let state = DocumentState::new(tagged, inputs.lang.as_ref().map(Lang::as_str), options.form);
  let structural = options.tagged.names_structure_destinations();
  let rendered = match options.page {
    Some(page) => {
      let bands = PageBands {
        header: options.header.as_ref(),
        footer: options.footer.as_ref(),
        page_ranges: options.page_ranges.as_deref(),
      };
      let plan = PagePlan::solve(&inputs, page, bands, options.node, structural)?;

      plan.compose(&mut pdf, &inputs, &state, options.background_color)?;
      Rendered::Paged(Box::new(plan))
    }
    None => {
      // A viewport render is one page, so the ranges only have page 1 to keep.
      PageSelection::resolve(options.page_ranges.as_deref(), 1)?;
      let viewport = options
        .viewport
        .ok_or(PdfError::MissingViewport)?
        .with_media_target(MediaTarget::Print);
      let content = inputs.prepare(options.node, viewport)?;
      let page = SinglePage::compose(
        &mut pdf,
        content,
        &state,
        options.background_color,
        structural,
      )?;

      Rendered::Single(Box::new(page))
    }
  };

  if (options.outline || options.tagged.requires_outline())
    && !rendered.interactive().headings.is_empty()
  {
    pdf.set_outline(
      rendered
        .interactive()
        .outline(|heading| rendered.destination(heading.top, &heading.path)),
    );
  }
  if let Some(collector) = &state.tags {
    pdf.set_tag_tree(collector.borrow_mut().build_tree(
      rendered.root(),
      state.lang,
      &rendered.interactive().destination_targets(),
      state.form,
    ));
  }
  if let Some(error) = state.into_error() {
    return Err(error);
  }

  pdf.finish().map_err(PdfError::Krilla)
}

/// A document with its validators, metadata and attachments set, before any
/// page is drawn.
fn open_document(
  options: &PdfOptions<'_>,
  attachments: Vec<Attachment>,
) -> Result<Document, PdfError> {
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
  let settings = SerializeSettings {
    producer: options
      .producer
      .clone()
      .unwrap_or_else(|| PRODUCER.to_string()),
    ..SerializeSettings::default()
  };
  let mut document = if validated {
    let configuration = builder.finish().map_err(|_| PdfError::InvalidStandard)?;

    Document::new_with(SerializeSettings {
      configuration,
      ..settings
    })
  } else {
    Document::new_with(settings)
  };
  let tagged = options.tagged != Tagging::Off || options.standard.requires_tagging();

  if let Some(metadata) = &options.metadata {
    validate_xmp_schemas(&metadata.xmp)?;
    document.set_metadata(build_metadata(metadata, options.lang));
  } else if tagged && options.lang.is_some() {
    // Tagged standards check the document language even without metadata.
    document.set_metadata(build_metadata(&PdfMetadata::default(), options.lang));
  }

  let fallback_date = options.metadata.as_ref().and_then(|m| m.creation_date);

  for attachment in attachments {
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
  Ok(document)
}

/// What the pages were drawn from, which the outline and structure tree read
/// after them.
enum Rendered {
  Paged(Box<PagePlan>),
  Single(Box<SinglePage>),
}

/// A viewport render: one page at the content's size.
struct SinglePage {
  content: PreparedTree,
  interactive: Interactive,
  structural: bool,
}

impl Rendered {
  fn root(&self) -> &RenderNode {
    match self {
      Self::Paged(plan) => &plan.paginated.content.root,
      Self::Single(page) => &page.content.root,
    }
  }

  fn interactive(&self) -> &Interactive {
    match self {
      Self::Paged(plan) => &plan.paginated.interactive,
      Self::Single(page) => &page.interactive,
    }
  }

  fn destination(&self, top: f32, path: &[usize]) -> Option<XyzDestination> {
    match self {
      Self::Paged(plan) => plan.destination(top, path),
      Self::Single(page) => Some(page.destination(top, path)),
    }
  }
}

impl SinglePage {
  /// Draws the one page and collects its interactive targets.
  fn compose(
    pdf: &mut Document,
    content: PreparedTree,
    state: &DocumentState<'_>,
    background: Option<Color>,
    structural: bool,
  ) -> Result<Self, PdfError> {
    let page_size = KrillaSize::from_wh(content.width * PT_PER_PX, content.height * PT_PER_PX)
      .ok_or(PdfError::InvalidPageSize)?;
    let text_boxes = TextBox::collect(&content);
    let inline_map = build_inline_map(&text_boxes)?;
    let mut page = pdf.start_page_with(PageSettings::new(page_size));
    let mut surface = page.surface();

    surface.push_transform(&Transform::from_scale(PT_PER_PX, PT_PER_PX));
    paint_page_background(background, (content.width, content.height), &mut surface);

    let mut emitter = content.emitter(state, Some(&inline_map), true);

    emitter.emit_context(0, Affine::IDENTITY, &mut surface)?;
    surface.pop();
    surface.finish();
    let rendered = Self {
      interactive: Interactive::collect(&content),
      content,
      structural,
    };

    add_link_annotations(
      &mut page,
      &rendered.interactive.links,
      Window {
        y: Some((0.0, rendered.content.height)),
        ..Window::default()
      },
      (0.0, 0.0),
      state.tags.as_ref(),
      |id| {
        rendered
          .interactive
          .anchors
          .get(id)
          .map(|anchor| rendered.destination(anchor.top, &anchor.path))
      },
    );
    add_field_annotations(
      &mut page,
      &rendered.interactive.fields,
      &rendered.interactive.labels,
      Window {
        y: Some((0.0, rendered.content.height)),
        ..Window::default()
      },
      (0.0, 0.0),
      state,
    );
    page.finish();
    Ok(rendered)
  }

  fn destination(&self, top: f32, path: &[usize]) -> XyzDestination {
    let dest = XyzDestination::new(0, Point::from_xy(0.0, top.max(0.0) * PT_PER_PX));

    match self.structural {
      true => dest.with_structure(tag_id(path)),
      false => dest,
    }
  }
}

#[cfg(test)]
mod tests {
  use takumi_core::units::ONE_IN_PX;

  use super::*;
  use crate::{
    atoms::Atoms,
    options::BAND_EDGE_PADDING,
    pagination::{Atom, MAX_PAGES, Paragraph},
  };

  fn atoms(extents: &[Atom], forced: &[f32], paragraphs: Vec<Paragraph>) -> Atoms {
    Atoms {
      extents: extents.to_vec(),
      forced: forced.to_vec(),
      paragraphs,
    }
  }

  const A4: (f32, f32) = (PageOptions::A4.width, PageOptions::A4.height);

  #[test]
  fn page_selection_maps_kept_pages_to_output_order() {
    let ranges = [
      PageRange::single(1),
      PageRange {
        from: Some(3),
        to: None,
      },
    ];
    let selection = PageSelection::resolve(Some(&ranges), 5).unwrap();

    assert_eq!(
      (0..5)
        .map(|index| selection.emitted(index))
        .collect::<Vec<_>>(),
      vec![Some(0), None, Some(1), Some(2), Some(3)]
    );
    assert!(selection.keeps(0));
    assert!(!selection.keeps(1));
  }

  #[test]
  fn page_selection_without_ranges_keeps_every_page() {
    let selection = PageSelection::resolve(None, 3).unwrap();

    assert_eq!(selection.emitted(2), Some(2));
  }

  #[test]
  fn page_selection_rejects_invalid_and_out_of_bounds_ranges() {
    assert!(matches!(
      PageSelection::resolve(
        Some(&[PageRange {
          from: Some(4),
          to: Some(2)
        }]),
        5
      ),
      Err(PdfError::InvalidPageRange(_))
    ));
    assert!(matches!(
      PageSelection::resolve(Some(&[PageRange::single(0)]), 5),
      Err(PdfError::InvalidPageRange(_))
    ));
    assert!(matches!(
      PageSelection::resolve(
        Some(&[PageRange {
          from: Some(6),
          to: None
        }]),
        5
      ),
      Err(PdfError::PageRangesOutOfBounds(5))
    ));
  }

  #[test]
  fn content_taller_than_a_render_allows_stops_counting() {
    let starts = Atoms::default().page_starts(&[], 2_000_000.0, 10.0);

    assert_eq!(starts.len(), MAX_PAGES, "the page count runs unbounded");
  }

  #[test]
  fn page_starts_without_atoms_cuts_at_window() {
    assert_eq!(
      Atoms::default().page_starts(&[], 250.0, 100.0),
      vec![0.0, 100.0, 200.0]
    );
  }

  #[test]
  fn page_starts_pushes_straddling_atom_to_next_page() {
    assert_eq!(
      atoms(&[(90.0, 110.0)], &[], Vec::new()).page_starts(&[], 250.0, 100.0),
      vec![0.0, 90.0, 190.0]
    );
  }

  #[test]
  fn page_starts_hard_cuts_atom_taller_than_window() {
    assert_eq!(
      atoms(&[(0.0, 300.0)], &[], Vec::new()).page_starts(&[], 300.0, 100.0),
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
    let paragraphs = vec![Paragraph {
      lines: lines.clone(),
      before: 2,
      after: 2,
    }];

    assert_eq!(
      atoms(&lines, &[], paragraphs).page_starts(&[], 110.0, 100.0),
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
    let paragraphs = vec![Paragraph {
      lines: lines.clone(),
      before: 2,
      after: 2,
    }];

    assert_eq!(
      atoms(&lines, &[], paragraphs).page_starts(&[], 140.0, 100.0),
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
    let paragraphs = vec![Paragraph {
      lines: lines.clone(),
      before: 2,
      after: 2,
    }];

    assert_eq!(
      atoms(&lines, &[], paragraphs).page_starts(&[], 110.0, 100.0),
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
    let paragraphs = vec![Paragraph {
      lines: lines.clone(),
      before: 20,
      after: 20,
    }];

    assert_eq!(
      atoms(&lines, &[], paragraphs).page_starts(&[], 300.0, 100.0),
      vec![0.0, 100.0, 200.0],
      "a paragraph that can never satisfy 20/20 still paginates at the window"
    );
  }

  /// `widows` past the paragraph's line count resolves to the orphans floor,
  /// like any other unsatisfiable widows value, instead of underflowing.
  #[test]
  fn page_starts_floors_widows_past_the_line_count() {
    let lines: Vec<Atom> = (0..3)
      .map(|i| (80.0 + i as f32 * 10.0, 90.0 + i as f32 * 10.0))
      .collect();
    let paragraphs = vec![Paragraph {
      lines: lines.clone(),
      before: 1,
      after: 5,
    }];

    assert_eq!(
      atoms(&lines, &[], paragraphs).page_starts(&[], 110.0, 100.0),
      vec![0.0, 90.0]
    );
  }

  #[test]
  fn page_starts_honors_forced_cuts() {
    assert_eq!(
      atoms(&[], &[40.0, 150.0], Vec::new()).page_starts(&[], 250.0, 100.0),
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
