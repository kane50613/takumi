//! Setting document metadata.
//!
//! PDF allows for the inclusion of metadata in a PDF document. To do so in krilla,
//! you can simply create a [`Metadata`] object, set the data, and then include it
//! in the document via [`Document::set_metadata`].
//!
//! [`Document::set_metadata`]: crate::krilla::document::Document::set_metadata
use pdf_writer::{Finish, Pdf, Ref, TextStr};
use std::cell::LazyCell;
use xmp_writer::pdfa::PdfAExtSchemasWriter;
use xmp_writer::{CustomNamespace, LangId, Namespace, Timezone, XmpWriter};

use crate::options::XmpSchema;

use crate::krilla::configure::{Configuration, PdfVersion, ValidationError};
use crate::krilla::serialize::SerializeContext;

/// Metadata for a PDF document.
#[derive(Default, Clone, Debug)]
pub struct Metadata {
  pub(crate) title: Option<String>,
  pub(crate) description: Option<String>,
  pub(crate) creator: Option<String>,
  pub(crate) keywords: Option<Vec<String>>,
  pub(crate) authors: Option<Vec<String>>,
  pub(crate) document_id: Option<String>,
  pub(crate) language: Option<String>,
  pub(crate) creation_date: Option<DateTime>,
  pub(crate) text_direction: Option<TextDirection>,
  pub(crate) page_layout: Option<PageLayout>,
  pub(crate) custom_schemas: Vec<XmpSchema>,
}

/// Writes the caller's namespaces as properties. Their schema descriptions are
/// written by the validators, which own the packet's single schema bag.
pub(crate) fn write_custom_properties<'n>(xmp: &mut XmpWriter<'n>, schemas: &'n [XmpSchema]) {
  for schema in schemas {
    for property in &schema.properties {
      xmp
        .element(&property.name, custom_namespace(schema))
        .value(property.value.as_str());
    }
  }
}

fn custom_namespace(schema: &XmpSchema) -> Namespace<'_> {
  Namespace::Custom(Box::new(CustomNamespace::new(
    &schema.name,
    &schema.prefix,
    &schema.namespace,
  )))
}

/// Describes the caller's namespaces in the schema bag PDF/A requires them to
/// appear in.
pub(crate) fn write_custom_schemas<'n>(
  extension_schemas: &mut PdfAExtSchemasWriter<'_, 'n>,
  schemas: &'n [XmpSchema],
) {
  for schema in schemas {
    let mut described = extension_schemas.add_schema();

    described.namespace(custom_namespace(schema));

    let mut properties = described.properties();

    for property in &schema.properties {
      properties
        .add_property()
        .name(&property.name)
        .value_type("Text")
        .category(false)
        .description(&property.description);
    }
  }
}

impl Metadata {
  /// Create new metadata.
  pub fn new() -> Self {
    Self {
      ..Default::default()
    }
  }

  /// The title of the document.
  pub fn title(mut self, title: String) -> Self {
    if !title.is_empty() {
      self.title = Some(title);
    }
    self
  }

  /// The description of the document.
  ///
  /// This should be a short, human-readable abstract, summary, or description
  /// of the topic of the document.
  pub fn description(mut self, description: String) -> Self {
    if !description.is_empty() {
      self.description = Some(description);
    }
    self
  }

  /// The keywords that describe the document.
  pub fn keywords(mut self, keywords: Vec<String>) -> Self {
    if !keywords.is_empty() {
      self.keywords = Some(keywords);
    }
    self
  }

  /// The main language of the document, as an RFC 3066 language tag.
  ///
  /// This property is required for some export modes, like for example PDF/A-3a.
  pub fn language(mut self, language: String) -> Self {
    self.language = Some(language);
    self
  }

  /// The creator tool of the document.
  pub fn creator(mut self, creator: String) -> Self {
    if !creator.is_empty() {
      self.creator = Some(creator);
    }
    self
  }

  /// The authors of the document.
  pub fn authors(mut self, authors: Vec<String>) -> Self {
    if !authors.is_empty() {
      self.authors = Some(authors);
    }
    self
  }

