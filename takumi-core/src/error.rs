use std::borrow::Cow;

use cssparser::{BasicParseErrorKind, ParseError, ParseErrorKind};
use selectors::parser::SelectorParseErrorKind;
use thiserror::Error;

use crate::{
  keyframes::KeyframePreludeParseError,
  resources::{font::FontError, image::ImageError},
};

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
  ImageResolveError(#[from] ImageError),

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

  /// Invalid viewport dimensions (e.g., zero-sized or over the pixel budget).
  #[error("Invalid viewport dimensions")]
  InvalidViewport,

  /// RGBA buffer length does not match `width * height * 4`.
  #[error("Invalid RGBA buffer length: expected {expected} bytes, got {actual}")]
  InvalidRgbaBufferLength {
    /// Actual RGBA byte length in the buffer.
    actual: usize,
    /// Expected RGBA byte length from dimensions.
    expected: usize,
  },

  /// Alpha buffer length does not match `width * height`.
  #[error("Invalid alpha buffer length: expected {expected} bytes, got {actual}")]
  InvalidAlphaBufferLength {
    /// Actual alpha byte length in the buffer.
    actual: usize,
    /// Expected alpha byte length from dimensions.
    expected: usize,
  },

  /// Animated encode was requested without any frames.
  #[error("Animation must contain at least one frame")]
  EmptyAnimationFrames,

  /// The requested frame rate is too high for the target format. Above this
  /// ceiling some frames fall to or below the shortest duration decoders honor,
  /// so browsers clamp them to 100ms and playback stalls.
  #[error("Frame rate {fps} fps exceeds the maximum {max_fps} fps for this animation format")]
  AnimationFrameRateTooHigh {
    /// The requested frame rate.
    fps: u32,
    /// The highest frame rate the format encodes without decoder clamping.
    max_fps: u32,
  },

  /// Animated frames for a given format did not all share the same dimensions.
  #[error("Animation frames must share the same dimensions")]
  MixedAnimationFrameDimensions,

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

  /// Parsing a CSS value or property failed.
  #[error("Parse error: {0}")]
  Parse(#[from] crate::style::properties::ParseError),

  /// Computing layout failed.
  #[error("Layout error: {0}")]
  Layout(String),

  /// The layout engine was asked for a node id it does not know.
  #[error("Invalid layout node id: {0}")]
  InvalidLayoutNode(u64),

  /// An invalid BCP-47 language tag was supplied to a node's `lang` attribute.
  #[error("Invalid language tag: {0}")]
  InvalidLanguageTag(String),
}

impl Error {
  /// Wraps an encoder-crate error opaquely so takumi's public API stays
  /// independent of the encoder crates' versions.
  pub fn encode(err: impl std::error::Error + Send + Sync + 'static) -> Self {
    Self::Encode(Box::new(err))
  }
}

/// A specialized Result type for Takumi operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors raised while parsing a CSS declaration block string.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
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
pub struct StyleSheetParseError {
  /// The stylesheet slice being parsed when the error was raised.
  pub context: Option<String>,
  /// The specific stylesheet parse failure.
  pub kind: StyleSheetParseErrorKind,
}

/// The specific stylesheet parse failure.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
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

  /// `@apply` only takes plain utilities, without variants.
  #[error("@apply expects plain utilities without variants")]
  InvalidApplyUtility,

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
  /// Builds an invalid-stylesheet error from a reason string.
  pub(crate) fn invalid_reason(reason: impl Into<String>) -> Self {
    Self::new(StyleSheetParseErrorKind::InvalidStyleSheet(reason.into()))
  }

  /// Error for an unsupported media feature.
  pub(crate) fn unsupported_media_feature() -> Self {
    Self::new(StyleSheetParseErrorKind::UnsupportedMediaFeature)
  }

  /// Error for a non-boolean `@property` `inherits` descriptor.
  pub(crate) fn property_inherits_must_be_boolean() -> Self {
    Self::new(StyleSheetParseErrorKind::PropertyInheritsMustBeBoolean)
  }

  /// Error for a `@property` missing its `syntax` descriptor.
  pub(crate) fn missing_property_syntax() -> Self {
    Self::new(StyleSheetParseErrorKind::MissingPropertySyntax)
  }

  /// Error for a `@property` missing its `inherits` descriptor.
  pub(crate) fn missing_property_inherits() -> Self {
    Self::new(StyleSheetParseErrorKind::MissingPropertyInherits)
  }

  /// Error for `@supports` mixing `and`/`or` without parentheses.
  pub(crate) fn supports_mixed_and_or_without_parentheses() -> Self {
    Self::new(StyleSheetParseErrorKind::SupportsMixedAndOrWithoutParentheses)
  }

  /// Error for a `@property` name that is not a custom property.
  pub(crate) fn property_name_must_be_custom_property() -> Self {
    Self::new(StyleSheetParseErrorKind::PropertyNameMustBeCustomProperty)
  }

  /// Error for an `@layer` block naming more than one layer.
  pub(crate) fn layer_block_multiple_names() -> Self {
    Self::new(StyleSheetParseErrorKind::LayerBlockMultipleNames)
  }

  /// Error for an unsupported nested at-rule.
  pub(crate) fn unsupported_nested_at_rule() -> Self {
    Self::new(StyleSheetParseErrorKind::UnsupportedNestedAtRule)
  }

  /// Error for an `@apply` token that is not a plain utility.
  pub(crate) fn invalid_apply_utility() -> Self {
    Self::new(StyleSheetParseErrorKind::InvalidApplyUtility)
  }

  fn new(kind: StyleSheetParseErrorKind) -> Self {
    Self {
      context: None,
      kind,
    }
  }

  fn with_context(self, context: &str) -> Self {
    Self {
      context: Some(context.to_owned()),
      kind: self.kind,
    }
  }

  /// Converts a `cssparser` parse error into a stylesheet error with context.
  pub(crate) fn from_parse_error(
    context: &str,
    error: ParseError<'_, StyleSheetParseError>,
  ) -> Self {
    match error.kind {
      ParseErrorKind::Basic(error) => Self::invalid_reason(format!("{error:?}")),
      ParseErrorKind::Custom(error) => error,
    }
    .with_context(context)
  }
}
