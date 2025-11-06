use std::fmt::Formatter;

use cssparser::{Parser, match_ignore_ascii_case};
use parley::style::FontWeight as ParleyFontWeight;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

use crate::layout::style::{FromCss, ParseResult, tw::TailwindPropertyParser};

/// Represents font weight value.
#[derive(Debug, Default, Copy, Clone, TS, PartialEq)]
#[ts(type = "string | number")]
pub struct FontWeight(ParleyFontWeight);

impl<'i> FromCss<'i> for FontWeight {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(FontWeight(
      ParleyFontWeight::parse(input.current_line()).unwrap(),
    ))
  }
}

impl TailwindPropertyParser for FontWeight {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {&token,
      "thin" => Some(100.0.into()),
      "extralight" => Some(200.0.into()),
      "light" => Some(300.0.into()),
      "normal" => Some(400.0.into()),
      "medium" => Some(500.0.into()),
      "semibold" => Some(600.0.into()),
      "bold" => Some(700.0.into()),
      "extrabold" => Some(800.0.into()),
      "black" => Some(900.0.into()),
      _ => None,
    }
  }
}

impl From<FontWeight> for ParleyFontWeight {
  fn from(value: FontWeight) -> Self {
    value.0
  }
}

impl From<f32> for FontWeight {
  fn from(value: f32) -> Self {
    FontWeight(ParleyFontWeight::new(value))
  }
}

impl<'de> Deserialize<'de> for FontWeight {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct Visitor;

    impl<'de> serde::de::Visitor<'de> for Visitor {
      type Value = FontWeight;

      fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str(r#""normal", "bold" or number from 0 to 1000"#)
      }

      fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        Ok(FontWeight(ParleyFontWeight::parse(v).ok_or_else(|| {
          serde::de::Error::custom(format!("Invalid font weight: {v}"))
        })?))
      }

      fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        Ok(FontWeight(ParleyFontWeight::new(v)))
      }

      fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        Ok(FontWeight(ParleyFontWeight::new(v as f32)))
      }
    }

    deserializer.deserialize_any(Visitor)
  }
}

impl Serialize for FontWeight {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_f32(self.0.value())
  }
}
