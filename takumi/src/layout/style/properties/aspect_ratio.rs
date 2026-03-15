use cssparser::Parser;

use crate::layout::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, lerp,
  tw::TailwindPropertyParser,
};
use crate::rendering::Sizing;

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

impl MakeComputed for AspectRatio {}

impl Animatable for AspectRatio {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &Sizing,
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
    Self::from_str(token).ok()
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
      return Ok(AspectRatio::Ratio(first_ratio));
    }

    let second_ratio = input.expect_number()?;
    Ok(AspectRatio::Ratio(first_ratio / second_ratio))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("auto"),
    CssToken::Syntax(CssSyntaxKind::Number),
  ];
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_auto_keyword() {
    assert_eq!(AspectRatio::from_str("auto"), Ok(AspectRatio::Auto));
  }

  #[test]
  fn parses_single_number_as_ratio() {
    assert_eq!(AspectRatio::from_str("1.5"), Ok(AspectRatio::Ratio(1.5)));
  }

  #[test]
  fn parses_ratio_with_slash() {
    assert_eq!(
      AspectRatio::from_str("16/9"),
      Ok(AspectRatio::Ratio(16.0 / 9.0))
    );
  }

  #[test]
  fn parses_ratio_with_decimal_values() {
    assert_eq!(
      AspectRatio::from_str("1.777/1"),
      Ok(AspectRatio::Ratio(1.777))
    );
  }

  #[test]
  fn errors_on_invalid_input() {
    assert!(AspectRatio::from_str("invalid").is_err());
  }

  #[test]
  fn errors_on_empty_slash() {
    assert!(AspectRatio::from_str("16/").is_err());
  }
}
