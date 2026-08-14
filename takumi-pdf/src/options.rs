//! Public option, metadata and error types for the render and measure entry
//! points, plus the page geometry constants.

use std::{collections::HashMap, sync::Arc};

use takumi_core::{
  Fonts,
  error::Error as TakumiError,
  layout::node::Node,
  resources::{font::FontError, image::ImageSource},
  style::{Color, FontFamily, Lang, StyleSheet},
  units::{ONE_IN_PX, ONE_MM_IN_PX, ONE_PT_IN_PX},
  viewport::Viewport,
};
use typed_builder::TypedBuilder;

use crate::krilla::{
  configure::{Accessibility, Archival},
  embed::AssociationKind,
  error::KrillaError,
  metadata::{DateTime, Metadata},
};

/// Errors from [`crate::render`].
#[derive(Debug)]
pub enum PdfError {
  /// Layout or resource resolution failed in takumi-core.
  Render(TakumiError),
  /// Font data could not be interpreted.
  Font(FontError),
  /// PDF serialization failed.
  Krilla(KrillaError),
  /// The computed page size is empty or non-finite.
  InvalidPageSize,
  /// Single-page output ([`PdfOptions::page`] unset) needs a viewport.
  MissingViewport,
  /// The requested archival standard could not be configured.
  InvalidStandard,
  /// An attachment's mime type is not a valid `type/subtype` pair.
  InvalidMimeType(String),
  /// Two attachments share the same file name.
  DuplicateAttachment(String),
  /// An XMP schema carries a prefix, property name or namespace URI that
  /// cannot be written into XML.
  InvalidXmpSchema(String),
  /// The content would cut into more pages than one render may produce.
  TooManyPages(usize),
  /// A `filter` function a PDF cannot express, named as a stylesheet writes
  /// it. It would silently drop the effect, so the render stops instead.
  UnsupportedFilter(String),
  /// An image arrived as bytes this build cannot turn into pixels. It would
  /// leave a hole where the page expects a picture, so the render stops.
  UndrawableImage(String),
  /// No registered font covers these characters. They would draw nothing and
  /// leave no trace in the text layer, so the render stops instead.
  MissingGlyphs(String),
}

impl std::fmt::Display for PdfError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Render(error) => write!(f, "{error}"),
      Self::Font(error) => write!(f, "Font error: {error}"),
      Self::Krilla(error) => write!(f, "Writing the PDF failed: {error}"),
      Self::InvalidPageSize => f.write_str("Page size must be finite and larger than zero"),
      Self::MissingViewport => {
        f.write_str("Single-page output needs a viewport. Pass `viewport` or `size`.")
      }
      Self::InvalidStandard => f.write_str("The requested archival standard is not available"),
      Self::InvalidMimeType(mime) => {
        write!(f, "Attachment mime type is not a type/subtype pair: {mime}")
      }
      Self::DuplicateAttachment(name) => write!(f, "Two attachments are named {name}"),
      Self::InvalidXmpSchema(schema) => write!(f, "XMP schema cannot be written as XML: {schema}"),
      Self::TooManyPages(pages) => write!(
        f,
        "The content spans more than {pages} pages. Give it a height that resolves."
      ),
      Self::UnsupportedFilter(filter) => {
        write!(f, "A PDF cannot draw filter: {filter}")
      }
      Self::UndrawableImage(reason) => write!(f, "Image could not be decoded: {reason}"),
      Self::MissingGlyphs(characters) => write!(
        f,
        "No registered font covers {characters}. Register one that does."
      ),
    }
  }
}

impl std::error::Error for PdfError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Self::Render(error) => Some(error),
      Self::Font(error) => Some(error),
      Self::Krilla(error) => Some(error),
      _ => None,
    }
  }
}

impl From<TakumiError> for PdfError {
  fn from(error: TakumiError) -> Self {
    Self::Render(error)
  }
}

