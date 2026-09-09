use pdf_writer::{Chunk, Finish, Name, Pdf, Ref, Str, TextStr};
use std::collections::HashMap;
use std::sync::OnceLock;
use xmp_writer::{RenditionClass, XmpWriter};

use crate::krilla::configure::{PdfVersion, ValidationError};
use crate::krilla::error::KrillaResult;
use crate::krilla::interactive::annotation::FORM_FONT;
use crate::krilla::interchange::metadata::{Metadata, write_custom_properties};
use crate::krilla::metadata::PageLayout;
use crate::krilla::object_stream::{self, ObjectStream};
use crate::krilla::serialize::SerializeContext;
use crate::krilla::util::{Deferred, stable_hash_base64};

type DChunk = Deferred<Chunk>;

/// Collects all chunks that we create while building
/// the PDF and then writes them out in an orderly manner.
pub(crate) struct ChunkContainer {
  pub(crate) streams: StreamChunks,
  pub(crate) mixed: MixedChunks,
  pub(crate) metadata: Option<Metadata>,
  pub(crate) non_stream: NonStreamChunks,
  /// The root fields of the AcroForm.
  form_fields: Vec<Ref>,
  /// The base-14 face the field appearances and `/DR` share.
  form_font: Option<Ref>,
  /// The non-terminal fields dotted names put above their own.
  form_parents: Vec<ParentField>,
}

/// A non-terminal field named by one segment of a dotted HTML name.
struct ParentField {
  /// The dotted name down to this field.
  qualified: String,
  field: Ref,
  partial: String,
  parent: Option<Ref>,
  kids: Vec<Ref>,
}

/// Where a field sits in the `/T` hierarchy.
pub(crate) struct FieldSlot {
  /// The non-terminal field above it, absent at the root.
  pub(crate) parent: Option<Ref>,
  /// `/T`, the last segment of the name.
  pub(crate) partial: String,
  /// `/TM`, the HTML name when the segments do not spell it back.
  pub(crate) mapping: Option<String>,
}

impl ChunkContainer {
  /// The place of the field named `name`, with `kid` hung there. A period
  /// delimits the PDF field hierarchy, so `a.b` is the field `b` under a
  /// non-terminal `a`, created on first use.
  pub(crate) fn field_slot(
    &mut self,
    sc: &mut SerializeContext,
    name: &str,
    kid: Ref,
  ) -> FieldSlot {
    let mut segments = name
      .split('.')
      .filter(|segment| !segment.is_empty())
      .collect::<Vec<_>>();

    if segments.is_empty() {
      segments.push(name);
    }
    let (partial, parents) = match segments.split_last() {
      Some((partial, parents)) => (*partial, parents),
      None => (name, &[][..]),
    };
    let mut parent = None;
    let mut qualified = String::new();

    for segment in parents {
      if !qualified.is_empty() {
        qualified.push('.');
      }
      qualified.push_str(segment);
      parent = Some(self.parent_field(sc, &qualified, segment, parent));
    }
    self.hang(parent, kid);

    FieldSlot {
      parent,
      partial: partial.to_string(),
      mapping: (segments.join(".") != name).then(|| name.to_string()),
    }
  }

  fn parent_field(
    &mut self,
    sc: &mut SerializeContext,
    qualified: &str,
    partial: &str,
    parent: Option<Ref>,
  ) -> Ref {
    if let Some(existing) = self
      .form_parents
      .iter()
      .find(|field| field.qualified == qualified)
    {
      return existing.field;
    }
    let field = sc.new_ref();

    self.hang(parent, field);
    self.form_parents.push(ParentField {
      qualified: qualified.to_string(),
      field,
      partial: partial.to_string(),
      parent,
      kids: vec![],
    });
    field
  }

  /// Adds `kid` under `parent`, or among the form's root fields.
  fn hang(&mut self, parent: Option<Ref>, kid: Ref) {
    match parent.and_then(|parent| {
      self
        .form_parents
        .iter_mut()
        .find(|field| field.field == parent)
    }) {
      Some(parent) => parent.kids.push(kid),
      None => self.form_fields.push(kid),
    }
  }

