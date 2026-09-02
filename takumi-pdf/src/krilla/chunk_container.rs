use pdf_writer::{Chunk, Finish, Name, Pdf, Ref, Str, TextStr};
use std::collections::HashMap;
use std::sync::OnceLock;
use xmp_writer::{RenditionClass, XmpWriter};

use crate::krilla::configure::{PdfVersion, ValidationError};
use crate::krilla::error::KrillaResult;
use crate::krilla::interactive::annotation::WidgetAnnotation;
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
  /// Refs of the widget annotations that double as AcroForm field dicts.
  pub(crate) form_fields: Vec<Ref>,
  /// The base-14 font the widget appearance streams and `/DR` share.
  pub(crate) form_font: Option<Ref>,
  /// Radio groups by field name: the field object every button hangs off.
  pub(crate) form_radio_groups: Vec<RadioGroup>,
}

pub(crate) struct RadioGroup {
  pub(crate) field: Ref,
  pub(crate) name: String,
  pub(crate) flags: i32,
  /// `/TU`, taken from the first button that carries one.
  pub(crate) description: Option<String>,
  /// The state name of the selected button, absent when none is.
  pub(crate) value: Option<String>,
  pub(crate) kids: Vec<Ref>,
  /// What each button submits, in the order their states are numbered.
  pub(crate) options: Vec<String>,
}

impl ChunkContainer {
  /// The base-14 face the field appearances draw with, written on first use.
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

  /// The field object for `name`, created on its first button.
  pub(crate) fn radio_group(
    &mut self,
    sc: &mut SerializeContext,
    widget: &WidgetAnnotation,
    index: usize,
    export: &str,
    on: bool,
    kid: Ref,
  ) -> Ref {
    let group = match self
      .form_radio_groups
      .iter_mut()
      .find(|group| group.name == widget.name)
    {
      Some(group) => group,
      None => {
        let field = sc.new_ref();

        self.form_fields.push(field);
        self.form_radio_groups.push(RadioGroup {
          field,
          name: widget.name.clone(),
          flags: widget.field.flags(&widget.style),
          description: widget.description.clone(),
          value: None,
          kids: vec![],
          options: vec![],
        });

        self
          .form_radio_groups
          .last_mut()
          .expect("just pushed a radio group")
      }
    };

    group.kids.push(kid);

    if group.options.len() <= index {
      group.options.resize(index + 1, String::new());
    }
    group.options[index] = export.to_string();

    if on {
      group.value = Some(index.to_string());
    }

    group.field
  }

  /// Writes the field object of every radio group, once its buttons are known.
  fn write_radio_groups(&mut self) {
    let chunk = &mut self.non_stream.annotations;

    for group in &self.form_radio_groups {
      let mut field = chunk.indirect(group.field).dict();

      field.pair(Name(b"FT"), Name(b"Btn"));
      field.pair(Name(b"T"), TextStr(&group.name));
      field.pair(Name(b"Ff"), group.flags);

      if let Some(description) = &group.description {
        field.pair(Name(b"TU"), TextStr(description));
      }
      field.pair(
        Name(b"V"),
        match &group.value {
          Some(value) => Name(value.as_bytes()),
          None => Name(b"Off"),
        },
      );
      field
        .insert(Name(b"Opt"))
        .array()
        .items(group.options.iter().map(|option| TextStr(option)));
      field
        .insert(Name(b"Kids"))
        .array()
        .items(group.kids.iter().copied());
      field.finish();
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
      form_fields: vec![],
      form_font: None,
      form_radio_groups: vec![],
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
    }
  }

  pub(crate) fn finish(
    mut self,
    sc: &mut SerializeContext,
  ) -> KrillaResult<(Pdf, Ref, Option<ObjectStream>)> {
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
    self.write_radio_groups();

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
      let form_font_ref = self.form_font.map(|font| remapper[&font]);

      let mut catalog = pdf.catalog(catalog_ref);

      if !self.form_fields.is_empty() {
        let mut form = catalog.form();

        form.fields(self.form_fields.iter().map(|field| remapper[field]));

        // A blank form needs no default appearance, so it carries no
        // unembedded font for a validator to reject.
        if let Some(font_ref) = form_font_ref {
          form.default_appearance(Str(b"/Helv 0 Tf 0 g"));
          form
            .default_resources()
            .fonts()
            .pair(Name(b"Helv"), font_ref);
        }
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
          sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

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