/// Archival standard the output conforms to.
///
/// PDF/A-1 is not offered: it prohibits transparency, which most takumi output
/// uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PdfStandard {
  /// Plain PDF 1.7, no validation.
  #[default]
  None,
  /// PDF/A-2b: archival, basic conformance.
  A2b,
  /// PDF/A-2u: archival with guaranteed Unicode mapping.
  A2u,
  /// PDF/A-3b: PDF/A-2b plus arbitrary file attachments.
  A3b,
  /// PDF/A-3u: PDF/A-2u plus arbitrary file attachments.
  A3u,
  /// PDF/A-4: archival, PDF 2.0.
  A4,
  /// PDF/A-4f: PDF/A-4 plus arbitrary file attachments.
  A4f,
  /// PDF/A-2a: PDF/A-2 with accessibility (tagged) conformance.
  A2a,
  /// PDF/A-3a: PDF/A-3 with accessibility (tagged) conformance.
  A3a,
}

impl PdfStandard {
  pub(crate) fn archival(self) -> Option<Archival> {
    match self {
      PdfStandard::None => None,
      PdfStandard::A2b => Some(Archival::A2_B),
      PdfStandard::A2u => Some(Archival::A2_U),
      PdfStandard::A3b => Some(Archival::A3_B),
      PdfStandard::A3u => Some(Archival::A3_U),
      PdfStandard::A4 => Some(Archival::A4),
      PdfStandard::A4f => Some(Archival::A4F),
      PdfStandard::A2a => Some(Archival::A2_A),
      PdfStandard::A3a => Some(Archival::A3_A),
    }
  }

  pub(crate) fn requires_tagging(self) -> bool {
    matches!(self, PdfStandard::A2a | PdfStandard::A3a)
  }
}

/// Whether the output carries a tagged structure tree, and to which standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tagging {
  /// No structure tree.
  Off,
  /// Structure tree from the HTML semantics, unvalidated.
  #[default]
  On,
  /// Structure tree validated against PDF/UA-1. Requires a PDF 1.7 level in
  /// [`PdfOptions::standard`] (PDF/A-4 is PDF 2.0; the ranges do not
  /// overlap).
  Ua1,
  /// Structure tree validated against PDF/UA-2. PDF 2.0 only, so it pairs with
  /// [`PdfStandard::A4`] and its variants and with nothing below them.
  Ua2,
}

impl Tagging {
  pub(crate) fn accessibility(self) -> Option<Accessibility> {
    match self {
      Self::Off | Self::On => None,
      Self::Ua1 => Some(Accessibility::UA1),
      Self::Ua2 => Some(Accessibility::UA2),
    }
  }

  /// PDF/UA requires a document outline whenever the document has headings.
  pub(crate) fn requires_outline(self) -> bool {
    self.accessibility().is_some()
  }

  /// PDF/UA-2 requires a destination inside the document to name the structure
  /// element it lands on. PDF/UA-1 predates them, and an untagged reader gains
  /// nothing from the indirection.
  pub(crate) fn names_structure_destinations(self) -> bool {
    self == Self::Ua2
  }
}

