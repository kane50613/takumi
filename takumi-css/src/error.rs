use cssparser::{BasicParseErrorKind, ParseError, ParseErrorKind};
use selectors::parser::SelectorParseErrorKind;
use std::borrow::Cow;
use thiserror::Error;

use crate::keyframes::KeyframePreludeParseError;

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
  pub fn invalid_reason(reason: impl Into<String>) -> Self {
    Self::new(StyleSheetParseErrorKind::InvalidStyleSheet(reason.into()))
  }

  /// Error for an unsupported media feature.
  pub fn unsupported_media_feature() -> Self {
    Self::new(StyleSheetParseErrorKind::UnsupportedMediaFeature)
  }

  /// Error for a non-boolean `@property` `inherits` descriptor.
  pub fn property_inherits_must_be_boolean() -> Self {
    Self::new(StyleSheetParseErrorKind::PropertyInheritsMustBeBoolean)
  }

  /// Error for a `@property` missing its `syntax` descriptor.
  pub fn missing_property_syntax() -> Self {
    Self::new(StyleSheetParseErrorKind::MissingPropertySyntax)
  }

  /// Error for a `@property` missing its `inherits` descriptor.
  pub fn missing_property_inherits() -> Self {
    Self::new(StyleSheetParseErrorKind::MissingPropertyInherits)
  }

  /// Error for `@supports` mixing `and`/`or` without parentheses.
  pub fn supports_mixed_and_or_without_parentheses() -> Self {
    Self::new(StyleSheetParseErrorKind::SupportsMixedAndOrWithoutParentheses)
  }

  /// Error for a `@property` name that is not a custom property.
  pub fn property_name_must_be_custom_property() -> Self {
    Self::new(StyleSheetParseErrorKind::PropertyNameMustBeCustomProperty)
  }

  /// Error for an `@layer` block naming more than one layer.
  pub fn layer_block_multiple_names() -> Self {
    Self::new(StyleSheetParseErrorKind::LayerBlockMultipleNames)
  }

  /// Error for an unsupported nested at-rule.
  pub fn unsupported_nested_at_rule() -> Self {
    Self::new(StyleSheetParseErrorKind::UnsupportedNestedAtRule)
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
  pub fn from_parse_error(context: &str, error: ParseError<'_, StyleSheetParseError>) -> Self {
    match error.kind {
      ParseErrorKind::Basic(error) => Self::invalid_reason(format!("{error:?}")),
      ParseErrorKind::Custom(error) => error,
    }
    .with_context(context)
  }
}
