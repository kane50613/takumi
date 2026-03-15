mod animation;
pub(crate) mod matching;
mod properties;
mod selector;
mod stylesheets;
pub(crate) mod tw;

use std::{borrow::Cow, fmt::Formatter};

pub(crate) use animation::apply_stylesheet_animations;
pub use animation::{KeyframeRule, KeyframesRule};
pub use properties::*;
pub use selector::*;
use serde::{
  Deserialize,
  de::{self, DeserializeSeed, Deserializer, Expected, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
pub use stylesheets::*;

#[derive(Clone, Copy)]
pub(super) enum RawCssNumber {
  Signed(i64),
  Unsigned(u64),
  Float(f64),
}

impl std::fmt::Display for RawCssNumber {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      RawCssNumber::Signed(value) => value.fmt(f),
      RawCssNumber::Unsigned(value) => value.fmt(f),
      RawCssNumber::Float(value) => value.fmt(f),
    }
  }
}

#[derive(Clone, Copy)]
pub(super) enum RawCssUnexpected {
  Bool(bool),
  Char(char),
  Bytes,
  Unit,
  Seq,
  Map,
  Other(&'static str),
}

#[derive(Clone)]
pub(super) enum RawCssInput<'a> {
  Str(Cow<'a, str>),
  Number(RawCssNumber),
  Unexpected(RawCssUnexpected),
}

impl std::fmt::Display for RawCssInput<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Str(value) => f.write_str(value),
      Self::Number(number) => number.fmt(f),
      Self::Unexpected(_) => Ok(()),
    }
  }
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

  fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
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
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    #[cfg(feature = "detailed_css_error")]
    {
      write!(
        f,
        "{}; also accepts 'initial', 'unset' or 'inherit'.",
        self.message
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
}

impl std::fmt::Display for CssExpectedMessage<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    Expected::fmt(self, f)
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
