mod animation;
mod math;
mod properties;
pub mod selector;
mod sizing;
mod stylesheets;
pub mod tw;

use std::{borrow::Cow, fmt::Formatter};

pub use animation::apply_stylesheet_animations;
pub use animation::{KeyframeRule, KeyframesRule};
pub use math::{fast_div_255, fast_div_255_u32};
pub(crate) use properties::unexpected_token;
pub use properties::*;
pub use selector::*;
use serde::{
  Deserialize,
  de::{self, DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
pub use sizing::SizingContext;
pub use stylesheets::*;

#[derive(Clone, Copy)]
pub enum CssNumber {
  Signed(i64),
  Unsigned(u64),
  Float(f64),
}

impl std::fmt::Display for CssNumber {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      CssNumber::Signed(value) => value.fmt(f),
      CssNumber::Unsigned(value) => value.fmt(f),
      CssNumber::Float(value) => value.fmt(f),
    }
  }
}

#[derive(Clone, Copy)]
pub enum CssUnexpected {
  Bool(bool),
  Char(char),
  Bytes,
  Unit,
  Seq,
  Map,
  Other(&'static str),
}

#[derive(Clone)]
pub enum CssInput<'a> {
  Str(Cow<'a, str>),
  Number(CssNumber),
  Unexpected(CssUnexpected),
}

impl std::fmt::Display for CssInput<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Str(value) => f.write_str(value),
      Self::Number(number) => number.fmt(f),
      Self::Unexpected(_) => Ok(()),
    }
  }
}

impl CssInput<'_> {
  pub fn into_string(self) -> String {
    match self {
      Self::Str(value) => value.into_owned(),
      Self::Number(number) => number.to_string(),
      Self::Unexpected(_) => String::new(),
    }
  }
}

struct CssInputVisitor;

impl CssInputVisitor {
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

impl<'de> Visitor<'de> for CssInputVisitor {
  type Value = CssInput<'de>;

  fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
    formatter.write_str("a CSS string or number")
  }

  fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(CssInput::Str(Cow::Borrowed(value)))
  }

  fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(CssInput::Str(Cow::Owned(value.to_owned())))
  }

  fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(CssInput::Str(Cow::Owned(value)))
  }

  fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(CssInput::Number(CssNumber::Signed(value)))
  }

  fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(CssInput::Number(CssNumber::Unsigned(value)))
  }

  fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(CssInput::Number(CssNumber::Float(value)))
  }

  fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(CssInput::Unexpected(CssUnexpected::Bool(value)))
  }

  fn visit_char<E>(self, value: char) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(CssInput::Unexpected(CssUnexpected::Char(value)))
  }

  fn visit_bytes<E>(self, _value: &[u8]) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(CssInput::Unexpected(CssUnexpected::Bytes))
  }

  fn visit_byte_buf<E>(self, _value: Vec<u8>) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(CssInput::Unexpected(CssUnexpected::Bytes))
  }

  fn visit_unit<E>(self) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(CssInput::Unexpected(CssUnexpected::Unit))
  }

  fn visit_none<E>(self) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    Ok(CssInput::Unexpected(CssUnexpected::Other("null")))
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
    Ok(CssInput::Unexpected(CssUnexpected::Seq))
  }

  fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
  where
    A: MapAccess<'de>,
  {
    Self::drain_map(map)?;
    Ok(CssInput::Unexpected(CssUnexpected::Map))
  }
}

pub struct CssValueSeed;

impl<'de> DeserializeSeed<'de> for CssValueSeed {
  type Value = CssInput<'de>;

  fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_any(CssInputVisitor)
  }
}