  fn write_parent_fields(&mut self) {
    let chunk = &mut self.non_stream.annotations;

    for parent in &self.form_parents {
      let mut field = chunk.indirect(parent.field).dict();

      field.pair(Name(b"T"), TextStr(&parent.partial));

      if let Some(above) = parent.parent {
        field.pair(Name(b"Parent"), above);
      }
      field
        .insert(Name(b"Kids"))
        .array()
        .items(parent.kids.iter().copied());
      field.finish();
    }
  }

  /// The face the field appearances draw with, written on first use. A viewer
  /// redrawing an edited field reads it from `/DR` by the name `/DA` gives.
  pub(crate) fn form_font(&mut self, sc: &mut SerializeContext) -> Ref {
    match self.form_font {
      Some(font) => font,
      None => {
        let font = sc.new_ref();

        self
          .non_stream
          .annotations
          .type1_font(font)
          .base_font(Name(b"Helvetica"))
          .encoding_predefined(Name(b"WinAnsiEncoding"))
          .finish();
        self.form_font = Some(font);
        font
      }
    }
  }
}

pub(crate) struct StreamChunks {
  pub(crate) fonts: Vec<Chunk>,
  pub(crate) shading_functions: Vec<Chunk>,
  pub(crate) patterns: Vec<Chunk>,
  pub(crate) pages: Vec<DChunk>,
  pub(crate) embedded_files: Vec<Chunk>,
  pub(crate) icc_profiles: Vec<Chunk>,
  pub(crate) x_objects: Vec<Chunk>,
  pub(crate) images: Vec<Deferred<KrillaResult<Chunk>>>,
}

pub(crate) struct MixedChunks {
  pub(crate) embedded_pdfs: Vec<Deferred<KrillaResult<EmbeddedPdfChunk>>>,
}

pub(crate) struct NonStreamChunks {
  pub(crate) page_tree: Option<(Ref, Chunk)>,
  pub(crate) outline: Option<(Ref, Chunk)>,
  pub(crate) page_label_tree: Option<(Ref, Chunk)>,
  pub(crate) destination_profiles: Option<(Ref, Chunk)>,
  pub(crate) struct_tree_root: Option<(Ref, Chunk)>,
  pub(crate) struct_elements: Option<Chunk>,
  pub(crate) page_labels: Chunk,
  pub(crate) annotations: Chunk,
  pub(crate) color_spaces: Chunk,
  pub(crate) destinations: Chunk,
  pub(crate) ext_g_states: Chunk,
  pub(crate) resource_dictionaries: Chunk,
  pub(crate) masks: Chunk,
  pub(crate) fonts: Chunk,
  pub(crate) shading_functions: Chunk,
  pub(crate) patterns: Chunk,
  pub(crate) pages: Chunk,
  pub(crate) embedded_files: Chunk,
}

impl ChunkContainer {
  pub(crate) fn new(sc: &SerializeContext) -> Self {
    Self {
      streams: StreamChunks {
        fonts: vec![],
        shading_functions: vec![],
        patterns: vec![],
        pages: vec![],
        embedded_files: vec![],
        icc_profiles: vec![],
        x_objects: vec![],
        images: vec![],
      },
      mixed: MixedChunks {
        embedded_pdfs: vec![],
      },
      metadata: None,
      non_stream: NonStreamChunks {
        page_tree: None,
        outline: None,
        page_label_tree: None,
        destination_profiles: None,
        struct_tree_root: None,
        struct_elements: None,
        page_labels: sc.new_chunk(),
        annotations: sc.new_chunk(),
        color_spaces: sc.new_chunk(),
        destinations: sc.new_chunk(),
        ext_g_states: sc.new_chunk(),
        resource_dictionaries: sc.new_chunk(),
        masks: sc.new_chunk(),
        fonts: sc.new_chunk(),
        shading_functions: sc.new_chunk(),
        patterns: sc.new_chunk(),
        pages: sc.new_chunk(),
        embedded_files: sc.new_chunk(),
      },
      form_fields: vec![],
      form_font: None,
      form_parents: vec![],
    }
  }

