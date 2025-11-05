use parley::style::FontStyle as ParleyFontStyle;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

/// Controls the slant (italic/oblique) of text rendering.
#[derive(Default, Debug, Clone, Copy, TS, PartialEq)]
#[ts(type = r#""normal" | "italic" | "oblique" | `oblique ${string}deg`"#)]
pub struct FontStyle(ParleyFontStyle);

impl FontStyle {
  /// The normal font style.
  pub const fn normal() -> Self {
    Self(ParleyFontStyle::Normal)
  }

  /// The italic font style.
  pub const fn italic() -> Self {
    Self(ParleyFontStyle::Italic)
  }

  /// The oblique font style with a given angle.
  pub const fn oblique(angle: f32) -> Self {
    Self(ParleyFontStyle::Oblique(Some(angle)))
  }
}

impl From<FontStyle> for ParleyFontStyle {
  fn from(value: FontStyle) -> Self {
    value.0
  }
}

impl<'de> Deserialize<'de> for FontStyle {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    Ok(FontStyle(ParleyFontStyle::parse(&s).ok_or_else(|| {
      serde::de::Error::custom(format!("Invalid font style: {s}"))
    })?))
  }
}

impl Serialize for FontStyle {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(&self.0.to_string())
  }
}
