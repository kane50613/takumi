#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Writes vector PDFs from HTML or a takumi node tree, without a browser. Text stays selectable and
//! searchable because the file embeds a subset of each font it uses. Layout comes from
//! `takumi-core`, the same engine that renders takumi images.
//!
//! JavaScript projects use the [`takumi-pdf`](https://www.npmjs.com/package/takumi-pdf) npm
//! package, which wraps this crate.
//!
//! # Example
//!
//! This renders an HTML file to an A4 PDF. It needs `takumi-core` with the `html` feature next to
//! `takumi-pdf`. Takumi loads no system fonts, so register every font the document uses.
//!
//! ```rust,no_run
//! use std::{error::Error, fs, sync::Arc};
//!
//! use takumi_core::{
//!   Fonts,
//!   html::{FromHtmlOptions, parse_html},
//!   resources::font::FontResource,
//!   style::StyleSheet,
//! };
//! use takumi_pdf::{PageOptions, PdfOptions, render};
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!   let mut fonts = Fonts::default();
//!   fonts.register(FontResource::new(fs::read("Geist.woff2")?))?;
//!
//!   let html = parse_html(&fs::read_to_string("invoice.html")?, FromHtmlOptions::default())?;
//!   let pdf = render(
//!     PdfOptions::builder()
//!       .node(html.node)
//!       .stylesheet(Arc::new(StyleSheet::parse_owned_list_loosy(html.css)))
//!       .page(PageOptions::A4)
//!       .fonts(&fonts)
//!       .build(),
//!   )?;
//!
//!   fs::write("invoice.pdf", pdf)?;
//!   Ok(())
//! }
//! ```
//!
//! [`parse_html`] returns the tree in `node` and the CSS of the document's `<style>` elements in
//! `css`, which the example passes as the stylesheet.
//!
//! Without [`PdfOptions::page`], the document is a single page the size of
//! [`PdfOptions::viewport`]. [`measure`] lays out a tree without writing a PDF, for example to find
//! how tall a header band is. [`needs`] lists the images and fonts a tree needs fetched before it
//! renders.
//!
//! # Pages
//!
//! With [`PdfOptions::page`] set, content flows across pages as Chromium lays out a printed
//! document. Lines, boxes, flex and grid items, and table rows break between pages. Content that
//! cannot break, such as an image, moves to the next page, and continues over the following pages
//! when it is taller than one.
//!
//! Pagination honors `break-before: page`, `break-after: page`, `break-inside: avoid`, and the
//! `widows` and `orphans` minimums, which default to 2 as in Chromium. A table repeats its header
//! or footer group on every page it crosses when the group has `break-inside: avoid`, which
//! `<thead>` and `<tfoot>` have by default, and is at most a quarter of the page tall.
//!
//! [`PdfOptions::header`] and [`PdfOptions::footer`] repeat a band in the top and bottom page
//! margins, like Chromium's print headers. A band is a node tree, such as the `node` of another
//! `parse_html` call. Inside a band, takumi fills elements with the class `pageNumber` or
//! `totalPages` with the current page number or the page count.
//!
//! # Document features
//!
//! Besides the pages, [`PdfOptions`] sets what else the file carries:
//!
//! - Links become link annotations, and [`PdfOptions::outline`] builds an outline from `h1`–`h6`.
//! - [`PdfOptions::metadata`] writes the document metadata.
//! - [`PdfOptions::tagged`] writes a structure tree, which is on by default.
//! - [`PdfOptions::standard`] makes the file conform to PDF/A, and the render fails when the file
//!   does not validate.
//! - [`PdfOptions::attachments`] embeds files.
//! - [`PdfOptions::page_ranges`] keeps only some of the pages.
//!
//! # What draws
//!
//! These CSS features draw in the PDF:
//!
//! - Backgrounds: colors, gradients, and `url()` layers, with `background-size`,
//!   `background-position`, `background-repeat`, `background-origin`, `background-clip` (including
//!   `text`), and a blend mode per layer.
//! - Borders, `border-radius`, `outline`, and `box-shadow`.
//! - Text with decorations, `text-shadow`, and `-webkit-text-stroke`.
//! - Images, with `object-fit` and `object-position`.
//! - `clip-path`, `mask-image`, and the color `filter` functions.
//! - Opacity, blend modes, overflow clipping, and affine transforms.
//!
//! Two of these are approximate. Shadow blur is drawn as bands. An inline element fills its
//! `background-color` for each line separately, as `box-decoration-break: slice` does.
//! [`InlineBackgroundFragment`] lists where that differs from Chromium.
//!
//! A PDF has no operator for `filter: blur()` or `drop-shadow()`, so [`render`] fails with
//! [`PdfError::UnsupportedFilter`] and names the function. `backdrop-filter` is ignored.
//!
//! [`InlineBackgroundFragment`]: https://docs.rs/takumi-core/latest/takumi_core/layout/inline/struct.InlineBackgroundFragment.html
//! [`PdfError::UnsupportedFilter`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/enum.PdfError.html#variant.UnsupportedFilter
//! [`PdfOptions`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html
//! [`PdfOptions::attachments`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.attachments
//! [`PdfOptions::footer`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.footer
//! [`PdfOptions::header`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.header
//! [`PdfOptions::metadata`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.metadata
//! [`PdfOptions::outline`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.outline
//! [`PdfOptions::page`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.page
//! [`PdfOptions::page_ranges`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.page_ranges
//! [`PdfOptions::standard`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.standard
//! [`PdfOptions::tagged`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.tagged
//! [`PdfOptions::viewport`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.viewport
//! [`measure`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/fn.measure.html
//! [`needs`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/fn.needs.html
//! [`parse_html`]: https://docs.rs/takumi-core/latest/takumi_core/html/fn.parse_html.html
//! [`render`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/fn.render.html
use std::{mem::take, rc::Rc};

