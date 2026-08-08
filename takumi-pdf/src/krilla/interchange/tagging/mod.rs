//! Creating accessible PDF documents.
//!
//! # Introduction
//!
//! A document usually consists of many smaller semantic building blocks, like for example
//! titles, headings, paragraphs, tables, headers, footers, and so on. However, it is important
//! to understand that, by default, when exporting a document to PDF, all of this semantic
//! information is lost.
//!
//! By default, a PDF document doesn't have a notion of a table or a
//! paragraph. Instead, it consists of very low-level instructions, such as "draw
//! a path at that location" or "draw a line of glyphs at that font size". A table is
//! not encoded as a "table", but instead as a number of rectangle-like paths that happen
//! to surround lines of text. This is what made PDF popular in the first place, as encoding
//! information at such a low level allows to ensure a consistent viewing experience across
//! different platforms and viewers.
//!
//! However, this design has drawbacks, one of the main ones being that it leads to the
//! production of non-accessible documents. Especially in recent years, ensuring the
//! accessibility of documents has become an increasingly important requirement.
//! To address this deficiency, PDF introduced the notion of "tagged PDF", which consists
//! of enriching PDF documents with additional semantic information in a way that can be
//! interpreted by different consumers. krilla supports the creation of such documents.
//!
//! # A word on krilla's implementation
//!
//! As nearly everything in PDF, tagging is a really complex topic, and getting it right
//! is very hard. Because of this, in line with the general philosophy of krilla, some
//! of the potential capabilities of tagged PDF are not directly supported. Instead, only
//! a specific subset has been implemented, with a focus on features that improve the
//! accessibility of documents.
//!
//! Please note that doing tagging 100% correctly as recommended by various PDF standards is very
//! difficult. While the documentation lists many of the recommendations to abstract away
//! as much as possible, users are still expected to consult the given specifications to
//! ensure their implementation matches the specification. A "reference implementation" that
//! could be good to consult is the [`typst-pdf`](https://github.com/typst/typst/tree/main/crates/typst-pdf)
//! crate which implements most of the expected features for well-tagged PDFs.
//!
//! # Basic Principles
//!
//! The way tagged PDFs are created is by attaching a tag tree to the PDF document
//! that encodes the logical structure of the document. As mentioned above, a raw PDF file
//! mainly consists of text- and path-drawing primitives, which are not necessarily
//! drawn in the logical reading order of the document. What the tag tree does is
//! maping the different "snippets" of the PDF file to the tree-like structure in a way
//! that reflects the logical structure of the document, in reading-order.
//!
//! For example, a document can consist of multiple "sections", where each section might contain
//! headings, paragraphs or figures. A figure might consist of a table as well as a caption.
//! A table consists yet again of smaller semantic components, like a header, footer
//! and the data cells, which usually contain some text. These kinds of hierarchical structures
//! can be encoded with the help of tagging.
//!
//! A tag tree consists of two components:
//! - Group nodes, which represents a component with certain semantics. A group node must have
//!   at least one child, otherwise it's discarded.
//! - Leaf nodes, which represent the actual pieces on the page that form part of a group.
//!
//! # How to create a tagged document
//!
//! If you want to create a tagged document, you need to follow the following steps:
//!
//! 1) Ensure that you activate the `enable_tagging` attribute in [`SerializeSettings`].
//! 2) Create a [tag tree](TagTree), which represents the "root" of a tag tree.
//! 3) As you create your document, create new [tag groups](TagGroup) with corresponding [tags](Tag).
//!    Nest them with other tag groups, if necessary, by using the `push` method.
//! 4) Populate tag groups with [identifiers](Identifier), which represent the leaf nodes
//!    in the tag tree. Identifiers are unique and point to a sequence of content on the
//!    page. If you push an identifier to a tag group, then all content that is marked by
//!    that identifier belongs semantically to that tag group. There are currently two ways
//!    of obtaining an identifier:
//!
//!    - Use the `add_tagged_annotation` method on [`Page`], which allows you to associate
//!      annotations to the content they correspond to. Currently, krilla only supports link
//!      annotations, and a link annotation should always be a child in a tag group with the
//!      [`TagKind`] [`Link`](TagKind::Link), with its sibling being an identifier or another tag group that is
//!      to be associated with the link.
//!    - Use the `start_tagged` command on [`Surface`], which returns an [Identifier], and
//!      indicates that all content drawn on the surface should be associated with that
//!      identifier, until you call the `end_tagged` method. *Important*: Note that you cannot
//!      nest calls to `start_tagged`, and you have to ensure that you always call a corresponding
//!      `end_tagged`. Otherwise, krilla will panic.
//!
//!      It is very important that each identifier you create has exactly one parent in the tag
//!      tree. This means that you cannot create an identifier and not use it at all (0 parents),
//!      or use the same identifier in two different parts of the tree (1+ parents). Otherwise,
//!      export will fail.
//!
//! 5) Once you have built your tag tree, simply call `set_tag_tree` on [`Document`]. That's it!
//!
//! # Other notes
//!
//! Make sure that you carefully read the documentation of the other parts of this module, as
//! there are some more points as well as best practices you need to keep in mind
//! when creating well-tagged documents. The PDF specification is in some places very vague
//! on how a tagged document should look like, so there is quite a bit of ambiguity.
//!
//! Apart from that, the PDF specification does make a few statements on requirements a well-tagged PDF
//! should follows, although  those are not really "strict" requirements in the sense that they can
//! be automatically checked by a PDF validator, so not conforming to some of those points does not
//! suddenly make your document a badly-tagged document! However, if possible, you should still
//! try to comply with the following requirements:
//!
//! - In general, all contents in your file should be tagged, either as an artifact or with
//!   Span/Other.
//! - The order of elements in the tag tree should represent the logical reading order, including
//!   annotations.
//! - Word breaks in text should be represented explicitly with spaces, instead of implicitly
//!   by not including them, but instead positioning text in a way that "simulates" the spaces.
//! - Hyphenation should be represented as a soft hyphen character (U+00AD) instead
//!   of a hard hyphen (U+002D).
//! - Tag groups should follow the best-practice of what kind of children they contain. See
//!   [`TagKind`] for more information.
//! - You should provide "Alt" descriptions for formulas and images.
//! - In case you have a link annotation that applies to text that wraps into one or multiple
//!   new lines, you should use the `quad_points` functionality to indicate the exact covered
//!   areas of the link.
//!
//! Once again, the above is only a best-effort summary, if you are interested in creating
//! completely well-tagged PDFs, you are advised to consult the given specifications.
//!
//! [`SerializeSettings`]: crate::krilla::SerializeSettings
//! [`Page`]: crate::krilla::page::Page
//! [`Surface`]: crate::krilla::surface::Surface
//! [`Document`]: crate::krilla::Document

