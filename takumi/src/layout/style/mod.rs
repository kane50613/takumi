#[cfg(feature = "css_stylesheet_parsing")]
pub(crate) mod matching;
mod properties;
#[cfg(feature = "css_stylesheet_parsing")]
pub(crate) mod selector;
mod stylesheets;
/// Tailwind CSS Parser.
pub mod tw;

use std::marker::PhantomData;

use cssparser::match_ignore_ascii_case;
pub use properties::*;
use serde::{
  Deserialize, Deserializer,
  de::{self, Visitor},
};
pub use stylesheets::*;

/// Represents a CSS property value that can be explicitly set, inherited from parent, or reset to initial value.
#[derive(Default, Clone, Debug, PartialEq)]
pub enum CssValue<T, const DEFAULT_INHERIT: bool = false> {
  /// Property was not set by the user
  #[default]
  Unset,
  /// Use the initial value of the property
  Initial,
  /// Inherit the computed value from the parent element
  Inherit,
  /// Explicit value set on the element
  Value(T),
}

// Visitor for CssValue<T>
struct CssValueVisitor<T, const DEFAULT_INHERIT: bool> {
  _marker: PhantomData<T>,
}

impl<T, const DEFAULT_INHERIT: bool> CssValueVisitor<T, DEFAULT_INHERIT> {
  fn new() -> Self {
    Self {
      _marker: PhantomData,
    }
  }
}

impl<'de, T: for<'i> FromCss<'i>, const DEFAULT_INHERIT: bool> Visitor<'de>
  for CssValueVisitor<T, DEFAULT_INHERIT>
{
  type Value = CssValue<T, DEFAULT_INHERIT>;

  fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    #[cfg(feature = "detailed_css_error")]
    {
      write!(
        f,
        "{}; also accepts 'initial' or 'inherit'.",
        T::expect_message()
      )
    }

    #[cfg(not(feature = "detailed_css_error"))]
    {
      write!(
        f,
        "CSS value, compile with --features detailed_css_error for more details"
      )
    }
  }

  fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    match_ignore_ascii_case! {value,
      "initial" => Ok(CssValue::Initial),
      "inherit" => Ok(CssValue::Inherit),
      "unset" => Ok(CssValue::Unset),
      _ => T::from_str(value)
        .map(CssValue::Value)
        .map_err(|_| E::invalid_value(de::Unexpected::Str(value), &self)),
    }
  }

  fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    T::from_str(&value.to_string())
      .map(CssValue::Value)
      .map_err(|_| E::invalid_type(de::Unexpected::Signed(value), &self))
  }

  fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    T::from_str(&value.to_string())
      .map(CssValue::Value)
      .map_err(|_| E::invalid_type(de::Unexpected::Unsigned(value), &self))
  }

  fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    T::from_str(&value.to_string())
      .map(CssValue::Value)
      .map_err(|_| E::invalid_type(de::Unexpected::Float(value), &self))
  }
}

impl<'de, T: for<'i> FromCss<'i>, const DEFAULT_INHERIT: bool> Deserialize<'de>
  for CssValue<T, DEFAULT_INHERIT>
{
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_any(CssValueVisitor::new())
  }
}

impl<T, const DEFAULT_INHERIT: bool> From<T> for CssValue<T, DEFAULT_INHERIT> {
  fn from(value: T) -> Self {
    CssValue::Value(value)
  }
}

impl<T, const DEFAULT_INHERIT: bool> From<T> for CssValue<Option<T>, DEFAULT_INHERIT> {
  fn from(value: T) -> Self {
    CssValue::Value(Some(value))
  }
}

impl<T, const N: usize, const DEFAULT_INHERIT: bool> From<[T; N]>
  for CssValue<Box<[T]>, DEFAULT_INHERIT>
{
  fn from(value: [T; N]) -> Self {
    CssValue::Value(Box::from(value))
  }
}

impl<T, const N: usize, const DEFAULT_INHERIT: bool> From<[T; N]>
  for CssValue<Option<Box<[T]>>, DEFAULT_INHERIT>
{
  fn from(value: [T; N]) -> Self {
    CssValue::Value(Some(Box::from(value)))
  }
}

impl<T: Default, const DEFAULT_INHERIT: bool> CssValue<T, DEFAULT_INHERIT> {
  /// Resolves this CssValue to a concrete value based on inheritance rules
  pub(crate) fn inherit_value(self, parent: &T) -> T
  where
    T: Clone,
  {
    match self {
      Self::Value(v) => v,
      Self::Inherit => parent.clone(),
      Self::Initial => T::default(),
      // Unset follows CSS spec: inherit if DEFAULT_INHERIT, otherwise initial
      Self::Unset if DEFAULT_INHERIT => parent.clone(),
      Self::Unset => T::default(),
    }
  }

  /// Returns self if it's not Unset, otherwise returns other.
  /// This is used to merge style layers (e.g., inline style over Tailwind).
  pub(crate) fn or(self, other: Self) -> Self {
    match self {
      Self::Unset => other,
      _ => self,
    }
  }
}

impl<T: Copy, const DEFAULT_INHERIT: bool> Copy for CssValue<T, DEFAULT_INHERIT> {}
