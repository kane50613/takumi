use std::fmt;

use cssparser::Parser;
use taffy::Dimension;

use crate::style::{
  Animatable, Color, CssToken, FromCss, Length, MakeComputed, ParseResult, SizingContext, ToCss,
};

/// Represents a `width`/`height`/`flex-basis` value.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum SizeValue {
  /// A `<length-percentage>` or `auto`.
  Length(Length),
}

impl Default for SizeValue {
  fn default() -> Self {
    Self::Length(Length::Auto)
  }
}

impl SizeValue {
  /// A zero length.
  pub const fn zero() -> Self {
    Self::Length(Length::zero())
  }

  /// Whether the value leaves the size to the layout algorithm.
  pub fn is_auto(self) -> bool {
    matches!(self, Self::Length(Length::Auto))
  }

  /// The length this value carries, if it is one.
  pub fn as_length(self) -> Option<Length> {
    match self {
      Self::Length(length) => Some(length),
    }
  }

  /// Resolves to a taffy `Dimension`.
  pub(crate) fn resolve_to_dimension(self, sizing: &SizingContext) -> Dimension {
    match self {
      Self::Length(length) => length.resolve_to_dimension(sizing),
    }
  }
}

impl From<Length> for SizeValue {
  fn from(length: Length) -> Self {
    Self::Length(length)
  }
}

impl MakeComputed for SizeValue {
  fn make_computed(&mut self, sizing: &SizingContext) {
    match self {
      Self::Length(length) => length.make_computed(sizing),
    }
  }
}

impl Animatable for SizeValue {
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
    };
  }
}

impl<'i> FromCss<'i> for SizeValue {
  const VALID_TOKENS: &'static [CssToken] = Length::VALID_TOKENS;

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Length::from_css(input).map(Self::Length)
  }
}

impl ToCss for SizeValue {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Length(length) => length.to_css(dest),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn parses_lengths_and_auto() {
    for (css, expected) in [
      ("auto", SizeValue::Length(Length::Auto)),
      ("100px", SizeValue::Length(Length::Px(100.0))),
      ("50%", SizeValue::Length(Length::Percentage(50.0))),
    ] {
      let parsed = SizeValue::from_css_str(css).unwrap();

      assert_eq!(parsed, expected, "{css}");
      assert_eq!(parsed.to_css_string(), css);
    }
  }
}
