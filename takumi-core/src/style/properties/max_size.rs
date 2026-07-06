use std::fmt;

use cssparser::Parser;
use taffy::Dimension;

use crate::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, Length, MakeComputed, ParseResult,
  SizingContext, ToCss,
};

/// Represents the `max-width`/`max-height` value: either `none` (unbounded) or a [`Length`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum MaxSize {
  /// No maximum; the box is unbounded on this axis.
  #[default]
  None,
  /// A concrete maximum length.
  Length(Length),
}

impl MakeComputed for MaxSize {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let Self::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}

impl Animatable for MaxSize {
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
      _ => {
        if progress >= 0.5 {
          *to
        } else {
          *from
        }
      }
    };
  }
}

impl<'i> FromCss<'i> for MaxSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("none"))
      .is_ok()
    {
      return Ok(Self::None);
    }

    // `auto` is not a valid `max-width`/`max-height` value, but historically
    // accepted here; keep treating it as `none` to avoid a behavior change.
    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Ok(Self::None);
    }

    Length::from_css(input).map(Self::Length)
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("none"),
    CssToken::Keyword("auto"),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

impl From<Length> for MaxSize {
  fn from(length: Length) -> Self {
    Self::Length(length)
  }
}

impl MaxSize {
  /// Resolves to a taffy `Dimension`, treating `none` as unbounded (same as `Length::Auto`).
  pub(crate) fn resolve_to_dimension(self, sizing: &SizingContext) -> Dimension {
    match self {
      Self::None => Length::Auto.resolve_to_dimension(sizing),
      Self::Length(length) => length.resolve_to_dimension(sizing),
    }
  }
}

impl ToCss for MaxSize {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::None => dest.write_str("none"),
      Self::Length(length) => length.to_css(dest),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn parses_none() {
    assert_eq!(MaxSize::from_css_str("none"), Ok(MaxSize::None));
  }

  #[test]
  fn parses_auto_as_none() {
    assert_eq!(MaxSize::from_css_str("auto"), Ok(MaxSize::None));
  }

  #[test]
  fn parses_length() {
    assert_eq!(
      MaxSize::from_css_str("10px"),
      Ok(MaxSize::Length(Length::Px(10.0)))
    );
  }

  #[test]
  fn round_trips_to_css() {
    let mut buf = String::new();
    MaxSize::None.to_css(&mut buf).unwrap();
    assert_eq!(buf, "none");

    let mut buf = String::new();
    MaxSize::Length(Length::Px(10.0)).to_css(&mut buf).unwrap();
    assert_eq!(buf, "10px");
  }
}