use std::cell::LazyCell;
use std::cmp::PartialEq;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::io::Write as _;

use pdf_writer::types::{RoleMapOpts, StructRole, StructRole2};
use pdf_writer::writers::{PropertyList, StructElement};
use pdf_writer::{Chunk, Finish, Name, Ref, Str, TextStr};
use smallvec::SmallVec;

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::configure::validate::VersionedFeature;
use crate::krilla::configure::{PdfVersion, ValidationError};
use crate::krilla::error::{KrillaError, KrillaResult};
use crate::krilla::geom::Rect;
use crate::krilla::page::page_root_transform;
use crate::krilla::serialize::SerializeContext;

pub use tag::*;

pub mod fmt;
mod tag;

/// An artifact that should not be part of the accessible structure.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Artifact {
  /// The type of the artifact.
  pub kind: ArtifactType,
  /// The bounding box that incloses the artifact's visual content. Required
  /// for background artifacts.
  pub bbox: Option<Rect>,
}

impl Artifact {
  /// Create a new artifact with a type and an optional BBox.
  ///
  /// You must provide a bounding box if `kind` is
  /// [`ArtifactType::Background`] and you are targetting PDF 1.7, otherwise,
  /// Krilla will later panic.
  pub fn new(kind: ArtifactType, bbox: Option<Rect>) -> Self {
    Self { kind, bbox }
  }

  /// Create a new artifact with a type and no bounding box.
  pub fn with_kind(kind: ArtifactType) -> Self {
    Self::new(kind, None)
  }

  /// Whether the artifacts requires a property list.
  pub(crate) fn requires_properties(self, pdf_version: PdfVersion) -> bool {
    self.bbox.is_some()
      || self
        .kind
        .map_pdf_version(pdf_version)
        .to_pdf_artifact_type()
        .is_some()
  }

  /// Whether the artifact requires a BBox.
  pub(crate) fn requires_bbox(self, pdf_version: PdfVersion) -> bool {
    // Background artifacts were introduced in PDF 1.7. In table 330, the
    // standard makes bounding boxes mandatory for background artifacts
    // only. PDF 2.0 lifts this requirement in table 363.
    //
    // When changing the below condition, also change the error at the
    // callsite.
    self.kind == ArtifactType::Background && pdf_version == PdfVersion::Pdf17
  }
}

/// A type of artifact.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub enum ArtifactType {
  /// The header of a page.
  Header,
  /// The footer of the page.
  Footer,
  /// For text in the back- or foreground of all pages.
  Watermark,
  /// Page numbers.
  PageNumber,
  /// Numbering artifacts before lines.
  LineNumber,
  /// Areas where there formerly was content, but which has been removed.
  Redaction,
  /// Bates numbering.
  Bates,
  /// Other artifacts arising from pagination not covered by the above variants.
  PaginationOther,
  /// Purely cosmetic typographical or design elements.
  Layout,
  /// Page artifacts, such as for example cut marks or color bars.
  Page,
  /// The background of a page or a graphical element.
  Background,
  /// Any other type of artifact.
  #[default]
  Other,
}

impl ArtifactType {
  pub(crate) fn map_pdf_version(self, version: PdfVersion) -> Self {
    match self {
      Self::PageNumber | Self::LineNumber | Self::Redaction | Self::Bates
        if version < PdfVersion::Pdf20 =>
      {
        ArtifactType::PaginationOther
      }
      Self::Header | Self::Footer | Self::Watermark if version < PdfVersion::Pdf17 => {
        ArtifactType::PaginationOther
      }
      Self::Background if version < PdfVersion::Pdf17 => ArtifactType::Other,
      _ => self,
    }
  }

  pub(crate) fn to_pdf_artifact_type(self) -> Option<pdf_writer::types::ArtifactType> {
    match self {
      ArtifactType::Header => Some(pdf_writer::types::ArtifactType::Pagination),
      ArtifactType::Footer => Some(pdf_writer::types::ArtifactType::Pagination),
      ArtifactType::Watermark => Some(pdf_writer::types::ArtifactType::Pagination),
      ArtifactType::PageNumber => Some(pdf_writer::types::ArtifactType::Pagination),
      ArtifactType::LineNumber => Some(pdf_writer::types::ArtifactType::Pagination),
      ArtifactType::Redaction => Some(pdf_writer::types::ArtifactType::Pagination),
      ArtifactType::Bates => Some(pdf_writer::types::ArtifactType::Pagination),
      ArtifactType::PaginationOther => Some(pdf_writer::types::ArtifactType::Pagination),
      ArtifactType::Layout => Some(pdf_writer::types::ArtifactType::Layout),
      ArtifactType::Page => Some(pdf_writer::types::ArtifactType::Page),
      ArtifactType::Background => Some(pdf_writer::types::ArtifactType::Background),
      ArtifactType::Other => None,
    }
  }

  pub(crate) fn to_pdf_artifact_subtype(
    self,
  ) -> Option<pdf_writer::types::ArtifactSubtype<'static>> {
    match self {
      ArtifactType::Header => Some(pdf_writer::types::ArtifactSubtype::Header),
      ArtifactType::Footer => Some(pdf_writer::types::ArtifactSubtype::Footer),
      ArtifactType::Watermark => Some(pdf_writer::types::ArtifactSubtype::Watermark),
      ArtifactType::PageNumber => Some(pdf_writer::types::ArtifactSubtype::PageNumber),
      ArtifactType::LineNumber => Some(pdf_writer::types::ArtifactSubtype::LineNumber),
      ArtifactType::Redaction => Some(pdf_writer::types::ArtifactSubtype::Redaction),
      ArtifactType::Bates => Some(pdf_writer::types::ArtifactSubtype::Bates),
      _ => None,
    }
  }
}

