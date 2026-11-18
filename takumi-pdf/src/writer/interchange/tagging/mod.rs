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
//! interpreted by different consumers. The writer supports the creation of such documents.
//!
//! # A word on the writer's implementation
//!
//! As nearly everything in PDF, tagging is a really complex topic, and getting it right
//! is very hard. Because of this, in line with the general philosophy of the writer, some
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
//!      annotations to the content they correspond to. Currently, the writer only supports link
//!      annotations, and a link annotation should always be a child in a tag group with the
//!      [`TagKind`] [`Link`](TagKind::Link), with its sibling being an identifier or another tag group that is
//!      to be associated with the link.
//!    - Use the `start_tagged` command on [`Surface`], which returns an [Identifier], and
//!      indicates that all content drawn on the surface should be associated with that
//!      identifier, until you call the `end_tagged` method. *Important*: Note that you cannot
//!      nest calls to `start_tagged`, and you have to ensure that you always call a corresponding
//!      `end_tagged`. Otherwise, the writer will panic.
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
//! [`SerializeSettings`]: crate::writer::SerializeSettings
//! [`Page`]: crate::writer::page::Page
//! [`Surface`]: crate::writer::surface::Surface
//! [`Document`]: crate::writer::Document

use std::cell::LazyCell;
use std::cmp::PartialEq;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap};

use pdf_writer::types::{RoleMapOpts, StructRole, StructRole2};
use pdf_writer::writers::{PropertyList, StructElement};
use pdf_writer::{Chunk, Finish, Name, Ref, Str, TextStr};

use crate::writer::chunk_container::ChunkContainer;
use crate::writer::configure::validate::VersionedFeature;
use crate::writer::configure::{PdfVersion, ValidationError};
use crate::writer::error::{WriterError, WriterResult};
use crate::writer::serialize::SerializeContext;

pub use tag::*;

mod tag;

/// A content tag associated with the content it wraps.
#[derive(Clone, Copy, Debug)]
pub enum ContentTag<'a> {
  /// Artifacts represent pieces of content that are not really part of the logical structure
  /// of a document and should be excluded in the logical tree. These include for example headers,
  /// footers, page background and similar.
  Artifact,
  /// A content tag that wraps some text with specific properties.
  ///
  /// Spans should not be too long. At most, they should contain a single line of text, but they
  /// can obviously be shorter, if text within a single line contains text with different styles
  /// or different languages.
  Span(&'a str),
  /// Use this tag for anything else that does not semantically fit into `Span` or `Artifact`.
  /// This includes for example arbitrary paths, images or a mix of different content that cannot
  /// be split up more.
  Other,
}

