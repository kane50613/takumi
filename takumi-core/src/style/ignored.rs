//! The declarations and at-rules the style parser drops, as Chromium DevTools strikes them through
//! in the Styles pane instead of logging them.

use std::fmt;

use rustc_hash::FxHashSet;
use serde::{Serialize, Serializer};

use crate::style::PropertyId;

/// Why the style parser dropped a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum IgnoredReason {
  /// Takumi does not support the property.
  UnknownProperty,
  /// The property is supported, but its value does not parse.
  InvalidValue,
  /// Takumi does not implement the at-rule, so it drops the rule with its block.
  UnknownAtRule,
}

impl IgnoredReason {
  /// The reason in kebab case, such as `unknown-property`.
  pub fn as_str(self) -> &'static str {
    match self {
      Self::UnknownProperty => "unknown-property",
      Self::InvalidValue => "invalid-value",
      Self::UnknownAtRule => "unknown-at-rule",
    }
  }
}

impl Serialize for IgnoredReason {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.as_str())
  }
}

/// Where a dropped declaration was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum DeclarationOrigin {
  /// An HTML `style` attribute.
  StyleAttribute,
  /// A style sheet, from the `css` option or a `<style>` element.
  StyleSheet,
  /// A node's `style` object.
  StyleObject,
}

impl DeclarationOrigin {
  /// The origin in kebab case, such as `style-attribute`.
  pub fn as_str(self) -> &'static str {
    match self {
      Self::StyleAttribute => "style-attribute",
      Self::StyleSheet => "style-sheet",
      Self::StyleObject => "style-object",
    }
  }
}

impl Serialize for DeclarationOrigin {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.as_str())
  }
}

/// A declaration or at-rule the style parser dropped.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
pub struct IgnoredDeclaration {
  /// The property as written, or the at-rule's name with its `@`.
  pub property: String,
  /// The value as written, or the at-rule's prelude.
  pub value: String,
  /// Why it was dropped.
  pub reason: IgnoredReason,
  /// Where it was written.
  pub origin: DeclarationOrigin,
}

impl IgnoredDeclaration {
  pub(crate) fn new(
    property: impl Into<String>,
    value: &str,
    reason: IgnoredReason,
    origin: DeclarationOrigin,
  ) -> Self {
    Self {
      property: property.into(),
      value: value.trim().to_owned(),
      reason,
      origin,
    }
  }

  /// The supported property's declaration a lossy parse dropped as `source`, `None` for a rule.
  pub(crate) fn invalid_value(source: &str, origin: DeclarationOrigin) -> Option<Self> {
    let (property, value) = source.split_once(':')?;
    let property = property.trim();

    if PropertyId::from_kebab_case(property) == PropertyId::Ignored || value.contains('{') {
      return None;
    }

    Some(Self::new(
      property,
      value.trim_end().trim_end_matches(';'),
      IgnoredReason::InvalidValue,
      origin,
    ))
  }

  /// Each of `declarations` once, in order.
  pub(crate) fn unique<'a>(declarations: impl IntoIterator<Item = &'a Self>) -> Vec<Self> {
    let mut seen = FxHashSet::default();

    declarations
      .into_iter()
      .filter(|declaration| seen.insert(*declaration))
      .cloned()
      .collect()
  }
}

impl fmt::Display for IgnoredDeclaration {
  /// The declaration as CSS writes it, such as `filter: blurry(4px)` or `@container (width > 0)`.
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.reason {
      IgnoredReason::UnknownAtRule if self.value.is_empty() => f.write_str(&self.property),
      IgnoredReason::UnknownAtRule => write!(f, "{} {}", self.property, self.value),
      IgnoredReason::UnknownProperty | IgnoredReason::InvalidValue => {
        write!(f, "{}: {}", self.property, self.value)
      }
    }
  }
}
