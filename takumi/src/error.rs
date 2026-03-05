use crate::resources::{font::FontError, image::ImageResourceError};
use thiserror::Error;

/// Alias to [`TakumiError`].
pub type Error = TakumiError;

/// Structured errors raised by the WebP encoding and container assembly paths.
#[derive(Error, Debug)]
pub enum WebPError {
  /// The encoder config could not be constructed.
  #[error("failed to construct WebP config")]
  ConfigConstruction,

  /// The constructed encoder config failed validation.
  #[error("invalid WebP config")]
  InvalidConfig,

  /// A `WebPPicture` could not be initialized.
  #[error("failed to initialize WebP picture")]
  PictureInitialization,

  /// Importing RGBA pixel data into a `WebPPicture` failed.
  #[error("WebP import error: {error_code}")]
  Import {
    /// The libwebp error code rendered as text.
    error_code: String,
  },

  /// Encoding failed without a more specific libwebp error code.
  #[error("WebP encode error")]
  Encode,

  /// Encoding failed with a libwebp error code.
  #[error("WebP encode error: {error_code}")]
  EncodeWithCode {
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

  /// The encoded RIFF container did not contain a VP8 or VP8L chunk.
  #[error("VP8/VP8L chunk not found")]
  MissingVp8Chunk,

  /// The encoded frame blob did not contain a VP8 or VP8L chunk.
  #[error("VP8/VP8L chunk not found in encoded frame")]
  MissingVp8ChunkInEncodedFrame,

  /// The VP8 or VP8L chunk tag could not be read from the buffer.
  #[error("missing VP8/VP8L chunk tag")]
  MissingVp8ChunkTag,

  /// The VP8 or VP8L chunk tag bytes were malformed.
  #[error("invalid VP8/VP8L chunk tag")]
  InvalidVp8ChunkTag,

  /// The VP8 or VP8L payload length did not fit in `u32`.
  #[error("VP8/VP8L payload size overflows u32")]
  Vp8PayloadSizeOverflow,

  /// The VP8 or VP8L chunk padding length did not fit in `u32`.
  #[error("VP8/VP8L padding size overflows u32")]
  Vp8PaddingSizeOverflow,

  /// Computing the estimated VP8 or VP8L payload size overflowed.
  #[error("estimated VP8/VP8L payload size overflow")]
  EstimatedVp8PayloadSizeOverflow,

  /// Computing the estimated RIFF size overflowed.
  #[error("estimated RIFF size overflow")]
  EstimatedRiffSizeOverflow,

  /// Computing the RIFF payload size overflowed `usize`.
  #[error("RIFF payload size overflow")]
  RiffPayloadSizeOverflow,

  /// The RIFF payload size did not fit in `u32`.
  #[error("RIFF payload size overflows u32")]
  RiffPayloadSizeTooLarge,

  /// Computing the ANMF chunk size overflowed.
  #[error("ANMF chunk size overflow")]
  AnmfChunkSizeOverflow,

  /// Computing the ANMF payload size overflowed `usize`.
  #[error("ANMF payload size overflow")]
  AnmfPayloadSizeOverflow,

  /// The ANMF payload size did not fit in `u32`.
  #[error("ANMF payload size overflows u32")]
  AnmfPayloadSizeTooLarge,

  /// The VP8 payload size did not fit in `u32`.
  #[error("VP8 payload size overflows u32")]
  Vp8PayloadSizeTooLarge,
}

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

  /// Structured errors from WebP encoding and RIFF container assembly.
  #[error("WebP error: {0}")]
  WebPError(#[from] WebPError),

  /// Error encoding a GIF image.
  #[error("GIF encoding error: {0}")]
  GifEncodingError(#[from] gif::EncodingError),

  /// Generic image processing error.
  #[error("Image error: {0}")]
  ImageError(#[from] image::ImageError),

  /// Invalid viewport dimensions (e.g., width or height is 0).
  #[error("Invalid viewport: width or height cannot be 0")]
  InvalidViewport,

  /// Animated encode was requested without any frames.
  #[error("{format} animation must contain at least one frame")]
  EmptyAnimationFrames {
    /// The animation format used in the error message.
    format: &'static str,
  },

  /// Animated frames for a given format did not all share the same dimensions.
  #[error("all {format} animation frames must share the same dimensions")]
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