/// Inputs for [`crate::render`], built with [`PdfOptions::builder`].
#[derive(TypedBuilder)]
pub struct PdfOptions<'g> {
  /// The viewport to render in. Required for single-page output; ignored when
  /// [`Self::page`] is set (the page geometry defines the layout width).
  #[builder(default, setter(strip_option))]
  pub viewport: Option<Viewport>,
  /// The font context.
  pub fonts: &'g Fonts,
  /// The root node to render.
  pub node: Node,
  /// CSS stylesheets to apply before layout.
  #[builder(default)]
  pub stylesheet: Arc<StyleSheet>,
  /// Resources fetched externally, keyed by URL.
  #[builder(default)]
  pub images: HashMap<Arc<str>, ImageSource>,
  /// Paged output; `None` renders a single page at the viewport size.
  #[builder(default, setter(strip_option))]
  pub page: Option<PageOptions>,
  /// Fills the page box, margins included, before the page draws anything
  /// else. Unset leaves the page empty, like Chromium's print path, so a
  /// viewer shows its own white and the file carries no extra rectangle.
  #[builder(default, setter(strip_option))]
  pub background_color: Option<Color>,
  /// Band repeated at the top of every page. Nodes classed `pageNumber` /
  /// `totalPages` receive the counters, optionally formatted by a
  /// supported `@counter-style` name in the same class list (e.g. `cjk-decimal`). The
  /// band lays out at full page width and draws in the top margin area, like
  /// Chromium's print templates; it does not shrink the content window.
  #[builder(default, setter(strip_option))]
  pub header: Option<Node>,
  /// Band repeated at the bottom of every page; same class hooks as `header`.
  #[builder(default, setter(strip_option))]
  pub footer: Option<Node>,
  /// Per-render font fallback chain (family names in order).
  #[builder(default)]
  pub font_families: Option<FontFamily>,
  /// Default BCP-47 language tag applied to the root.
  #[builder(default)]
  pub lang: Option<Lang>,
  /// Document metadata written to the PDF's info dictionary.
  #[builder(default, setter(strip_option))]
  pub metadata: Option<PdfMetadata>,
  /// Generates a PDF outline (bookmarks) from `h1`–`h6` headings.
  #[builder(default)]
  pub outline: bool,
  /// Archival standard the output conforms to. Validation failures fail the
  /// render.
  #[builder(default)]
  pub standard: PdfStandard,
  /// Structure-tree emission: on by default like Chromium's print-to-PDF,
  /// optionally validated against PDF/UA-1 or PDF/UA-2. The tagged standards
  /// (`A2a`, `A3a`) force it on.
  #[builder(default)]
  pub tagged: Tagging,
  /// Files attached to the document, shown in the viewer's attachment panel.
  /// The PDF/A-3 levels require each to carry a mime type, a description, and
  /// a modification date ([`PdfMetadata::creation_date`] is the fallback).
  #[builder(default)]
  pub attachments: Vec<Attachment>,
}

/// A file attached to the document.
#[derive(Clone)]
pub struct Attachment {
  /// The file name in the PDF, e.g. `factur-x.xml`.
  pub name: String,
  /// The file's bytes.
  pub data: Vec<u8>,
  /// IANA media type, e.g. `application/xml`.
  pub mime_type: Option<String>,
  /// Human-readable description.
  pub description: Option<String>,
  /// How the file relates to the document (the PDF/A-3 AFRelationship).
  pub relationship: AttachmentRelationship,
  /// UTC modification date; falls back to [`PdfMetadata::creation_date`].
  pub modification_date: Option<PdfDate>,
}

/// How an attached file relates to the document it is embedded in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AttachmentRelationship {
  /// The document was created from this file.
  Source,
  /// The file was used to derive a visual presentation in the document.
  Data,
  /// An alternative representation of the document.
  Alternative,
  /// Additional resources for the document.
  Supplement,
  /// No clear relationship, or it is not known.
  #[default]
  Unspecified,
}

impl AttachmentRelationship {
  pub(crate) fn association_kind(self) -> AssociationKind {
    match self {
      Self::Source => AssociationKind::Source,
      Self::Data => AssociationKind::Data,
      Self::Alternative => AssociationKind::Alternative,
      Self::Supplement => AssociationKind::Supplement,
      Self::Unspecified => AssociationKind::Unspecified,
    }
  }
}

/// Document metadata for the PDF's info dictionary. [`PdfOptions::lang`]
/// doubles as the metadata language.
#[derive(Default, Clone)]
pub struct PdfMetadata {
  /// The document title.
  pub title: Option<String>,
  /// The document description (the info dictionary's subject).
  pub description: Option<String>,
  /// The document authors.
  pub authors: Vec<String>,
  /// The document keywords.
  pub keywords: Vec<String>,
  /// The tool that created the source document.
  pub creator: Option<String>,
  /// The document creation date, interpreted as UTC. Tagged archival
  /// standards require one; supplying it keeps output deterministic.
  pub creation_date: Option<PdfDate>,
  /// Custom XMP schemas written into the packet, for metadata the renderer
  /// knows nothing about, e.g. the `fx:` properties a Factur-X invoice needs.
  pub xmp: Vec<XmpSchema>,
}

