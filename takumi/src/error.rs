use crate::resources::{font::FontError, image::ImageResourceError};
use thiserror::Error;

/// Alias to [`TakumiError`].
pub type Error = TakumiError;

/// The main error type for the Takumi crate.
#[derive(Error, Debug)]
pub enum TakumiError {
  /// Error resolving an image resource.
  #[error("Image resolution error: {0}")]
  ImageResolveError(#[from] ImageResourceError),

  /// Standard IO error.
  #[error("IO error: {0}")]
  IoError(#[from] std::io::Error),

  /// Error encoding a PNG image.
  #[error("PNG encoding error: {0}")]
  PngError(#[from] png::EncodingError),

  /// Error encoding a WebP image.
  #[error("WebP encoding error: {0}")]
  WebPEncodingError(#[from] image_webp::EncodingError),

  /// Generic image processing error.
  #[error("Image error: {0}")]
  ImageError(#[from] image::ImageError),

  /// Invalid viewport dimensions (e.g., width or height is 0).
  #[error("Invalid viewport: width or height cannot be 0")]
  InvalidViewport,

  /// Error related to font processing.
  #[error("Font error: {0}")]
  FontError(#[from] FontError),

  /// Error during layout computation.
  #[error("Layout error: {0}")]
  LayoutError(taffy::TaffyError),
}

impl From<taffy::TaffyError> for TakumiError {
  fn from(err: taffy::TaffyError) -> Self {
    Self::LayoutError(err)
  }
}

/// A specialized Result type for Takumi operations.
pub type Result<T> = std::result::Result<T, TakumiError>;
