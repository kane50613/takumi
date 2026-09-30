use std::fmt;

use cssparser::{Parser, match_ignore_ascii_case};

use crate::style::{
  Animatable, CssToken, FromCss, ListInterpolationStrategy, MakeComputed, ParseResult, ToCss,
  impl_css_enum, parse_comma_list,
};

/// Per-axis repeat style.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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

impl_css_enum!(
  BackgroundRepeatStyle,
  "repeat" => BackgroundRepeatStyle::Repeat,
  "no-repeat" => BackgroundRepeatStyle::NoRepeat,
  "space" => BackgroundRepeatStyle::Space,
  "round" => BackgroundRepeatStyle::Round,
);

/// Combined repeat for X and Y axes.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BackgroundRepeat(pub BackgroundRepeatStyle, pub BackgroundRepeatStyle);

impl MakeComputed for BackgroundRepeat {}

impl Animatable for BackgroundRepeat {
  fn list_interpolation_strategy() -> ListInterpolationStrategy {
    ListInterpolationStrategy::RepeatToLcm
  }
}

impl BackgroundRepeat {
  /// Returns a repeat value that tiles on both the X and Y axes.
  pub const fn repeat() -> Self {
    Self(BackgroundRepeatStyle::Repeat, BackgroundRepeatStyle::Repeat)
  }

  /// Returns a repeat value that does not tile on either axis.
  pub(crate) const fn no_repeat() -> Self {
    Self(
      BackgroundRepeatStyle::NoRepeat,
      BackgroundRepeatStyle::NoRepeat,
    )
  }

  /// Returns a repeat value that distributes leftover space evenly between tiles; edges flush with sides.
  pub const fn space() -> Self {
    Self(BackgroundRepeatStyle::Space, BackgroundRepeatStyle::Space)
  }

  /// Returns a repeat value that scales tile so an integer number fits exactly.
  pub const fn round() -> Self {
    Self(BackgroundRepeatStyle::Round, BackgroundRepeatStyle::Round)
  }
}

impl<'i> FromCss<'i> for BackgroundRepeat {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let state = input.state();
    let ident = input.expect_ident()?;

    match_ignore_ascii_case! { ident,
      "repeat-x" => return Ok(BackgroundRepeat(BackgroundRepeatStyle::Repeat, BackgroundRepeatStyle::NoRepeat)),
      "repeat-y" => return Ok(BackgroundRepeat(BackgroundRepeatStyle::NoRepeat, BackgroundRepeatStyle::Repeat)),
      _ => {}
    }

    input.reset(&state);

    let x = BackgroundRepeatStyle::from_css(input)?;
    let y = input
      .try_parse(BackgroundRepeatStyle::from_css)
      .unwrap_or(x);
    Ok(BackgroundRepeat(x, y))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("repeat-x"),
    CssToken::Keyword("repeat-y"),
    CssToken::Keyword("repeat"),
    CssToken::Keyword("no-repeat"),
    CssToken::Keyword("space"),
    CssToken::Keyword("round"),
  ];
}

/// An ordered list of [`BackgroundRepeat`] values.
pub type BackgroundRepeats = Box<[BackgroundRepeat]>;

impl<'i> FromCss<'i> for BackgroundRepeats {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_comma_list(input, BackgroundRepeat::from_css)
  }

  const VALID_TOKENS: &'static [CssToken] = BackgroundRepeat::VALID_TOKENS;
}

impl ToCss for BackgroundRepeat {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match (self.0, self.1) {
      (BackgroundRepeatStyle::Repeat, BackgroundRepeatStyle::NoRepeat) => {
        dest.write_str("repeat-x")
      }
      (BackgroundRepeatStyle::NoRepeat, BackgroundRepeatStyle::Repeat) => {
        dest.write_str("repeat-y")
      }
      (x, y) => {
        if x == y {
          x.to_css(dest)
        } else {
          x.to_css(dest)?;
          dest.write_str(" ")?;
          y.to_css(dest)
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_background_repeat() {
    for (css, expected) in [
      (
        "repeat-x",
        BackgroundRepeat(
          BackgroundRepeatStyle::Repeat,
          BackgroundRepeatStyle::NoRepeat,
        ),
      ),
      (
        "repeat-y",
        BackgroundRepeat(
          BackgroundRepeatStyle::NoRepeat,
          BackgroundRepeatStyle::Repeat,
        ),
      ),
      (
        "repeat no-repeat",
        BackgroundRepeat(
          BackgroundRepeatStyle::Repeat,
          BackgroundRepeatStyle::NoRepeat,
        ),
      ),
      (
        "space",
        BackgroundRepeat(BackgroundRepeatStyle::Space, BackgroundRepeatStyle::Space),
      ),
    ] {
      assert_eq!(
        BackgroundRepeat::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  #[test]
  fn test_background_repeat_round_trip() {
    for css in ["repeat-x", "repeat-y", "repeat no-repeat", "space", "round"] {
      let parsed = BackgroundRepeat::from_css_str(css).unwrap();
      let reparsed = BackgroundRepeat::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn test_parse_background_repeat_invalid() {
    assert!(BackgroundRepeat::from_css_str("123").is_err());
    assert!(BackgroundRepeat::from_css_str("bogus").is_err());
  }
}
