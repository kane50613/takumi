use cssparser::{Parser, match_ignore_ascii_case};
use smallvec::SmallVec;

use crate::layout::style::{CssToken, FromCss, ParseResult, declare_enum_from_css_impl};

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

declare_enum_from_css_impl!(
  BackgroundRepeatStyle,
  "repeat" => BackgroundRepeatStyle::Repeat,
  "no-repeat" => BackgroundRepeatStyle::NoRepeat,
  "space" => BackgroundRepeatStyle::Space,
  "round" => BackgroundRepeatStyle::Round,
);

/// Combined repeat for X and Y axes.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("repeat-x"),
      CssToken::Keyword("repeat-y"),
      CssToken::Keyword("repeat"),
      CssToken::Keyword("no-repeat"),
      CssToken::Keyword("space"),
      CssToken::Keyword("round"),
    ]
  }
}

/// A list of background-repeat values (one per layer).
pub type BackgroundRepeats = SmallVec<[BackgroundRepeat; 4]>;

impl<'i> FromCss<'i> for BackgroundRepeats {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut values = SmallVec::new();
    values.push(BackgroundRepeat::from_css(input)?);

    while input.expect_comma().is_ok() {
      values.push(BackgroundRepeat::from_css(input)?);
    }

    Ok(values)
  }

  fn valid_tokens() -> &'static [CssToken] {
    BackgroundRepeat::valid_tokens()
  }
}