  pub(crate) fn finish(
    mut self,
    sc: &mut SerializeContext,
  ) -> KrillaResult<(Pdf, Ref, Option<ObjectStream>)> {
    self.write_parent_fields();

    let mut remapped_ref = Ref::new(1);
    let mut remapper = HashMap::new();

    // Allows us to estimate the capacity we will need for the new PDF.
    let mut chunks_byte_len = 0;

    // This traverses the chunks in the order that we will write them to the PDF and assigns new
    // references as we go. This gives us the advantage that the PDF will be numbered with
    // monotonically increasing numbers, which, while it is not a strict requirement for a valid
    // PDF, makes it a lot cleaner and might make implementing features like object streams
    // easier down the road.
    //
    // It also allows us to estimate the capacity we will need for the new PDF.
    self.visit(sc, &mut |chunk| {
      for object_ref in chunk.refs() {
        let existing = remapper.insert(object_ref, remapped_ref.bump());
        debug_assert!(existing.is_none());
      }
      chunks_byte_len += chunk.len();
    })?;

    // Chunk length is not an exact number because the length might change as we renumber,
    // so we add a bit of a padding by multiplying with 1.1. The 200 is additional padding
    // for the document catalog. This hopefully allows us to avoid re-alloactions in the general
    // case, and thus give us better performance.
    let capacity = (chunks_byte_len as f32 * 1.1 + 200.0) as usize;
    let mut pdf = sc.new_pdf_with_capacity(capacity);
    sc.serialize_settings().pdf_version().set_version(&mut pdf);

    if sc.serialize_settings().ascii_compatible
      && !sc
        .serialize_settings()
        .validators()
        .requires_binary_header()
    {
      pdf.set_binary_marker(b"AAAA")
    }

    // The structure tree's dictionaries compress well together and are the
    // bulk of a tagged document, so they move into an object stream, which
    // exists from PDF 1.5 onwards like the cross-reference stream that has to
    // point at them.
    let object_stream = if sc.serialize_settings().pdf_version() >= PdfVersion::Pdf15 {
      self.non_stream.struct_elements.take().and_then(|chunk| {
        let renumbered = chunk.renumber(|old| remapper[&old]);
        let packed = object_stream::pack(&renumbered, remapped_ref.bump(), &mut pdf);

        // Put the chunk back for the visit below to write as it stands.
        if packed.is_none() {
          self.non_stream.struct_elements = Some(chunk);
        }

        packed
      })
    } else {
      None
    };

    // Write the chunks in all the fields.
    self.visit(sc, &mut |chunk| {
      chunk.renumber_into(&mut pdf, |old| remapper[&old]);
    })?;

    let missing_title = self.metadata.as_ref().is_none_or(|m| m.title.is_none());

    if missing_title {
      sc.register_validation_error(ValidationError::NoDocumentTitle);
    }

    // Write the PDF document info metadata.
    let producer = sc.serialize_settings().producer.clone();

    Metadata::serialize_document_info(
      self.metadata.as_ref(),
      &producer,
      &mut remapped_ref,
      &mut pdf,
      sc.serialize_settings().configuration,
    );

    let instance_id = stable_hash_base64(pdf.as_bytes());

    let document_id = if let Some(metadata) = &self.metadata {
      if let Some(document_id) = &metadata.document_id {
        stable_hash_base64(&(sc.serialize_settings().pdf_version().as_str(), document_id))
      } else if metadata.title.is_some() && metadata.authors.is_some() {
        stable_hash_base64(&(
          sc.serialize_settings().pdf_version().as_str(),
          &metadata.title,
          &metadata.authors,
        ))
      } else {
        instance_id.clone()
      }
    } else {
      instance_id.clone()
    };

    let mut xmp = XmpWriter::new();
    if let Some(metadata) = &self.metadata {
      metadata.serialize_xmp_metadata(&mut xmp, sc, &instance_id);
    }

    let custom_schemas = self
      .metadata
      .as_ref()
      .map(|metadata| metadata.custom_schemas.as_slice())
      .unwrap_or_default();
    let settings = sc.serialize_settings();
    let validators = settings.validators();
    validators.write_xmp(&mut xmp, custom_schemas);

    write_custom_properties(&mut xmp, custom_schemas);

    xmp.producer(&producer);
    xmp.num_pages(sc.page_infos().len() as u32);
    xmp.format("application/pdf");
    xmp.instance_id(&instance_id);
    xmp.document_id(&document_id);
    pdf.set_file_id((
      document_id.as_bytes().to_vec(),
      instance_id.as_bytes().to_vec(),
    ));

    xmp.rendition_class(RenditionClass::Proof);
    sc.serialize_settings().pdf_version().write_xmp(&mut xmp);

    let named_destinations = sc.global_objects.named_destinations.take();
    let embedded_files = sc.global_objects.embedded_files.take();

    // We only write a catalog if a page tree exists. Every valid PDF must have one
    // and krilla ensures that there always is one, but for snapshot tests, it can be
    // useful to not write a document catalog if we don't actually need it for the test.
    if self.non_stream.page_tree.is_some()
      || self.non_stream.outline.is_some()
      || self.non_stream.page_label_tree.is_some()
      || self.non_stream.destination_profiles.is_some()
      || self.non_stream.struct_tree_root.is_some()
    {
      let meta_ref = if sc.serialize_settings().xmp_metadata {
        let meta_ref = remapped_ref.bump();
        let xmp_buf = xmp.finish(None);
        pdf
          .stream(meta_ref, xmp_buf.as_bytes())
          .pair(Name(b"Type"), Name(b"Metadata"))
          .pair(Name(b"Subtype"), Name(b"XML"));
        Some(meta_ref)
      } else {
        None
      };

      let catalog_ref = remapped_ref.bump();
      let form_font = self.form_font.map(|font| remapper[&font]);

      let mut catalog = pdf.catalog(catalog_ref);

      if let Some(font) = form_font {
        let mut form = catalog.form();

        form.fields(self.form_fields.iter().map(|field| remapper[field]));
        form.default_appearance(Str(format!("/{FORM_FONT} 0 Tf 0 g").as_bytes()));
        form
          .default_resources()
          .fonts()
          .pair(Name(FORM_FONT.as_bytes()), font);
        form.finish();
      }

      if let Some(pt) = &self.non_stream.page_tree {
        catalog.pages(remapper[&pt.0]);
      }

      if let Some(meta_ref) = meta_ref {
        catalog.metadata(meta_ref);
      }

      if let Some(pl) = &self.non_stream.page_label_tree {
        catalog.pair(Name(b"PageLabels"), remapper[&pl.0]);
      }

      if let Some(oi) = &self.non_stream.destination_profiles {
        catalog.pair(Name(b"OutputIntents"), remapper[&oi.0]);
      }

      if let Some(lang) = self.metadata.as_ref().and_then(|m| m.language.as_ref()) {
        catalog.lang(TextStr(lang));
      } else {
        sc.register_validation_error(ValidationError::NoDocumentLanguage);
      }

      if let Some(st) = &self.non_stream.struct_tree_root {
        catalog.pair(Name(b"StructTreeRoot"), remapper[&st.0]);
        let mut mark_info = catalog.mark_info();
        mark_info.marked(true);
        if sc.serialize_settings().pdf_version() >= PdfVersion::Pdf16
          && sc.serialize_settings().pdf_version() < PdfVersion::Pdf20
        {
          // We always set suspects to false because it's required by PDF/UA.
          mark_info.suspects(false);
        }
        mark_info.finish();
      }

      let write_doc_title = sc
        .serialize_settings()
        .validators()
        .requires_display_doc_title();
      let text_direction = self.metadata.as_ref().and_then(|m| m.text_direction);

      if write_doc_title || text_direction.is_some() {
        let mut vp = catalog.viewer_preferences();

        if write_doc_title {
          vp.display_doc_title(true);
        }

        if let Some(dir) = text_direction {
          vp.direction(dir.to_pdf());
        }
      }

      let page_layout = self.metadata.as_ref().and_then(|m| m.page_layout);
      if let Some(layout) = page_layout {
        // TwoPageLeft and TwoPageRight are only available PDF 1.5+
        if sc.serialize_settings().pdf_version() >= PdfVersion::Pdf15
          || !matches!(layout, PageLayout::TwoPageLeft | PageLayout::TwoPageRight)
        {
          catalog.page_layout(layout.to_pdf());
        }
      }

      if let Some(ol) = &self.non_stream.outline {
        catalog.outlines(remapper[&ol.0]);
      }

      let settings = sc.serialize_settings();
      let validators = settings.validators();
      let write_embedded_files = self.non_stream.embedded_files.len() != 0
        || validators.requires_embedded_files_when_empty();

      if !named_destinations.is_empty() || write_embedded_files {
        // Cannot use pdf-writer API here because it requires Ref's, while
        // we write our destinations directly into the array.
        let mut names = catalog.names();

        if !named_destinations.is_empty() {
          let mut dest_name_tree = names.destinations();
          let mut dest_name_entries = dest_name_tree.names();

          // "The Names entries in the leaf (or root) nodes shall
          // contain the tree’s keys and their associated values,
          // arranged in key-value pairs and shall be sorted lexically
          // in ascending order by key. Shorter keys shall appear
          // before longer ones beginning with the same byte sequence.
          // Any encoding of the keys may be used as long as it is
          // self-consistent; keys shall be compared for equality on
          // a simple byte-by-byte basis."
          let mut sorted = named_destinations.into_iter().collect::<Vec<_>>();
          // Note that named destinations are guaranteed to be unique,
          // hence just comparing by the name is enough.
          sorted.sort_unstable_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

          for (name, (dest_ref, _)) in sorted {
            dest_name_entries.insert(Str(name.as_bytes()), remapper[&dest_ref]);
          }

          dest_name_entries.finish();
          dest_name_tree.finish();
        }

        if write_embedded_files {
          let mut embedded_files_name_tree = names.embedded_files();
          let mut embedded_name_entries = embedded_files_name_tree.names();

          for (name, _ref) in &embedded_files {
            embedded_name_entries.insert(Str(name.as_bytes()), remapper[_ref]);
          }
        }
      }

      if !embedded_files.is_empty() && settings.supports_associated_files() {
        let mut associated_files = catalog.insert(Name(b"AF")).array().typed();
        for _ref in embedded_files.values() {
          associated_files.item(remapper[_ref]).finish();
        }
      }

      catalog.finish();
    }

    Ok((pdf, remapped_ref.bump(), object_stream))
  }
}

