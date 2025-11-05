mod properties;
mod stylesheets;

/// Handle Tailwind CSS properties.
pub mod tw;

use std::ops::Deref;

use cssparser::match_ignore_ascii_case;
pub use properties::*;
use serde::{Deserialize, Serialize, de::value::*};
use serde_untagged::UntaggedEnumVisitor;
pub use stylesheets::*;
use ts_rs::TS;

/// Represents a CSS property value that can be explicitly set, inherited from parent, or reset to initial value.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(untagged)]
pub enum CssValue<T: TS> {
  /// Global keyword
  Global(CssGlobalKeyword),
  /// Explicit value set on the element
  Value(T),
}

impl<'de, T: TS + Deserialize<'de>> Deserialize<'de> for CssValue<T> {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    UntaggedEnumVisitor::new()
      .expecting("`initial` or `inherit` or T")
      .string(|str| {
        match_ignore_ascii_case! {str,
          "initial" => Ok(CssValue::initial()),
          "inherit" => Ok(CssValue::inherit()),
          _ => T::deserialize(StrDeserializer::new(str)).map(CssValue::Value),
        }
      })
      .bool(|value| T::deserialize(BoolDeserializer::new(value)).map(CssValue::Value))
      .i8(|num| T::deserialize(I8Deserializer::new(num)).map(CssValue::Value))
      .i16(|num| T::deserialize(I16Deserializer::new(num)).map(CssValue::Value))
      .i32(|num| T::deserialize(I32Deserializer::new(num)).map(CssValue::Value))
      .i64(|num| T::deserialize(I64Deserializer::new(num)).map(CssValue::Value))
      .i128(|num| T::deserialize(I128Deserializer::new(num)).map(CssValue::Value))
      .u8(|num| T::deserialize(U8Deserializer::new(num)).map(CssValue::Value))
      .u16(|num| T::deserialize(U16Deserializer::new(num)).map(CssValue::Value))
      .u32(|num| T::deserialize(U32Deserializer::new(num)).map(CssValue::Value))
      .u64(|num| T::deserialize(U64Deserializer::new(num)).map(CssValue::Value))
      .u128(|num| T::deserialize(U128Deserializer::new(num)).map(CssValue::Value))
      .f32(|num| T::deserialize(F32Deserializer::new(num)).map(CssValue::Value))
      .f64(|num| T::deserialize(F64Deserializer::new(num)).map(CssValue::Value))
      .seq(|seq| T::deserialize(SeqAccessDeserializer::new(seq)).map(CssValue::Value))
      .map(|map| T::deserialize(MapAccessDeserializer::new(map)).map(CssValue::Value))
      .unit(|| T::deserialize(UnitDeserializer::new()).map(CssValue::Value))
      .none(|| T::deserialize(UnitDeserializer::new()).map(CssValue::Value))
      .deserialize(deserializer)
  }
}

#[derive(Clone, Debug, Serialize, Default, PartialEq, TS)]
/// Proxy type for CSS `Option` serialization/deserialization.
pub struct CssOption<T: TS>(pub Option<T>);

impl<T: TS + Copy> Copy for CssOption<T> {}

impl<T: TS> Deref for CssOption<T> {
  type Target = Option<T>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T: TS> From<CssOption<T>> for Option<T> {
  fn from(val: CssOption<T>) -> Self {
    val.0
  }
}

impl<T: TS> From<T> for CssOption<T> {
  fn from(val: T) -> Self {
    Self::some(val)
  }
}

impl<T: TS> CssOption<T> {
  /// Create a new CssOption with the none value
  pub const fn none() -> Self {
    Self(None)
  }

  /// Create a new CssOption with the value
  pub const fn some(value: T) -> Self {
    Self(Some(value))
  }
}

impl<'de, T: TS + Deserialize<'de>> Deserialize<'de> for CssOption<T> {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    UntaggedEnumVisitor::new()
      .unit(|| Ok(Self(None)))
      .none(|| Ok(Self(None)))
      .string(|str| T::deserialize(StrDeserializer::new(str)).map(Self::some))
      .bool(|value| T::deserialize(BoolDeserializer::new(value)).map(Self::some))
      .seq(|seq| T::deserialize(SeqAccessDeserializer::new(seq)).map(Self::some))
      .map(|map| T::deserialize(MapAccessDeserializer::new(map)).map(Self::some))
      .f32(|num| T::deserialize(F32Deserializer::new(num)).map(Self::some))
      .f64(|num| T::deserialize(F64Deserializer::new(num)).map(Self::some))
      .i8(|num| T::deserialize(I8Deserializer::new(num)).map(Self::some))
      .i16(|num| T::deserialize(I16Deserializer::new(num)).map(Self::some))
      .i32(|num| T::deserialize(I32Deserializer::new(num)).map(Self::some))
      .i64(|num| T::deserialize(I64Deserializer::new(num)).map(Self::some))
      .i128(|num| T::deserialize(I128Deserializer::new(num)).map(Self::some))
      .u8(|num| T::deserialize(U8Deserializer::new(num)).map(Self::some))
      .u16(|num| T::deserialize(U16Deserializer::new(num)).map(Self::some))
      .u32(|num| T::deserialize(U32Deserializer::new(num)).map(Self::some))
      .u64(|num| T::deserialize(U64Deserializer::new(num)).map(Self::some))
      .u128(|num| T::deserialize(U128Deserializer::new(num)).map(Self::some))
      .deserialize(deserializer)
  }
}

/// CSS Global keyword
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CssGlobalKeyword {
  /// Use the initial value of the property
  #[default]
  Initial,
  /// Inherit the computed value from the parent element
  Inherit,
}

impl<T: TS> Default for CssValue<T> {
  fn default() -> Self {
    Self::Global(CssGlobalKeyword::Initial)
  }
}

impl<T: TS> CssValue<T> {
  /// Create a new CssValue with the initial global keyword
  pub const fn initial() -> Self {
    Self::Global(CssGlobalKeyword::Initial)
  }

  /// Create a new CssValue with the inherit global keyword
  pub const fn inherit() -> Self {
    Self::Global(CssGlobalKeyword::Inherit)
  }
}

impl<T: TS> From<T> for CssValue<T> {
  fn from(value: T) -> Self {
    CssValue::Value(value)
  }
}

impl<T: TS> CssValue<T> {
  /// Resolves this CssValue to a concrete value based on inheritance rules
  pub(crate) fn inherit_value(self, parent: &T, initial_value: T) -> T
  where
    T: Clone,
  {
    match self {
      Self::Value(v) => v,
      Self::Global(CssGlobalKeyword::Inherit) => parent.clone(),
      Self::Global(CssGlobalKeyword::Initial) => initial_value,
    }
  }
}

impl<T: Copy + TS> Copy for CssValue<T> {}