/// A language identifier as specified in RFC 3066. It will not be validated, so
/// it's on the user of the library to ensure the tag is valid.
pub type Lang<'a> = &'a str;
/// An alternate text that describes some element in natural language.
pub type Alt<'a> = &'a str;
/// The actual intended textual content of a span. For example,
/// if you have a hyphenated word, you can use `ActualText` to describe
/// the same word without hyphens.
pub type ActualText<'a> = &'a str;
/// The expanded form of an abbreviation.
pub type Expanded<'a> = &'a str;

/// A content tag associated with the content it wraps.
#[derive(Clone, Copy, Debug)]
pub enum ContentTag<'a> {
  /// Artifacts represent pieces of content that are not really part of the logical structure
  /// of a document and should be excluded in the logical tree. These include for example headers,
  /// footers, page background and similar.
  Artifact(Artifact),
  /// A content tag that wraps some text with specific properties.
  ///
  /// Spans should not be too long. At most, they should contain a single line of text, but they
  /// can obviously be shorter, if text within a single line contains text with different styles
  /// or different languages.
  Span(SpanTag<'a>),
  /// Use this tag for anything else that does not semantically fit into `Span` or `Artifact`.
  /// This includes for example arbitrary paths, images or a mix of different content that cannot
  /// be split up more.
  Other,
}

impl ContentTag<'_> {
  pub(crate) fn name(&self) -> Name<'static> {
    match self {
      ContentTag::Artifact(_) => Name(b"Artifact"),
      ContentTag::Span(_) => Name(b"Span"),
      ContentTag::Other => Name(b"P"),
    }
  }

  pub(crate) fn write_properties(
    &self,
    sc: &mut SerializeContext,
    mut properties: PropertyList,
    page_height: f32,
  ) {
    match self {
      ContentTag::Artifact(artifact) => {
        let at = artifact
          .kind
          .map_pdf_version(sc.serialize_settings().pdf_version());
        let mut artifact_props = properties.artifact();

        if let Some(bbox) = artifact.bbox {
          let transform = page_root_transform(page_height);
          let actual_rect = bbox.transform(transform).unwrap();
          artifact_props.bounding_box(actual_rect.to_pdf_rect());
        }

        if sc.serialize_settings().pdf_version() >= PdfVersion::Pdf17 {
          if at == ArtifactType::Header {
            artifact_props.attached([pdf_writer::types::ArtifactAttachment::Top]);
          }

          if at == ArtifactType::Footer {
            artifact_props.attached([pdf_writer::types::ArtifactAttachment::Bottom]);
          }

          if let Some(subtype) = at.to_pdf_artifact_subtype() {
            artifact_props.subtype(subtype);
          }
        }

        if let Some(artifact_type) = at.to_pdf_artifact_type() {
          artifact_props.kind(artifact_type);
        }
      }
      ContentTag::Span(SpanTag {
        lang,
        alt_text,
        expanded,
        actual_text,
      }) => {
        if let Some(lang) = lang {
          properties.pair(Name(b"Lang"), TextStr(lang));
        }

        if let Some(alt) = alt_text {
          if sc.serialize_settings().pdf_version() >= PdfVersion::Pdf15 {
            properties.pair(Name(b"Alt"), TextStr(alt));
          }
        }

        if let Some(exp) = expanded {
          properties.pair(Name(b"E"), TextStr(exp));
        }

        if let Some(actual) = actual_text {
          if sc.serialize_settings().pdf_version() >= PdfVersion::Pdf15 {
            properties.actual_text(TextStr(actual));
          }
        }
      }
      ContentTag::Other => {}
    }
  }
}

/// A span tag.
#[derive(Clone, Copy, Debug)]
pub struct SpanTag<'a> {
  /// The language of the text.
  pub lang: Option<Lang<'a>>,
  /// An optional alternate text that describes the text (for example, if the text consists
  /// of a star symbol, the alt text should describe that in natural language).
  pub alt_text: Option<Alt<'a>>,
  /// If the content of the span is an abbreviation, the expanded form of the
  /// abbreviation should be provided here.
  pub expanded: Option<Expanded<'a>>,
  /// The actual text represented by the glyphs, i.e. if you have a hyphenated span
  /// `row-`, then you can wrap it in an `ActualText` to remove the hyphenation
  /// when copy-pasting.
  pub actual_text: Option<ActualText<'a>>,
}

impl<'a> SpanTag<'a> {
  /// An empty span tag.
  pub fn empty() -> Self {
    Self {
      lang: None,
      alt_text: None,
      expanded: None,
      actual_text: None,
    }
  }

  /// Sets [`SpanTag::lang`].
  pub fn with_lang(mut self, lang: Option<&'a str>) -> Self {
    self.lang = lang;
    self
  }

  /// Sets [`SpanTag::alt_text`].
  pub fn with_alt_text(mut self, alt_text: Option<&'a str>) -> Self {
    self.alt_text = alt_text;
    self
  }

  /// Sets [`SpanTag::expanded`].
  pub fn with_expanded(mut self, expanded: Option<&'a str>) -> Self {
    self.expanded = expanded;
    self
  }

  /// Sets [`SpanTag::actual_text`].
  pub fn with_actual_text(mut self, actual_text: Option<&'a str>) -> Self {
    self.actual_text = actual_text;
    self
  }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PageTagIdentifier {
  pub(crate) page_index: usize,
  pub(crate) mcid: i32,
}

impl From<PageTagIdentifier> for IdentifierType {
  fn from(value: PageTagIdentifier) -> Self {
    IdentifierType::PageIdentifier(value)
  }
}

impl From<PageTagIdentifier> for Identifier {
  fn from(value: PageTagIdentifier) -> Self {
    Identifier(IdentifierInner::Real(value.into()))
  }
}

impl PageTagIdentifier {
  pub(crate) fn new(page_index: usize, mcid: i32) -> Self {
    Self { page_index, mcid }
  }