/// A namespace written into the XMP packet, with the schema description PDF/A
/// requires for it.
#[derive(Debug, Default, Clone)]
pub struct XmpSchema {
  /// Human-readable name, e.g. `Factur-X PDF/A Extension`.
  pub name: String,
  /// Namespace prefix the properties are written under, e.g. `fx`.
  pub prefix: String,
  /// Namespace URI.
  pub namespace: String,
  /// Properties written under the namespace. Each is written as a value and
  /// described in the schema, so the two cannot drift apart.
  pub properties: Vec<XmpProperty>,
}

/// A property of an [`XmpSchema`].
#[derive(Debug, Default, Clone)]
pub struct XmpProperty {
  /// Property name, e.g. `DocumentFileName`.
  pub name: String,
  /// Property value.
  pub value: String,
  /// What the property means. PDF/A requires one.
  pub description: String,
}

/// Rejects schemas the XMP writer would serialize into broken XML: it escapes
/// property values but writes names, prefixes and namespace URIs verbatim.
pub(crate) fn validate_xmp_schemas(schemas: &[XmpSchema]) -> Result<(), PdfError> {
  let name_ok = |name: &str| {
    let mut chars = name.chars();

    chars
      .next()
      .is_some_and(|first| first.is_alphabetic() || first == '_')
      && chars.all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
  };

  for schema in schemas {
    if !name_ok(&schema.prefix) {
      return Err(PdfError::InvalidXmpSchema(schema.prefix.clone()));
    }
    if schema.namespace.is_empty()
      || schema
        .namespace
        .contains(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '&'))
    {
      return Err(PdfError::InvalidXmpSchema(schema.namespace.clone()));
    }
    for property in &schema.properties {
      if !name_ok(&property.name) {
        return Err(PdfError::InvalidXmpSchema(property.name.clone()));
      }
    }
  }
  Ok(())
}

/// A UTC timestamp for [`PdfMetadata::creation_date`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PdfDate {
  /// Full year, e.g. 2026.
  pub year: u16,
  /// Month `1..=12`.
  pub month: u8,
  /// Day of month `1..=31`.
  pub day: u8,
  /// Hour `0..=23`.
  pub hour: u8,
  /// Minute `0..=59`.
  pub minute: u8,
  /// Second `0..=59`.
  pub second: u8,
}

pub(crate) fn build_metadata(metadata: &PdfMetadata, lang: Option<Lang>) -> Metadata {
  let mut result = Metadata::new();

  if let Some(title) = &metadata.title {
    result = result.title(title.clone());
  }
  if let Some(description) = &metadata.description {
    result = result.description(description.clone());
  }
  if !metadata.authors.is_empty() {
    result = result.authors(metadata.authors.clone());
  }
  if !metadata.keywords.is_empty() {
    result = result.keywords(metadata.keywords.clone());
  }
  if let Some(creator) = &metadata.creator {
    result = result.creator(creator.clone());
  }
  if let Some(lang) = lang {
    result = result.language(lang.as_str().to_string());
  }
  if let Some(date) = metadata.creation_date {
    result = result.creation_date(krilla_datetime(date));
  }
  if !metadata.xmp.is_empty() {
    result = result.custom_schemas(metadata.xmp.clone());
  }
  result
}

pub(crate) fn krilla_datetime(date: PdfDate) -> DateTime {
  DateTime::new(date.year)
    .month(date.month)
    .day(date.day)
    .hour(date.hour)
    .minute(date.minute)
    .second(date.second)
    .utc_offset_hour(0)
}

/// Per-side page margins in px.
#[derive(Clone, Copy)]
pub struct PageMargins {
  /// Top margin.
  pub top: f32,
  /// Right margin.
  pub right: f32,
  /// Bottom margin.
  pub bottom: f32,
  /// Left margin.
  pub left: f32,
}

impl PageMargins {
  /// The same margin on all four sides.
  pub const fn uniform(value: f32) -> Self {
    Self {
      top: value,
      right: value,
      bottom: value,
      left: value,
    }
  }
}

/// Paged output geometry: fixed page size with margins. Content lays out at
/// the width inside the margins and flows across as many pages as it needs.
#[derive(Clone, Copy)]
pub struct PageOptions {
  /// Page width in px (A4 at 96 dpi ≈ 794).
  pub width: f32,
  /// Page height in px (A4 at 96 dpi ≈ 1123).
  pub height: f32,
  /// Page margins in px.
  pub margin: PageMargins,
}

