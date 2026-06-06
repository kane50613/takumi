//! Shared keyframe input parsing used by external bindings.

use cssparser::{ParseError, ParseErrorKind, Parser, ParserInput, Token};
use serde::{
  Deserialize, Deserializer,
  de::{self, MapAccess, SeqAccess, Visitor},
};

use crate::style::{KeyframeRule, KeyframesRule, StyleDeclarationBlock};

struct StageMap(Vec<(String, StyleDeclarationBlock)>);

impl<'de> Deserialize<'de> for StageMap {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct StageMapVisitor;

    impl<'de> Visitor<'de> for StageMapVisitor {
      type Value = StageMap;

      fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a map from keyframe selector to declaration block")
      }

      fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
      where
        A: MapAccess<'de>,
      {
        let mut stages = Vec::with_capacity(access.size_hint().unwrap_or(0));
        while let Some((selector, declarations)) =
          access.next_entry::<String, StyleDeclarationBlock>()?
        {
          stages.push((selector, declarations));
        }
        stages.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        Ok(StageMap(stages))
      }
    }

    deserializer.deserialize_map(StageMapVisitor)
  }
}

struct KeyframesVec(Vec<KeyframesRule>);

impl<'de> Deserialize<'de> for KeyframesVec {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct KeyframesVecVisitor;

    impl<'de> Visitor<'de> for KeyframesVecVisitor {
      type Value = KeyframesVec;

      fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("keyframes rules array or shorthand keyframes object")
      }

      fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
      where
        A: SeqAccess<'de>,
      {
        let mut rules = Vec::with_capacity(access.size_hint().unwrap_or(0));
        while let Some(rule) = access.next_element::<KeyframesRule>()? {
          rules.push(rule);
        }
        Ok(KeyframesVec(rules))
      }

      fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
      where
        A: MapAccess<'de>,
      {
        let mut shorthand = Vec::with_capacity(access.size_hint().unwrap_or(0));
        while let Some((name, stages)) = access.next_entry::<String, StageMap>()? {
          shorthand.push((name, stages));
        }
        shorthand.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));

        let rules = shorthand
          .into_iter()
          .map(|(name, StageMap(stages))| {
            let keyframes = stages
              .into_iter()
              .map(|(selector, declarations)| {
                Ok(KeyframeRule {
                  offsets: parse_keyframe_offsets(&selector).map_err(de::Error::custom)?,
                  declarations,
                })
              })
              .collect::<Result<Vec<_>, _>>()?;

            Ok(KeyframesRule {
              name,
              keyframes,
              media_queries: Vec::new(),
            })
          })
          .collect::<Result<Vec<_>, _>>()?;

        Ok(KeyframesVec(rules))
      }
    }

    deserializer.deserialize_any(KeyframesVecVisitor)
  }
}

/// Deserializes either structured keyframes or shorthand keyframe maps.
pub fn deserialize_keyframes<'de, D>(deserializer: D) -> Result<Vec<KeyframesRule>, D::Error>
where
  D: Deserializer<'de>,
{
  Ok(KeyframesVec::deserialize(deserializer)?.0)
}

/// Deserializes optional keyframes while preserving missing-field behavior.
pub fn deserialize_optional_keyframes<'de, D>(
  deserializer: D,
) -> Result<Option<Vec<KeyframesRule>>, D::Error>
where
  D: Deserializer<'de>,
{
  Ok(Option::<KeyframesVec>::deserialize(deserializer)?.map(|keyframes| keyframes.0))
}

fn parse_keyframe_offsets(selector: &str) -> Result<Vec<f32>, String> {
  if selector
    .trim_matches(|c: char| c.is_ascii_whitespace() || c == ',')
    .is_empty()
  {
    return Err(
      "empty keyframe selector; expected at least one of `from`, `to`, or percentage values"
        .to_owned(),
    );
  }

  let mut input = ParserInput::new(selector);
  let mut parser = Parser::new(&mut input);
  parse_keyframe_prelude::<KeyframePreludeParseError<'_>>(&mut parser).map_err(|error| match error
    .kind
  {
    ParseErrorKind::Custom(KeyframePreludeParseError::InvalidPercentage(part)) => {
      format!("invalid keyframe percentage `{part}`; expected a value in 0%..=100%")
    }
    ParseErrorKind::Custom(KeyframePreludeParseError::InvalidSelector(part)) => {
      unsupported_keyframe_selector(part)
    }
    ParseErrorKind::Basic(_) => unsupported_keyframe_selector(selector.trim()),
  })
}

pub fn parse_keyframe_prelude<'i, E>(
  input: &mut Parser<'i, '_>,
) -> Result<Vec<f32>, ParseError<'i, E>>
where
  KeyframePreludeParseError<'i>: Into<E>,
{
  input.parse_comma_separated(parse_keyframe_offset)
}

#[derive(Clone, Copy)]
pub enum KeyframePreludeParseError<'i> {
  InvalidSelector(&'i str),
  InvalidPercentage(&'i str),
}

fn parse_keyframe_offset<'i, E>(input: &mut Parser<'i, '_>) -> Result<f32, ParseError<'i, E>>
where
  KeyframePreludeParseError<'i>: Into<E>,
{
  if input
    .try_parse(|parser| parser.expect_ident_matching("from"))
    .is_ok()
  {
    return Ok(0.0);
  }

  if input
    .try_parse(|parser| parser.expect_ident_matching("to"))
    .is_ok()
  {
    return Ok(1.0);
  }

  let start_position = input.position();
  let offset = match input.next() {
    Ok(Token::Percentage { unit_value, .. }) => *unit_value,
    Ok(_) | Err(_) => {
      let part = input.slice_from(start_position).trim();
      return Err(input.new_custom_error(KeyframePreludeParseError::InvalidSelector(part)));
    }
  };

  if !(0.0..=1.0).contains(&offset) {
    let part = input.slice_from(start_position).trim();
    return Err(input.new_custom_error(KeyframePreludeParseError::InvalidPercentage(part)));
  }

  Ok(offset)
}

fn unsupported_keyframe_selector(selector: &str) -> String {
  format!(
    "unsupported keyframe selector `{selector}`; use `from`, `to`, or percentage values like `50%`"
  )
}

#[cfg(test)]
mod tests {
  use std::assert_matches;

  use serde::Deserialize;
  use serde_json::from_value;

  use super::{deserialize_keyframes, deserialize_optional_keyframes};
  use crate::style::KeyframesRule;

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
    assert_matches!(
      result.as_ref(),
      Err(error) if error.to_string().contains("empty keyframe selector")
    );
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
