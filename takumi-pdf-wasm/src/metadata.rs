//! Document metadata, attachment and conformance inputs, and their conversions
//! into the takumi-pdf types.

use serde::Deserialize;
use serde_bytes::ByteBuf;
use takumi_pdf::{
  Attachment, AttachmentRelationship, PdfMetadata, PdfStandard, Tagging, XmpProperty, XmpSchema,
};

use crate::date::parse_date;

/// A file attached to the document.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachmentInput {
  name: String,
  data: AttachmentData,
  mime_type: Option<String>,
  description: Option<String>,
  relationship: Option<RelationshipInput>,
  /// UTC modification date as `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`.
  modification_date: Option<String>,
}

/// Attachment bytes, or a string encoded as UTF-8.
#[derive(Deserialize)]
#[serde(untagged)]
enum AttachmentData {
  Bytes(ByteBuf),
  Text(String),
}

/// AFRelationship names accepted from JS.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum RelationshipInput {
  Source,
  Data,
  Alternative,
  Supplement,
  Unspecified,
}

impl From<RelationshipInput> for AttachmentRelationship {
  fn from(relationship: RelationshipInput) -> Self {
    match relationship {
      RelationshipInput::Source => Self::Source,
      RelationshipInput::Data => Self::Data,
      RelationshipInput::Alternative => Self::Alternative,
      RelationshipInput::Supplement => Self::Supplement,
      RelationshipInput::Unspecified => Self::Unspecified,
    }
  }
}

impl TryFrom<AttachmentInput> for Attachment {
  type Error = js_sys::Error;

  fn try_from(input: AttachmentInput) -> Result<Self, Self::Error> {
    let modification_date = input
      .modification_date
      .as_deref()
      .map(|value| {
        parse_date(value).ok_or_else(|| {
          js_sys::Error::new("invalid modificationDate: expected YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS")
        })
      })
      .transpose()?;

    Ok(Self {
      name: input.name,
      data: match input.data {
        AttachmentData::Bytes(bytes) => bytes.into_vec(),
        AttachmentData::Text(text) => text.into_bytes(),
      },
      mime_type: input.mime_type,
      description: input.description,
      relationship: input.relationship.map(Into::into).unwrap_or_default(),
      modification_date,
    })
  }
}

/// `tagged` values accepted from JS.
#[derive(Deserialize, Clone, Copy)]
#[serde(untagged)]
pub(crate) enum TaggedInput {
  Enabled(bool),
  Accessibility(AccessibilityInput),
}

/// The accessibility standard names accepted from JS.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AccessibilityInput {
  Ua1,
  Ua2,
}

impl From<TaggedInput> for Tagging {
  fn from(tagged: TaggedInput) -> Self {
    match tagged {
      TaggedInput::Enabled(false) => Tagging::Off,
      TaggedInput::Enabled(true) => Tagging::On,
      TaggedInput::Accessibility(AccessibilityInput::Ua1) => Tagging::Ua1,
      TaggedInput::Accessibility(AccessibilityInput::Ua2) => Tagging::Ua2,
    }
  }
}

/// PDF/A conformance level names accepted from JS.
#[derive(Deserialize, Clone, Copy)]
pub(crate) enum PdfaInput {
  #[serde(rename = "2a")]
  A2a,
  #[serde(rename = "3a")]
  A3a,
  #[serde(rename = "2b")]
  A2b,
  #[serde(rename = "2u")]
  A2u,
  #[serde(rename = "3b")]
  A3b,
  #[serde(rename = "3u")]
  A3u,
  #[serde(rename = "4")]
  A4,
  #[serde(rename = "4f")]
  A4f,
}

impl From<PdfaInput> for PdfStandard {
  fn from(pdfa: PdfaInput) -> Self {
    match pdfa {
      PdfaInput::A2a => PdfStandard::A2a,
      PdfaInput::A3a => PdfStandard::A3a,
      PdfaInput::A2b => PdfStandard::A2b,
      PdfaInput::A2u => PdfStandard::A2u,
      PdfaInput::A3b => PdfStandard::A3b,
      PdfaInput::A3u => PdfStandard::A3u,
      PdfaInput::A4 => PdfStandard::A4,
      PdfaInput::A4f => PdfStandard::A4f,
    }
  }
}

/// Document metadata fields.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetadataInput {
  title: Option<String>,
  description: Option<String>,
  authors: Option<Vec<String>>,
  keywords: Option<Vec<String>>,
  creator: Option<String>,
  /// UTC creation date as `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`.
  creation_date: Option<String>,
  /// Custom XMP schemas written into the packet.
  xmp: Option<Vec<XmpSchemaInput>>,
}

/// A custom XMP namespace and the properties written under it.
#[derive(Deserialize)]
pub(crate) struct XmpSchemaInput {
  name: String,
  prefix: String,
  namespace: String,
  properties: Vec<XmpPropertyInput>,
}

/// A property of an [`XmpSchemaInput`].
#[derive(Deserialize)]
pub(crate) struct XmpPropertyInput {
  name: String,
  value: String,
  description: String,
}

impl From<XmpSchemaInput> for XmpSchema {
  fn from(input: XmpSchemaInput) -> Self {
    Self {
      name: input.name,
      prefix: input.prefix,
      namespace: input.namespace,
      properties: input.properties.into_iter().map(Into::into).collect(),
    }
  }
}

impl From<XmpPropertyInput> for XmpProperty {
  fn from(input: XmpPropertyInput) -> Self {
    Self {
      name: input.name,
      value: input.value,
      description: input.description,
    }
  }
}

impl TryFrom<MetadataInput> for PdfMetadata {
  type Error = js_sys::Error;

  fn try_from(input: MetadataInput) -> Result<Self, Self::Error> {
    let creation_date = input
      .creation_date
      .as_deref()
      .map(|value| {
        parse_date(value).ok_or_else(|| {
          js_sys::Error::new("invalid creationDate: expected YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS")
        })
      })
      .transpose()?;

    Ok(Self {
      title: input.title,
      description: input.description,
      authors: input.authors.unwrap_or_default(),
      keywords: input.keywords.unwrap_or_default(),
      creator: input.creator,
      creation_date,
      xmp: input
        .xmp
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect(),
    })
  }
}
