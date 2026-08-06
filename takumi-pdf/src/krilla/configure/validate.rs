//! Exporting with a specific PDF conformance level.
//!
//! PDF defines a number of additional conformance levels that restrict the features of PDF that
//! can be used to a specific subset.
//!
//! You can use a [`Validator`] by creating a corresponding [`Configuration`]
//! you want to build the document with. There are three important aspects that play into this:
//! - krilla will internally write the file in a way that conforms to the given standard, i.e.
//!   by settings appropriate metadata. This happens under-the-hood and is completely abstracted
//!   away from the user.
//! - For aspects that are out of control of krilla and dependent on the input, krilla will perform
//!   a validation that the input is compatible with the standard. krilla will record all violations,
//!   and when calling `document.finish()`, in case there is at least one violation, krilla will
//!   return them as an error, instead of returning the finished document. See [`ValidationError`].
//! - Finally, some standards have requirements that cannot possibly be validated by krilla, as
//!   they are semantic in nature. It is upon you, as a user of that library, to ensure that those
//!   requirements are fulfilled. Therefore, while krilla tries to make it as easy as possible
//!   to generate compliant PDFs, it is still highly recommended that you familiarize yourself
//!   with the PDF specification as well as the specifications for the substandards. This is
//!   especially true for standards related to universal accessibility.
//!   
//!  You can find some requirements below **Requirements** for each [`Validator`].
//!
//! [`Configuration`]: crate::krilla::configure::Configuration

use std::collections::HashMap;
use std::fmt::Debug;

use pdf_writer::types::OutputIntentSubtype;
use xmp_writer::XmpWriter;
use xmp_writer::pdfa::PdfAExtSchemasWriter;

use crate::krilla::color::RegularColor;
use crate::krilla::color::separation::SeparationColorant;
use crate::krilla::color::separation::SeparationSpace;
use crate::krilla::configure::PdfVersion;
use crate::krilla::interchange::embed::EmbedError;
use crate::krilla::surface::Location;
use crate::krilla::text::Font;
use crate::krilla::text::GlyphId;

/// An error that occurred during validation/
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValidationError {
  /// There was a string that was longer than the maximum allowed length (32767).
  ///
  /// Can for example occur if you set a title or an author that is longer than
  /// the given length.
  TooLongString,
  /// There was a name that was longer than the maximum allowed length (127).
  ///
  /// Can for example occur if the font name is too long.
  TooLongName,
  /// There was an array that was longer than the maximum allowed length (8191).
  /// Can only occur for PDF 1.4.
  ///
  /// Can for example occur if a text too long was written.
  TooLongArray,
  /// There was a dictionary with more entries than the maximum allowed (4095).
  /// Can only occur for PDF 1.4.
  ///
  /// Can for example occur if too many annotations are added to a page.
  TooLongDictionary,
  /// There was a float that is higher than the maximum allowed (32767).
  /// Can only occur for PDF 1.4.
  TooLargeFloat,
  /// The PDF exceeds the upper limit for indirect objects (8388607).
  ///
  /// Occurs if the PDF is simply too long.
  TooManyIndirectObjects,
  /// The PDF contains a content stream that exceeds maximum allowed q/Q nesting level (28).
  ///
  /// Can only occur if the user stacks many clip paths.
  TooHighQNestingLevel,
  /// The PDF contains PostScript code, which is forbidden by some export formats.
  ///
  /// Occurs if a gradient with spread method `Repeat`/`Reflect` or a sweep gradient was used.
  ContainsPostScript(Option<Location>),
  /// No CMYK ICC profile was provided, even though one is necessary.
  ///
  /// Occurs if the export format requires a device-independent color representation,
  /// and a CMYK color was used in the document.
  MissingCMYKProfile,
  /// The same Separation colorant was used with multiple different fallback colors.
  ///
  /// Occurs if the user specified multiple Separation color spaces with the same colorant but a different fallback color.
  InconsistentSeparationFallback(SeparationColorant),
  /// The `.notdef` glyph was used, which is forbidden by some export formats.
  ///
  /// Can occur if a glyph could not be found in the font for a corresponding codepoint
  /// in the input text, or if it was explicitly mapped that way.
  ///
  /// The third argument contains the text range of the glyph.
  ContainsNotDefGlyph(Font, Option<Location>, String),
  /// A glyph was mapped to no codepoint at all, which is forbidden by some
  /// standards.
  NoCodepointMapping(Font, GlyphId, Option<Location>),
  /// A glyph was mapped either to the codepoint 0x0, 0xFEFF or 0xFFFE, which
  /// is forbidden by some standards.
  ///
  /// Can occur if those codepoints appeared in the input text, or were
  /// explicitly mapped to that glyph.
  InvalidCodepointMapping(Font, GlyphId, char, Option<Location>),
  /// A glyph was mapped to a codepoint in the Unicode private use area, which is forbidden
  /// by some standards, like for example PDF/A-2a.
  // Note that the standard doesn't explicitly forbid it, but instead requires an ActualText
  // attribute to be present. But we just completely forbid it, for simplicity.
  UnicodePrivateArea(Font, GlyphId, char, Option<Location>),
  /// A font has a license that requires explicit permission of the legal owner for embedding
  /// but the standard requires font programs to be legally embeddable for universal rendering.
  RestrictedLicense(Font),
  /// No document language was set via the metadata, even though it is required
  /// by the standard.
  NoDocumentLanguage,
  /// No title was provided for the document, even though it is required by
  /// the standard.
  NoDocumentTitle,
  /// A figure or formula is missing an alt text.
  MissingAltText(Option<Location>),
  /// A heading is missing a title.
  MissingHeadingTitle,
  /// The document does not contain an outline.
  MissingDocumentOutline,
  /// An annotation is missing an alt text.
  MissingAnnotationAltText(Option<Location>),
  /// The date of the document is missing.
  // We need this because for some standards we need to add the
  // xmp:History attribute.
  MissingDocumentDate,
  /// The PDF contains transparency, which is forbidden by some standards (e.g. PDF/A-1).
  Transparency(Option<Location>),
  /// The PDF contains an image with `interpolate` set to `true`.
  ImageInterpolation(Option<Location>),
  /// The PDF contains an embedded file.
  EmbeddedFile(EmbedError, Option<Location>),
  /// The PDF contains no tagging.
  MissingTagging,
  /// The PDF contains another embedded PDF.
  ///
  /// This is currently forbidden in validated export because we cannot manually verify
  /// whether the file actually fulfills all the criteria for the export mode.
  EmbeddedPDF(Option<Location>),
  /// A feature only available in a later PDF version was required.
  RequiresNewerPdfVersion(VersionedFeature, Option<Location>),
}

