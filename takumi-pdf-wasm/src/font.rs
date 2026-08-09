//! Deserialization of the font input accepted by `registerFont`.

use serde::{Deserialize, Deserializer, de::Error as DeError};
use serde_bytes::ByteBuf;
use takumi_core::style::{FontStyle as CssFontStyle, FromCssStr};

/// Details for loading a custom font.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FontDetails {
  pub(crate) name: Option<String>,
  pub(crate) data: ByteBuf,
  pub(crate) weight: Option<f64>,
  pub(crate) style: Option<FontStyle>,
  pub(crate) subset_of: Option<String>,
  pub(crate) subset_rank: Option<u32>,
  pub(crate) generic: Option<String>,
}

/// Font input, either as detailed object or raw buffer.
#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum Font {
  Object(FontDetails),
  Buffer(ByteBuf),
}

pub(crate) struct FontStyle(pub(crate) CssFontStyle);

impl<'de> Deserialize<'de> for FontStyle {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let value = String::deserialize(deserializer)?;
    Ok(Self(
      CssFontStyle::from_css_str(&value).map_err(D::Error::custom)?,
    ))
  }
}
