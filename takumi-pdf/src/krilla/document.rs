//! Creating new PDF documents.
//!
//! When using krilla, the starting point is always the creation of a [`Document`]. A document
//! represents _one_ PDF document, to which you can add pages or configure them in any
//! other way you want.
//!
//! Unfortunately, creating PDFs always requires some kind of global state to keep track
//! of different aspects in the creation process, meaning that it is not possible to
//! generate multiple pages at the same time. Instead, you need to add pages separately
//! by calling the [`Document::start_page`] method, which returns a new [`Page`] object that mutably
//! borrows the global state from the document. Once the page is dropped, the global
//! state is passed back to the original document, which you can then use to add even
//! more pages.
//!
//! [`Page`]: Page

use pdf_writer::{Filter, XRefFilter};

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::configure::PdfVersion;
use crate::krilla::error::KrillaResult;
use crate::krilla::interchange::embed::EmbeddedFile;
use crate::krilla::interchange::metadata::Metadata;
use crate::krilla::interchange::outline::Outline;
use crate::krilla::interchange::tagging::TagTree;
use crate::krilla::page::{Page, PageSettings};
use crate::krilla::serialize::{SerializeContext, SerializeSettings};
use crate::krilla::stream::deflate_encode;

/// A PDF document.
pub struct Document {
  pub(crate) serializer_context: SerializeContext,
  pub(crate) chunk_container: ChunkContainer,
}

impl Default for Document {
  fn default() -> Self {
    Self::new()
  }
}

impl Document {
  /// Create a new document with default serialize settings.
  pub fn new() -> Self {
    let serializer_context = SerializeContext::new(SerializeSettings::default());
    let chunk_container = ChunkContainer::new(&serializer_context);

    Self {
      serializer_context,
      chunk_container,
    }
  }

  /// Create a new document with custom serialize settings.
  pub fn new_with(serialize_settings: SerializeSettings) -> Self {
    let serializer_context = SerializeContext::new(serialize_settings);
    let chunk_container = ChunkContainer::new(&serializer_context);

    Self {
      serializer_context,
      chunk_container,
    }
  }

  /// Start a new page with default settings.
  pub fn start_page(&mut self) -> Page<'_> {
    let page_index = self.serializer_context.page_infos().iter().len();
    Page::new(
      &mut self.serializer_context,
      &mut self.chunk_container,
      page_index,
      PageSettings::default(),
    )
  }

  /// Start a new page with specific page settings.
  pub fn start_page_with(&mut self, page_settings: PageSettings) -> Page<'_> {
    let page_index = self.serializer_context.page_infos().iter().len();
    Page::new(
      &mut self.serializer_context,
      &mut self.chunk_container,
      page_index,
      page_settings,
    )
  }

  /// Set the outline of the document.
  pub fn set_outline(&mut self, outline: Outline) {
    self.serializer_context.set_outline(outline);
  }

  /// Set the metadata of the document.
  pub fn set_metadata(&mut self, metadata: Metadata) {
    self.chunk_container.metadata = Some(metadata);
  }

  /// Set the tag tree of the document.
  pub fn set_tag_tree(&mut self, tag_tree: TagTree) {
    self.serializer_context.set_tag_tree(tag_tree);
  }

  /// Embed a new file in the PDF document.
  ///
  /// Returns `None` if the file couldn't be embedded because a file
  /// with the same name has already been embedded.
  pub fn embed_file(&mut self, file: EmbeddedFile) -> Option<()> {
    self
      .serializer_context
      .embed_file(&mut self.chunk_container, file)
  }

  /// Attempt to export the document to a PDF file.
  pub fn finish(mut self) -> KrillaResult<Vec<u8>> {
    // Write empty page if none has been created yet.
    if self.serializer_context.page_infos().is_empty() {
      self.start_page();
    }

    let Self {
      serializer_context,
      chunk_container,
    } = self;

    let version = serializer_context.serialize_settings().pdf_version();
    let (pdf, xref_ref, object_stream) = serializer_context.finish(chunk_container)?;

    // Cross-reference streams compress the per-object xref rows; they exist
    // from PDF 1.5 onwards.
    if version >= PdfVersion::Pdf15 {
      // The xref stream is the last object, so its number is the row count.
      let xref_len = xref_ref.get() as usize + 1;

      Ok(pdf.finish_with_xref_stream_and_filter(xref_ref, |data| {
        let mut rows = data.to_vec();

        if let Some(object_stream) = &object_stream {
          object_stream.patch_xref(&mut rows, xref_len);
        }

        (
          deflate_encode(&rows),
          XRefFilter::Single(Filter::FlateDecode),
        )
      }))
    } else {
      Ok(pdf.finish())
    }
  }
}
