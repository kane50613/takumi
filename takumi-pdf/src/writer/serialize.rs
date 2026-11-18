use std::cell::{OnceCell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::num::NonZeroU16;
use std::rc::Rc;
use std::sync::Arc;

use indexmap::IndexMap;
use pdf_writer::types::{StructRole, StructRole2};
use pdf_writer::writers::{OutputIntent, StructTreeRoot};
use pdf_writer::{Chunk, Content, Finish, Limits, Name, Obj, Pdf, Ref, Settings, Str, TextStr};
use rustc_hash::FxHashMap;

use crate::writer::chunk_container::ChunkContainer;
use crate::writer::color::{CieBasedColorSpace, DeviceColorSpace};
use crate::writer::configure::{Configuration, PdfVersion, ValidationError, Validators};
use crate::writer::error::{LimitError, WriterError, WriterResult};
use crate::writer::geom::Size;
use crate::writer::graphics::color::{ColorSpace, DEVICE_CMYK, DEVICE_GRAY, DEVICE_RGB, rgb};
use crate::writer::graphics::icc::ICCBasedColorSpace;
#[cfg(feature = "images")]
use crate::writer::graphics::image::Image;
use crate::writer::interactive::destination::XyzDestination;
use crate::writer::interchange::embed::EmbeddedFile;
use crate::writer::interchange::outline::Outline;
use crate::writer::interchange::tagging::{
  AnnotationIdentifier, PageTagIdentifier, TagId, TagTree,
};
use crate::writer::object_stream::ObjectStreams;
use crate::writer::page::InternalPage;
use crate::writer::resource;
use crate::writer::resource::{Resource, Resourceable};
use crate::writer::surface::{Location, Surface};
use crate::writer::text::GlyphId;
use crate::writer::text::{Font, FontContainer, FontIdentifier};
use crate::writer::util::{NameExt, SipHashable};

const STR_LEN: usize = 32767;
const NAME_LEN: usize = 127;

// These only apply to PDF 1.4 and PDF/A-1.
const MAX_FLOAT: f32 = 32767.0;
const DICT_LEN: usize = 4095;
const ARRAY_LEN: usize = 8191;

/// Settings that should be applied when creating a PDF document.
#[derive(Clone, Debug)]
pub struct SerializeSettings {
  /// Whether to write PDFs in a way that is easier to inspect manually. This
  /// will result in larger file sizes.
  pub pretty: bool,
  /// Whether content streams should be compressed. Leads to significantly smaller file sizes,
  /// but also longer running times. It is highly recommended that you set this to `true`.
  pub compress_content_streams: bool,
  /// Whether device-independent colors should be used instead of
  /// device-dependent ones.
  ///
  /// Note that this value might be overridden depending on which validator
  /// you use. For example, when exporting to PDF/A, this value will be set to
  /// true, regardless of what value will be passed.
  pub no_device_cs: bool,
  /// Whether the PDF should be ASCII-compatible, i.e. only consist of
  /// characters in the ASCII range.
  ///
  /// Note that this only on a best-effort basis. For example, XMP metadata always
  /// contains a binary marker. In addition to that, some validators,
  /// like PDF/A, require that the file header be a binary marker, meaning
  /// that the header itself will not be ASCII-compatible. Finally, embedded PDFs will
  /// be embedded as is and not re-encoded with ASCII-compatible encoding.
  pub ascii_compatible: bool,
  /// Whether the PDF should include XMP metadata.
  ///
  /// Note that this value might be overridden depending on which validator
  /// you use. For example, when exporting to PDF/A, this value will be set to
  /// true, regardless of what value will be passed.
  pub xmp_metadata: bool,
  /// A validator and PDF version used for export.
  ///
  /// In case validation fails, export will fail, and a list of validation errors that
  /// occurred will be returned instead of the PDF.
  ///
  /// **Important**: Make sure to carefully read the documentation of the [`validate`] module
  /// before using this feature! Just setting a validator might not be enough to ensure that
  /// your output conforms to the given standard, as some requirements are semantic in nature
  /// and cannot possibly be verified by the writer!
  ///
  /// However, as long as you carefully read and follow the documentation,
  /// you can be certain that the resulting document will conform to the standard (unless there
  /// is a bug).
  ///
  /// [`validate`]: crate::writer::configure::validate
  pub configuration: Configuration,
  /// Whether to enable the creation of tagged documents. See the module documentation
  /// of [`tagging`] for more information about tagged PDF documents.
  ///
  /// Note that enabling this does not automatically make your documents tagged, as tagging implies
  /// enriching the document with semantic information, which the writer cannot do
  /// for you, since it's content-agnostic. All this setting does is to allow you
  /// to dynamically disable tagging if you wish to do so. This allows you to write
  /// your code primarily with tagging in mind, but still allows you to
  /// disable it dynamically, without having to make any changes to your code.
  ///
  /// Note that this value might be overridden depending on which validator
  /// you use. For example, when exporting with PDF/UA, this value will always
  /// be set to `true`.
  ///
  /// [`tagging`]: crate::writer::interchange::tagging
  pub enable_tagging: bool,
  /// Written as `/Producer` and `pdf:Producer`, whether or not the document
  /// carries metadata.
  pub producer: String,
  /// A function that should be used to render SVG glyphs. If you don't need this, yu can
  /// just use the default function which doesn't render them at all. If you do want this, it
  /// is recommended that you use the function provided by the `krilla-svg` crate.
  pub render_svg_glyph_fn: RenderSvgGlyphFn,
}

pub type RenderSvgGlyphFn = fn(&[u8], rgb::Color, GlyphId, (f32, f32), &mut Surface) -> Option<()>;

impl SerializeSettings {
  pub(crate) fn pdf_version(&self) -> PdfVersion {
    self.configuration.version()
  }

  pub(crate) fn validators(&self) -> Validators {
    self.configuration.validators()
  }

  /// Whether the `/AF` key is supported, accounting for the PDF version and active standards.
  pub(crate) fn supports_associated_files(&self) -> bool {
    self.configuration.version().specifies_associated_files()
      || self.configuration.validators().specifies_associated_files()
  }
}

impl Default for SerializeSettings {
  fn default() -> Self {
    Self {
      pretty: false,
      ascii_compatible: false,
      compress_content_streams: true,
      no_device_cs: false,
      xmp_metadata: true,
      configuration: Configuration::default(),
      enable_tagging: true,
      producer: crate::PRODUCER.to_string(),
      render_svg_glyph_fn: |_, _, _, _, _| None,
    }
  }
}

pub(crate) struct PageInfo {
  ref_: Ref,
  surface_size: Size,
  annotations: Vec<(Ref, OnceCell<Ref>)>,
}

impl PageInfo {
  pub(crate) fn ref_(&self) -> Ref {
    self.ref_
  }

  pub(crate) fn size(&self) -> Size {
    self.surface_size
  }

  pub(crate) fn annotations(&self) -> &[(Ref, OnceCell<Ref>)] {
    &self.annotations
  }

  pub(crate) fn annotations_mut(&mut self) -> &mut [(Ref, OnceCell<Ref>)] {
    &mut self.annotations
  }

  pub(crate) fn set_annotations(&mut self, annotations: Vec<(Ref, OnceCell<Ref>)>) {
    self.annotations = annotations;
  }
}

enum StructParentElement {
  /// The index of the page and the number of marked content IDs present on that page.
  Page(usize, i32),
  /// The index of the page where the annotation is present, as well as the index of the
  /// annotation within that one page.
  Annotation(AnnotationIdentifier),
}

#[derive(Debug)]
pub(crate) enum MaybeDeviceColorSpace {
  DeviceRgb,
  DeviceGray,
  DeviceCMYK,
  ColorSpace(resource::ColorSpace),
}

impl MaybeDeviceColorSpace {
  pub(crate) fn write(self, target: Obj<'_>) {
    match self {
      Self::DeviceRgb => target.primitive(DEVICE_RGB.to_pdf_name()),
      Self::DeviceGray => target.primitive(DEVICE_GRAY.to_pdf_name()),
      Self::DeviceCMYK => target.primitive(DEVICE_CMYK.to_pdf_name()),
      Self::ColorSpace(cs) => target.primitive(cs.get_ref()),
    }
  }
}

/// The serializer context is more or less the core piece of the writer. It is passed around
/// throughout pretty much the whole conversion process, and contains all mutable state
/// that is needed when writing a PDF file. This includes for example:
/// - Storing all chunks that are produced.
/// - The mappings from OTF fonts to CID/Type 3 fonts.
/// - Annotations used in the document.
///   etc.
pub(crate) struct SerializeContext {
  /// The ref of the page tree.
  page_tree_ref: Ref,
  /// The PDF 2.0 standard structure namespace (`http://iso.org/pdf2/ssn`).
  pub(crate) ssn_ref: Ref,
  /// All global objects, such as PDF fonts, that are populated over time.
  pub(crate) global_objects: GlobalObjects,
  /// Information for each page written so far, index by the page index.
  page_infos: Vec<PageInfo>,
  /// Keep track of object hashes and their corresponding reference. This is used for
  /// caching, so that for example same images will not be embedded twice in the document.
  cached_mappings: FxHashMap<u128, Ref>,
  /// The current ref in use. All serializers should use the `new_ref` method (which indirectly
  /// is based on this field) to generate a new Ref, instead of creating one manually with
  /// `Ref::new`.
  pub(crate) cur_ref: Ref,
  /// The ref of every structure element that carries a [`TagId`], filled in
  /// once the tag tree is serialized. Structure destinations resolve through
  /// it, so they can only be written afterwards.
  tag_refs: BTreeMap<TagId, Ref>,
  /// All validation errors that are collected as part of the export process
  /// alongside the validators that raised the error.
  validation_errors: Vec<(ValidationError, Validators)>,
  /// Settings used for serialization.
  serialize_settings: Arc<SerializeSettings>,
  /// Settings used for all PDF object chunks.
  chunk_settings: Settings,
  /// The limits created as part of the serialization process. In principle, we could
  /// just keep track of this in `ChunkContainer`, where all used chunks are stored.
  /// The only reason why `SerializeContext` needs to know about them is that we also
  /// need to merge limits from postscript functions, which are not directly accessible
  /// from the chunk they are written to.
  limits: Limits,
  /// Additional information stored during serialization that allows us to
  /// raise standards errors later.
  /// The current location, if set.
  pub(crate) location: Option<Location>,
}

impl SerializeContext {
  pub(crate) fn new(mut serialize_settings: SerializeSettings) -> Self {
    // Override flags as required by the validator
    serialize_settings.no_device_cs |= serialize_settings.validators().requires_no_device_cs();
    serialize_settings.enable_tagging |= serialize_settings.validators().requires_tagging();
    serialize_settings.xmp_metadata |= serialize_settings.validators().requires_xmp_metadata();

    let mut cur_ref = Ref::new(1);
    let page_tree_ref = cur_ref.bump();
    let ssn_ref = cur_ref.bump();

    let chunk_settings = Settings {
      pretty: serialize_settings.pretty,
    };

    Self {
      cached_mappings: FxHashMap::default(),
      ssn_ref,
      global_objects: GlobalObjects::default(),
      cur_ref,
      page_tree_ref,
      page_infos: vec![],
      location: None,
      tag_refs: BTreeMap::new(),
      validation_errors: vec![],
      serialize_settings: Arc::new(serialize_settings),
      chunk_settings,
      limits: Limits::new(),
    }
  }

  pub(crate) fn page_infos(&self) -> &[PageInfo] {
    &self.page_infos
  }

  /// The structure element `id` names, once the tag tree has been serialized.
  pub(crate) fn tag_ref(&self, id: &TagId) -> Option<Ref> {
    self.tag_refs.get(id).copied()
  }

  pub(crate) fn page_infos_mut(&mut self) -> &mut [PageInfo] {
    &mut self.page_infos
  }

  pub(crate) fn set_outline(&mut self, outline: Outline) {
    // Only set it if it's not empty or if the current validator requires an
    // outline.
    if !outline.is_empty()
      || self
        .serialize_settings
        .validators()
        .prohibits(&ValidationError::MissingDocumentOutline)
        .is_some()
    {
      self.global_objects.outline = Some(outline);
    }
  }

  pub(crate) fn embed_file(
    &mut self,
    chunk_container: &mut ChunkContainer,
    file: EmbeddedFile,
  ) -> Option<()> {
    let name = file.path.clone();
    let ref_ = self.register_cacheable(chunk_container, file);
    if self
      .global_objects
      .embedded_files
      .insert(name, ref_)
      .is_some()
    {
      None
    } else {
      Some(())
    }
  }

  pub(crate) fn set_tag_tree(&mut self, root: TagTree) {
    // Only set the tag tree if the user actually enabled tagging.
    if self.serialize_settings.enable_tagging {
      self.global_objects.tag_tree = Some(root)
    }
  }

  pub(crate) fn new_ref(&mut self) -> Ref {
    self.cur_ref.bump()
  }

  pub(crate) fn serialize_settings(&self) -> Arc<SerializeSettings> {
    self.serialize_settings.clone()
  }

  // IMPORTANT: DO NEVER CALL `Chunk::new`, `Pdf::new` or `Content::new` directly! Instead,
  // always make sure to use the methods on `SerializeContext`, to ensure the
  // flags are applied consistently across all chunks.

  pub(crate) fn new_chunk(&self) -> Chunk {
    Chunk::with_settings(self.chunk_settings)
  }

  pub(crate) fn new_content(&self) -> Content {
    Content::with_settings(self.chunk_settings)
  }

  pub(crate) fn new_pdf_with_capacity(&self, capacity: usize) -> Pdf {
    Pdf::with_settings_and_capacity(self.chunk_settings, capacity)
  }

  pub(crate) fn page_tree_ref(&mut self) -> Ref {
    self.page_tree_ref
  }

  pub(crate) fn register_font_container(&mut self, font: Font) -> Rc<RefCell<FontContainer>> {
    self
      .global_objects
      .font_map
      .entry(font.clone())
      .or_insert_with(|| Rc::new(RefCell::new(FontContainer::new(font.clone()))))
      .clone()
  }

  pub(crate) fn finish(
    mut self,
    mut chunk_container: ChunkContainer,
  ) -> WriterResult<(Pdf, Ref, Option<ObjectStreams>)> {
    // We need to be careful here that we serialize the objects in the right order,
    // as in some cases we use MaybeTake::take to remove an object, which means that
    // no object that is serialized afterwards must depend on it.

    // Serialize all objects that can only be written in the end.
    self.serialize_destination_profiles(&mut chunk_container);
    self.serialize_outline(&mut chunk_container);
    self.serialize_fonts(&mut chunk_container)?;
    self.serialize_pages(&mut chunk_container)?;
    self.serialize_page_tree(&mut chunk_container);
    // It is important that we serialize the tags AFTER we have serialized the pages,
    // because page serialization will update the annotation refs of the page infos,
    // and when serializing the parent tree map we need to know the refs of the annotations.
    // Destinations follow the tags, because a structure destination names the
    // structure element the tag tree just assigned a ref to.
    self.serialize_tag_tree(&mut chunk_container)?;
    self.serialize_xyz_destinations(&mut chunk_container)?;

    // Create the final PDF.
    let (pdf, next_ref, object_stream) = chunk_container.finish(&mut self)?;
    self.register_limits(pdf.limits());

    self.check_validator_limits();

    if !self.validation_errors.is_empty() {
      // Deduplicate errors, while still preserving order.
      let mut errors = vec![];
      let mut seen = HashSet::new();

      for error in self.validation_errors {
        if !seen.contains(&error) {
          seen.insert(error.clone());
          errors.push(error);
        }
      }

      return Err(WriterError::Validation(errors));
    }

    if let Some(limit_error) = self.check_version_limits() {
      return Err(WriterError::Limit(limit_error));
    }

    // Just a sanity check that we've actually processed all items.
    Ok((pdf, next_ref, object_stream))
  }
}

/// Various registration methods.
impl SerializeContext {
  pub(crate) fn register_validation_error(&mut self, error: ValidationError) {
    if let Some(validators) = self.serialize_settings().validators().prohibits(&error) {
      self.validation_errors.push((error, validators))
    }
  }

  pub(crate) fn register_limits(&mut self, limits: &Limits) {
    self.limits.merge(limits);
  }

  pub(crate) fn register_page_struct_parent(
    &mut self,
    page_index: usize,
    num_mcids: i32,
  ) -> Option<i32> {
    if self.serialize_settings.enable_tagging {
      if num_mcids == 0 {
        return None;
      }

      let id = self.global_objects.struct_parents.len();
      self
        .global_objects
        .struct_parents
        .push(StructParentElement::Page(page_index, num_mcids));
      Some(i32::try_from(id).unwrap_or(i32::MAX))
    } else {
      None
    }
  }

  /// Register the struct parent integer in the parent tree.
  /// The annotation parent must be later set using [`Self::set_annotation_parent`].
  pub(crate) fn register_annotation_parent(&mut self, ai: AnnotationIdentifier) -> Option<i32> {
    if self.serialize_settings.enable_tagging {
      let id = self.global_objects.struct_parents.len();
      self
        .global_objects
        .struct_parents
        .push(StructParentElement::Annotation(ai));
      Some(i32::try_from(id).unwrap_or(i32::MAX))
    } else {
      None
    }
  }

  pub(crate) fn register_page(&mut self, page: InternalPage) {
    let ref_ = self.new_ref();
    self.page_infos.push(PageInfo {
      ref_,
      surface_size: page.page_settings.surface_size(),
      // Will be populated when the page is serialized.
      annotations: vec![],
    });
    self.global_objects.pages.push((ref_, page));
  }

  fn register_cached<T: SipHashable>(
    &mut self,
    item: T,
    mut func: impl FnMut(&mut Self, T, Ref),
  ) -> Ref {
    let hash = item.sip_hash();
    if let Some(_ref) = self.cached_mappings.get(&hash) {
      *_ref
    } else {
      let root_ref = self.new_ref();
      func(self, item, root_ref);
      self.cached_mappings.insert(hash, root_ref);
      root_ref
    }
  }

  pub(crate) fn register_cacheable<T>(
    &mut self,
    chunk_container: &mut ChunkContainer,
    object: T,
  ) -> Ref
  where
    T: Cacheable,
  {
    self.register_cached(object, |sc, object, root_ref| {
      object.serialize(sc, chunk_container, root_ref);
    })
  }

  pub(crate) fn register_resourceable<T>(
    &mut self,
    chunk_container: &mut ChunkContainer,
    object: T,
  ) -> T::Resource
  where
    T: Resourceable,
  {
    Resource::new(self.register_cacheable(chunk_container, object))
  }

  #[cfg(feature = "images")]
  pub(crate) fn register_image(
    &mut self,
    chunk_container: &mut ChunkContainer,
    image: Image,
  ) -> Ref {
    self.register_cached(image, |sc, object, root_ref| {
      object.serialize(sc, chunk_container, root_ref);
    })
  }

  pub(crate) fn register_xyz_destination(&mut self, dest: XyzDestination) -> Ref {
    self.register_cached(dest, |sc, dest, root_ref| {
      sc.global_objects.xyz_destinations.push((root_ref, dest));
    })
  }

  pub(crate) fn register_font_identifier(&mut self, f: FontIdentifier) -> resource::Font {
    let hash = f.sip_hash();
    if let Some(_ref) = self.cached_mappings.get(&hash) {
      resource::Font::new(*_ref)
    } else {
      let root_ref = self.new_ref();
      self.cached_mappings.insert(hash, root_ref);
      resource::Font::new(root_ref)
    }
  }

  pub(crate) fn register_colorspace(
    &mut self,
    chunk_container: &mut ChunkContainer,
    cs: ColorSpace,
  ) -> MaybeDeviceColorSpace {
    match cs {
      ColorSpace::CieBased(CieBasedColorSpace::Srgb) => {
        MaybeDeviceColorSpace::ColorSpace(self.register_resourceable(
          chunk_container,
          ICCBasedColorSpace(self.serialize_settings.pdf_version().rgb_icc()),
        ))
      }
      ColorSpace::CieBased(CieBasedColorSpace::Luma) => {
        MaybeDeviceColorSpace::ColorSpace(self.register_resourceable(
          chunk_container,
          ICCBasedColorSpace(self.serialize_settings.pdf_version().grey_icc()),
        ))
      }
      ColorSpace::Device(DeviceColorSpace::Gray) => MaybeDeviceColorSpace::DeviceGray,
      ColorSpace::Device(DeviceColorSpace::Rgb) => MaybeDeviceColorSpace::DeviceRgb,
      ColorSpace::Device(DeviceColorSpace::Cmyk) => MaybeDeviceColorSpace::DeviceCMYK,
    }
  }
}

/// Various serialization methods.
/// All methods are supposed to only be called once in `SerializeContext::finish`!
impl SerializeContext {
  fn serialize_destination_profiles(&mut self, chunk_container: &mut ChunkContainer) {
    let validators = self.serialize_settings.validators();
    chunk_container.non_stream.destination_profiles = validators.output_intent().map(|subtype| {
      let root_ref = self.new_ref();
      let mut chunk = self.new_chunk();

      let oi_ref = self.new_ref();
      let mut oi = chunk.indirect(oi_ref).start::<OutputIntent>();
      let icc_profile = self.serialize_settings.pdf_version().rgb_icc();

      oi.dest_output_profile(self.register_cacheable(chunk_container, icc_profile.clone()))
        .subtype(subtype)
        .output_condition_identifier(TextStr("Custom"))
        .output_condition(TextStr("sRGB"))
        .registry_name(TextStr(""))
        .info(TextStr(
          format!(
            "sRGB v{}.{}",
            icc_profile.metadata().major,
            icc_profile.metadata().minor
          )
          .as_str(),
        ));
      oi.finish();

      let mut array = chunk.indirect(root_ref).array();
      array.item(oi_ref);
      array.finish();

      (root_ref, chunk)
    });
  }

  fn serialize_outline(&mut self, chunk_container: &mut ChunkContainer) {
    let outline = self.global_objects.outline.take();
    if let Some(outline) = &outline {
      let outline_ref = self.new_ref();
      outline.serialize(self, chunk_container, outline_ref);
    } else {
      self.register_validation_error(ValidationError::MissingDocumentOutline);
    }
  }

  fn serialize_fonts(&mut self, chunk_container: &mut ChunkContainer) -> WriterResult<()> {
    let fonts = std::mem::take(&mut self.global_objects.font_map);
    for font_container in fonts.values() {
      let borrowed = font_container.borrow();

      if !borrowed.type3_mapper().is_empty() {
        for t3_font in borrowed.type3_mapper().fonts() {
          let f = self.register_font_identifier(t3_font.identifier());
          t3_font.serialize(self, chunk_container, f.get_ref());
        }
      }

      if !borrowed.cid_font().is_empty() {
        let f = self.register_font_identifier(borrowed.cid_font().identifier());
        borrowed
          .cid_font()
          .serialize(self, chunk_container, f.get_ref())?;
      }
    }

    Ok(())
  }

  fn serialize_pages(&mut self, chunk_container: &mut ChunkContainer) -> WriterResult<()> {
    let pages = std::mem::take(&mut self.global_objects.pages);
    for (ref_, page) in pages {
      page.serialize(self, chunk_container, ref_)?;
    }

    Ok(())
  }

  fn serialize_page_tree(&mut self, chunk_container: &mut ChunkContainer) {
    let mut page_tree_chunk = self.new_chunk();
    page_tree_chunk
      .pages(self.page_tree_ref)
      .count(self.page_infos.len() as i32)
      .kids(self.page_infos.iter().map(|i| i.ref_()));
    chunk_container.non_stream.page_tree = Some((self.page_tree_ref, page_tree_chunk));
  }

  fn serialize_xyz_destinations(
    &mut self,
    chunk_container: &mut ChunkContainer,
  ) -> WriterResult<()> {
    let xyz_destinations = std::mem::take(&mut self.global_objects.xyz_destinations);
    for (ref_, dest) in &xyz_destinations {
      dest.serialize(self, chunk_container, *ref_);
    }

    Ok(())
  }

  fn serialize_tag_tree(&mut self, chunk_container: &mut ChunkContainer) -> WriterResult<()> {
    let tag_tree = self.global_objects.tag_tree.take();
    let struct_parents = std::mem::take(&mut self.global_objects.struct_parents);
    if let Some(root) = &tag_tree {
      let mut parent_tree_map = HashMap::new();
      let mut id_tree_map = BTreeMap::new();
      let struct_tree_root_ref = self.new_ref();
      let document_ref = root.serialize(
        self,
        chunk_container,
        &mut parent_tree_map,
        &mut id_tree_map,
        struct_tree_root_ref,
      )?;

      self.tag_refs = id_tree_map.clone();

      let mut chunk = self.new_chunk();
      let mut tree = chunk
        .indirect(struct_tree_root_ref)
        .start::<StructTreeRoot>();

      let mut sub_chunks = vec![];

      if self.serialize_settings.pdf_version() < PdfVersion::Pdf20 {
        let mut role_map = tree.role_map();
        // PDF 2.0 exclusive structure elements.
        role_map.insert(Name(b"Strong"), StructRole::Span);
        role_map.insert(Name(b"Em"), StructRole::Span);
        for level in self.global_objects.custom_heading_roles.iter() {
          let role2 = StructRole2::Heading(*level);
          role_map.insert(role2.to_name(&mut [0; 6]), StructRole::P);
        }
      } else {
        let mut namespaces = tree.namespaces();

        // PDF 2.0 standard structure namespace
        namespaces.item(self.ssn_ref);
        let mut ns_chunk = self.new_chunk();
        // TODO: restore `pdf_2_ns()` once pdf-writer > 0.15.0 ships the fix. It
        // writes `https://www.iso.org/pdf2/ssn`, while ISO 32000-2:2020, 14.8.6
        // defines the identifier below.
        // https://github.com/typst/pdf-writer/pull/85
        ns_chunk
          .namespace(self.ssn_ref)
          .ns(TextStr("http://iso.org/pdf2/ssn"));
        sub_chunks.push(ns_chunk);
      }
      tree.children().item(document_ref);

      if !struct_parents.is_empty() {
        let mut parent_tree = tree.parent_tree();
        let mut tree_nums = parent_tree.nums();

        for (index, struct_parent) in struct_parents.iter().enumerate() {
          match *struct_parent {
            StructParentElement::Page(page_index, num_mcids) => {
              let mut list_chunk = self.new_chunk();
              let list_ref = self.new_ref();

              let mut refs = list_chunk.indirect(list_ref).array();

              for mcid in 0..num_mcids {
                let rci = PageTagIdentifier::new(page_index, mcid);
                refs.item(parent_tree_map.get(&rci.into()).ok_or_else(|| {
                  WriterError::TagTree(format!(
                    "page tag identifier {rci:?} doesn't appear in the tag tree"
                  ))
                })?);
              }

              refs.finish();

              sub_chunks.push(list_chunk);
              tree_nums.insert(index as i32, list_ref);
            }
            StructParentElement::Annotation(ai) => {
              // Write a reference to the parent structure element.
              // From the PDF 1.7 spec (14.7.5.4 Finding structure elements from content items):
              // > For an object identified as a content item by means of an object reference
              // > (see 14.7.5.3, "PDF objects as content items"), the value shall be an
              // > indirect reference to the parent structure element.
              let page_annotations = &self.page_infos[ai.page_index].annotations();
              let parent_ref = *page_annotations[ai.annot_index].1.get().ok_or_else(|| {
                WriterError::TagTree(format!(
                  "annotation identifier {ai:?} doesn't appear in the tag tree"
                ))
              })?;
              tree_nums.insert(index as i32, parent_ref);
            }
          }
        }

        tree_nums.finish();
        parent_tree.finish();
      }

      if !id_tree_map.is_empty() {
        let mut id_tree = tree.id_tree();
        let mut names = id_tree.names();

        for (name, ref_) in id_tree_map {
          names.insert(Str(name.as_bytes()), ref_);
        }
      }

      if !struct_parents.is_empty() {
        tree.parent_tree_next_key(struct_parents.len() as i32);
      }
      tree.finish();

      for sub_chunk in sub_chunks {
        chunk.extend(&sub_chunk);
      }

      chunk_container.non_stream.struct_tree_root = Some((struct_tree_root_ref, chunk));
    } else {
      self.register_validation_error(ValidationError::MissingTagging);
    }

    Ok(())
  }

  fn check_validator_limits(&mut self) {
    if self.cur_ref > Ref::new(8388607) {
      self.register_validation_error(ValidationError::TooManyIndirectObjects)
    }

    if self.limits.str_len() > STR_LEN {
      self.register_validation_error(ValidationError::TooLongString);
    }

    if self.limits.name_len() > NAME_LEN {
      self.register_validation_error(ValidationError::TooLongName);
    }

    if self.limits.real() > MAX_FLOAT {
      self.register_validation_error(ValidationError::TooLargeFloat);
    }

    if self.limits.array_len() > ARRAY_LEN {
      self.register_validation_error(ValidationError::TooLongArray);
    }

    if self.limits.dict_entries() > DICT_LEN {
      self.register_validation_error(ValidationError::TooLongDictionary);
    }
  }

  fn check_version_limits(&self) -> Option<LimitError> {
    if self.serialize_settings.pdf_version() != PdfVersion::Pdf14 {
      return None;
    }

    if self.limits.real() > MAX_FLOAT {
      return Some(LimitError::TooLargeFloat);
    }

    if self.limits.array_len() > ARRAY_LEN {
      return Some(LimitError::TooLongArray);
    }

    if self.limits.dict_entries() > DICT_LEN {
      return Some(LimitError::TooLongDictionary);
    }

    None
  }
}

#[derive(Default)]
pub(crate) struct GlobalObjects {
  /// A map from fonts to font container.
  font_map: IndexMap<Font, Rc<RefCell<FontContainer>>>,
  /// All XYZ destinations used in the document. The reason we need to store them
  /// separately is that we can only serialize them in the very end, once all pages
  /// have been written, so that we know the Ref of the page they belong to.
  xyz_destinations: Vec<(Ref, XyzDestination)>,
  /// All pages and their corresponding chunks. Similarly to destinations, they need
  /// to be written in the very end, because pages might contain annotations which in turn
  /// depend on future pages (not written yet), so pages must also only be written in the
  /// very end.
  pages: Vec<(Ref, InternalPage)>,
  /// Stores the struct parent elements.
  struct_parents: Vec<StructParentElement>,
  /// Stores the document outline.
  outline: Option<Outline>,
  /// Stores the tag tree.
  tag_tree: Option<TagTree>,
  /// Stores the association of the names of embedded files to their refs,
  /// for the catalog dictionary.
  pub(crate) embedded_files: BTreeMap<String, Ref>,
  /// A list of custom headings numbers used in the document.
  pub(crate) custom_heading_roles: BTreeSet<NonZeroU16>,
}

pub(crate) trait Cacheable: SipHashable {
  fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  );
}