/// CSS px (96 dpi) to PDF pt (72 dpi). Layout runs in px; page geometry,
/// annotations, and destinations are written in pt so pages print at their
/// physical size.
pub(crate) const PT_PER_PX: f32 = 1.0 / ONE_PT_IN_PX;

/// Chromium's print template page insets bands 15pt from the paper edge
/// (`#header { padding-top: 15pt }`, `#footer { padding-bottom: 15pt }` in
/// components/printing/resources/print_header_footer_template_page.html).
pub(crate) const BAND_EDGE_PADDING: f32 = 15.0 * ONE_PT_IN_PX;

/// Presets are portrait with a half-inch margin; chain
/// [`landscape`](Self::landscape) and [`with_margin`](Self::with_margin) to
/// adjust, or fill the fields directly for any other size.
// ponytail: A4 + LETTER only; add other @page keywords when someone asks.
impl PageOptions {
  const DEFAULT_MARGIN: f32 = 0.5 * ONE_IN_PX;

  /// ISO A4: 210 × 297 mm.
  pub const A4: Self = Self::preset(210.0 * ONE_MM_IN_PX, 297.0 * ONE_MM_IN_PX);

  /// US Letter: 8.5 × 11 in.
  pub const LETTER: Self = Self::preset(8.5 * ONE_IN_PX, 11.0 * ONE_IN_PX);

  const fn preset(width: f32, height: f32) -> Self {
    Self {
      width,
      height,
      margin: PageMargins::uniform(Self::DEFAULT_MARGIN),
    }
  }

  /// Swaps width and height.
  pub const fn landscape(self) -> Self {
    Self {
      width: self.height,
      height: self.width,
      ..self
    }
  }

  /// Replaces the margins with a uniform value.
  pub const fn with_margin(self, margin: f32) -> Self {
    Self {
      margin: PageMargins::uniform(margin),
      ..self
    }
  }

  pub(crate) const fn content_size(&self) -> (f32, f32) {
    (
      self.width - self.margin.left - self.margin.right,
      self.height - self.margin.top - self.margin.bottom,
    )
  }
}

/// Inputs for [`crate::measure`], built with [`MeasureOptions::builder`].
#[derive(TypedBuilder)]
pub struct MeasureOptions<'g> {
  /// The viewport to lay out in. Required unless [`Self::page`] is set.
  #[builder(default, setter(strip_option))]
  pub viewport: Option<Viewport>,
  /// The font context.
  pub fonts: &'g Fonts,
  /// The node tree to measure.
  pub node: Node,
  /// CSS stylesheets to apply before layout.
  #[builder(default)]
  pub stylesheet: Arc<StyleSheet>,
  /// Resources fetched externally, keyed by URL.
  #[builder(default)]
  pub images: HashMap<Arc<str>, ImageSource>,
  /// Lays out at the full page width with unbounded height, exactly how
  /// [`crate::render`] measures a header/footer band. Margins do not affect the
  /// result.
  #[builder(default, setter(strip_option))]
  pub page: Option<PageOptions>,
  /// Per-render font fallback chain (family names in order).
  #[builder(default)]
  pub font_families: Option<FontFamily>,
  /// Default BCP-47 language tag applied to the root.
  #[builder(default)]
  pub lang: Option<Lang>,
}

/// A node tree's laid-out size in px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasuredSize {
  /// The layout width.
  pub width: f32,
  /// The laid-out content height.
  pub height: f32,
}

#[cfg(test)]
mod tests {
  use super::PdfError;

  #[test]
  fn error_messages_read_as_sentences() {
    assert_eq!(
      PdfError::MissingGlyphs("क (U+0915)".into()).to_string(),
      "No registered font covers क (U+0915). Register one that does."
    );
    assert_eq!(
      PdfError::UnsupportedFilter("blur(2px)".into()).to_string(),
      "A PDF cannot draw filter: blur(2px)"
    );
    assert_eq!(
      PdfError::TooManyPages(20_000).to_string(),
      "The content spans more than 20000 pages. Give it a height that resolves."
    );
  }
}