/// Features that may require a later PDF version than the current one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VersionedFeature {
  /// Tabbing through the document according to the structure order.
  StructureOrderTabbing,
  /// Header and footer artifact subtypes.
  HeaderFooterArtifactSubtypes,
  /// Scope attribute for table header cells.
  TableHeaderScope,
}

impl VersionedFeature {
  /// Get the minimum PDF version required for this feature.
  pub fn minimum_pdf_version(&self) -> PdfVersion {
    match self {
      VersionedFeature::StructureOrderTabbing => PdfVersion::Pdf15,
      VersionedFeature::HeaderFooterArtifactSubtypes => PdfVersion::Pdf17,
      VersionedFeature::TableHeaderScope => PdfVersion::Pdf15,
    }
  }
}

/// Collection of validators with at most one validator for each standard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct Validators {
  a: Option<Archival>,
  ua: Option<Accessibility>,
}

impl Validators {
  /// Returns a filtered `Validators` containing only validators that prohibit the given error,
  /// or `None` if no validator prohibits it.
  pub fn prohibits(self, error: &ValidationError) -> Option<Self> {
    let a = self.a.filter(|v| v.prohibits(error));
    let ua = self.ua.filter(|v| v.prohibits(error));

    let any = a.is_some() || ua.is_some();
    any.then_some(Self { a, ua })
  }

  /// Returns `true` if no validators are set.
  pub fn is_empty(self) -> bool {
    self.a.is_none() && self.ua.is_none()
  }

  /// Returns the number of set validators.
  pub fn len(self) -> usize {
    (if self.a.is_some() { 1 } else { 0 }) + (if self.ua.is_some() { 1 } else { 0 })
  }

  /// Returns the PDF/A validator, if set.
  pub fn archival(self) -> Option<Archival> {
    self.a
  }

  /// Returns the PDF/UA accessibility validator, if set.
  pub fn accessibility(self) -> Option<Accessibility> {
    self.ua
  }

  /// Whether the font must supply valid Unicode code points for each of the
  /// drawn glyphs.
  pub(crate) fn requires_codepoint_mappings(self) -> bool {
    self.into_iter().any(Validator::requires_codepoint_mappings)
  }

  /// Force the `DisplayDocTitle` flag set.
  pub(crate) fn requires_display_doc_title(self) -> bool {
    self
      .ua
      .is_some_and(Accessibility::requires_display_doc_title)
  }

  /// Force sRGB profiles for `DeviceGray` and `DeviceRgb` colorspaces.
  pub(crate) fn requires_no_device_cs(self) -> bool {
    self.a.is_some_and(Archival::requires_no_device_cs)
  }

  /// Force the `Print` flag set and the `Hidden`, `Invisible`,
  /// `ToggleNoView`, and `NoView` flags unset.
  pub(crate) fn requires_annotation_flags(self) -> bool {
    self.a.is_some_and(Archival::requires_annotation_flags)
  }

  /// Whether Tagged PDF must be enabled.
  pub(crate) fn requires_tagging(self) -> bool {
    self.into_iter().any(Validator::requires_tagging)
  }

  /// Whether XMP metadata must be written.
  pub(crate) fn requires_xmp_metadata(self) -> bool {
    self.into_iter().any(Validator::requires_xmp_metadata)
  }

  /// Whether any extension schemata should be descibed using the "pdfaSchema"
  /// namespace.
  pub(crate) fn requires_xmp_metadata_extension_schema(self) -> bool {
    self
      .a
      .is_some_and(Archival::requires_xmp_metadata_extension_schema)
  }

  /// Whether the `instanceID` field is allowed in XMP.
  pub(crate) fn prohibits_instance_id_in_xmp_metadata(self) -> bool {
    self
      .a
      .is_some_and(Archival::prohibits_instance_id_in_xmp_metadata)
  }

  /// Whether the xmpMM:History entry is required.
  pub(crate) fn requires_file_provenance_information(self) -> bool {
    self
      .a
      .is_some_and(Archival::requires_file_provenance_information)
  }

  /// Whether the `/Info` dictionary is allowed in the file trailer.
  pub(crate) fn prohibits_info_dict(self) -> bool {
    self.a.is_some_and(Archival::prohibits_info_dict)
  }

