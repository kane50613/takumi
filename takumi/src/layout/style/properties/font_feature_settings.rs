use parley::FontFeature;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smallvec::SmallVec;
use ts_rs::TS;

/// Controls OpenType font features via CSS font-feature-settings property.
///
/// This allows enabling/disabling specific typographic features in OpenType fonts
/// such as ligatures, kerning, small caps, and other advanced typography features.
#[derive(Debug, Clone, Default, PartialEq, TS)]
#[ts(type = "string")]
pub struct FontFeatureSettings(pub SmallVec<[FontFeature; 4]>);

impl<'de> Deserialize<'de> for FontFeatureSettings {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;

    Ok(FontFeatureSettings(FontFeature::parse_list(&s).collect()))
  }
}

impl Serialize for FontFeatureSettings {
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
