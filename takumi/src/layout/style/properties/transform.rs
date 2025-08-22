use cssparser::{Parser, Token};
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use super::LengthUnit;
use crate::layout::style::{FromCss, ParseResult};

/// Collection of transform functions in order.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Transform(pub Vec<TransformFunction>);

impl<'de> Deserialize<'de> for Transform {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let functions: Vec<TransformFunction> = Vec::deserialize(deserializer)?;

    Ok(Transform(functions))
  }
}

/// Transform origin (x,y) relative or absolute lengths.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct TransformOrigin(pub LengthUnit, pub LengthUnit);

impl<'de> Deserialize<'de> for TransformOrigin {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    // Accept array [x, y]
    let v: serde_json::Value = Deserialize::deserialize(deserializer)?;
    match v {
      serde_json::Value::Array(mut arr) => {
        if arr.len() != 2 {
          return Err(serde::de::Error::custom(
            "transformOrigin expects 2 elements",
          ));
        }
        let y = arr.pop().unwrap();
        let x = arr.pop().unwrap();
        let parse_len = |val: serde_json::Value| -> Result<LengthUnit, D::Error> {
          serde_json::from_value(val).map_err(serde::de::Error::custom)
        };
        Ok(TransformOrigin(parse_len(x)?, parse_len(y)?))
      }
      _ => Err(serde::de::Error::custom("transformOrigin must be an array")),
    }
  }
}

/// Represents a single transform function (internal canonical form).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TransformFunction {
  /// Translation by x,y.
  Translate([LengthUnit; 2]),
  /// Scale by (sx, sy).
  Scale([f32; 2]),
  /// Rotation in degrees.
  Rotate(f32),
  /// Skew by (ax, ay) in degrees.
  Skew([f32; 2]),
}

impl TransformFunction {
  fn from_value(v: serde_json::Value) -> Result<Self, String> {
    let obj = match v {
      serde_json::Value::Object(map) => map,
      other => return Err(format!("expected object, got {other:?}")),
    };
    if obj.len() != 1 {
      return Err("transform function object must have exactly one key".into());
    }
    let (k, v) = obj.into_iter().next().unwrap();
    match k.as_str() {
      "rotate" => {
        let deg: f32 = serde_json::from_value(v).map_err(|e| e.to_string())?;
        Ok(TransformFunction::Rotate(deg))
      }
      "scale" => match v {
        serde_json::Value::Number(num) => {
          let f = num.as_f64().ok_or("invalid number")? as f32;
          Ok(TransformFunction::Scale([f, f]))
        }
        serde_json::Value::Array(arr) => {
          if arr.is_empty() || arr.len() > 2 {
            return Err("scale array must have 1 or 2 numbers".into());
          }
          let mut it = arr.into_iter();
          let sx: f32 = serde_json::from_value(it.next().unwrap()).map_err(|e| e.to_string())?;
          let sy: f32 = if let Some(vy) = it.next() {
            serde_json::from_value(vy).map_err(|e| e.to_string())?
          } else {
            sx
          };
          Ok(TransformFunction::Scale([sx, sy]))
        }
        other => Err(format!("invalid scale value: {other:?}")),
      },
      "translate" => {
        let arr = match v {
          serde_json::Value::Array(a) => a,
          _ => return Err("translate expects array".into()),
        };
        if arr.len() != 2 {
          return Err("translate array must have 2 values".into());
        }
        let x: LengthUnit = serde_json::from_value(arr[0].clone()).map_err(|e| e.to_string())?;
        let y: LengthUnit = serde_json::from_value(arr[1].clone()).map_err(|e| e.to_string())?;
        Ok(TransformFunction::Translate([x, y]))
      }
      "skew" => {
        let arr = match v {
          serde_json::Value::Array(a) => a,
          _ => return Err("skew expects array".into()),
        };
        if arr.len() != 2 {
          return Err("skew array must have 2 values".into());
        }
        let ax: f32 = serde_json::from_value(arr[0].clone()).map_err(|e| e.to_string())?;
        let ay: f32 = serde_json::from_value(arr[1].clone()).map_err(|e| e.to_string())?;
        Ok(TransformFunction::Skew([ax, ay]))
      }
      other => Err(format!("unknown transform function: {other}")),
    }
  }
}

impl<'de> Deserialize<'de> for TransformFunction {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let v: serde_json::Value = Deserialize::deserialize(deserializer)?;
    TransformFunction::from_value(v).map_err(serde::de::Error::custom)
  }
}

impl<'i> FromCss<'i> for Transform {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut functions = Vec::new();
    while let Ok(f) = input.try_parse(TransformFunction::from_css) {
      functions.push(f);
    }
    Ok(Transform(functions))
  }
}

impl<'i> FromCss<'i> for TransformFunction {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let name = input.expect_function()?.clone();
    let name_lc = name.to_ascii_lowercase();
    input.parse_nested_block(|input| {
      let parse_num = |input: &mut Parser<'i, '_>| -> ParseResult<'i, f32> {
        let token = input.next()?;
        match token {
          Token::Number { value, .. } => Ok(*value),
          Token::Dimension { value, .. } => Ok(*value),
          _ => Err(input.new_error_for_next_token()),
        }
      };
      let parse_len =
        |input: &mut Parser<'i, '_>| -> ParseResult<'i, LengthUnit> { LengthUnit::from_css(input) };
      match &*name_lc {
        "translate" => {
          let x = parse_len(input)?;
          if input.expect_comma().is_err() {
            return Err(input.new_error_for_next_token());
          }
          let y = parse_len(input)?;
          Ok(TransformFunction::Translate([x, y]))
        }
        "scale" => {
          let sx = parse_num(input)?;
          let sy = if input.expect_comma().is_ok() {
            parse_num(input)?
          } else {
            sx
          };
          Ok(TransformFunction::Scale([sx, sy]))
        }
        "rotate" => Ok(TransformFunction::Rotate(parse_num(input)?)),
        "skew" => {
          let ax = parse_num(input)?;
          if input.expect_comma().is_err() {
            return Err(input.new_error_for_next_token());
          }
          let ay = parse_num(input)?;
          Ok(TransformFunction::Skew([ax, ay]))
        }
        _ => Err(input.new_error_for_next_token()),
      }
    })
  }
}
