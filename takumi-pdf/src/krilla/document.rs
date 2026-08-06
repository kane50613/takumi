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

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::destination::NamedDestination;
use crate::krilla::error::KrillaResult;
use crate::krilla::interchange::embed::EmbeddedFile;
use crate::krilla::interchange::metadata::Metadata;
use crate::krilla::interchange::outline::Outline;
use crate::krilla::page::{Page, PageSettings};
use crate::krilla::serialize::{SerializeContext, SerializeSettings};
use crate::krilla::surface::Location;

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

  /// Set the location that should be assumed for subsequent operations.
  pub fn set_location(&mut self, location: Location) {
    self.serializer_context.set_location(location);
  }

  /// Reset the location that should be assumed for subsequent operations.
  pub fn reset_location(&mut self) {
    self.serializer_context.reset_location();
  }

  /// Set the outline of the document.
  pub fn set_outline(&mut self, outline: Outline) {
    self.serializer_context.set_outline(outline);
  }

  /// Set the metadata of the document.
  pub fn set_metadata(&mut self, metadata: Metadata) {
    self.chunk_container.metadata = Some(metadata);
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

  /// Manually register a global named destination.
  ///
  /// Named destinations used in link annotations are automatically registered, so you don't need
  /// to call this function for them.
  ///
  /// Returns `None` if a named destination with the same name and a different destination has
  /// already been registered, and therefore could not be registered again.
  #[must_use]
  pub fn register_named_destination(&mut self, dest: NamedDestination) -> Option<()> {
    self
      .serializer_context
      .register_named_destination(dest)
      .map(|_| ())
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

    Ok(serializer_context.finish(chunk_container)?.finish())
  }
}
