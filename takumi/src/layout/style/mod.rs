#[cfg(feature = "css_stylesheet_parsing")]
pub(crate) mod matching;
mod properties;
#[cfg(feature = "css_stylesheet_parsing")]
pub(crate) mod selector;
mod stylesheets;
/// Tailwind CSS Parser.
pub mod tw;

use std::borrow::Cow;

use cssparser::match_ignore_ascii_case;
pub use properties::*;
use serde::{
  Deserialize,
  de::{self, DeserializeSeed, Deserializer, Expected, IgnoredAny, MapAccess, SeqAccess, Visitor},
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

impl<T, const DEFAULT_INHERIT: bool> From<T> for CssValue<Box<T>, DEFAULT_INHERIT> {
  fn from(value: T) -> Self {
    CssValue::Value(Box::new(value))
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

pub(super) enum RawCssNumber {
  Signed(i64),
  Unsigned(u64),
  Float(f64),
}

pub(super) enum RawCssUnexpected {
  Bool(bool),
  Char(char),
  Bytes,
  Unit,
  Seq,
  Map,
  Other(&'static str),
}

impl RawCssUnexpected {
  fn as_serde_unexpected(&self) -> de::Unexpected<'_> {
    match self {
      Self::Bool(value) => de::Unexpected::Bool(*value),
      Self::Char(value) => de::Unexpected::Char(*value),
      Self::Bytes => de::Unexpected::Other("bytes"),
      Self::Unit => de::Unexpected::Unit,
      Self::Seq => de::Unexpected::Seq,
      Self::Map => de::Unexpected::Map,
      Self::Other(kind) => de::Unexpected::Other(kind),
    }
  }
}

pub(super) enum RawCssInput<'a> {
  Str(Cow<'a, str>),
  Number(RawCssNumber),
  Unexpected(RawCssUnexpected),
}

struct RawCssInputVisitor;

impl RawCssInputVisitor {
  fn drain_seq<'de, A>(mut seq: A) -> Result<(), A::Error>
  where
    A: SeqAccess<'de>,
  {
    while seq.next_element::<IgnoredAny>()?.is_some() {}
    Ok(())
  }

  fn drain_map<'de, A>(mut map: A) -> Result<(), A::Error>
  where
    A: MapAccess<'de>,
  {
    while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
    Ok(())
  }
}

impl<'de> Visitor<'de> for RawCssInputVisitor {
  type Value = RawCssInput<'de>;

  fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
    formatter.write_str("a CSS string or number")
  }

  fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(RawCssInput::Str(Cow::Borrowed(value)))
  }

  fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(RawCssInput::Str(Cow::Owned(value.to_owned())))
  }

  fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(RawCssInput::Str(Cow::Owned(value)))
  }

  fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(RawCssInput::Number(RawCssNumber::Signed(value)))
  }

  fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(RawCssInput::Number(RawCssNumber::Unsigned(value)))
  }

  fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(RawCssInput::Number(RawCssNumber::Float(value)))
  }

  fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(RawCssInput::Unexpected(RawCssUnexpected::Bool(value)))
  }

  fn visit_char<E>(self, value: char) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(RawCssInput::Unexpected(RawCssUnexpected::Char(value)))
  }

  fn visit_bytes<E>(self, _value: &[u8]) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(RawCssInput::Unexpected(RawCssUnexpected::Bytes))
  }

  fn visit_byte_buf<E>(self, _value: Vec<u8>) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(RawCssInput::Unexpected(RawCssUnexpected::Bytes))
  }

  fn visit_unit<E>(self) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(RawCssInput::Unexpected(RawCssUnexpected::Unit))
  }

  fn visit_none<E>(self) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(RawCssInput::Unexpected(RawCssUnexpected::Other("null")))
  }

  fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_any(self)
  }

  fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
  where
    A: SeqAccess<'de>,
  {
    Self::drain_seq(seq)?;
    Ok(RawCssInput::Unexpected(RawCssUnexpected::Seq))
  }

  fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
  where
    A: MapAccess<'de>,
  {
    Self::drain_map(map)?;
    Ok(RawCssInput::Unexpected(RawCssUnexpected::Map))
  }
}