  pub(crate) fn bump(&mut self) -> PageTagIdentifier {
    let old = *self;

    self.mcid = self.mcid.checked_add(1).unwrap();

    old
  }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct AnnotationIdentifier {
  pub(crate) page_index: usize,
  pub(crate) annot_index: usize,
}

impl From<AnnotationIdentifier> for IdentifierType {
  fn from(value: AnnotationIdentifier) -> Self {
    IdentifierType::AnnotationIdentifier(value)
  }
}

impl From<AnnotationIdentifier> for Identifier {
  fn from(value: AnnotationIdentifier) -> Self {
    Identifier(IdentifierInner::Real(value.into()))
  }
}

impl AnnotationIdentifier {
  pub fn new(page_index: usize, annot_index: usize) -> Self {
    Self {
      page_index,
      annot_index,
    }
  }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum IdentifierType {
  PageIdentifier(PageTagIdentifier),
  AnnotationIdentifier(AnnotationIdentifier),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum IdentifierInner {
  Real(IdentifierType),
  Dummy,
}

/// An identifier for an annotation or certain parts of page content.
///
/// Need to be used as a leaf node in a tag tree.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Identifier(pub(crate) IdentifierInner);

impl Identifier {
  pub(crate) fn new_annotation(page_index: usize, annot_index: usize) -> Self {
    AnnotationIdentifier::new(page_index, annot_index).into()
  }

  pub(crate) fn dummy() -> Self {
    Self(IdentifierInner::Dummy)
  }
}

impl TagKind {
  pub(crate) fn write_kind(&self, struct_elem: &mut StructElement, sc: &mut SerializeContext) {
    let pdf_version = sc.serialize_settings().pdf_version();
    if pdf_version < self.minimum_version() {
      // Fall back to P in case the tag is not supported with the current
      // PDF version
      struct_elem.kind(StructRole::P);
      return;
    }

    match self {
      Self::Part(_) => write_kind_compat(sc, struct_elem, StructRole2::Part),
      Self::Article(_) => write_kind_1_7(struct_elem, StructRole::Art),
      Self::Section(_) => write_kind_compat(sc, struct_elem, StructRole2::Sect),
      Self::Div(_) => write_kind_compat(sc, struct_elem, StructRole2::Div),
      Self::BlockQuote(_) => write_kind_1_7(struct_elem, StructRole::BlockQuote),
      Self::Caption(_) => write_kind_compat(sc, struct_elem, StructRole2::Caption),
      Self::TOC(_) => write_kind_1_7(struct_elem, StructRole::TOC),
      Self::TOCI(_) => write_kind_1_7(struct_elem, StructRole::TOCI),
      Self::Index(_) => write_kind_1_7(struct_elem, StructRole::Index),
      Self::P(_) => write_kind_compat(sc, struct_elem, StructRole2::P),
      Self::L(_) => write_kind_compat(sc, struct_elem, StructRole2::L),
      Self::LI(_) => write_kind_compat(sc, struct_elem, StructRole2::LI),
      Self::Lbl(_) => write_kind_compat(sc, struct_elem, StructRole2::Lbl),
      Self::LBody(_) => write_kind_compat(sc, struct_elem, StructRole2::LBody),
      Self::Table(_) => write_kind_compat(sc, struct_elem, StructRole2::Table),
      Self::TR(_) => write_kind_compat(sc, struct_elem, StructRole2::TR),
      Self::TH(_) => write_kind_compat(sc, struct_elem, StructRole2::TH),
      Self::TD(_) => write_kind_compat(sc, struct_elem, StructRole2::TD),
      Self::THead(_) => write_kind_compat(sc, struct_elem, StructRole2::THead),
      Self::TBody(_) => write_kind_compat(sc, struct_elem, StructRole2::TBody),
      Self::TFoot(_) => write_kind_compat(sc, struct_elem, StructRole2::TFoot),
      Self::Span(_) => write_kind_compat(sc, struct_elem, StructRole2::Span),
      Self::InlineQuote(_) => write_kind_1_7(struct_elem, StructRole::Quote),
      Self::Note(_) => write_kind_1_7(struct_elem, StructRole::Note),
      Self::Reference(_) => write_kind_1_7(struct_elem, StructRole::Reference),
      Self::BibEntry(_) => write_kind_1_7(struct_elem, StructRole::BibEntry),
      Self::Code(_) => write_kind_1_7(struct_elem, StructRole::Code),
      Self::Link(_) => write_kind_compat(sc, struct_elem, StructRole2::Link),
      Self::Annot(_) => write_kind_compat(sc, struct_elem, StructRole2::Annot),
      Self::Figure(_) => write_kind_compat(sc, struct_elem, StructRole2::Figure),
      Self::Formula(_) => write_kind_compat(sc, struct_elem, StructRole2::Formula),
      Self::Form(_) => write_kind_compat(sc, struct_elem, StructRole2::Form),
      Self::NonStruct(_) => write_kind_compat(sc, struct_elem, StructRole2::NonStruct),
      // Custom structure roles that are registered in the `RoleMap`.
      Self::Datetime(_) => write_kind_custom(sc, struct_elem, Name(b"Datetime")),
      Self::Terms(_) => write_kind_custom(sc, struct_elem, Name(b"Terms")),
      Self::Title(_) => write_kind_custom(sc, struct_elem, Name(b"Title")),
      // PDF 2.0 structure roles that are conditionally registered.
      Self::Hn(tag) => {
        let role2 = StructRole2::Heading(tag.level());
        if pdf_version < PdfVersion::Pdf20 {
          // Dynamically register custom headings `Hn` if the level
          // (`n >= 7`) isn't supported by PDF 1.7 and below.
          let compat = role2.compatibility_1_7(RoleMapOpts::default());
          if compat.into_pdf_1_7().is_none() {
            sc.global_objects.custom_heading_roles.insert(tag.level());
          }
          struct_elem.custom_kind(role2.to_name(&mut [0; 6]));
        } else {
          struct_elem.kind_2(role2, sc.pdf2_ns.ssn_ref);
        }
      }
      Self::Strong(_) => {
        if pdf_version < PdfVersion::Pdf20 {
          struct_elem.custom_kind(Name(b"Strong"));
        } else {
          struct_elem.kind_2(StructRole2::Strong, sc.pdf2_ns.ssn_ref);
        }
      }
      Self::Em(_) => {
        if pdf_version < PdfVersion::Pdf20 {
          struct_elem.custom_kind(Name(b"Em"));
        } else {
          struct_elem.kind_2(StructRole2::Em, sc.pdf2_ns.ssn_ref);
        }
      }
    };
  }

  pub(crate) fn minimum_version(&self) -> PdfVersion {
    match self {
      Self::Part(_) => PdfVersion::Pdf14,
      Self::Article(_) => PdfVersion::Pdf14,
      Self::Section(_) => PdfVersion::Pdf14,
      Self::Div(_) => PdfVersion::Pdf14,
      Self::BlockQuote(_) => PdfVersion::Pdf14,
      Self::Caption(_) => PdfVersion::Pdf14,
      Self::TOC(_) => PdfVersion::Pdf14,
      Self::TOCI(_) => PdfVersion::Pdf14,
      Self::Index(_) => PdfVersion::Pdf14,
      Self::P(_) => PdfVersion::Pdf14,
      Self::Hn(_) => PdfVersion::Pdf14,
      Self::L(_) => PdfVersion::Pdf14,
      Self::LI(_) => PdfVersion::Pdf14,
      Self::Lbl(_) => PdfVersion::Pdf14,
      Self::LBody(_) => PdfVersion::Pdf14,
      Self::Table(_) => PdfVersion::Pdf14,
      Self::TR(_) => PdfVersion::Pdf14,
      Self::TH(_) => PdfVersion::Pdf14,
      Self::TD(_) => PdfVersion::Pdf14,
      // TODO: writing `P` tags in PDF 1.4 will break the table structure.
      // Instead consider just transparently inserting all children, which
      // should be `TR`s anyway.
      Self::THead(_) => PdfVersion::Pdf15,
      Self::TBody(_) => PdfVersion::Pdf15,
      Self::TFoot(_) => PdfVersion::Pdf15,
      Self::Span(_) => PdfVersion::Pdf14,
      Self::InlineQuote(_) => PdfVersion::Pdf14,
      Self::Note(_) => PdfVersion::Pdf14,
      Self::Reference(_) => PdfVersion::Pdf14,
      Self::BibEntry(_) => PdfVersion::Pdf14,
      Self::Code(_) => PdfVersion::Pdf14,
      Self::Link(_) => PdfVersion::Pdf14,
      Self::Annot(_) => PdfVersion::Pdf15,
      Self::Figure(_) => PdfVersion::Pdf14,
      Self::Formula(_) => PdfVersion::Pdf14,
      Self::Form(_) => PdfVersion::Pdf14,
      Self::NonStruct(_) => PdfVersion::Pdf14,
      Self::Datetime(_) => PdfVersion::Pdf14,
      Self::Terms(_) => PdfVersion::Pdf14,
      Self::Title(_) => PdfVersion::Pdf14,
      Self::Strong(_) => PdfVersion::Pdf14,
      Self::Em(_) => PdfVersion::Pdf14,
    }
  }

  pub(crate) fn should_have_alt(&self) -> bool {
    matches!(self, TagKind::Figure(_) | TagKind::Formula(_))
  }

  pub(crate) fn can_have_title(&self) -> bool {
    matches!(self, Self::Hn(_))
  }
}

fn write_kind_1_7(struct_elem: &mut StructElement, role: StructRole) {
  struct_elem.kind(role);
}

/// If serializing a PDF 2.0 document, write a PDF 2.0 structure role, otherwise
/// fall back to the compatible PDF 1.7 role.
fn write_kind_compat(
  sc: &mut SerializeContext,
  struct_elem: &mut StructElement,
  role: StructRole2,
) {
  if sc.serialize_settings().pdf_version() < PdfVersion::Pdf20 {
    let compat = role.compatibility_1_7(RoleMapOpts::default());
    struct_elem.kind(compat.role());
  } else {
    struct_elem.kind_2(role, sc.pdf2_ns.ssn_ref);
  }
}

/// Write a custom role-mapped structure role. If serializing a PDF 2.0 document
/// also write the custom krilla namespace.
fn write_kind_custom(sc: &mut SerializeContext, struct_elem: &mut StructElement, name: Name) {
  struct_elem.custom_kind(name);
  if sc.serialize_settings().pdf_version() >= PdfVersion::Pdf20 {
    struct_elem.namespace(sc.pdf2_ns.krilla_ref);
  }
}

/// A node in a tag tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
  /// A group node.
  Group(TagGroup),
  /// A leaf node.
  Leaf(Identifier),
}

impl Node {
  pub(crate) fn serialize(
    &self,
    sc: &mut SerializeContext,
    parent_tree_map: &mut HashMap<IdentifierType, Ref>,
    id_tree: &mut BTreeMap<TagId, Ref>,
    parent: Ref,
    note_id: &mut u32,
    struct_elems: &mut Chunk,
  ) -> KrillaResult<Option<Reference>> {
    match self {
      Node::Group(g) => Ok(Some(g.serialize(
        sc,
        parent_tree_map,
        id_tree,
        parent,
        note_id,
        struct_elems,
      )?)),
      Node::Leaf(ci) => match ci.0 {
        IdentifierInner::Real(rci) => Ok(Some(Reference::ContentIdentifier(rci))),
        IdentifierInner::Dummy => Ok(None),
      },
    }
  }
}

impl From<TagGroup> for Node {
  fn from(value: TagGroup) -> Self {
    Node::Group(value)
  }
}

impl From<Identifier> for Node {
  fn from(value: Identifier) -> Self {
    Node::Leaf(value)
  }
}

#[derive(Clone, Copy)]
pub(crate) enum Reference {
  Ref(Ref),
  ContentIdentifier(IdentifierType),
}

/// A tag group.
#[derive(Debug, Clone, PartialEq)]
pub struct TagGroup {
  /// The tag of the tag group.
  pub tag: TagKind,
  /// The children of the tag group.
  pub children: Vec<Node>,
}

impl TagGroup {
  /// Create a new tag group with a specific tag.
  pub fn new(tag: impl Into<TagKind>) -> Self {
    Self {
      tag: tag.into(),
      children: vec![],
    }
  }