  /// The creation date of the document.
  pub fn creation_date(mut self, creation_date: DateTime) -> Self {
    self.creation_date = Some(creation_date);
    self
  }

  /// A document ID.
  ///
  /// This attribute will be used as an identifier for identifying
  /// different versions of the same document.
  pub fn document_id(mut self, document_id: String) -> Self {
    self.document_id = Some(document_id);
    self
  }

  /// Custom namespaces written into the XMP packet.
  pub fn custom_schemas(mut self, custom_schemas: Vec<XmpSchema>) -> Self {
    self.custom_schemas = custom_schemas;
    self
  }

  /// The main text direction of the document.
  pub fn text_direction(mut self, text_direction: TextDirection) -> Self {
    self.text_direction = Some(text_direction);
    self
  }

  /// How the viewer should lay out the pages.
  pub fn page_layout(mut self, page_layout: PageLayout) -> Self {
    self.page_layout = Some(page_layout);
    self
  }

  pub(crate) fn serialize_xmp_metadata(
    &self,
    xmp: &mut XmpWriter,
    sc: &mut SerializeContext,
    instance_id: &str,
  ) {
    if let Some(title) = &self.title {
      xmp.title([(None, title.as_str())]);
    }

    if let Some(description) = &self.description {
      xmp.description([(None, description.as_str())]);
    }

    if let Some(keywords) = &self.keywords {
      let joined = keywords.join(", ");
      xmp.pdf_keywords(joined.as_str());
    }

    match &self.authors {
      Some(authors) if sc.serialize_settings().pdf_version() >= PdfVersion::Pdf20 => {
        // PDF 2.0+ deprecates the document information dictionary, so
        // we can use the array here.
        xmp.creator(authors.iter().map(String::as_str));
      }
      Some(authors) => {
        // Turns out that if the authors are given in both the document
        // information dictionary and the XMP metadata, Acrobat takes a
        // little bit of both: The first author from the document
        // information dictionary and the remaining authors from the XMP
        // metadata.
        //
        // To fix this for Acrobat, we could omit the remaining authors
        // or all metadata from the document information catalog (it is
        // optional) and only write XMP. However, not all other tools
        // (including Apple Preview) read the XMP data. This means we do
        // want to include all authors in the document information
        // dictionary.
        //
        // Thus, the only alternative is to fold all authors into a
        // single `<rdf:li>` in the XMP metadata. This is, in fact,
        // exactly what the PDF/A spec Part 1 section 6.7.3 has to say
        // about the matter. It's a bit weird to not use the array (and
        // it makes Acrobat show the author list in quotes), but there's
        // not much we can do about that.
        let joined = authors.join(", ");
        xmp.creator([joined.as_str()]);
      }
      None => {}
    }

    if let Some(creator) = &self.creator {
      xmp.creator_tool(creator);
    }

    if let Some(lang) = &self.language {
      xmp.language([LangId(lang)]);
    }

    if let Some(date) = self.creation_date.map(xmp_date) {
      xmp.modify_date(date);
      xmp.create_date(date);

      if sc
        .serialize_settings()
        .validators()
        .requires_file_provenance_information()
      {
        let mut history = xmp.history();
        let mut saved = history.add_event();

        saved
          .action(xmp_writer::ResourceEventAction::Saved)
          .when(date);

        if !sc
          .serialize_settings()
          .validators()
          .prohibits_instance_id_in_xmp_metadata()
        {
          saved.instance_id(&format!("{instance_id}_source"));
        }

        saved.finish();

        let mut converted = history.add_event();

        converted
          .action(xmp_writer::ResourceEventAction::Converted)
          .when(date);

        if let Some(creator) = &self.creator {
          converted.software_agent(creator);
        }

        if !sc
          .serialize_settings()
          .validators()
          .prohibits_instance_id_in_xmp_metadata()
        {
          converted.instance_id(&format!("{instance_id}_source"));
        }
      }
    } else {
      sc.register_validation_error(ValidationError::MissingDocumentDate);
    }
  }