pub(super) struct RawCssValueSeed;

impl<'de> DeserializeSeed<'de> for RawCssValueSeed {
  type Value = RawCssInput<'de>;

  fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_any(RawCssInputVisitor)
  }
}

struct CssExpectedMessage<'a> {
  #[cfg(feature = "detailed_css_error")]
  message: Cow<'a, str>,
}

impl Expected for CssExpectedMessage<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    #[cfg(feature = "detailed_css_error")]
    {
      write!(f, "{}; also accepts 'initial' or 'inherit'.", self.message)
    }

    #[cfg(not(feature = "detailed_css_error"))]
    {
      write!(
        f,
        "CSS value, compile with --features detailed_css_error for more details"
      )
    }
  }
}

#[cold]
#[inline(never)]
fn css_expected_message<'a, T>() -> CssExpectedMessage<'a>
where
  T: for<'i> FromCss<'i>,
{
  #[cfg(feature = "detailed_css_error")]
  {
    CssExpectedMessage {
      message: T::expect_message(),
    }
  }

  #[cfg(not(feature = "detailed_css_error"))]
  {
    let _ = std::marker::PhantomData::<T>;
    CssExpectedMessage {}
  }
}

#[cold]
#[inline(never)]
fn css_invalid_type<T, E, R>(unexpected: RawCssUnexpected) -> Result<R, E>
where
  T: for<'i> FromCss<'i>,
  E: de::Error,
{
  let expected = css_expected_message::<T>();
  Err(E::invalid_type(unexpected.as_serde_unexpected(), &expected))
}

#[cold]
#[inline(never)]
fn css_invalid_string<T, E, R>(value: &str) -> Result<R, E>
where
  T: for<'i> FromCss<'i>,
  E: de::Error,
{
  let expected = css_expected_message::<T>();
  Err(E::invalid_value(de::Unexpected::Str(value), &expected))
}

#[cold]
#[inline(never)]
fn css_invalid_number<T, E, R>(number: &RawCssNumber) -> Result<R, E>
where
  T: for<'i> FromCss<'i>,
  E: de::Error,
{
  let expected = css_expected_message::<T>();
  let unexpected = match number {
    RawCssNumber::Signed(value) => de::Unexpected::Signed(*value),
    RawCssNumber::Unsigned(value) => de::Unexpected::Unsigned(*value),
    RawCssNumber::Float(value) => de::Unexpected::Float(*value),
  };
  Err(E::invalid_type(unexpected, &expected))
}

pub(super) fn parse_css_value_from_raw<'de, T, E, const DEFAULT_INHERIT: bool>(
  raw: RawCssInput<'de>,
) -> Result<CssValue<T, DEFAULT_INHERIT>, E>
where
  T: for<'i> FromCss<'i>,
  E: de::Error,
{
  match raw {
    RawCssInput::Str(value) => match_ignore_ascii_case! {value.as_ref(),
      "initial" => Ok(CssValue::Initial),
      "inherit" => Ok(CssValue::Inherit),
      "unset" => Ok(CssValue::Unset),
      _ => match T::from_str(value.as_ref()) {
        Ok(parsed) => Ok(CssValue::Value(parsed)),
        Err(_) => css_invalid_string::<T, E, CssValue<T, DEFAULT_INHERIT>>(value.as_ref()),
      },
    },
    RawCssInput::Number(number) => {
      let source = match &number {
        RawCssNumber::Signed(value) => value.to_string(),
        RawCssNumber::Unsigned(value) => value.to_string(),
        RawCssNumber::Float(value) => value.to_string(),
      };
      match T::from_str(&source) {
        Ok(parsed) => Ok(CssValue::Value(parsed)),
        Err(_) => css_invalid_number::<T, E, CssValue<T, DEFAULT_INHERIT>>(&number),
      }
    }
    RawCssInput::Unexpected(unexpected) => {
      css_invalid_type::<T, E, CssValue<T, DEFAULT_INHERIT>>(unexpected)
    }
  }
}
