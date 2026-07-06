use std::fmt;

use cssparser::Parser;
use taffy::LengthPercentage;

use crate::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, Length, MakeComputed, ParseResult,
  SizingContext, ToCss,
};

/// Represents the `column-gap`/`row-gap` value: either `normal` (computes to `0`) or a [`Length`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Gap {
  /// No explicit gap; computes to `0`.
  #[default]
  Normal,
  /// A concrete gap length.
  Length(Length),
}

impl MakeComputed for Gap {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let Self::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}

impl Animatable for Gap {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self = match (from, to) {
      (Self::Length(from), Self::Length(to)) => {
        let mut length = *from;
        length.interpolate(from, to, progress, sizing, current_color);
        Self::Length(length)
      }
      (Self::Normal, Self::Normal) => Self::Normal,
      (Self::Normal, Self::Length(to)) => {
        let mut length = Length::zero();
        length.interpolate(&Length::zero(), to, progress, sizing, current_color);
        Self::Length(length)
      }
      (Self::Length(from), Self::Normal) => {
        let mut length = *from;
        length.interpolate(from, &Length::zero(), progress, sizing, current_color);
        Self::Length(length)
      }
    };
  }
}

impl<'i> FromCss<'i> for Gap {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("normal"))
      .is_ok()
    {
      return Ok(Self::Normal);
    }

    Length::from_css(input).map(Self::Length)
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

impl From<Length> for Gap {
  fn from(length: Length) -> Self {
    Self::Length(length)
  }
}

impl Gap {
  /// Resolves to a taffy `LengthPercentage`, treating `normal` as `0`.
  pub(crate) fn resolve_to_length_percentage(self, sizing: &SizingContext) -> LengthPercentage {
    match self {
      Self::Normal => Length::zero().resolve_to_length_percentage(sizing),
      Self::Length(length) => length.resolve_to_length_percentage(sizing),
    }
  }
}

impl ToCss for Gap {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Normal => dest.write_str("normal"),
      Self::Length(length) => length.to_css(dest),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn parses_normal() {
    assert_eq!(Gap::from_css_str("normal"), Ok(Gap::Normal));
  }

  #[test]
  fn parses_length() {
    assert_eq!(Gap::from_css_str("10px"), Ok(Gap::Length(Length::Px(10.0))));
  }

  #[test]
  fn round_trips_to_css() {
    let mut buf = String::new();
    Gap::Normal.to_css(&mut buf).unwrap();
    assert_eq!(buf, "normal");

    let mut buf = String::new();
    Gap::Length(Length::Px(10.0)).to_css(&mut buf).unwrap();
    assert_eq!(buf, "10px");
  }
}