  /// Writes the info dictionary. The producer lands there whether or not the
  /// document carries metadata, so every render names its renderer.
  pub(crate) fn serialize_document_info(
    metadata: Option<&Self>,
    producer: &str,
    ref_: &mut Ref,
    pdf: &mut Pdf,
    config: Configuration,
  ) {
    if config.validators().prohibits_info_dict() {
      return;
    }

    let ref_ = ref_.bump();
    let mut document_info = LazyCell::new(|| pdf.document_info(ref_));

    // ALl of those are deprecated in PDF 2.0 and will only be written
    // to the XMP metadata.
    if config.version() < PdfVersion::Pdf20 {
      document_info.producer(TextStr(producer));

      if let Some(title) = metadata.and_then(|m| m.title.as_ref()) {
        document_info.title(TextStr(title));
      }

      if let Some(description) = metadata.and_then(|m| m.description.as_ref()) {
        document_info.subject(TextStr(description));
      }

      if let Some(keywords) = metadata.and_then(|m| m.keywords.as_ref()) {
        let joined = keywords.join(", ");
        document_info.keywords(TextStr(&joined));
      }

      if let Some(authors) = metadata.and_then(|m| m.authors.as_ref()) {
        let joined = authors.join(", ");
        document_info.author(TextStr(&joined));
      }

      if let Some(creator) = metadata.and_then(|m| m.creator.as_ref()) {
        document_info.creator(TextStr(creator));
      }
    }

    if let Some(date_time) = metadata.and_then(|m| m.creation_date) {
      document_info.modified_date(pdf_date(date_time));
      document_info.creation_date(pdf_date(date_time));
    }
  }
}

/// A datetime. Invalid values will be clamped.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct DateTime {
  /// The year (0-9999).
  pub(crate) year: u16,
  /// The month (0-11).
  pub(crate) month: Option<u8>,
  /// The day (0-31).
  pub(crate) day: Option<u8>,
  /// The hour (0-23).
  pub(crate) hour: Option<u8>,
  /// The minute (0-59).
  pub(crate) minute: Option<u8>,
  /// The second (0-59).
  pub(crate) second: Option<u8>,
  /// The hour offset from UTC (-23 through 23).
  pub(crate) utc_offset_hour: Option<i8>,
  /// The minute offset from UTC (0-59). Will carry over the sign from
  /// `utc_offset_hour`.
  pub(crate) utc_offset_minute: u8,
}

impl DateTime {
  /// Create a new, minimal date. The year will be clamped within the range
  /// 0-9999.
  #[inline]
  pub fn new(year: u16) -> Self {
    Self {
      year: year.min(9999),
      month: None,
      day: None,
      hour: None,
      minute: None,
      second: None,
      utc_offset_hour: None,
      utc_offset_minute: 0,
    }
  }

  /// Add the month field. It will be clamped within the range 1-12.
  #[inline]
  pub fn month(mut self, month: u8) -> Self {
    self.month = Some(month.clamp(1, 12));
    self
  }

  /// Add the day field. It will be clamped within the range 1-31.
  #[inline]
  pub fn day(mut self, day: u8) -> Self {
    self.day = Some(day.clamp(1, 31));
    self
  }

  /// Add the hour field. It will be clamped within the range 0-23.
  #[inline]
  pub fn hour(mut self, hour: u8) -> Self {
    self.hour = Some(hour.min(23));
    self
  }

  /// Add the minute field. It will be clamped within the range 0-59.
  #[inline]
  pub fn minute(mut self, minute: u8) -> Self {
    self.minute = Some(minute.min(59));
    self
  }

  /// Add the second field. It will be clamped within the range 0-59.
  #[inline]
  pub fn second(mut self, second: u8) -> Self {
    self.second = Some(second.min(59));
    self
  }

  /// Add the offset from UTC in hours. If not specified, the time will be
  /// assumed to be local to the viewer's time zone. It will be clamped within
  /// the range -23-23.
  #[inline]
  pub fn utc_offset_hour(mut self, hour: i8) -> Self {
    self.utc_offset_hour = Some(hour.clamp(-23, 23));
    self
  }

