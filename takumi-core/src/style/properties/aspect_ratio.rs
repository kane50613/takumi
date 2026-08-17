use std::fmt;

use cssparser::Parser;

use crate::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, FromCssStr, MakeComputed, ParseResult,
  SizingContext, ToCss, lerp, tw::TailwindPropertyParser,
};

#[derive(Default, Debug, Clone, Copy, PartialEq)]
/// Represents a aspect ratio.
#[non_exhaustive]
pub enum AspectRatio {
  /// The aspect ratio is determined by the content.
  #[default]
  Auto,
  /// The aspect ratio is a fixed ratio.
  Ratio(f32),
}

impl AspectRatio {
  /// A degenerate ratio (zero, negative, or non-finite) behaves as `auto`.
  fn ratio(value: f32) -> Self {
    if value.is_finite() && value > 0.0 {
      Self::Ratio(value)
    } else {
      Self::Auto
    }
  }
}

impl MakeComputed for AspectRatio {}

impl Animatable for AspectRatio {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &SizingContext,
    _current_color: Color,
  ) {
    *self = match (*from, *to) {
      (AspectRatio::Ratio(lhs), AspectRatio::Ratio(rhs)) => {
        AspectRatio::Ratio(lerp(lhs, rhs, progress))
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

impl TailwindPropertyParser for AspectRatio {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_css_str(token).ok()
  }
}

impl From<AspectRatio> for Option<f32> {
  fn from(value: AspectRatio) -> Self {
    match value {
      AspectRatio::Auto => None,
      AspectRatio::Ratio(ratio) => Some(ratio),
    }
  }
}

impl<'i> FromCss<'i> for AspectRatio {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Ok(AspectRatio::Auto);
    }

    let first_ratio = input.expect_number()?;

    if input.try_parse(|input| input.expect_delim('/')).is_err() {
      return Ok(AspectRatio::ratio(first_ratio));
    }

    let second_ratio = input.expect_number()?;
    Ok(AspectRatio::ratio(first_ratio / second_ratio))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("auto"),
    CssToken::Syntax(CssSyntaxKind::Number),
  ];
}

impl ToCss for AspectRatio {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Auto => dest.write_str("auto"),
      Self::Ratio(v) => write!(dest, "{}", v),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn degenerate_ratios_behave_as_auto() {
    assert_eq!(AspectRatio::from_css_str("1/0"), Ok(AspectRatio::Auto));
    assert_eq!(AspectRatio::from_css_str("0/0"), Ok(AspectRatio::Auto));
    assert_eq!(AspectRatio::from_css_str("0"), Ok(AspectRatio::Auto));
    assert_eq!(AspectRatio::from_css_str("-2"), Ok(AspectRatio::Auto));
  }

  #[test]
  fn parses_auto_keyword() {
    assert_eq!(AspectRatio::from_css_str("auto"), Ok(AspectRatio::Auto));
  }

  #[test]
  fn parses_single_number_as_ratio() {
    assert_eq!(
      AspectRatio::from_css_str("1.5"),
      Ok(AspectRatio::Ratio(1.5))
    );
  }

  #[test]
  fn parses_ratio_with_slash() {
    assert_eq!(
      AspectRatio::from_css_str("16/9"),
      Ok(AspectRatio::Ratio(16.0 / 9.0))
    );
  }

  #[test]
  fn parses_ratio_with_decimal_values() {
    assert_eq!(
      AspectRatio::from_css_str("1.777/1"),
      Ok(AspectRatio::Ratio(1.777))
    );
  }

  #[test]
  fn errors_on_invalid_input() {
    assert!(AspectRatio::from_css_str("invalid").is_err());
  }

  #[test]
  fn errors_on_empty_slash() {
    assert!(AspectRatio::from_css_str("16/").is_err());
  }
}