pub(crate) struct EmbeddedPdfChunk {
  pub(crate) original_chunk: Chunk,
  pub(crate) root_ref_mappings: HashMap<Ref, Ref>,
  pub(crate) new_chunk: OnceLock<Chunk>,
}

/// Visits all chunks in a type.
trait Visit {
  fn visit(&self, sc: &mut SerializeContext, f: &mut impl FnMut(&Chunk)) -> KrillaResult<()>;
}

impl Visit for EmbeddedPdfChunk {
  fn visit(&self, sc: &mut SerializeContext, f: &mut impl FnMut(&Chunk)) -> KrillaResult<()> {
    // Now, we have a chunk that contains everything we need to fully embed the PDF, including
    // the pages we wanted to extract into, as well as all their dependencies. The
    // problem is: during the document creation, we already assigned references to the
    // pages (stored in `SerializerContex::page_infos`), but `hayro_write` created new references
    // for those (stored in `result.root_refs`).

    // Because of this, embedded PDF chunks will be renumbered twice: First, we preprocess the
    // chunk such that page/XObjects are reassigned their original references from the serialize
    // context, and all other objects are assigned new, unique references provided by the
    // serialize context. Then, we renumber them once again by treating them like any other chunk.

    // Since we are calling `visit` twice, we also cache the renumbered chunk.

    let renumbered = self.new_chunk.get_or_init(|| {
      let mut remapper = self.root_ref_mappings.clone();

      self
        .original_chunk
        .renumber(|old| *remapper.entry(old).or_insert_with(|| sc.new_ref()))
    });

    renumbered.visit(sc, f)
  }
}