  /// Add the offset from UTC in minutes. This will have the same sign as set in
  /// [`Self::utc_offset_hour`]. It will be clamped within the range 0-59.
  #[inline]
  pub fn utc_offset_minute(mut self, minute: u8) -> Self {
    self.utc_offset_minute = minute.min(59);
    self
  }
}

/// Converts a datetime to a pdf-writer date.
pub(crate) fn pdf_date(date_time: DateTime) -> pdf_writer::Date {
  // We always assume a full date with all fields because for some reason
  // Acrobat doesn't like PDF/A-1 files without everything set.
  pdf_writer::Date::new(date_time.year)
    .month(date_time.month.unwrap_or(1))
    .day(date_time.day.unwrap_or(1))
    .hour(date_time.hour.unwrap_or(0))
    .minute(date_time.minute.unwrap_or(0))
    .second(date_time.second.unwrap_or(0))
    .utc_offset_hour(date_time.utc_offset_hour.unwrap_or(0))
    .utc_offset_minute(date_time.utc_offset_minute)
}

/// Converts a datetime to an xmp-writer datetime.
fn xmp_date(datetime: DateTime) -> xmp_writer::DateTime {
  let timezone = match (datetime.utc_offset_hour, datetime.utc_offset_minute) {
    (Some(h), m) => Some(Timezone::Local {
      hour: h,
      minute: m as i8,
    }),
    _ => Some(Timezone::Utc),
  };

  // We always assume a full date with all fields because for some reason
  // Acrobat doesn't like PDF/A-1 files without everything set.
  xmp_writer::DateTime {
    year: datetime.year,
    month: Some(datetime.month.unwrap_or(1)),
    day: Some(datetime.day.unwrap_or(1)),
    hour: Some(datetime.hour.unwrap_or(0)),
    minute: Some(datetime.minute.unwrap_or(0)),
    second: Some(datetime.second.unwrap_or(0)),
    timezone,
  }
}

/// The main text direction of the document.
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug)]
pub enum TextDirection {
  LeftToRight,
  RightToLeft,
}

impl TextDirection {
  pub(crate) fn to_pdf(self) -> pdf_writer::types::Direction {
    match self {
      TextDirection::LeftToRight => pdf_writer::types::Direction::L2R,
      TextDirection::RightToLeft => pdf_writer::types::Direction::R2L,
    }
  }
}

/// How the viewer should lay out the pages.
#[derive(Copy, Clone, Debug)]
pub enum PageLayout {
  /// Only a single page at a time.
  SinglePage,
  /// A single, continuously scrolling column of pages.
  OneColumn,
  /// Two continuously scrolling columns of pages, laid out with odd-numbered
  /// pages on the left.
  TwoColumnLeft,
  /// Two continuously scrolling columns of pages, laid out with odd-numbered
  /// pages on the right (like in a left-bound book).
  TwoColumnRight,
  /// Only two pages are visible at a time, laid out with odd-numbered pages
  /// on the left. PDF 1.5+.
  TwoPageLeft,
  /// Only two pages are visible at a time, laid out with odd-numbered pages
  /// on the right (like in a left-bound book). PDF 1.5+.
  TwoPageRight,
}

impl PageLayout {
  pub(crate) fn to_pdf(self) -> pdf_writer::types::PageLayout {
    match self {
      PageLayout::SinglePage => pdf_writer::types::PageLayout::SinglePage,
      PageLayout::OneColumn => pdf_writer::types::PageLayout::OneColumn,
      PageLayout::TwoColumnLeft => pdf_writer::types::PageLayout::TwoColumnLeft,
      PageLayout::TwoColumnRight => pdf_writer::types::PageLayout::TwoColumnRight,
      PageLayout::TwoPageLeft => pdf_writer::types::PageLayout::TwoPageLeft,
      PageLayout::TwoPageRight => pdf_writer::types::PageLayout::TwoPageRight,
    }
  }
}
