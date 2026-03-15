use cssparser::{BasicParseErrorKind, ParseError, ParseErrorKind};
use selectors::parser::SelectorParseErrorKind;
use std::borrow::Cow;

use crate::{
  keyframes::KeyframePreludeParseError,
  resources::{font::FontError, image::ImageResourceError},
};
use thiserror::Error;

/// Alias to [`TakumiError`].
pub type Error = TakumiError;

/// Errors raised while parsing a CSS declaration block string.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StyleDeclarationBlockParseError {
  /// The declaration block could not be parsed as CSS declarations.
  #[error("failed to parse CSS declaration block `{input}` near `{context}`: {reason}")]
  InvalidDeclarationBlock {
    /// The original declaration block input.
    input: String,
    /// The declaration slice being parsed when the error was raised.
    context: String,
    /// The parser failure rendered as text.
    reason: String,
  },
}

/// Errors raised while parsing a CSS stylesheet string.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct StyleSheetParseError {
  /// The original stylesheet input.
  pub input: Option<String>,
  /// The stylesheet slice being parsed when the error was raised.
  pub context: Option<String>,
  /// The specific stylesheet parse failure.
  pub kind: StyleSheetParseErrorKind,
}

/// The specific stylesheet parse failure.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StyleSheetParseErrorKind {
  /// The stylesheet could not be parsed as valid CSS.
  #[error("{0}")]
  InvalidStyleSheet(String),

  /// The stylesheet uses an unsupported media feature.
  #[error("unsupported media feature")]
  UnsupportedMediaFeature,

  /// `@property` requires `inherits` to be `true` or `false`.
  #[error("@property inherits must be true or false")]
  PropertyInheritsMustBeBoolean,

  /// `@property` is missing its `syntax` descriptor.
  #[error("missing `@property` syntax")]
  MissingPropertySyntax,

  /// `@property` is missing its `inherits` descriptor.
  #[error("missing `@property` inherits")]
  MissingPropertyInherits,

  /// `@supports` mixed `and` and `or` without parentheses.
  #[error("@supports cannot mix `and` and `or` without parentheses")]
  SupportsMixedAndOrWithoutParentheses,

  /// `@property` names must be custom properties.
  #[error("@property name must be a custom property")]
  PropertyNameMustBeCustomProperty,

  /// `@layer` blocks accept at most one name.
  #[error("@layer blocks accept at most one name")]
  LayerBlockMultipleNames,

  /// Nested `@keyframes` and `@property` rules are not supported.
  #[error("unsupported nested at-rule")]
  UnsupportedNestedAtRule,
}

impl std::fmt::Display for StyleSheetParseError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some(context) = &self.context {
      write!(
        f,
        "failed to parse stylesheet near `{context}`: {}",
        self.kind
      )
    } else {
      write!(f, "failed to parse stylesheet: {}", self.kind)
    }
  }
}

impl std::error::Error for StyleSheetParseError {}

impl<'i> From<SelectorParseErrorKind<'i>> for StyleSheetParseError {
  fn from(err: SelectorParseErrorKind<'i>) -> Self {
    Self::invalid_reason(format!("{err:?}"))
  }
}

impl<'i> From<Cow<'i, str>> for StyleSheetParseError {
  fn from(err: Cow<'i, str>) -> Self {
    Self::invalid_reason(err.into_owned())
  }
}

impl<'i> From<KeyframePreludeParseError<'i>> for StyleSheetParseError {
  fn from(_err: KeyframePreludeParseError<'i>) -> Self {
    Self::invalid_reason(format!("{:?}", BasicParseErrorKind::QualifiedRuleInvalid))
  }
}

impl StyleSheetParseError {
  pub(crate) fn invalid_reason(reason: impl Into<String>) -> Self {
    Self::new(StyleSheetParseErrorKind::InvalidStyleSheet(reason.into()))
  }

  pub(crate) fn unsupported_media_feature() -> Self {
    Self::new(StyleSheetParseErrorKind::UnsupportedMediaFeature)
  }

  pub(crate) fn property_inherits_must_be_boolean() -> Self {
    Self::new(StyleSheetParseErrorKind::PropertyInheritsMustBeBoolean)
  }

  pub(crate) fn missing_property_syntax() -> Self {
    Self::new(StyleSheetParseErrorKind::MissingPropertySyntax)
  }

  pub(crate) fn missing_property_inherits() -> Self {
    Self::new(StyleSheetParseErrorKind::MissingPropertyInherits)
  }

  pub(crate) fn supports_mixed_and_or_without_parentheses() -> Self {
    Self::new(StyleSheetParseErrorKind::SupportsMixedAndOrWithoutParentheses)
  }

  pub(crate) fn property_name_must_be_custom_property() -> Self {
    Self::new(StyleSheetParseErrorKind::PropertyNameMustBeCustomProperty)
  }

  pub(crate) fn layer_block_multiple_names() -> Self {
    Self::new(StyleSheetParseErrorKind::LayerBlockMultipleNames)
  }

  pub(crate) fn unsupported_nested_at_rule() -> Self {
    Self::new(StyleSheetParseErrorKind::UnsupportedNestedAtRule)
  }

  fn new(kind: StyleSheetParseErrorKind) -> Self {
    Self {
      input: None,
      context: None,
      kind,
    }
  }

  fn with_context(self, input: &str, context: &str) -> Self {
    Self {
      input: Some(input.to_owned()),
      context: Some(context.to_owned()),
      kind: self.kind,
    }
  }

  pub(crate) fn from_parse_error(
    input: &str,
    context: &str,
    error: ParseError<'_, StyleSheetParseError>,
  ) -> Self {
    match error.kind {
      ParseErrorKind::Basic(error) => Self::invalid_reason(format!("{error:?}")),
      ParseErrorKind::Custom(error) => error,
    }
    .with_context(input, context)
  }
}

/// Structured errors raised by the WebP encoding and container assembly paths.
#[derive(Error, Debug)]
#[non_exhaustive]
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
#[non_exhaustive]
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
  #[cfg(target_arch = "wasm32")]
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