  /// Whether a non-printable file header is mandatory.
  pub(crate) fn requires_binary_header(self) -> bool {
    self.a.is_some_and(Archival::requires_binary_header)
  }

  /// Whether the `EmbeddedFiles` key in the name dictionary of the document
  /// catalog dictionary should be written even if empty.
  pub(crate) fn requires_embedded_files_when_empty(self) -> bool {
    self
      .a
      .is_some_and(Archival::requires_embedded_files_when_empty)
  }

  /// Whether any of these standards explicitly specifies the `/AF` key.
  ///
  /// The `/AF` key may be supported by the underlying PDF version instead:
  /// Starting at PDF 2.0, the key is specified by ISO 32000 and does not need
  /// to be added by PDF/A.
  pub(crate) fn specifies_associated_files(self) -> bool {
    self.a.is_some_and(Archival::specifies_associated_files)
  }

  pub(crate) fn output_intent(self) -> Option<OutputIntentSubtype<'static>> {
    self.a.map(Archival::output_intent)
  }

  pub(crate) fn write_xmp(self, xmp: &mut XmpWriter) {
    if self.requires_xmp_metadata_extension_schema() {
      let mut extension_schemas = xmp.extension_schemas();
      if let Some(a) = self.a {
        a.write_xmp_extension_schema_description(&mut extension_schemas);
      }
      if let Some(ua) = self.ua {
        ua.write_xmp_extension_schema_description(&mut extension_schemas);
      }
    }

    if let Some(a) = self.a {
      a.write_xmp(xmp);
    }

    if let Some(ua) = self.ua {
      ua.write_xmp(xmp);
    }
  }

  /// Returns the maximum PDF version allowed by all active validators.
  pub fn max(self) -> PdfVersion {
    self
      .a
      .map_or(PdfVersion::MAX, |v| v.max())
      .min(self.ua.map_or(PdfVersion::MAX, |v| v.max()))
  }

  /// Returns the minimum PDF version required by all active validators, if any.
  pub fn min(self) -> Option<PdfVersion> {
    self
      .a
      .and_then(|v| v.min())
      .max(self.ua.and_then(|v| v.min()))
  }
}

impl IntoIterator for Validators {
  type Item = Validator;
  type IntoIter = std::iter::Flatten<std::array::IntoIter<Option<Validator>, 2>>;

  fn into_iter(self) -> Self::IntoIter {
    [self.a.map(Validator::A), self.ua.map(Validator::Ua)]
      .into_iter()
      .flatten()
  }
}

/// A builder for constructing a [`Validators`] collection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct ValidatorsBuilder(Validators);

impl ValidatorsBuilder {
  /// Set a validator, overwriting the current one if the same standard family is already set.
  pub fn set_validator(self, validator: Validator) -> Self {
    match validator {
      Validator::A(a) => self.with_archival_validator(a),
      Validator::Ua(ua) => self.with_accessibility_validator(ua),
    }
  }

  /// Set the PDF/A validator, overwriting the current one if already set.
  pub fn with_archival_validator(mut self, archival: Archival) -> Self {
    self.0.a = Some(archival);
    self
  }

  /// Set the PDF/UA accessibility validator, overwriting the current one if already set.
  pub fn with_accessibility_validator(mut self, accessibility: Accessibility) -> Self {
    self.0.ua = Some(accessibility);
    self
  }

  pub(crate) fn finish(self) -> Result<Validators, Validators> {
    let min = self.0.min().unwrap_or(PdfVersion::MIN);
    let max = self.0.max();

    if min > max { Err(self.0) } else { Ok(self.0) }
  }
}

/// A PDF validator for a specific conformance standard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Validator {
  /// A PDF/A validator.
  A(Archival),
  /// A PDF/UA accessibility validator.
  Ua(Accessibility),
}

impl Validator {
  fn requires_codepoint_mappings(self) -> bool {
    match self {
      Self::A(a) => a.requires_codepoint_mappings(),
      Self::Ua(ua) => ua.requires_codepoint_mappings(),
    }
  }

  fn requires_tagging(self) -> bool {
    match self {
      Self::A(a) => a.requires_tagging(),
      Self::Ua(ua) => ua.requires_tagging(),
    }
  }

  fn requires_xmp_metadata(self) -> bool {
    match self {
      Self::A(a) => a.requires_xmp_metadata(),
      Self::Ua(ua) => ua.requires_xmp_metadata(),
    }
  }

  /// Minimum PDF version required to use this validator, if any.
  pub fn min(self) -> Option<PdfVersion> {
    match self {
      Self::A(a) => a.min(),
      Self::Ua(ua) => ua.min(),
    }
  }

  /// Maximum PDF version this standard can be used with.
  pub fn max(self) -> PdfVersion {
    match self {
      Self::A(a) => a.max(),
      Self::Ua(ua) => ua.max(),
    }
  }

  /// Returns a human-readable string representation of the validator.
  pub fn as_str(self) -> &'static str {
    match self {
      Self::A(a) => a.as_str(),
      Self::Ua(ua) => ua.as_str(),
    }
  }
}

impl From<Archival> for Validator {
  fn from(a: Archival) -> Self {
    Self::A(a)
  }
}

impl From<Accessibility> for Validator {
  fn from(ua: Accessibility) -> Self {
    Self::Ua(ua)
  }
}