  /// Create a new tag group with a specific tag and a list of children.
  pub fn with_children(tag: impl Into<TagKind>, children: Vec<Node>) -> Self {
    Self {
      tag: tag.into(),
      children,
    }
  }

  /// Append a new child to the tag group.
  pub fn push(&mut self, child: impl Into<Node>) {
    self.children.push(child.into())
  }

  pub(crate) fn serialize(
    &self,
    sc: &mut SerializeContext,
    parent_tree_map: &mut HashMap<IdentifierType, Ref>,
    id_tree: &mut BTreeMap<TagId, Ref>,
    parent_ref: Ref,
    note_id: &mut u32,
    struct_elems: &mut Chunk,
  ) -> KrillaResult<Reference> {
    let elem_ref = sc.new_ref();
    let mut children_refs = vec![];

    for child in &self.children {
      let serialized = child.serialize(
        sc,
        parent_tree_map,
        id_tree,
        elem_ref,
        note_id,
        struct_elems,
      )?;
      if let Some(ref_) = serialized {
        children_refs.push(ref_);
      }
    }

    let mut struct_elem = struct_elems.struct_element(elem_ref);
    self.tag.write_kind(&mut struct_elem, sc);
    struct_elem.parent(parent_ref);

    let tag = self.tag.as_any();
    let pdf_version = sc.serialize_settings().pdf_version();

    if let Some(id) = tag.id() {
      match id_tree.entry(id.clone()) {
        Entry::Vacant(vacant) => {
          struct_elem.id(Str(id.as_bytes()));
          vacant.insert(elem_ref);
        }
        Entry::Occupied(_) => {
          return Err(KrillaError::DuplicateTagId(id.clone(), tag.location));
        }
      }
    } else if matches!(self.tag, TagKind::Note(_)) {
      // Explicitly don't use `TagId::from_bytes` to disambiguate note IDs
      // from user provided IDs.
      let mut id = TagId(SmallVec::new());
      _ = write!(&mut id.0, "Note {note_id}");
      struct_elem.id(Str(id.as_bytes()));
      id_tree.insert(id, elem_ref);

      *note_id += 1;
    }

    if self.tag.can_have_title() && tag.title().is_none_or(str::is_empty) {
      sc.register_validation_error(ValidationError::MissingHeadingTitle);
    }

    if self.tag.should_have_alt() && tag.alt_text().is_none_or(str::is_empty) {
      sc.register_validation_error(ValidationError::MissingAltText(tag.location));
    }

    // TODO: Once we have a generalized mechanism for validating tag trees,
    // validate TH-outside-THead structure for PDF < 1.5.

    for attr in tag.attrs.iter() {
      let Attr::Struct(attr) = attr else {
        continue;
      };
      match attr {
        StructAttr::Id(_) => (), // Handled above
        StructAttr::Title(title) => {
          struct_elem.title(TextStr(title));
        }
        StructAttr::Lang(lang) => {
          if pdf_version >= PdfVersion::Pdf14 {
            struct_elem.lang(TextStr(lang));
          }
        }
        StructAttr::AltText(alt) => {
          struct_elem.alt(TextStr(alt));
        }
        StructAttr::Expanded(expanded) => {
          if pdf_version >= PdfVersion::Pdf15 {
            struct_elem.expanded(TextStr(expanded));
          }
        }
        StructAttr::ActualText(actual_text) => {
          if pdf_version >= PdfVersion::Pdf14 {
            struct_elem.actual_text(TextStr(actual_text));
          }
        }

        // Not really an attribute
        StructAttr::HeadingLevel(_) => (),
      }
    }

    let mut attributes = LazyCell::new(|| struct_elem.attributes());

    // Lazily initialize the list attributes to avoid an empty array.
    let mut list_attributes = LazyCell::new(|| attributes.push().list());
    for attr in tag.attrs.iter() {
      let Attr::List(attr) = attr else {
        continue;
      };
      match attr {
        ListAttr::Numbering(numbering) => {
          list_attributes.list_numbering(numbering.to_pdf());
        }
      }
    }
    list_attributes.finish();

    // Lazily initialize the table attributes to avoid an empty array.
    let mut table_attributes = LazyCell::new(|| attributes.push().table());
    for attr in tag.attrs.iter() {
      let Attr::Table(attr) = attr else {
        continue;
      };
      match attr {
        TableAttr::Summary(summary) => {
          if pdf_version >= PdfVersion::Pdf17 {
            table_attributes.summary(TextStr(summary));
          }
        }
        TableAttr::HeaderScope(scope) => {
          if pdf_version >= VersionedFeature::TableHeaderScope.minimum_pdf_version() {
            table_attributes.scope(scope.to_pdf());
          } else if scope == &TableHeaderScope::Row {
            // Without `Scope`, the correct cell to point to is ambiguous.
            sc.register_validation_error(ValidationError::RequiresNewerPdfVersion(
              VersionedFeature::TableHeaderScope,
              tag.location,
            ));
          }
        }
        TableAttr::CellHeaders(headers) => {
          if pdf_version >= PdfVersion::Pdf15 {
            let id_strs = headers.iter().map(|id| Str(id.as_bytes()));
            table_attributes.headers().items(id_strs);
          }
        }
        TableAttr::RowSpan(n) => {
          table_attributes.row_span(n.get() as i32);
        }
        TableAttr::ColSpan(n) => {
          table_attributes.col_span(n.get() as i32);
        }
      }
    }
    table_attributes.finish();

    // Lazily initialize the list attributes to avoid an empty array.
    let mut layout_attributes = LazyCell::new(|| attributes.push().layout());
    for attr in tag.attrs.iter() {
      let Attr::Layout(attr) = attr else {
        continue;
      };
      match attr {
        LayoutAttr::Placement(placement) => {
          layout_attributes.placement(placement.to_pdf());
        }
        LayoutAttr::WritingMode(writing_mode) => {
          layout_attributes.writing_mode(writing_mode.to_pdf());
        }
        &LayoutAttr::BBox(BBox { page_idx, rect }) => {
          let Some(page_info) = sc.page_infos().get(page_idx) else {
            panic!(
              "tag tree contains bounding box with page index {page_idx}, \
                            but document only has {} pages",
              sc.page_infos().len()
            );
          };
          let transform = page_root_transform(page_info.size().height());
          let actual_rect = rect.transform(transform).unwrap();
          layout_attributes.bbox(actual_rect.to_pdf_rect());
        }
        &LayoutAttr::Width(width) => {
          layout_attributes.width(width);
        }
        &LayoutAttr::Height(height) => {
          layout_attributes.height(height);
        }
        &LayoutAttr::BackgroundColor(color) => {
          if pdf_version >= PdfVersion::Pdf15 {
            layout_attributes.background_color(color.into());
          }
        }
        LayoutAttr::BorderColor(sides) => {
          if pdf_version >= PdfVersion::Pdf15 {
            let sides = sides.map_pdf(NaiveRgbColor::into_f32_array);
            layout_attributes.border_color(sides);
          }
        }
        LayoutAttr::BorderStyle(sides) => {
          if pdf_version >= PdfVersion::Pdf15 {
            let sides = sides.map_pdf(BorderStyle::to_pdf);
            layout_attributes.border_style(sides);
          }
        }
        LayoutAttr::BorderThickness(sides) => {
          if pdf_version >= PdfVersion::Pdf15 {
            layout_attributes.border_thickness(sides.into_pdf());
          }
        }
        LayoutAttr::Padding(sides) => {
          if pdf_version >= PdfVersion::Pdf15 {
            layout_attributes.padding(sides.into_pdf());
          }
        }
        &LayoutAttr::Color(color) => {
          if pdf_version >= PdfVersion::Pdf15 {
            layout_attributes.color(color.into());
          }
        }
        &LayoutAttr::SpaceBefore(margin) => {
          layout_attributes.space_before(margin);
        }
        &LayoutAttr::SpaceAfter(margin) => {
          layout_attributes.space_after(margin);
        }
        &LayoutAttr::StartIndent(margin) => {
          layout_attributes.start_indent(margin);
        }
        &LayoutAttr::EndIndent(margin) => {
          layout_attributes.end_indent(margin);
        }
        &LayoutAttr::TextIndent(indent) => {
          layout_attributes.text_indent(indent);
        }
        LayoutAttr::BlockAlign(alignment) => {
          layout_attributes.block_align(alignment.to_pdf());
        }
        LayoutAttr::InlineAlign(alignment) => {
          layout_attributes.inline_align(alignment.to_pdf());
        }
        LayoutAttr::TextAlign(alignment) => {
          layout_attributes.text_align(alignment.to_pdf());
        }
        LayoutAttr::TableBorderStyle(sides) => {
          if pdf_version >= PdfVersion::Pdf15 {
            let sides = sides.map_pdf(BorderStyle::to_pdf);
            layout_attributes.table_border_style(sides);
          }
        }
        LayoutAttr::TablePadding(sides) => {
          if pdf_version >= PdfVersion::Pdf15 {
            layout_attributes.table_padding(sides.into_pdf());
          }
        }
        &LayoutAttr::BaselineShift(shift) => {
          layout_attributes.baseline_shift(shift);
        }
        LayoutAttr::LineHeight(height) => {
          layout_attributes.line_height(height.to_pdf());
        }
        &LayoutAttr::TextDecorationColor(color) => {
          if pdf_version >= PdfVersion::Pdf15 {
            layout_attributes.text_decoration_color(color.into());
          }
        }
        &LayoutAttr::TextDecorationThickness(thickness) => {
          if pdf_version >= PdfVersion::Pdf15 {
            layout_attributes.text_decoration_thickness(thickness);
          }
        }
        LayoutAttr::TextDecorationType(style) => {
          if pdf_version >= PdfVersion::Pdf15 {
            layout_attributes.text_decoration_type(style.to_pdf());
          }
        }
        &LayoutAttr::GlyphOrientationVertical(orientation) => {
          if pdf_version >= PdfVersion::Pdf15 {
            layout_attributes.glyph_orientation_vertical(orientation.to_pdf());
          }
        }
        LayoutAttr::ColumnCount(columns) => {
          if pdf_version >= PdfVersion::Pdf16 {
            layout_attributes.column_count(columns.get() as i32);
          }
        }
        LayoutAttr::ColumnGap(gap) => {
          if pdf_version >= PdfVersion::Pdf16 {
            let sizes = layout_attributes.column_gap();
            match gap {
              ColumnDimensions::All(gap) => sizes.uniform(*gap),
              ColumnDimensions::Specific(values) => {
                sizes.individual().items(values.iter().copied());
              }
            }
          }
        }
        LayoutAttr::ColumnWidths(width) => {
          if pdf_version >= PdfVersion::Pdf16 {
            let sizes = layout_attributes.column_widths();
            match width {
              ColumnDimensions::All(width) => sizes.uniform(*width),
              ColumnDimensions::Specific(values) => {
                sizes.individual().items(values.iter().copied());
              }
            }
          }
        }
      }
    }
    layout_attributes.finish();

    attributes.finish();

    serialize_children(
      sc,
      elem_ref,
      children_refs,
      parent_tree_map,
      &mut struct_elem,
    )?;
    struct_elem.finish();

    Ok(Reference::Ref(elem_ref))
  }

