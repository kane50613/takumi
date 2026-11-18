//! Working with pages of a PDF document.

use std::cell::OnceCell;
use std::ops::DerefMut;

use pdf_writer::types::TabOrder;
use pdf_writer::{Chunk, Finish, Ref};

use crate::writer::chunk_container::ChunkContainer;
use crate::writer::configure::ValidationError;
use crate::writer::configure::validate::VersionedFeature;
use crate::writer::content::ContentBuilder;
use crate::writer::error::WriterResult;
use crate::writer::geom::{Rect, Size, Transform};
use crate::writer::interactive::annotation::Annotation;
use crate::writer::interchange::tagging::{AnnotationIdentifier, Identifier, PageTagIdentifier};
use crate::writer::resource::ResourceDictionary;
use crate::writer::serialize::SerializeContext;
use crate::writer::stream::{FilterStreamBuilder, Stream};
use crate::writer::surface::Surface;
use crate::writer::util::Deferred;

/// The settings of a page.
#[derive(Clone, Debug)]
pub struct PageSettings {
  /// The size of the surface, which the media box spans.
  surface_size: Size,
}

impl PageSettings {
  /// Create new page settings and define the size of the page surface.
  pub fn new(size: Size) -> Self {
    Self { surface_size: size }
  }

  /// The visible area of the surface.
  pub(crate) fn media_box(&self) -> Rect {
    Rect::from_xywh(
      0.0,
      0.0,
      self.surface_size.width(),
      self.surface_size.height(),
    )
    .unwrap_or_else(Rect::unit)
  }

  /// The current surface size.
  pub(crate) fn surface_size(&self) -> Size {
    self.surface_size
  }
}

impl Default for PageSettings {
  /// An A4 page.
  #[expect(clippy::unwrap_used, reason = "A4 is a valid size")]
  fn default() -> Self {
    Self::new(Size::from_wh(595.0, 842.0).unwrap())
  }
}

/// A single page.
///
/// You cannot create an instance of this type yourself. Instead, you should use the
/// [`Document::start_page`] (or a related method) to add a new page to a document. In most cases, all
/// you need to do is to call the [`Page::surface`] method, so you can start drawing on the page.
/// However, there are a few other operations you can perform, such as adding annotations
/// to a page.
///
/// [`Document::start_page`]: crate::writer::Document::start_page
pub struct Page<'a> {
  sc: &'a mut SerializeContext,
  chunk_container: &'a mut ChunkContainer,
  page_settings: PageSettings,
  page_index: usize,
  page_stream: Stream,
  num_mcids: i32,
  annotations: Vec<Annotation>,
}

impl<'a> Page<'a> {
  pub(crate) fn new(
    sc: &'a mut SerializeContext,
    chunk_container: &'a mut ChunkContainer,
    page_index: usize,
    page_settings: PageSettings,
  ) -> Self {
    Self {
      sc,
      chunk_container,
      page_settings,
      page_index,
      num_mcids: 0,
      page_stream: Stream::empty(),
      annotations: vec![],
    }
  }

  pub(crate) fn root_transform(&self) -> Transform {
    page_root_transform(self.page_settings.surface_size().height())
  }

  /// Add an annotation to the page.
  pub fn add_annotation(&mut self, annotation: Annotation) {
    self.annotations.push(annotation);
  }

  /// Add a tagged annotation to the page.
  pub fn add_tagged_annotation(&mut self, mut annotation: Annotation) -> Identifier {
    let annot_index = self.annotations.len();
    let ai = AnnotationIdentifier::new(self.page_index, annot_index);
    let struct_parent = self.sc.register_annotation_parent(ai);
    annotation.struct_parent = struct_parent;
    self.add_annotation(annotation);

    match struct_parent {
      None => Identifier::dummy(),
      Some(_) => Identifier::new_annotation(self.page_index, annot_index),
    }
  }

  /// Get the surface of the page to draw on. Calling this multiple times
  /// on the same page will reset any previous drawings.
  pub fn surface(&mut self) -> Surface<'_> {
    let root_builder = ContentBuilder::new(self.root_transform(), false, self.sc);

    let finish_fn = Box::new(|stream, num_mcids| {
      self.page_stream = stream;
      self.num_mcids = num_mcids;
    });

    let page_identifier = if self.sc.serialize_settings().enable_tagging {
      Some(PageTagIdentifier::new(self.page_index, 0))
    } else {
      None
    };

    Surface::new(
      self.sc,
      self.chunk_container,
      root_builder,
      page_identifier,
      finish_fn,
    )
  }

  /// A shorthand for `std::mem::drop`.
  pub fn finish(self) {}
}

pub(crate) fn page_root_transform(height: f32) -> Transform {
  Transform::from_row(1.0, 0.0, 0.0, -1.0, 0.0, height)
}

