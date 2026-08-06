//! Embedding attachments to a PDF file.

use std::ops::DerefMut;

use pdf_writer::{Finish, Name, Ref, Str, TextStr};

use crate::krilla::Data;
use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::configure::{PdfVersion, ValidationError};
use crate::krilla::interchange::metadata::pdf_date;
use crate::krilla::metadata::DateTime;
use crate::krilla::serialize::{Cacheable, SerializeContext};
use crate::krilla::stream::FilterStreamBuilder;
use crate::krilla::surface::Location;
use crate::krilla::util::NameExt;

/// An error while embedding the file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EmbedError {
  /// The selected standard does not support embedding files.
  Existence,
  /// The document doesn't contain a modification date, which is required for embedded files
  /// in some export modes.
  MissingDate,
  /// The embedded file is missing a human-readable description.
  MissingDescription,
  /// The mime type of the embedded file is missing.
  MissingMimeType,
}

/// An embedded file.
#[derive(Debug, Clone, Hash)]
pub struct EmbeddedFile {
  /// The name of the embedded file.
  pub path: String,
  /// The mime type of the embedded file.
  pub mime_type: Option<MimeType>,
  /// A description of the embedded file.
  pub description: Option<String>,
  /// The association kind of the embedded file.
  pub association_kind: AssociationKind,
  /// The raw data of the embedded file.
  pub data: Data,
  /// The modification date of the embedded file.
  pub modification_date: Option<DateTime>,
  /// Whether the embedded file should be compressed (recommended to turn off if the
  /// original file already has compression). If `None`, krilla will use its own logic
  /// for determining whether to compress the file or not.
  pub compress: Option<bool>,
  /// The location of the embedded file.
  pub location: Option<Location>,
}

impl Cacheable for EmbeddedFile {
  fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    sc.register_validation_error(ValidationError::EmbeddedFile(
      EmbedError::Existence,
      self.location,
    ));

    let chunk = &mut chunk_container.non_stream.embedded_files;
    let mut stream_chunk = sc.new_chunk();
    let stream_ref = sc.new_ref();

    let file_stream = match self.compress {
      Some(true) => FilterStreamBuilder::new_from_binary_data(self.data.as_ref()),
      Some(false) => FilterStreamBuilder::new_from_uncompressed(self.data.as_ref()),
      None => FilterStreamBuilder::new_auto_compressed(self.data.as_ref()),
    }
    .finish(&sc.serialize_settings());

    let mut embedded_file_stream =
      stream_chunk.embedded_file(stream_ref, file_stream.encoded_data());
    file_stream.write_filters(embedded_file_stream.deref_mut().deref_mut());

    if let Some(mime_type) = &self.mime_type {
      embedded_file_stream.subtype(mime_type.0.to_pdf_name());
    } else {
      sc.register_validation_error(ValidationError::EmbeddedFile(
        EmbedError::MissingMimeType,
        self.location,
      ));
    }

    let mut params = embedded_file_stream.params();
    params.size(self.data.as_ref().len() as i32);

    if let Some(date_time) = &self.modification_date {
      let date = pdf_date(*date_time);
      params.modification_date(date);
    } else {
      sc.register_validation_error(ValidationError::EmbeddedFile(
        EmbedError::MissingDate,
        self.location,
      ));
    }

    params.finish();
    embedded_file_stream.finish();

    let mut file_spec = chunk.file_spec(root_ref);
    file_spec.path(Str(self.path.as_bytes()));

    if sc.serialize_settings().pdf_version() >= PdfVersion::Pdf17 {
      file_spec.unic_file(TextStr(&self.path));
    }

    let mut ef = file_spec.insert(Name(b"EF")).dict();
    ef.pair(Name(b"F"), stream_ref);

    if sc.serialize_settings().pdf_version() >= PdfVersion::Pdf17 {
      ef.pair(Name(b"UF"), stream_ref);
    }

    ef.finish();

    if sc.serialize_settings().supports_associated_files() {
      file_spec.association_kind(self.association_kind.to_pdf());
    }

    if let Some(description) = self.description {
      file_spec.description(TextStr(&description));
    } else {
      sc.register_validation_error(ValidationError::EmbeddedFile(
        EmbedError::MissingDescription,
        self.location,
      ));
    }

    file_spec.finish();
    chunk_container.streams.embedded_files.push(stream_chunk);
  }
}

/// How an embedded file relates to the PDF document it is embedded in.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum AssociationKind {
  /// The PDF document was created from this source file.
  Source,
  /// This file was used to derive a visual presentation in the PDF.
  Data,
  /// An alternative representation of this document.
  Alternative,
  /// Additional resources for this document.
  Supplement,
  /// There is no clear relationship or it is not known.
  Unspecified,
}

impl AssociationKind {
  fn to_pdf(self) -> pdf_writer::types::AssociationKind {
    match self {
      AssociationKind::Source => pdf_writer::types::AssociationKind::Source,
      AssociationKind::Data => pdf_writer::types::AssociationKind::Data,
      AssociationKind::Alternative => pdf_writer::types::AssociationKind::Alternative,
      AssociationKind::Supplement => pdf_writer::types::AssociationKind::Supplement,
      AssociationKind::Unspecified => pdf_writer::types::AssociationKind::Unspecified,
    }
  }
}

/// A mime type.
#[derive(Debug, Clone, Hash)]
pub struct MimeType(String);

impl MimeType {
  /// Create a new mime type.
  ///
  /// Returns `None` if the mime type is invalid.
  pub fn new(mime_type: &str) -> Option<Self> {
    if valid_mime_type(mime_type) {
      Some(Self(mime_type.to_string()))
    } else {
      None
    }
  }
}

fn valid_mime_type(mime_type: &str) -> bool {
  let parts: Vec<&str> = mime_type.split('/').collect();
  if parts.len() != 2 {
    return false;
  }

  let (type_part, subtype_part) = (parts[0], parts[1]);

  valid_mime_part(type_part) && valid_mime_part(subtype_part)
}

fn valid_mime_part(part: &str) -> bool {
  if part.is_empty() {
    return false;
  }

  part
    .chars()
    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '+' || c == '.')
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn valid_mime_types() {
    assert!(valid_mime_type("text/plain"));
    assert!(valid_mime_type("application/pdf"));
    assert!(valid_mime_type("image/jpeg"));
    assert!(valid_mime_type("application/octet-stream"));
    assert!(valid_mime_type("text/html"));
    assert!(valid_mime_type("application/json"));
    assert!(valid_mime_type("image/png"));
    assert!(valid_mime_type("video/mp4"));
    assert!(valid_mime_type("audio/mpeg"));
    assert!(valid_mime_type("application/vnd.ms-excel"));
    assert!(valid_mime_type("text/css"));
    assert!(valid_mime_type("application/x-www-form-urlencoded"));
    assert!(valid_mime_type("multipart/form-data"));
    assert!(valid_mime_type("application/xml+svg"));
    assert!(valid_mime_type("text/plain+charset"));
  }

  #[test]
  fn invalid_mime_types() {
    assert!(!valid_mime_type("source"));
    assert!(!valid_mime_type("text/"));
    assert!(!valid_mime_type("/plain"));
    assert!(!valid_mime_type(""));
    assert!(!valid_mime_type("text"));
    assert!(!valid_mime_type("text/plain/extra"));
    assert!(!valid_mime_type("text plain"));
    assert!(!valid_mime_type("text\\plain"));
    assert!(!valid_mime_type("text@plain"));
    assert!(!valid_mime_type("text/plain*"));
  }
}