/// A PDF/A conformance level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum Archival {
  /// The validator for the PDF/A-1a standard.
  ///
  /// **Requirements**:
  /// - All requirements of PDF/A-1b.
  /// - You need to follow all requirements outlined in the _Other Notes_ section of the
  ///   [`tagging`] module.
  /// - You need to follow all best practices when using [tags](`crate::krilla::interchange::tagging::Tag`), as outlined in the documentation
  ///   of each tag.
  /// - Artifacts such as page numbers, backgrounds, cut marks and color bars should be specified
  ///   correspondingly as artifacts.
  /// - Word boundaries need to be explicitly specified with a space. The same applies to words at
  ///   the end of a line that are not followed by punctuation.
  /// - To the fullest extent possible, the logical structure of the document should be encoded
  ///   correspondingly in the tag tree using appropriate grouping tags.
  /// - Language identifiers used must be valid according to RFC 3066.
  /// - You should provide an alternate text to span content tags, if applicable.
  /// - You should provide the expansion of abbreviations to span content tags, if applicable.
  ///
  /// [`tagging`]: crate::krilla::interchange::tagging
  A1_A,
  /// The validator for the PDF/A-1b standard.
  ///
  /// **Requirements**: -
  A1_B,
  /// The validator for the PDF/A-2a standard.
  ///
  /// **Requirements**:
  /// - All requirements of PDF/A-2b.
  /// - You need to follow all requirements outlined in the _Other Notes_ section of the
  ///   [`tagging`] module.
  /// - You need to follow all best practices when using [tags](`crate::krilla::interchange::tagging::Tag`), as outlined in the documentation
  ///   of each tag.
  /// - Artifacts such as page numbers, backgrounds, cut marks and color bars should be specified
  ///   correspondingly as artifacts.
  /// - Word boundaries need to be explicitly specified with a space. The same applies to words at
  ///   the end of a line that are not followed by punctuation.
  /// - To the fullest extent possible, the logical structure of the document should be encoded
  ///   correspondingly in the tag tree using appropriate grouping tags.
  /// - Language identifiers used must be valid according to RFC 3066.
  /// - You should provide an alternate text to span content tags, if applicable.
  /// - You should provide the expansion of abbreviations to span content tags, if applicable.
  ///
  /// [`tagging`]: crate::krilla::interchange::tagging
  A2_A,
  /// The validator for the PDF/A-2b standard.
  ///
  /// **Requirements**:
  /// - You should only use fonts that are legally embeddable in a file for unlimited,
  ///   universal rendering.
  A2_B,
  /// The validator for the PDF/A-2u standard.
  ///
  /// **Requirements**:
  /// - All requirements of PDF/A-2b
  A2_U,
  /// The validator for the PDF/A-3a standard.
  ///
  /// **Requirements**:
  /// - All requirements of PDF/A-2a
  A3_A,
  /// The validator for the PDF/A-3b standard.
  ///
  /// **Requirements**:
  /// - All requirements of PDF/A-2b
  A3_B,
  /// The validator for the PDF/A-3u standard.
  ///
  /// **Requirements**:
  /// - All requirements of PDF/A-2b
  A3_U,
  /// The validator for the PDF/A-4 standard.
  ///
  /// **Requirements**:
  /// - While not required, it's recommended to enable tagging.
  A4,
  /// The validator for the PDF/A-4f standard.
  ///
  /// **Requirements**:
  /// - All requirements of PDF/A-4
  A4F,
  /// The validator for the PDF/A-4e standard.
  ///
  /// **Requirements**:
  /// - All requirements of PDF/A-4
  A4E,
}