  fn validate(&self, id_tree: &BTreeMap<TagId, Ref>) -> KrillaResult<()> {
    if let Some(headers) = self.tag.headers() {
      for id in headers.iter() {
        if !id_tree.contains_key(id) {
          return Err(KrillaError::UnknownTagId(id.clone(), self.tag.location()));
        }
      }
    }

    for child in self.children.iter() {
      if let Node::Group(group) = child {
        group.validate(id_tree)?;
      }
    }
    Ok(())
  }
}

/// A tag tree.
#[derive(Default)]
pub struct TagTree {
  /// The children of the tag tree.
  pub children: Vec<Node>,
  /// Language attribute for the auto-generated Document root structure element.
  pub lang: Option<String>,
}

impl From<Vec<Node>> for TagTree {
  fn from(children: Vec<Node>) -> Self {
    Self {
      children,
      lang: None,
    }
  }
}

impl TagTree {
  /// Create a new tag tree.
  pub fn new() -> Self {
    Self {
      children: vec![],
      lang: None,
    }
  }

  /// Set the language attribute on the Document root structure element.
  ///
  /// This sets `/Lang` on the auto-generated Document structure element,
  /// which is distinct from the catalog-level `/Lang` set via
  /// [`Metadata::language`](crate::krilla::metadata::Metadata::language).
  pub fn with_lang(mut self, lang: Option<String>) -> Self {
    self.lang = lang;
    self
  }

