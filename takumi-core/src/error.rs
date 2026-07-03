use thiserror::Error;

use crate::resources::{font::FontError, image::ImageResourceError};

/// Structured errors raised by the WebP encoding and container assembly paths.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum WebPError {
  /// Encoder setup failed before actual frame encoding.
  #[error("WebP encoder setup failed")]
  EncoderSetupFailed,

  /// Encoding failed.
  #[error("WebP encode failed")]
  EncodeFailed,

  /// Encoding failed with a libwebp error code.
  #[error("WebP encode failed ({error_code})")]
  EncodeFailedWithCode {
    /// The libwebp error code rendered as text.
    error_code: String,
  },

  /// A named dimension exceeded the supported WebP 24-bit range.
  #[error("{name} must be in 1..={max}, got {value}")]
  InvalidDimension {
    /// The dimension name used in the error message.
    name: &'static str,
    /// The invalid dimension value that was supplied.
    value: u32,
    /// The maximum accepted dimension value.
    max: u32,
  },

  /// The animation frame size exceeded the supported WebP 24-bit range.
  #[error("WebP animation frame dimensions must be in 1..={max}, got {width}x{height}")]
  InvalidFrameDimensions {
    /// The invalid frame width.
    width: u32,
    /// The invalid frame height.
    height: u32,
    /// The maximum accepted dimension value.
    max: u32,
  },

  /// An animated encode was requested without any frames.
  #[error("animation must contain at least one frame")]
  EmptyAnimation,

  /// A frame exceeded the dimensions of the animation canvas.
  #[error(
    "frame {index} dimensions {frame_width}x{frame_height} exceed canvas {canvas_width}x{canvas_height}"
  )]
  FrameExceedsCanvas {
    /// The zero-based frame index.
    index: usize,
    /// The frame width.
    frame_width: u32,
    /// The frame height.
    frame_height: u32,
    /// The canvas width.
    canvas_width: u32,
    /// The canvas height.
    canvas_height: u32,
  },

  /// Animated frames did not all share the same dimensions.
  #[error("all animation frames must have the same dimensions")]
  MixedFrameDimensions,

  /// Encoded data cannot be parsed as the expected WebP structure.
  #[error("WebP encoded data is invalid or unsupported")]
  InvalidEncodedData,

  /// Internal WebP container size calculations exceeded supported limits.
  #[error("WebP container size exceeds supported limits")]
  ContainerSizeOverflow,
}

/// The main error type for the Takumi crate.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
  /// Error resolving an image resource.
  #[error("Image resolution error: {0}")]
  ImageResolveError(#[from] ImageResourceError),

  /// Standard IO error.
  #[error("IO error: {0}")]
  IoError(#[from] std::io::Error),

  /// Encoding an image or animation failed.
  ///
  /// Wraps the underlying encoder error opaquely so takumi's public API stays
  /// independent of the encoder crates' versions.
  #[error("Encoding error: {0}")]
  Encode(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),

  /// Structured errors from WebP encoding and RIFF container assembly.
  #[error("WebP error: {0}")]
  WebPError(#[from] WebPError),

  /// Invalid viewport dimensions (e.g., width or height is 0).
  #[error("Invalid viewport: width or height cannot be 0")]
  InvalidViewport,

  /// RGBA buffer length does not match `width * height * 4`.
  #[error("Invalid RGBA buffer length: expected {expected} bytes, got {actual}")]
  InvalidRgbaBufferLength {
    /// Actual RGBA byte length in the buffer.
    actual: usize,
    /// Expected RGBA byte length from dimensions.
    expected: usize,
  },

  /// Animated encode was requested without any frames.
  #[error("{format} animation must contain at least one frame")]
  EmptyAnimationFrames {
    /// The animation format used in the error message.
    format: &'static str,
  },

  /// Animated frames for a given format did not all share the same dimensions.
  #[error("All {format} animation frames must share the same dimensions")]
  MixedAnimationFrameDimensions {
    /// The animation format used in the error message.
    format: &'static str,
  },

  /// GIF frame dimensions exceeded the format limits.
  #[error("GIF frame dimensions must be <= {max}x{max}, got {width}x{height}")]
  GifFrameDimensionsTooLarge {
    /// The invalid frame width.
    width: u32,
    /// The invalid frame height.
    height: u32,
    /// The maximum accepted dimension value.
    max: u16,
  },

  /// Error related to font processing.
  #[error("Font error: {0}")]
  FontError(#[from] FontError),

  /// Computing layout failed.
  #[error("Layout error: {0}")]
  Layout(String),
}

impl From<taffy::TaffyError> for Error {
  fn from(err: taffy::TaffyError) -> Self {
    Self::Layout(err.to_string())
  }
}

impl From<png::EncodingError> for Error {
  fn from(err: png::EncodingError) -> Self {
    Self::Encode(Box::new(err))
  }
}

impl From<gif::EncodingError> for Error {
  fn from(err: gif::EncodingError) -> Self {
    Self::Encode(Box::new(err))
  }
}

impl From<image::ImageError> for Error {
  fn from(err: image::ImageError) -> Self {
    Self::Encode(Box::new(err))
  }
}

#[cfg(target_arch = "wasm32")]
impl From<image_webp::EncodingError> for Error {
  fn from(err: image_webp::EncodingError) -> Self {
    Self::Encode(Box::new(err))
  }
}

/// A specialized Result type for Takumi operations.
pub type Result<T> = std::result::Result<T, Error>;