impl Archival {
  fn prohibits(self, error: &ValidationError) -> bool {
    match (self, error) {
      // Forbidden under all PDF/A-1 profiles.
      (
        Self::A1_A | Self::A1_B,
        ValidationError::TooLongString
        | ValidationError::TooLongName
        | ValidationError::TooLongArray
        | ValidationError::TooLongDictionary
        | ValidationError::TooLargeFloat
        | ValidationError::TooManyIndirectObjects
        | ValidationError::TooHighQNestingLevel
        | ValidationError::ContainsPostScript(_)
        | ValidationError::MissingCMYKProfile
        | ValidationError::RestrictedLicense(_)
        | ValidationError::MissingDocumentDate
        | ValidationError::Transparency(_)
        | ValidationError::ImageInterpolation(_)
        | ValidationError::EmbeddedFile(EmbedError::Existence, _)
        | ValidationError::EmbeddedPDF(_),
      ) => true,
      // Allowed under all PDF/A-1 profiles.
      (
        Self::A1_A | Self::A1_B,
        ValidationError::InconsistentSeparationFallback(_)
        | ValidationError::InvalidCodepointMapping(_, _, _, _)
        | ValidationError::UnicodePrivateArea(_, _, _, _)
        | ValidationError::NoDocumentTitle
        | ValidationError::MissingHeadingTitle
        | ValidationError::MissingDocumentOutline
        | ValidationError::EmbeddedFile(_, _)
        | ValidationError::RequiresNewerPdfVersion(
          VersionedFeature::HeaderFooterArtifactSubtypes
          | VersionedFeature::StructureOrderTabbing
          | VersionedFeature::TableHeaderScope,
          _,
        ),
      ) => false,
      // Forbidden under PDF/A-1a but allowed under PDF/A-1b.
      (
        Self::A1_A | Self::A1_B,
        ValidationError::ContainsNotDefGlyph(_, _, _)
        | ValidationError::NoCodepointMapping(_, _, _)
        | ValidationError::NoDocumentLanguage
        | ValidationError::MissingAltText(_)
        | ValidationError::MissingAnnotationAltText(_)
        | ValidationError::MissingTagging,
      ) => self == Self::A1_A,

      // Forbidden under all PDF/A-2 and PDF/A-3 profiles.
      (
        Self::A2_A | Self::A2_B | Self::A2_U | Self::A3_A | Self::A3_B | Self::A3_U,
        ValidationError::TooLongString
        | ValidationError::TooLongName
        | ValidationError::TooManyIndirectObjects
        | ValidationError::TooHighQNestingLevel
        | ValidationError::ContainsPostScript(_)
        | ValidationError::MissingCMYKProfile
        | ValidationError::InconsistentSeparationFallback(_)
        | ValidationError::ContainsNotDefGlyph(_, _, _)
        | ValidationError::RestrictedLicense(_)
        | ValidationError::MissingDocumentDate
        | ValidationError::ImageInterpolation(_)
        | ValidationError::EmbeddedPDF(_),
      ) => true,
      // Allowed under all PDF/A-2 and PDF/A-3 profiles.
      (
        Self::A2_A | Self::A2_B | Self::A2_U | Self::A3_A | Self::A3_B | Self::A3_U,
        ValidationError::TooLongArray
        | ValidationError::TooLongDictionary
        | ValidationError::TooLargeFloat
        | ValidationError::NoDocumentTitle
        | ValidationError::Transparency(_)
        | ValidationError::MissingHeadingTitle
        | ValidationError::MissingDocumentOutline
        | ValidationError::RequiresNewerPdfVersion(
          VersionedFeature::HeaderFooterArtifactSubtypes
          | VersionedFeature::StructureOrderTabbing
          | VersionedFeature::TableHeaderScope,
          _,
        ),
      ) => false,
      // Forbidden under PDF/A-2 but allowed under PDF/A-3.
      (
        Self::A2_A | Self::A2_B | Self::A2_U | Self::A3_A | Self::A3_B | Self::A3_U,
        ValidationError::EmbeddedFile(EmbedError::Existence, _),
      ) => self == Self::A2_A || self == Self::A2_B || self == Self::A2_U,
      // Forbidden under PDF/A-3 but allowed under PDF/A-2.
      (
        Self::A2_A | Self::A2_B | Self::A2_U | Self::A3_A | Self::A3_B | Self::A3_U,
        ValidationError::EmbeddedFile(
          EmbedError::MissingDate | EmbedError::MissingDescription | EmbedError::MissingMimeType,
          _,
        ),
      ) => self == Self::A3_A || self == Self::A3_B || self == Self::A3_U,
      // Forbidden under PDF/A-2 and PDF/A-3 accessible profiles.
      (
        Self::A2_A | Self::A2_B | Self::A2_U | Self::A3_A | Self::A3_B | Self::A3_U,
        ValidationError::UnicodePrivateArea(_, _, _, _)
        | ValidationError::NoDocumentLanguage
        | ValidationError::MissingAltText(_)
        | ValidationError::MissingAnnotationAltText(_)
        | ValidationError::MissingTagging,
      ) => self == Self::A2_A || self == Self::A3_A,
      // Forbidden under PDF/A-2 and PDF/A-3 accessible and Unicode profiles.
      (
        Self::A2_A | Self::A2_B | Self::A2_U | Self::A3_A | Self::A3_B | Self::A3_U,
        ValidationError::NoCodepointMapping(_, _, _)
        | ValidationError::InvalidCodepointMapping(_, _, _, _),
      ) => self == Self::A2_A || self == Self::A2_U || self == Self::A3_A || self == Self::A3_U,

      // Forbidden under all PDF/A-4 profiles.
      (
        Self::A4 | Self::A4F | Self::A4E,
        ValidationError::MissingCMYKProfile
        | ValidationError::InconsistentSeparationFallback(_)
        | ValidationError::ContainsNotDefGlyph(_, _, _)
        | ValidationError::NoCodepointMapping(_, _, _)
        | ValidationError::InvalidCodepointMapping(_, _, _, _)
        | ValidationError::UnicodePrivateArea(_, _, _, _)
        | ValidationError::RestrictedLicense(_)
        | ValidationError::MissingDocumentDate
        | ValidationError::ImageInterpolation(_)
        | ValidationError::EmbeddedPDF(_),
      ) => true,
      // Allowed under all PDF/A-4 profiles.
      (
        Self::A4 | Self::A4F | Self::A4E,
        ValidationError::TooLongString
        | ValidationError::TooLongName
        | ValidationError::TooLongArray
        | ValidationError::TooLongDictionary
        | ValidationError::TooLargeFloat
        | ValidationError::TooManyIndirectObjects
        | ValidationError::TooHighQNestingLevel
        | ValidationError::ContainsPostScript(_)
        | ValidationError::NoDocumentLanguage
        | ValidationError::NoDocumentTitle
        | ValidationError::MissingAltText(_)
        | ValidationError::MissingHeadingTitle
        | ValidationError::MissingDocumentOutline
        | ValidationError::MissingAnnotationAltText(_)
        | ValidationError::Transparency(_)
        | ValidationError::EmbeddedFile(EmbedError::MissingDate | EmbedError::MissingMimeType, _)
        | ValidationError::MissingTagging
        | ValidationError::RequiresNewerPdfVersion(
          VersionedFeature::HeaderFooterArtifactSubtypes
          | VersionedFeature::StructureOrderTabbing
          | VersionedFeature::TableHeaderScope,
          _,
        ),
      ) => false,
      // Forbidden under PDF/A-4 but allowed under other PDF/A-4 profiles.
      (
        Self::A4 | Self::A4F | Self::A4E,
        ValidationError::EmbeddedFile(EmbedError::Existence, _),
      ) => self == Self::A4,
      // Allowed under PDF/A-4 but forbidden under other profiles.
      (
        Self::A4 | Self::A4F | Self::A4E,
        ValidationError::EmbeddedFile(EmbedError::MissingDescription, _),
      ) => self == Self::A4,
    }
  }