  /// Append a new child to the tag tree.
  pub fn push(&mut self, child: impl Into<Node>) {
    self.children.push(child.into())
  }

  pub(crate) fn serialize(
    &self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    parent_tree_map: &mut HashMap<IdentifierType, Ref>,
    id_tree_map: &mut BTreeMap<TagId, Ref>,
    struct_tree_ref: Ref,
  ) -> KrillaResult<Ref> {
    let root_ref = sc.new_ref();
    let mut struct_elems = sc.new_chunk();

    // Keeps track of the ID of notes in the IDTree. We currently only write IDs for notes,
    // which is why we use this simple variable, but this should be refactored if we write
    // the IDs for multiple types of struct elements in the future.
    let mut note_id = 1;

    let mut children_refs = vec![];

    for child in &self.children {
      let serialized = child.serialize(
        sc,
        parent_tree_map,
        id_tree_map,
        root_ref,
        &mut note_id,
        &mut struct_elems,
      )?;

      if let Some(ref_) = serialized {
        children_refs.push(ref_);
      }
    }

    let mut struct_elem = struct_elems.indirect(root_ref).start::<StructElement>();
    write_kind_compat(sc, &mut struct_elem, StructRole2::Document);
    struct_elem.parent(struct_tree_ref);
    if let Some(lang) = &self.lang {
      if sc.serialize_settings().pdf_version() >= PdfVersion::Pdf14 {
        struct_elem.lang(TextStr(lang));
      }
    }
    serialize_children(
      sc,
      root_ref,
      children_refs,
      parent_tree_map,
      &mut struct_elem,
    )?;

    struct_elem.finish();
    chunk_container.non_stream.struct_elements = Some(struct_elems);

    Ok(root_ref)
  }