impl ContentTag<'_> {
  pub(crate) fn name(&self) -> Name<'static> {
    match self {
      ContentTag::Artifact => Name(b"Artifact"),
      ContentTag::Span(_) => Name(b"Span"),
      ContentTag::Other => Name(b"P"),
    }
  }

  pub(crate) fn write_properties(&self, mut properties: PropertyList) {
    match self {
      ContentTag::Span(lang) => {
        properties.pair(Name(b"Lang"), TextStr(lang));
      }
      ContentTag::Artifact | ContentTag::Other => {}
    }
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

    self.mcid = self.mcid.saturating_add(1);

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

impl Tag {
  pub(crate) fn write_kind(&self, struct_elem: &mut StructElement, sc: &mut SerializeContext) {
    let pdf_version = sc.serialize_settings().pdf_version();
    if pdf_version < self.minimum_version() {
      // Fall back to P in case the tag is not supported with the current
      // PDF version
      struct_elem.kind(StructRole::P);
      return;
    }

    match &self.kind {
      TagKind::Article => write_kind_1_7(struct_elem, StructRole::Art),
      TagKind::Section => write_kind_compat(sc, struct_elem, StructRole2::Sect),
      TagKind::BlockQuote => write_kind_1_7(struct_elem, StructRole::BlockQuote),
      TagKind::Caption => write_kind_compat(sc, struct_elem, StructRole2::Caption),
      TagKind::P => write_kind_compat(sc, struct_elem, StructRole2::P),
      TagKind::L => write_kind_compat(sc, struct_elem, StructRole2::L),
      TagKind::LI => write_kind_compat(sc, struct_elem, StructRole2::LI),
      TagKind::Lbl => write_kind_compat(sc, struct_elem, StructRole2::Lbl),
      TagKind::LBody => write_kind_compat(sc, struct_elem, StructRole2::LBody),
      TagKind::Table => write_kind_compat(sc, struct_elem, StructRole2::Table),
      TagKind::TR => write_kind_compat(sc, struct_elem, StructRole2::TR),
      TagKind::TH => write_kind_compat(sc, struct_elem, StructRole2::TH),
      TagKind::TD => write_kind_compat(sc, struct_elem, StructRole2::TD),
      TagKind::THead => write_kind_compat(sc, struct_elem, StructRole2::THead),
      TagKind::TBody => write_kind_compat(sc, struct_elem, StructRole2::TBody),
      TagKind::TFoot => write_kind_compat(sc, struct_elem, StructRole2::TFoot),
      TagKind::Code => write_kind_1_7(struct_elem, StructRole::Code),
      TagKind::Link => write_kind_compat(sc, struct_elem, StructRole2::Link),
      TagKind::Figure => write_kind_compat(sc, struct_elem, StructRole2::Figure),
      // PDF 2.0 structure roles that are conditionally registered.
      TagKind::Hn { level } => {
        let role2 = StructRole2::Heading(*level);
        if pdf_version < PdfVersion::Pdf20 {
          // Dynamically register custom headings `Hn` if the level
          // (`n >= 7`) isn't supported by PDF 1.7 and below.
          let compat = role2.compatibility_1_7(RoleMapOpts::default());
          if compat.into_pdf_1_7().is_none() {
            sc.global_objects.custom_heading_roles.insert(*level);
          }
          struct_elem.custom_kind(role2.to_name(&mut [0; 6]));
        } else {
          struct_elem.kind_2(role2, sc.ssn_ref);
        }
      }
      TagKind::Strong => {
        if pdf_version < PdfVersion::Pdf20 {
          struct_elem.custom_kind(Name(b"Strong"));
        } else {
          struct_elem.kind_2(StructRole2::Strong, sc.ssn_ref);
        }
      }
      TagKind::Em => {
        if pdf_version < PdfVersion::Pdf20 {
          struct_elem.custom_kind(Name(b"Em"));
        } else {
          struct_elem.kind_2(StructRole2::Em, sc.ssn_ref);
        }
      }
    };
  }

  pub(crate) fn minimum_version(&self) -> PdfVersion {
    match self.kind {
      TagKind::THead | TagKind::TBody | TagKind::TFoot => PdfVersion::Pdf15,
      // TODO: writing `P` tags in PDF 1.4 will break the table structure.
      // Instead consider just transparently inserting all children, which
      // should be `TR`s anyway.
      _ => PdfVersion::Pdf14,
    }
  }

  pub(crate) fn should_have_alt(&self) -> bool {
    matches!(self.kind, TagKind::Figure)
  }

  pub(crate) fn can_have_title(&self) -> bool {
    matches!(self.kind, TagKind::Hn { .. })
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
    struct_elem.kind_2(role, sc.ssn_ref);
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
    struct_elems: &mut Chunk,
  ) -> WriterResult<Option<Reference>> {
    match self {
      Node::Group(g) => Ok(Some(g.serialize(
        sc,
        parent_tree_map,
        id_tree,
        parent,
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
  pub tag: Tag,
  /// The children of the tag group.
  pub children: Vec<Node>,
}

impl TagGroup {
  /// Create a new tag group with a specific tag.
  pub fn new(tag: Tag) -> Self {
    Self {
      tag,
      children: vec![],
    }
  }

  /// Create a new tag group with a specific tag and a list of children.
  pub fn with_children(tag: Tag, children: Vec<Node>) -> Self {
    Self { tag, children }
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
    struct_elems: &mut Chunk,
  ) -> WriterResult<Reference> {
    let elem_ref = sc.new_ref();
    let mut children_refs = vec![];

    for child in &self.children {
      let serialized = child.serialize(sc, parent_tree_map, id_tree, elem_ref, struct_elems)?;
      if let Some(ref_) = serialized {
        children_refs.push(ref_);
      }
    }

    let mut struct_elem = struct_elems.struct_element(elem_ref);
    self.tag.write_kind(&mut struct_elem, sc);
    struct_elem.parent(parent_ref);

    let pdf_version = sc.serialize_settings().pdf_version();

    if let Some(id) = self.tag.id() {
      match id_tree.entry(id.clone()) {
        Entry::Vacant(vacant) => {
          struct_elem.id(Str(id.as_bytes()));
          vacant.insert(elem_ref);
        }
        Entry::Occupied(_) => {
          return Err(WriterError::DuplicateTagId(id.clone(), self.tag.location));
        }
      }
    }

    if self.tag.can_have_title() && self.tag.title().is_none_or(str::is_empty) {
      sc.register_validation_error(ValidationError::MissingHeadingTitle);
    }

    if self.tag.should_have_alt() && self.tag.alt_text().is_none_or(str::is_empty) {
      sc.register_validation_error(ValidationError::MissingAltText(self.tag.location));
    }

    // TODO: Once we have a generalized mechanism for validating tag trees,
    // validate TH-outside-THead structure for PDF < 1.5.

    for attr in self.tag.attrs.iter() {
      let Attr::Struct(attr) = attr else {
        continue;
      };
      match attr {
        StructAttr::Id(_) => (), // Handled above
        StructAttr::Title(title) => {
          struct_elem.title(TextStr(title));
        }
        StructAttr::AltText(alt) => {
          struct_elem.alt(TextStr(alt));
        }

        // Not really an attribute
        StructAttr::HeadingLevel(_) => (),
      }
    }

    let mut attributes = LazyCell::new(|| struct_elem.attributes());

    // Lazily initialize the list attributes to avoid an empty array.
    let mut list_attributes = LazyCell::new(|| attributes.push().list());
    for attr in self.tag.attrs.iter() {
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
    for attr in self.tag.attrs.iter() {
      let Attr::Table(attr) = attr else {
        continue;
      };
      match attr {
        TableAttr::HeaderScope(scope) => {
          if pdf_version >= VersionedFeature::TableHeaderScope.minimum_pdf_version() {
            table_attributes.scope(scope.to_pdf());
          } else if scope == &TableHeaderScope::Row {
            // Without `Scope`, the correct cell to point to is ambiguous.
            sc.register_validation_error(ValidationError::RequiresNewerPdfVersion(
              VersionedFeature::TableHeaderScope,
              self.tag.location,
            ));
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
  /// [`Metadata::language`](crate::writer::metadata::Metadata::language).
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
  ) -> WriterResult<Ref> {
    let root_ref = sc.new_ref();
    let mut struct_elems = sc.new_chunk();

    let mut children_refs = vec![];

    for child in &self.children {
      let serialized = child.serialize(
        sc,
        parent_tree_map,
        id_tree_map,
        root_ref,
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
}

fn serialize_children(
  sc: &mut SerializeContext,
  parent_ref: Ref,
  children_refs: Vec<Reference>,
  parent_tree_map: &mut HashMap<IdentifierType, Ref>,
  struct_elem: &mut StructElement,
) -> WriterResult<()> {
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
              WriterError::TagTree(format!(
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
            return Err(WriterError::TagTree(format!(
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
            return Err(WriterError::TagTree(format!(
              "tag tree contains identifier from page {}, but document only has {page_count} pages",
              ai.page_index + 1,
            )));
          };

          let page_ref = page_info.ref_();
          let annotation_count = page_info.annotations().len();
          let Some((annotation_ref, struct_parent)) =
            page_info.annotations_mut().get_mut(ai.annot_index)
          else {
            return Err(WriterError::TagTree(format!(
              "tag tree contains identifier from annotation {} on page {}, but page only has {annotation_count} annotations",
              ai.annot_index + 1,
              ai.page_index + 1,
            )));
          };

          if parent_tree_map.contains_key(&ai.into()) {
            return Err(WriterError::TagTree(format!(
              "identifier {ai:?} appears twice in the tag tree"
            )));
          }
          parent_tree_map.insert(ai.into(), *annotation_ref);

          // The duplicate check above keeps each annotation to one parent.
          let _ = struct_parent.set(parent_ref);

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