  fn requires_codepoint_mappings(self) -> bool {
    match self {
      Self::A1_A
      | Self::A2_A
      | Self::A2_U
      | Self::A3_A
      | Self::A3_U
      | Self::A4
      | Self::A4F
      | Self::A4E => true,
      Self::A1_B | Self::A2_B | Self::A3_B => false,
    }
  }

  fn requires_no_device_cs(self) -> bool {
    match self {
      Self::A1_A
      | Self::A1_B
      | Self::A2_A
      | Self::A2_B
      | Self::A2_U
      | Self::A3_A
      | Self::A3_B
      | Self::A3_U
      | Self::A4
      | Self::A4F
      | Self::A4E => true,
    }
  }

  fn requires_annotation_flags(self) -> bool {
    match self {
      Self::A1_A
      | Self::A1_B
      | Self::A2_A
      | Self::A2_B
      | Self::A2_U
      | Self::A3_A
      | Self::A3_B
      | Self::A3_U
      | Self::A4
      | Self::A4F
      | Self::A4E => true,
    }
  }

  fn requires_tagging(self) -> bool {
    match self {
      Self::A1_A | Self::A2_A | Self::A3_A => true,
      Self::A1_B
      | Self::A2_B
      | Self::A2_U
      | Self::A3_B
      | Self::A3_U
      | Self::A4
      | Self::A4F
      | Self::A4E => false,
    }
  }

  fn requires_xmp_metadata(self) -> bool {
    match self {
      Self::A1_A
      | Self::A1_B
      | Self::A2_A
      | Self::A2_B
      | Self::A2_U
      | Self::A3_A
      | Self::A3_B
      | Self::A3_U
      | Self::A4
      | Self::A4F
      | Self::A4E => true,
    }
  }

  fn requires_xmp_metadata_extension_schema(self) -> bool {
    match self {
      Self::A1_A
      | Self::A1_B
      | Self::A2_A
      | Self::A2_B
      | Self::A2_U
      | Self::A3_A
      | Self::A3_B
      | Self::A3_U => true,
      // Clause 6.7.2.3 of PDF/A-4 recommends ("should") a RELAX NG
      // definition of its metadata contents to be embedded as an
      // associated file. It no longer uses the inline schema definition
      // using the "pdfaSchema" namespaces for extension schemata.
      Self::A4 | Self::A4F | Self::A4E => false,
    }
  }

  fn prohibits_instance_id_in_xmp_metadata(self) -> bool {
    match self {
      Self::A1_A | Self::A1_B => true,
      Self::A2_A
      | Self::A2_B
      | Self::A2_U
      | Self::A3_A
      | Self::A3_B
      | Self::A3_U
      | Self::A4
      | Self::A4F
      | Self::A4E => false,
    }
  }

  fn requires_file_provenance_information(self) -> bool {
    match self {
      Self::A1_A
      | Self::A1_B
      | Self::A2_A
      | Self::A2_B
      | Self::A2_U
      | Self::A3_A
      | Self::A3_B
      | Self::A3_U
      | Self::A4
      | Self::A4F
      | Self::A4E => true,
    }
  }

  fn prohibits_info_dict(self) -> bool {
    match self {
      Self::A1_A
      | Self::A1_B
      | Self::A2_A
      | Self::A2_B
      | Self::A2_U
      | Self::A3_A
      | Self::A3_B
      | Self::A3_U => false,
      Self::A4 | Self::A4F | Self::A4E => true,
    }
  }

  fn requires_binary_header(self) -> bool {
    match self {
      Self::A1_A
      | Self::A1_B
      | Self::A2_A
      | Self::A2_B
      | Self::A2_U
      | Self::A3_A
      | Self::A3_B
      | Self::A3_U
      | Self::A4
      | Self::A4F
      | Self::A4E => true,
    }
  }

  fn requires_embedded_files_when_empty(self) -> bool {
    match self {
      Self::A1_A
      | Self::A1_B
      | Self::A2_A
      | Self::A2_B
      | Self::A2_U
      | Self::A3_A
      | Self::A3_B
      | Self::A3_U
      | Self::A4
      | Self::A4E => false,
      Self::A4F => true,
    }
  }

  /// Whether this standard explicitly specifies the `/AF` key.
  ///
  /// The `/AF` key may be supported by the underlying PDF version instead:
  /// Starting at PDF 2.0, the key is specified by ISO 32000 and does not need
  /// to be added by PDF/A.
  fn specifies_associated_files(self) -> bool {
    match self {
      Self::A3_A | Self::A3_B | Self::A3_U => true,
      Self::A1_A
      | Self::A1_B
      | Self::A2_A
      | Self::A2_B
      | Self::A2_U
      | Self::A4
      | Self::A4F
      | Self::A4E => false,
    }
  }

