use cssparser::Parser;

use crate::style::{AlignItems, CssToken, FromCss, JustifyContent, ParseResult, unexpected_token};

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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_place_items() {
    for (css, expected) in [
      (
        "start end",
        PlaceItems {
          align: AlignItems::Start,
          justify: AlignItems::End,
        },
      ),
      (
        "center",
        PlaceItems {
          align: AlignItems::Center,
          justify: AlignItems::Center,
        },
      ),
      (
        "safe start",
        PlaceItems {
          align: AlignItems::SafeStart,
          justify: AlignItems::SafeStart,
        },
      ),
    ] {
      assert_eq!(
        PlaceItems::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  #[test]
  fn test_parse_place_content() {
    assert_eq!(
      PlaceContent::from_css_str("start end"),
      Ok(PlaceContent {
        align: JustifyContent::Start,
        justify: JustifyContent::End,
      })
    );
  }

  // PlaceItems/PlaceContent/PlaceSelf have no ToCss impl, so no round-trip test.

  #[test]
  fn test_parse_place_items_invalid() {
    assert!(PlaceItems::from_css_str("bogus").is_err());
    assert!(PlaceItems::from_css_str("123").is_err());
  }
}
