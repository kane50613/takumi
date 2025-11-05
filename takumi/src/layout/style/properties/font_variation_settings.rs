use parley::FontVariation;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smallvec::SmallVec;
use ts_rs::TS;

/// Controls variable font axis values via CSS font-variation-settings property.
///
/// This allows fine-grained control over variable font characteristics like weight,
/// width, slant, and other custom axes defined in the font.
#[derive(Debug, Clone, Default, PartialEq, TS)]
#[ts(type = "string")]
pub struct FontVariationSettings(pub SmallVec<[FontVariation; 4]>);

impl<'de> Deserialize<'de> for FontVariationSettings {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;

    Ok(FontVariationSettings(
      FontVariation::parse_list(&s).collect(),
    ))
  }
}

impl Serialize for FontVariationSettings {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(
      &self
        .0
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(","),
    )
  }
}
