//! Shared keyframe input parsing used by external bindings.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, de};

use crate::layout::style::{KeyframeRule, KeyframesRule, StyleDeclarationBlock};

#[derive(Deserialize)]
#[serde(untagged)]
enum RawKeyframesInput {
  Rules(Vec<KeyframesRule>),
  Shorthand(BTreeMap<String, BTreeMap<String, StyleDeclarationBlock>>),
}

/// Deserializes either structured keyframes or shorthand keyframe maps.
pub fn deserialize_keyframes<'de, D>(deserializer: D) -> Result<Vec<KeyframesRule>, D::Error>
where
  D: Deserializer<'de>,
{
  match RawKeyframesInput::deserialize(deserializer)? {
    RawKeyframesInput::Rules(rules) => Ok(rules),
    RawKeyframesInput::Shorthand(shorthand) => raw_keyframes_to_rules(shorthand),
  }
}

/// Deserializes optional keyframes while preserving missing-field behavior.
pub fn deserialize_optional_keyframes<'de, D>(
  deserializer: D,
) -> Result<Option<Vec<KeyframesRule>>, D::Error>
where
  D: Deserializer<'de>,
{
  Option::<RawKeyframesInput>::deserialize(deserializer)?
    .map(|raw| match raw {
      RawKeyframesInput::Rules(rules) => Ok(rules),
      RawKeyframesInput::Shorthand(shorthand) => raw_keyframes_to_rules(shorthand),
    })
    .transpose()
}

fn raw_keyframes_to_rules<E>(
  shorthand: BTreeMap<String, BTreeMap<String, StyleDeclarationBlock>>,
) -> Result<Vec<KeyframesRule>, E>
where
  E: de::Error,
{
  shorthand
    .into_iter()
    .map(|(name, stages)| {
      let keyframes = stages
        .into_iter()
        .map(|(selector, declarations)| {
          Ok(KeyframeRule {
            offsets: parse_keyframe_offsets(&selector).map_err(E::custom)?,
            declarations,
          })
        })
        .collect::<Result<Vec<_>, E>>()?;

      Ok(KeyframesRule { name, keyframes })
    })
    .collect::<Result<Vec<_>, E>>()
}

fn parse_keyframe_offsets(selector: &str) -> Result<Vec<f32>, String> {
  let parsed_offsets = selector
    .split(',')
    .map(str::trim)
    .filter(|part| !part.is_empty())
    .map(|part| match part {
      "from" => Ok(0.0),
      "to" => Ok(1.0),
      _ => {
        let Some(percent) = part.strip_suffix('%') else {
          return Err(format!(
            "unsupported keyframe selector `{part}`; use `from`, `to`, or percentage values like `50%`"
          ));
        };
        let value = percent
          .parse::<f32>()
          .map_err(|_| format!("invalid keyframe percentage `{part}`"))?;
        if !(0.0..=100.0).contains(&value) {
          return Err(format!(
            "invalid keyframe percentage `{part}`; expected a value in 0%..=100%"
          ));
        }
        Ok(value / 100.0)
      }
    })
    .collect::<Result<Vec<_>, _>>()?;

  if parsed_offsets.is_empty() {
    return Err(
      "empty keyframe selector; expected at least one of `from`, `to`, or percentage values"
        .to_owned(),
    );
  }

  Ok(parsed_offsets)
}

#[cfg(test)]
mod tests {
  use serde::Deserialize;
  use serde_json::from_value;

  use super::{deserialize_keyframes, deserialize_optional_keyframes};
  use crate::layout::style::KeyframesRule;

  #[derive(Debug, Deserialize)]
  struct KeyframesDocument {
    #[serde(deserialize_with = "deserialize_keyframes")]
    keyframes: Vec<KeyframesRule>,
  }

  #[derive(Debug, Deserialize)]
  struct OptionalKeyframesDocument {
    #[serde(default, deserialize_with = "deserialize_optional_keyframes")]
    keyframes: Option<Vec<KeyframesRule>>,
  }

  #[test]
  fn rejects_empty_keyframe_selector() {
    let result = from_value::<KeyframesDocument>(serde_json::json!({
      "keyframes": {
        "fade": {
          " , ": {
            "opacity": 0
          }
        }
      }
    }));

    assert!(
      result.is_err(),
      "expected empty selector to fail: {result:?}"
    );
    assert!(matches!(
      result.as_ref(),
      Err(error) if error.to_string().contains("empty keyframe selector")
    ));
  }

  #[test]
  fn parses_shorthand_keyframes() {
    let result = from_value::<KeyframesDocument>(serde_json::json!({
      "keyframes": {
        "fade": {
          "from, 50%, to": {
            "opacity": 1
          }
        }
      }
    }));

    assert!(
      result.is_ok(),
      "expected valid shorthand keyframes: {result:?}"
    );
    let keyframes = result.as_ref().ok();

    assert_eq!(keyframes.map(|value| value.keyframes.len()), Some(1));
    assert_eq!(
      keyframes.map(|value| value.keyframes[0].keyframes[0].offsets.clone()),
      Some(vec![0.0, 0.5, 1.0])
    );
  }

  #[test]
  fn keeps_missing_optional_keyframes_as_none() {
    let result = from_value::<OptionalKeyframesDocument>(serde_json::json!({}));

    assert!(
      result.is_ok(),
      "expected missing keyframes to deserialize: {result:?}"
    );
    assert_eq!(result.ok().and_then(|document| document.keyframes), None);
  }
}
