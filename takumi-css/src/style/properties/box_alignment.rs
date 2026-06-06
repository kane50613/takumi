use crate::style::unexpected_token;
use cssparser::Parser;

use crate::style::{AlignItems, CssToken, FromCss, JustifyContent, ParseResult};

/// Represents the `place-items` shorthand.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlaceItems {
  /// The alignment value for the block axis.
  pub align: AlignItems,
  /// The alignment value for the inline axis.
  pub justify: AlignItems,
}

impl<'i> FromCss<'i> for PlaceItems {
  const VALID_TOKENS: &'static [CssToken] = AlignItems::VALID_TOKENS;

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_pair(input).map(|(align, justify)| Self { align, justify })
  }
}

/// Represents the `place-content` shorthand.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlaceContent {
  /// The alignment value for the block axis.
  pub align: JustifyContent,
  /// The alignment value for the inline axis.
  pub justify: JustifyContent,
}

impl<'i> FromCss<'i> for PlaceContent {
  const VALID_TOKENS: &'static [CssToken] = JustifyContent::VALID_TOKENS;

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_pair(input).map(|(align, justify)| Self { align, justify })
  }
}

/// Represents the `place-self` shorthand.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlaceSelf {
  /// The alignment value for the block axis.
  pub align: AlignItems,
  /// The alignment value for the inline axis.
  pub justify: AlignItems,
}

impl<'i> FromCss<'i> for PlaceSelf {
  const VALID_TOKENS: &'static [CssToken] = AlignItems::VALID_TOKENS;

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_pair(input).map(|(align, justify)| Self { align, justify })
  }
}

fn parse_pair<'i, T>(input: &mut Parser<'i, '_>) -> ParseResult<'i, (T, T)>
where
  T: FromCss<'i> + Copy,
{
  let first = T::from_css(input)?;
  let second = if input.is_exhausted() {
    first
  } else {
    T::from_css(input)?
  };

  if !input.is_exhausted() {
    return Err(unexpected_token!(
      T,
      input.current_source_location(),
      input.next()?,
    ));
  }

  Ok((first, second))
}