impl Visit for ChunkContainer {
  fn visit(&self, sc: &mut SerializeContext, f: &mut impl FnMut(&Chunk)) -> KrillaResult<()> {
    self.non_stream.visit(sc, f)?;
    self.mixed.visit(sc, f)?;
    self.streams.visit(sc, f)?;
    Ok(())
  }
}

impl Visit for StreamChunks {
  fn visit(&self, sc: &mut SerializeContext, f: &mut impl FnMut(&Chunk)) -> KrillaResult<()> {
    self.fonts.visit(sc, f)?;
    self.shading_functions.visit(sc, f)?;
    self.patterns.visit(sc, f)?;
    self.pages.visit(sc, f)?;
    self.embedded_files.visit(sc, f)?;
    self.icc_profiles.visit(sc, f)?;
    self.x_objects.visit(sc, f)?;
    self.images.visit(sc, f)?;

    Ok(())
  }
}

impl Visit for MixedChunks {
  fn visit(&self, sc: &mut SerializeContext, f: &mut impl FnMut(&Chunk)) -> KrillaResult<()> {
    self.embedded_pdfs.visit(sc, f)?;

    Ok(())
  }
}

impl Visit for NonStreamChunks {
  fn visit(&self, sc: &mut SerializeContext, f: &mut impl FnMut(&Chunk)) -> KrillaResult<()> {
    self.page_tree.visit(sc, f)?;
    self.outline.visit(sc, f)?;
    self.page_label_tree.visit(sc, f)?;
    self.destination_profiles.visit(sc, f)?;
    self.struct_tree_root.visit(sc, f)?;
    self.struct_elements.visit(sc, f)?;
    self.page_labels.visit(sc, f)?;
    self.annotations.visit(sc, f)?;
    self.color_spaces.visit(sc, f)?;
    self.destinations.visit(sc, f)?;
    self.ext_g_states.visit(sc, f)?;
    self.resource_dictionaries.visit(sc, f)?;
    self.masks.visit(sc, f)?;
    self.fonts.visit(sc, f)?;
    self.shading_functions.visit(sc, f)?;
    self.patterns.visit(sc, f)?;
    self.pages.visit(sc, f)?;
    self.embedded_files.visit(sc, f)?;

    Ok(())
  }
}

