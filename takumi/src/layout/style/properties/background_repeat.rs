use cssparser::{Parser, Token, match_ignore_ascii_case};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::layout::style::{FromCss, ParseResult, tw::TailwindPropertyParser};

/// Per-axis repeat style.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum BackgroundRepeatStyle {
  /// Tile as many times as needed with no extra spacing
  #[default]
  Repeat,
  /// Do not tile on this axis
  NoRepeat,
  /// Distribute leftover space evenly between tiles; edges flush with sides
  Space,
  /// Scale tile so an integer number fits exactly
  Round,
}

/// Combined repeat for X and Y axes.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub struct BackgroundRepeat(pub BackgroundRepeatStyle, pub BackgroundRepeatStyle);

impl BackgroundRepeat {
  /// Returns a repeat value that tiles on both the X and Y axes.
  pub const fn repeat() -> Self {
    Self(BackgroundRepeatStyle::Repeat, BackgroundRepeatStyle::Repeat)
  }

  /// Returns a repeat value that does not tile on either axis.
  pub const fn no_repeat() -> Self {
    Self(
      BackgroundRepeatStyle::NoRepeat,
      BackgroundRepeatStyle::NoRepeat,
    )
  }
}

impl TailwindPropertyParser for BackgroundRepeat {
  fn parse_tw(token: &str) -> Option<Self> {
    match token {
      "repeat" => Some(BackgroundRepeat::repeat()),
      "no-repeat" => Some(BackgroundRepeat::no_repeat()),
      "repeat-x" => Some(BackgroundRepeat(
        BackgroundRepeatStyle::Repeat,
        BackgroundRepeatStyle::NoRepeat,
      )),
      "repeat-y" => Some(BackgroundRepeat(
        BackgroundRepeatStyle::NoRepeat,
        BackgroundRepeatStyle::Repeat,
      )),
      _ => None,
    }
  }
}

impl<'i> FromCss<'i> for BackgroundRepeat {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let first_ident = input.expect_ident_cloned()?;
    let second_ident = input.try_parse(Parser::expect_ident_cloned).ok();

    let parse_axis = |ident: &str| -> Option<BackgroundRepeatStyle> {
      match_ignore_ascii_case! {ident,
        "repeat" => Some(BackgroundRepeatStyle::Repeat),
        "no-repeat" => Some(BackgroundRepeatStyle::NoRepeat),
        "space" => Some(BackgroundRepeatStyle::Space),
        "round" => Some(BackgroundRepeatStyle::Round),
        _ => None,
      }
    };

    match second_ident {
      None => {
        // single keyword forms
        if first_ident.eq_ignore_ascii_case("repeat-x") {
          return Ok(Self(
            BackgroundRepeatStyle::Repeat,
            BackgroundRepeatStyle::NoRepeat,
          ));
        }
        if first_ident.eq_ignore_ascii_case("repeat-y") {
          return Ok(Self(
            BackgroundRepeatStyle::NoRepeat,
            BackgroundRepeatStyle::Repeat,
          ));
        }
        if let Some(axis) = parse_axis(&first_ident) {
          return Ok(Self(axis, axis));
        }
        Err(
          location
            .new_basic_unexpected_token_error(Token::Ident(first_ident.clone()))
            .into(),
        )
      }
      Some(second) => {
        let x = parse_axis(&first_ident).ok_or_else(|| {
          location.new_basic_unexpected_token_error(Token::Ident(first_ident.clone()))
        })?;
        let y = parse_axis(&second)
          .ok_or_else(|| location.new_basic_unexpected_token_error(Token::Ident(second.clone())))?;
        Ok(Self(x, y))
      }
    }
  }
}

/// Proxy type to deserialize CSS background-repeat as either a list or CSS string.
#[derive(Debug, Clone, PartialEq, TS, Deserialize)]
#[serde(untagged)]
pub(crate) enum BackgroundRepeatsValue {
  /// Parsed repeats for one or more layers.
  Repeats(Vec<BackgroundRepeat>),
  /// Raw CSS to be parsed at runtime.
  Css(String),
}

/// A list of background-repeat values (layered).
#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq, TS)]
#[ts(as = "BackgroundRepeatsValue")]
#[serde(try_from = "BackgroundRepeatsValue")]
pub struct BackgroundRepeats(pub Vec<BackgroundRepeat>);

impl TryFrom<BackgroundRepeatsValue> for BackgroundRepeats {
  type Error = String;

  fn try_from(value: BackgroundRepeatsValue) -> Result<Self, Self::Error> {
    match value {
      BackgroundRepeatsValue::Repeats(v) => Ok(Self(v)),
      BackgroundRepeatsValue::Css(css) => Self::from_str(&css).map_err(|e| e.to_string()),
    }
  }
}

impl<'i> FromCss<'i> for BackgroundRepeats {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut values = Vec::new();
    values.push(BackgroundRepeat::from_css(input)?);

    while input.expect_comma().is_ok() {
      values.push(BackgroundRepeat::from_css(input)?);
    }

    Ok(Self(values))
  }
}
