use std::fmt;

use cssparser::{Parser, Token};

use crate::style::{
  FromCss, Length, MakeComputed, ParseResult, SizingContext, ToCss, merge_token_lists,
  merged_token_len, unexpected_token,
};

/// Represents a grid track breadth, Blink's `ConsumeGridBreadth`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum GridLength {
  /// A fraction of the available space
  Fr(f32),
  /// A fixed length
  Unit(Length),
  /// `min-content`: the largest minimum content contribution of the track's items.
  MinContent,
  /// `max-content`: the largest maximum content contribution of the track's items.
  MaxContent,
}

impl<'i> FromCss<'i> for GridLength {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if let Ok(unit) = input.try_parse(Length::from_css) {
      return Ok(GridLength::Unit(unit));
    }
    if input
      .try_parse(|input| input.expect_ident_matching("min-content"))
      .is_ok()
    {
      return Ok(GridLength::MinContent);
    }
    if input
      .try_parse(|input| input.expect_ident_matching("max-content"))
      .is_ok()
    {
      return Ok(GridLength::MaxContent);
    }

    let token = input.next()?;

    let Token::Dimension { value, unit, .. } = &token else {
      return Err(unexpected_token!(token));
    };

    if !unit.eq_ignore_ascii_case("fr") {
      return Err(unexpected_token!(token));
    }

    Ok(GridLength::Fr(*value))
  }

  const VALID_TOKENS: &'static [&'static str] = &BREADTH_TOKENS;
}

const BREADTH_TOKEN_LISTS: &[&[&str]] = &[
  Length::VALID_TOKENS,
  &["min-content", "max-content", "<flex>"],
];

const BREADTH_TOKENS: [&str; merged_token_len(BREADTH_TOKEN_LISTS)] =
  merge_token_lists(BREADTH_TOKEN_LISTS);

impl MakeComputed for GridLength {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let GridLength::Unit(unit) = self {
      unit.make_computed(sizing);
    }
  }
}

impl ToCss for GridLength {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Fr(fr) => write!(dest, "{fr}fr"),
      Self::Unit(u) => u.to_css(dest),
      Self::MinContent => dest.write_str("min-content"),
      Self::MaxContent => dest.write_str("max-content"),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_fr_and_unit() {
    assert_eq!(GridLength::from_css_str("1fr"), Ok(GridLength::Fr(1.0)));
    assert_eq!(
      GridLength::from_css_str("min-content"),
      Ok(GridLength::MinContent)
    );
    assert_eq!(
      GridLength::from_css_str("max-content"),
      Ok(GridLength::MaxContent)
    );

    assert_eq!(
      GridLength::from_css_str("10px"),
      Ok(GridLength::Unit(Length::Px(10.0)))
    );
  }
}