  pub(crate) fn validate(&self, id_tree: &BTreeMap<TagId, Ref>) -> KrillaResult<()> {
    for child in self.children.iter() {
      if let Node::Group(group) = child {
        group.validate(id_tree)?;
      }
    }
    Ok(())
  }
}

fn serialize_children(
  sc: &mut SerializeContext,
  parent_ref: Ref,
  children_refs: Vec<Reference>,
  parent_tree_map: &mut HashMap<IdentifierType, Ref>,
  struct_elem: &mut StructElement,
) -> KrillaResult<()> {
  // We can define a /Pg element on the struct element. If a marked content reference
  // is part of the same page as that entry, we can just write the mcid, otherwise, we
  // need to write a full marked content reference.
  // In our case, we just use the first marked content reference we can find as the
  // entry in the /Pg dict.
  let mut struct_page_ref = None;
  let mut struct_children = struct_elem.children();

  for child in children_refs {
    match child {
      Reference::Ref(r) => {
        struct_children.struct_element(r);
      }
      Reference::ContentIdentifier(it) => match it {
        IdentifierType::PageIdentifier(pi) => {
          let page_ref = sc
            .page_infos()
            .get(pi.page_index)
            .ok_or_else(|| {
              KrillaError::TagTree(format!(
                "tag tree contains identifier from page {}, but document only has {} pages",
                pi.page_index + 1,
                sc.page_infos().len()
              ))
            })?
            .ref_();

          if struct_page_ref.is_none() {
            struct_page_ref = Some(page_ref);
          }

          if parent_tree_map.contains_key(&pi.into()) {
            return Err(KrillaError::TagTree(format!(
              "the identifier {pi:?} appears twice in the tag tree"
            )));
          }

          parent_tree_map.insert(pi.into(), parent_ref);

          if struct_page_ref == Some(page_ref) {
            struct_children.marked_content_id(pi.mcid);
          } else {
            struct_children
              .marked_content_ref()
              .marked_content_id(pi.mcid)
              .page(page_ref);
          }
        }
        IdentifierType::AnnotationIdentifier(ai) => {
          let page_count = sc.page_infos().len();
          let Some(page_info) = sc.page_infos_mut().get_mut(ai.page_index) else {
            return Err(KrillaError::TagTree(format!(
              "tag tree contains identifier from page {}, but document only has {page_count} pages",
              ai.page_index + 1,
            )));
          };

          let page_ref = page_info.ref_();
          let annotation_count = page_info.annotations().len();
          let Some((annotation_ref, struct_parent)) =
            page_info.annotations_mut().get_mut(ai.annot_index)
          else {
            return Err(KrillaError::TagTree(format!(
              "tag tree contains identifier from annotation {} on page {}, but page only has {annotation_count} annotations",
              ai.annot_index + 1,
              ai.page_index + 1,
            )));
          };

          if parent_tree_map.contains_key(&ai.into()) {
            return Err(KrillaError::TagTree(format!(
              "identifier {ai:?} appears twice in the tag tree"
            )));
          }
          parent_tree_map.insert(ai.into(), *annotation_ref);

          struct_parent.set(parent_ref).expect("only one parent");

          struct_children
            .object_ref()
            .page(page_ref)
            .object(*annotation_ref);
        }
      },
    }
  }
  struct_children.finish();

  if let Some(spr) = struct_page_ref {
    struct_elem.page(spr);
  }

  Ok(())
}

/// Where a layout artifact is attached to the page edge.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[allow(missing_docs)]
pub enum ArtifactAttachment {
  Left,
  Top,
  Right,
  Bottom,
}