  fn output_intent(self) -> OutputIntentSubtype<'static> {
    match self {
      Self::A1_A
      | Self::A1_B
      | Self::A2_A
      | Self::A2_B
      | Self::A2_U
      | Self::A3_A
      | Self::A3_B
      | Self::A3_U
      | Self::A4
      | Self::A4F
      | Self::A4E => OutputIntentSubtype::PDFA,
    }
  }

  fn write_xmp(self, xmp: &mut XmpWriter) {
    match self {
      Self::A1_A => {
        xmp.pdfa_part(1);
        xmp.pdfa_conformance("A");
      }
      Self::A1_B => {
        xmp.pdfa_part(1);
        xmp.pdfa_conformance("B");
      }
      Self::A2_A => {
        xmp.pdfa_part(2);
        xmp.pdfa_conformance("A");
      }
      Self::A2_B => {
        xmp.pdfa_part(2);
        xmp.pdfa_conformance("B");
      }
      Self::A2_U => {
        xmp.pdfa_part(2);
        xmp.pdfa_conformance("U");
      }
      Self::A3_A => {
        xmp.pdfa_part(3);
        xmp.pdfa_conformance("A");
      }
      Self::A3_B => {
        xmp.pdfa_part(3);
        xmp.pdfa_conformance("B");
      }
      Self::A3_U => {
        xmp.pdfa_part(3);
        xmp.pdfa_conformance("U");
      }
      Self::A4 => {
        xmp.pdfa_part(4);
        xmp.pdfa_rev(2020);
      }
      Self::A4F => {
        xmp.pdfa_part(4);
        xmp.pdfa_rev(2020);
        xmp.pdfa_conformance("F");
      }
      Self::A4E => {
        xmp.pdfa_part(4);
        xmp.pdfa_rev(2020);
        xmp.pdfa_conformance("E");
      }
    }
  }

  fn write_xmp_extension_schema_description(
    self,
    extension_schemas: &mut PdfAExtSchemasWriter<'_, '_>,
  ) {
    if !self.requires_xmp_metadata_extension_schema() {
      return;
    }

    extension_schemas
      .xmp_media_management()
      .properties()
      .describe_instance_id();
    extension_schemas.pdf().properties().describe_all();
  }

  /// Returns a human-readable string representation of the conformance level.
  pub fn as_str(self) -> &'static str {
    match self {
      Self::A1_A => "PDF/A-1a",
      Self::A1_B => "PDF/A-1b",
      Self::A2_A => "PDF/A-2a",
      Self::A2_B => "PDF/A-2b",
      Self::A2_U => "PDF/A-2u",
      Self::A3_A => "PDF/A-3a",
      Self::A3_B => "PDF/A-3b",
      Self::A3_U => "PDF/A-3u",
      Self::A4 => "PDF/A-4",
      Self::A4F => "PDF/A-4f",
      Self::A4E => "PDF/A-4e",
    }
  }

  /// Minimum PDF version required to use this standard, if any.
  pub const fn min(self) -> Option<PdfVersion> {
    match self {
      // PDF/A-1 through 3 require XMP `/Metadata` streams, which require PDF 1.4.
      Self::A1_A | Self::A1_B => Some(PdfVersion::Pdf14),
      Self::A2_A | Self::A2_B | Self::A2_U => Some(PdfVersion::Pdf14),
      Self::A3_A | Self::A3_B | Self::A3_U => Some(PdfVersion::Pdf14),
      Self::A4 | Self::A4F | Self::A4E => Some(PdfVersion::Pdf20),
    }
  }

  /// Maximum PDF version this standard can be used with.
  pub const fn max(self) -> PdfVersion {
    match self {
      Self::A1_A | Self::A1_B => PdfVersion::Pdf14,
      Self::A2_A | Self::A2_B | Self::A2_U | Self::A3_A | Self::A3_B | Self::A3_U => {
        PdfVersion::Pdf17
      }
      Self::A4 | Self::A4F | Self::A4E => PdfVersion::Pdf20,
    }
  }
}

/// A validator for exporting PDF documents to a specific subset of PDF.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum Accessibility {
  /// The validator for the PDF/UA-1 standard.
  ///
  /// **Requirements**:
  ///
  /// General:
  /// - All real content should be tagged accordingly.
  /// - All artifacts should be marked accordingly.
  /// - The tag tree should reflect the logical reading order of the
  ///   document.
  /// - Information should not be conveyed by contrast, color, format
  ///   or layout.
  /// - All "best practice" notes in [`TagKind`] need to be complied with.
  ///
  /// Text:
  /// - You should make use of the `Alt`, `ActualText`, `Lang` and `Expansion` attributes
  ///   whenever possible.
  /// - Usually, you can provide an empty string as `Lang` to indicate that a language is unknown.
  ///   You should not do that in PDF/UA.
  /// - Stretchable characters (such as brackets, which often consist of several glyphs)
  ///   should be marked accordingly with `ActualText`.
  ///
  ///  Graphics:
  /// - Graphics should be tagged as figures (unless they are an artifact).
  /// - Graphics need to be followed by a caption.
  /// - Graphics that possess semantic values only in combination with other graphics
  ///   should be tagged with a single Figure tag for each figure.
  /// - If a more accessible representation exists, it should be used over graphics.
  ///
  /// Headings:
  /// - Headings should be tagged as such.
  /// - For not strongly structured documents, H1 should be the first
  ///   heading.
  ///
  /// Tables:
  /// - Tables should include headers and be tagged accordingly.
  /// - Tables should only be used to represent content within logical row/column relationship.
  ///
  /// Lists:
  /// - List items should be tagged with Li tags, if necessary also with
  ///   Lbl and LBody tags.
  /// - Lists should only be used when the content is intended to be read
  ///   as a list.
  ///
  /// Mathematical expressions:
  /// - All mathematical expressions should be enclosed with
  ///   a `Formula` tag.
  ///
  /// Headers and footers:
  /// - Headers and footers should be marked as corresponding
  ///   artifacts.
  ///
  /// Notes and references:
  /// - Footnotes, endnotes, note labels and references should be
  ///   tagged accordingly and use tagged annotations.
  /// - Footnotes and end notes should use the `Note` tag.
  ///
  /// Navigation:
  /// - The document must contain an outline, and it should reflect
  ///   the reading order of the document.
  /// - Page labels should be semantically appropriate.
  ///
  /// Annotations:
  /// - Annotations should be present in the tag tree in the correct
  ///   reading order.
  ///
  /// Fonts:
  /// - You should only use fonts that are legally embeddable in a file for unlimited,
  ///   universal rendering.
  ///
  /// [`TagKind`]: crate::krilla::interchange::tagging::TagKind
  UA1,
}