impl Drop for Page<'_> {
  fn drop(&mut self) {
    // Since we cannot take ownership in `drop`, just make use `mem::take` to pick
    // what we need.
    let annotations = std::mem::take(&mut self.annotations);
    let page_settings = std::mem::take(&mut self.page_settings);

    let struct_parent = self
      .sc
      .register_page_struct_parent(self.page_index, self.num_mcids);

    let stream = std::mem::replace(&mut self.page_stream, Stream::empty());
    let page = InternalPage::new(
      stream,
      self.sc,
      annotations,
      struct_parent,
      page_settings,
      self.page_index,
    );
    self.sc.register_page(page);
  }
}

pub(crate) struct InternalPage {
  pub stream_ref: Ref,
  pub stream_resources: ResourceDictionary,
  pub stream_chunk: Deferred<Chunk>,
  pub page_settings: PageSettings,
  pub page_index: usize,
  pub struct_parent: Option<i32>,
  pub annotations: Vec<Annotation>,
}

impl InternalPage {
  pub(crate) fn new(
    mut stream: Stream,
    sc: &mut SerializeContext,
    annotations: Vec<Annotation>,
    struct_parent: Option<i32>,
    page_settings: PageSettings,
    page_index: usize,
  ) -> Self {
    for validation_error in stream.validation_errors {
      sc.register_validation_error(validation_error)
    }

    let stream_ref = sc.new_ref();
    let serialize_settings = sc.serialize_settings().clone();
    let stream_resources = std::mem::take(&mut stream.resource_dictionary);
    let mut chunk = sc.new_chunk();

    let stream_chunk = Deferred::new(move || {
      let page_stream =
        FilterStreamBuilder::new_from_content_stream(&stream.content, &serialize_settings)
          .finish(&serialize_settings.clone());

      let mut stream = chunk.stream(stream_ref, page_stream.encoded_data());
      page_stream.write_filters(stream.deref_mut());

      stream.finish();
      chunk
    });

    Self {
      stream_resources,
      stream_ref,
      stream_chunk,
      struct_parent,
      annotations,
      page_settings,
      page_index,
    }
  }

  pub(crate) fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) -> WriterResult<()> {
    let mut annotation_refs = vec![];

    if !self.annotations.is_empty() {
      for annotation in &self.annotations {
        let annot_ref = sc.new_ref();

        annotation.serialize(
          sc,
          chunk_container,
          annot_ref,
          self.page_settings.surface_size().height(),
        )?;
        annotation_refs.push((annot_ref, OnceCell::new()));
      }
    }

    let chunk = &mut chunk_container.non_stream.pages;
    let mut page = chunk.page(root_ref);
    self.stream_resources.to_pdf_resources(
      &mut page,
      sc,
      &mut chunk_container.non_stream.resource_dictionaries,
    );

    let transform_rect = |rect: Rect| {
      rect
        .transform(page_root_transform(
          self.page_settings.surface_size().height(),
        ))
        .unwrap_or(rect)
    };

    let media_box = transform_rect(self.page_settings.media_box());
    page.media_box(media_box.to_pdf_rect());

    if let Some(struct_parent) = self.struct_parent {
      page.struct_parents(struct_parent);
    }

    // Only required for PDF/UA, but might as well always set it if there
    // are annotations.
    //
    // Since the navigation order of annotations only has an effect if there
    // are annotations, we usually only set the key in that case. However,
    // the accessibility audit in Adobe Acrobat always [requires the key to
    // be set][1], even if there are no annotations (perhaps the rationale
    // is that the user can add some). Hence, we check if there is an
    // accessibility validator or one requiring tags to decide whether to
    // force the key.
    //
    // Since forcing `/Tabs S` unconditionally for PDF/A and PDF/UA files
    // targeting PDF 1.4 and below would unconditionally raise a
    // `RequiresNewerPdfVersion(StructureOrderTabbing, _)` error, we also
    // check the target version.
    //
    // [1]: https://helpx.adobe.com/acrobat/using/create-verify-pdf-accessibility.html#TabOrder "Create and verify PDF accessibility (Acrobat Pro): Tab order"
    if (!self.annotations.is_empty()
      || ((sc
        .serialize_settings()
        .validators()
        .accessibility()
        .is_some()
        || sc.serialize_settings().validators().requires_tagging())
        && sc.serialize_settings().pdf_version()
          >= VersionedFeature::StructureOrderTabbing.minimum_pdf_version()))
      && sc.serialize_settings().enable_tagging
    {
      if sc.serialize_settings().pdf_version()
        >= VersionedFeature::StructureOrderTabbing.minimum_pdf_version()
      {
        page.tab_order(TabOrder::StructureOrder);
      } else {
        sc.register_validation_error(ValidationError::RequiresNewerPdfVersion(
          VersionedFeature::StructureOrderTabbing,
          sc.location,
        ));
      }
    }

    page.parent(sc.page_tree_ref());
    page.contents(self.stream_ref);

    if !annotation_refs.is_empty() {
      page.annotations(annotation_refs.iter().map(|(r, _)| *r));
    }

    // Populate the refs for each annotation in page infos.
    sc.page_infos_mut()[self.page_index].set_annotations(annotation_refs);

    page.finish();
    chunk_container.streams.pages.push(self.stream_chunk);

    Ok(())
  }
}