impl Visit for Chunk {
  fn visit(&self, _: &mut SerializeContext, f: &mut impl FnMut(&Chunk)) -> KrillaResult<()> {
    f(self);
    Ok(())
  }
}

impl Visit for Option<Chunk> {
  fn visit(&self, sc: &mut SerializeContext, f: &mut impl FnMut(&Chunk)) -> KrillaResult<()> {
    if let Some(chunk) = self {
      chunk.visit(sc, f)?;
    }
    Ok(())
  }
}

impl Visit for Option<(Ref, Chunk)> {
  fn visit(&self, sc: &mut SerializeContext, f: &mut impl FnMut(&Chunk)) -> KrillaResult<()> {
    if let Some((_, chunk)) = self {
      chunk.visit(sc, f)?;
    }
    Ok(())
  }
}

impl<T: Visit + Send + Sync + 'static> Visit for Deferred<T> {
  fn visit(&self, sc: &mut SerializeContext, f: &mut impl FnMut(&Chunk)) -> KrillaResult<()> {
    self.wait().visit(sc, f)
  }
}

impl<T: Visit> Visit for KrillaResult<T> {
  fn visit(&self, sc: &mut SerializeContext, f: &mut impl FnMut(&Chunk)) -> KrillaResult<()> {
    self.as_ref().map_err(|e| e.clone())?.visit(sc, f)
  }
}

impl<T: Visit> Visit for Vec<T> {
  fn visit(&self, sc: &mut SerializeContext, f: &mut impl FnMut(&Chunk)) -> KrillaResult<()> {
    for field in self {
      field.visit(sc, f)?;
    }
    Ok(())
  }
}