impl Accessibility {
  fn prohibits(self, error: &ValidationError) -> bool {
    match (self, error) {
      (
        Self::UA1,
        ValidationError::ContainsNotDefGlyph(_, _, _)
        | ValidationError::NoCodepointMapping(_, _, _)
        | ValidationError::InvalidCodepointMapping(_, _, _, _)
        | ValidationError::RestrictedLicense(_)
        | ValidationError::NoDocumentTitle
        | ValidationError::MissingAltText(_)
        | ValidationError::MissingHeadingTitle
        | ValidationError::MissingDocumentOutline
        | ValidationError::MissingAnnotationAltText(_)
        | ValidationError::EmbeddedFile(EmbedError::MissingDescription, _)
        | ValidationError::MissingTagging
        | ValidationError::EmbeddedPDF(_)
        | ValidationError::RequiresNewerPdfVersion(
          VersionedFeature::HeaderFooterArtifactSubtypes
          | VersionedFeature::StructureOrderTabbing
          | VersionedFeature::TableHeaderScope,
          _,
        ),
      ) => true,
      (
        Self::UA1,
        ValidationError::TooLongString
        | ValidationError::TooLongName
        | ValidationError::TooLongArray
        | ValidationError::TooLongDictionary
        | ValidationError::TooLargeFloat
        | ValidationError::TooManyIndirectObjects
        | ValidationError::TooHighQNestingLevel
        | ValidationError::ContainsPostScript(_)
        | ValidationError::MissingCMYKProfile
        | ValidationError::InconsistentSeparationFallback(_)
        | ValidationError::UnicodePrivateArea(_, _, _, _)
        | ValidationError::NoDocumentLanguage
        | ValidationError::Transparency(_)
        | ValidationError::ImageInterpolation(_)
        | ValidationError::EmbeddedFile(
          EmbedError::Existence | EmbedError::MissingDate | EmbedError::MissingMimeType,
          _,
        )
        | ValidationError::MissingDocumentDate,
      ) => false,
    }
  }

  fn requires_codepoint_mappings(self) -> bool {
    match self {
      Self::UA1 => true,
    }
  }

  fn requires_display_doc_title(self) -> bool {
    match self {
      Self::UA1 => true,
    }
  }

  const fn requires_tagging(self) -> bool {
    true
  }

  fn requires_xmp_metadata(self) -> bool {
    match self {
      Self::UA1 => true,
    }
  }

  fn write_xmp(self, xmp: &mut XmpWriter) {
    match self {
      Self::UA1 => {
        xmp.pdfua_part(1);
      }
    }
  }

  fn write_xmp_extension_schema_description(
    self,
    extension_schemas: &mut PdfAExtSchemasWriter<'_, '_>,
  ) {
    // Needs to be updated if [`Self::write_xmp`] gains more properties.
    extension_schemas.pdfua_id().properties().describe_part();
  }

  /// Returns a human-readable string representation of the accessibility level.
  pub fn as_str(self) -> &'static str {
    match self {
      Self::UA1 => "PDF/UA-1",
    }
  }

  /// Minimum PDF version required to use this standard, if any.
  pub const fn min(self) -> Option<PdfVersion> {
    match self {
      // PDF/UA-1 requires Tagged PDF and XMP `/Metadata` streams, which both require PDF 1.4.
      Self::UA1 => Some(PdfVersion::Pdf14),
    }
  }

  /// Maximum PDF version this standard can be used with.
  pub const fn max(self) -> PdfVersion {
    match self {
      // PDF/UA-1 is specified against PDF 1.7.
      Self::UA1 => PdfVersion::Pdf17,
    }
  }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct ValidationStore {
  /// Maps from the name of a Separation colorant to a hash of its fallback
  /// color. Used to track that a name is only ever matched with a single
  /// fallback color. Since Krilla manages the `tintTransform` functions,
  /// those are always equivalent.
  separation_fallback_map: HashMap<SeparationColorant, RegularColor>,
}

impl ValidationStore {
  pub(crate) fn new() -> Self {
    Default::default()
  }

  /// Register a colorant and its fallback and raise an error if it already
  /// exists.
  pub(crate) fn validate_separation(
    &mut self,
    separation: &SeparationSpace,
  ) -> Result<(), ValidationError> {
    if self
      .separation_fallback_map
      .entry(separation.colorant.clone())
      .or_insert(separation.fallback)
      == &separation.fallback
    {
      Ok(())
    } else {
      Err(ValidationError::InconsistentSeparationFallback(
        separation.colorant.clone(),
      ))
    }
  }
}
