//! Error handling.
//!
//! There are a lot of things that can go wrong when writing a PDF, like for example when
//! invalid fonts are provided. This module provides the basic error types the writer uses.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::writer::configure::{ValidationError, Validators};
#[cfg(feature = "images")]
use crate::writer::graphics::image::Image;
use crate::writer::surface::Location;
use crate::writer::tagging::TagId;
use crate::writer::text::Font;

/// A wrapper type for writer errors.
pub type WriterResult<T> = Result<T, WriterError>;

/// An error in the writer.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum WriterError {
  /// An error while attempting to embed a font.
  Font(Font, String),
  /// A list of validation errors. Can only occur if you set the `validator` in
  /// the [`SerializeSettings`] to something else than the dummy validator.
  ///
  /// [`SerializeSettings`]: crate::writer::SerializeSettings
  Validation(Vec<(ValidationError, Validators)>),
  /// A hard limit of the selected PDF version was exceeded.
  Limit(LimitError),
  /// A duplicate [`Tag::id`] was provided.
  ///
  /// [`Tag::id`]: crate::writer::interchange::tagging::Tag::id
  DuplicateTagId(TagId, Option<Location>),
  /// The tag tree references content that does not exist, references it
  /// twice, or omits recorded content.
  ///
  /// [`TagTree`]: crate::writer::interchange::tagging::TagTree
  TagTree(String),
  /// An image couldn't be processed properly.
  ///
  /// The third argument contains the error message.
  #[cfg(feature = "images")]
  Image(Image, Option<Location>, String),
  /// A sixteen bit image was used, even though it isn't
  /// supported by the used PDF version (only available in PDF 1.5+).
  #[cfg(feature = "images")]
  SixteenBitImage(Image, Option<Location>),
}

impl Display for WriterError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      WriterError::Font(_, message) => write!(f, "failed to embed font: {message}"),
      WriterError::Validation(errors) => {
        let count = errors.len();
        write!(
          f,
          "validation failed with {count} {}",
          if count == 1 { "error" } else { "errors" }
        )
      }
      WriterError::Limit(error) => write!(f, "PDF version limit exceeded: {error}"),
      WriterError::DuplicateTagId(id, location) => {
        write!(
          f,
          "duplicate tag id {}",
          String::from_utf8_lossy(id.as_bytes())
        )?;
        write_location(f, *location)
      }
      WriterError::TagTree(message) => write!(f, "invalid tag tree: {message}"),
      #[cfg(feature = "images")]
      WriterError::Image(_, location, message) => {
        write!(f, "failed to process image")?;
        write_location(f, *location)?;
        write!(f, ": {message}")
      }
      #[cfg(feature = "images")]
      WriterError::SixteenBitImage(_, location) => {
        write!(
          f,
          "sixteen bit images require PDF 1.5 or newer, but the selected PDF version does not support them"
        )?;
        write_location(f, *location)
      }
    }
  }
}

impl Error for WriterError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      WriterError::Limit(error) => Some(error),
      _ => None,
    }
  }
}

/// A limit imposed by the selected PDF version.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LimitError {
  /// A float exceeded the maximum allowed size for the PDF version.
  TooLargeFloat,
  /// An array exceeded the maximum allowed length for the PDF version.
  TooLongArray,
  /// A dictionary exceeded the maximum allowed number of entries for the PDF version.
  TooLongDictionary,
}

impl Display for LimitError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      LimitError::TooLargeFloat => write!(f, "a float exceeded the maximum allowed size"),
      LimitError::TooLongArray => write!(f, "an array exceeded the maximum allowed length"),
      LimitError::TooLongDictionary => {
        write!(
          f,
          "a dictionary exceeded the maximum allowed number of entries"
        )
      }
    }
  }
}

impl Error for LimitError {}

fn write_location(f: &mut Formatter<'_>, location: Option<Location>) -> fmt::Result {
  if let Some(location) = location {
    write!(f, " at location {location}")
  } else {
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use std::error::Error as _;

  use super::{LimitError, WriterError};

  #[test]
  fn writer_error_implements_error() {
    let error = WriterError::Limit(LimitError::TooLongArray);

    assert_eq!(
      error.to_string(),
      "PDF version limit exceeded: an array exceeded the maximum allowed length"
    );
    assert!(error.source().is_some());
  }
}