mod bands;
mod chunks;
mod counters;
mod emitter;
mod filter;
mod glyph;
mod inline;
mod interactive;
#[cfg(test)]
mod memory_tests;
mod options;
mod page;
mod pagination;
mod paint;
#[cfg(feature = "images")]
mod raster;
mod shadow;
#[cfg(test)]
mod streaming_tests;
#[cfg(all(feature = "svg", feature = "images"))]
mod svg;
mod tags;
#[cfg(test)]
mod test_support;
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
  geometry::Point as CorePoint,
  layout::{needs::RenderNeeds, tree::RenderNode},
  page_stream::{SETTLED_PAGES, Settle},
  style::{Color, Lang},
  viewport::MediaTarget,
};

/// Written as `/Producer` and `pdf:Producer` in every document takumi renders.
pub const PRODUCER: &str = concat!("takumi-pdf ", env!("CARGO_PKG_VERSION"));

pub use crate::options::{
  Attachment, AttachmentRelationship, Band, MeasureOptions, MeasuredSize, PageBand, PageMargin,
  PageMargins, PageOptions, PageOverride, PageRange, PageRules, PdfDate, PdfError, PdfMetadata,
  PdfOptions, PdfStandard, Tagging, UncoveredText, XmpProperty, XmpSchema,
};
use crate::{
  counters::substitute_counter_characters,
  emitter::{DocumentState, Emitter},
  glyph::Uncovered,
  inline::{InlineCache, TextBox},
  interactive::{Interactive, PageSummary, add_link_annotations, xyz_destination},
  krilla::{
    Document, SerializeSettings,
    configure::ConfigurationBuilder,
    destination::XyzDestination,
    geom::{Point, Size as KrillaSize, Transform},
    page::PageSettings,
  },
  options::{PT_PER_PX, PageSelection},
  page::{PageBands, PageDrawing, PagePlan, band_viewport},
  paint::paint_page_background,
  tree::{PreparedTree, TreeInputs},
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
    (Some(page), _) => inputs.prepare_band(&options.node, 999, 999, band_viewport(&page))?,
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

/// What a node tree needs fetched before it renders, with page counter hooks filled. See [`RenderNode::needs`].
pub fn needs(options: MeasureOptions<'_>) -> Result<RenderNeeds, PdfError> {
  let inputs = TreeInputs {
    fonts: options.fonts,
    stylesheet: options.stylesheet,
    images: Rc::new(options.images),
    font_families: options.font_families,
    lang: options.lang,
  };
  let viewport = match (options.page, options.viewport) {
    (Some(page), _) => band_viewport(&page),
    (None, Some(viewport)) => viewport.with_media_target(MediaTarget::Print),
    (None, None) => return Err(PdfError::MissingViewport),
  };

  let mut node = options.node;

  substitute_counter_characters(&mut node);

  Ok(RenderNode::from_initial_containing_block(&inputs.context(viewport), node).needs())
}

/// Renders a node tree to a PDF: single-page at the viewport size, or paged
/// when [`PdfOptions::page`] is set.
pub fn render(options: PdfOptions<'_>) -> Result<Vec<u8>, PdfError> {
  render_settling(options, SETTLED_PAGES)
}

/// [`render`] with `settle`, given the layout, saying how many of the first pages of a document
/// that streams are safe to draw.
fn render_settling(mut options: PdfOptions<'_>, settle: Settle<'_>) -> Result<Vec<u8>, PdfError> {
  let attachments = take(&mut options.attachments);
  let mut pdf = open_document(&options, attachments)?;
  let tagged = options.writes_structure();
  let inputs = TreeInputs {
    fonts: options.fonts,
    stylesheet: options.stylesheet,
    images: Rc::new(options.images),
    font_families: options.font_families,
    lang: options.lang,
  };
  let uncovered = Uncovered::new(
    options.uncovered_text,
    options
      .standard
      .forbids_placeholder()
      .or_else(|| options.tagged.forbids_placeholder()),
  );
  let state = DocumentState::new(tagged, inputs.lang.as_ref().map(Lang::as_str), uncovered);
  let structural = options.tagged.names_structure_destinations();
  let rendered = match options.page {
    Some(page) => {
      let bands = PageBands {
        header: options.pages.header_band(options.header.as_ref()),
        footer: options.pages.footer_band(options.footer.as_ref()),
        page_ranges: options.page_ranges.as_deref(),
      };
      let plan = PageDrawing {
        pdf: &mut pdf,
        inputs: &inputs,
        state: &state,
        background: options.background_color,
      }
      .render(page, bands, options.node, structural, settle)?;

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
    && !rendered.summary().headings.is_empty()
  {
    pdf.set_outline(
      rendered
        .summary()
        .outline(|heading| rendered.destination(heading.top, &heading.path)),
    );
  }
  if let Some(collector) = &state.tags {
    pdf.set_tag_tree(collector.borrow_mut().build_tree(
      rendered.root(),
      state.lang,
      &rendered.summary().destination_targets(),
    ));
  }
  // The layout and scene are done with; serializing the fonts and objects needs neither.
  drop(rendered);
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

  if let Some(metadata) = &options.metadata {
    document.set_metadata(metadata.metadata(options.lang)?);
  } else if options.writes_structure() && options.lang.is_some() {
    // Tagged standards check the document language even without metadata.
    document.set_metadata(PdfMetadata::default().metadata(options.lang)?);
  }

  let fallback_date = options.metadata.as_ref().and_then(|m| m.creation_date);

  for attachment in attachments {
    let name = attachment.name.clone();

    document
      .embed_file(attachment.embedded_file(fallback_date)?)
      .ok_or(PdfError::DuplicateAttachment(name))?;
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
      Self::Paged(plan) => &plan.root,
      Self::Single(page) => &page.content.scene.root,
    }
  }

  fn summary(&self) -> &PageSummary {
    match self {
      Self::Paged(plan) => &plan.summary,
      Self::Single(page) => &page.interactive.summary,
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
    let page_size = KrillaSize::from_wh(
      content.scene.size.width * PT_PER_PX,
      content.scene.size.height * PT_PER_PX,
    )
    .ok_or(PdfError::InvalidPageSize)?;
    let text_boxes = TextBox::collect(content.view());
    let inline = InlineCache::new(&text_boxes);
    let mut page = pdf.start_page_with(PageSettings::new(page_size));
    let mut surface = page.surface();

    surface.push_transform(&Transform::from_scale(PT_PER_PX, PT_PER_PX));
    paint_page_background(background, content.scene.size, &mut surface);

    let mut emitter = Emitter::new(content.view(), state, Some(&inline), None, true);

    emitter.emit(&mut surface)?;
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
      (0.0, rendered.content.scene.size.height),
      CorePoint::ZERO,
      state.tags.as_ref(),
      |id| {
        rendered
          .interactive
          .summary
          .anchors
          .get(id)
          .map(|anchor| rendered.destination(anchor.top, &anchor.path))
      },
    );
    page.finish();
    Ok(rendered)
  }

  fn destination(&self, top: f32, path: &[usize]) -> XyzDestination {
    xyz_destination(
      0,
      Point::from_xy(0.0, top.max(0.0) * PT_PER_PX),
      path,
      self.structural,
    )
  }
}

#[cfg(test)]
mod tests {
  use takumi_core::{geometry::Size, units::ONE_IN_PX};

  use super::*;
  use crate::options::BAND_EDGE_PADDING;

  const A4: Size<f32> = PageOptions::A4.size();

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

  /// Chromium drops the default margin on an axis of at most an inch.
  #[test]
  fn a_page_under_an_inch_keeps_no_margin_on_that_axis() {
    for width in [ONE_IN_PX, ONE_IN_PX - 1.0] {
      let margins = PageMargins::AUTO.resolve(Size::new(width, 4.0 * ONE_IN_PX), None, None);

      assert_eq!(margins.left, 0.0, "width of {width}");
      assert_eq!(margins.right, 0.0, "width of {width}");
      assert_eq!(margins.top, PageOptions::DEFAULT_MARGIN);
      assert_eq!(margins.bottom, PageOptions::DEFAULT_MARGIN);
    }
    assert_eq!(
      PageMargins::AUTO
        .resolve(Size::new(ONE_IN_PX + 1.0, 4.0 * ONE_IN_PX), None, None)
        .left,
      PageOptions::DEFAULT_MARGIN,
      "an axis past the inch keeps its margin"
    );
  }
}
