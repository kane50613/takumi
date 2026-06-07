use crate::style::unexpected_token;
use cssparser::{Parser, match_ignore_ascii_case};

use crate::{
  style::SizingContext,
  style::{
    AspectRatio, CssSyntaxKind, CssToken, FlexDirection, FlexWrap, FromCss, Length, MakeComputed,
    ParseResult, tw::TailwindPropertyParser,
  },
};

#[derive(Debug, Clone, Copy, PartialEq)]
/// Represents a flex shorthand property for flex-grow, flex-shrink, and flex-basis.
#[non_exhaustive]
pub struct Flex {
  /// The flex-grow value.
  pub grow: f32,
  /// The flex-shrink value.
  pub shrink: f32,
  /// The flex-basis value.
  pub basis: Length,
}

impl TailwindPropertyParser for Flex {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "auto" => return Some(Flex::auto()),
      "none" => return Some(Flex::none()),
      "initial" => return Some(Flex::initial()),
      _ => {}
    }

    let Ok(AspectRatio::Ratio(ratio)) = AspectRatio::from_str(token) else {
      return None;
    };

    Some(Flex::from_number(ratio))
  }
}

impl Flex {
  /// The flex-grow value is 1.
  pub const fn auto() -> Self {
    Self {
      grow: 1.0,
      shrink: 1.0,
      basis: Length::Auto,
    }
  }

  /// The flex-grow value is 0.
  pub const fn none() -> Self {
    Self {
      grow: 0.0,
      shrink: 0.0,
      basis: Length::Auto,
    }
  }

  /// The flex-grow value is 0 and the flex-shrink value is 1.
  pub const fn initial() -> Self {
    Self {
      grow: 0.0,
      shrink: 1.0,
      basis: Length::Auto,
    }
  }

  /// Create a new Flex from a number.
  pub const fn from_number(number: f32) -> Self {
    Self {
      grow: number,
      shrink: 1.0,
      basis: Length::zero(),
    }
  }
}

/// Represents the `flex-flow` shorthand.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FlexFlow {
  /// The flex direction.
  pub direction: FlexDirection,
  /// The flex wrapping mode.
  pub wrap: FlexWrap,
}

impl<'i> FromCss<'i> for FlexFlow {
  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("column"),
    CssToken::Keyword("column-reverse"),
    CssToken::Keyword("nowrap"),
    CssToken::Keyword("row"),
    CssToken::Keyword("row-reverse"),
    CssToken::Keyword("wrap"),
    CssToken::Keyword("wrap-reverse"),
  ];

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut direction = None;
    let mut wrap = None;

    while !input.is_exhausted() {
      if direction.is_none()
        && let Ok(value) = input.try_parse(FlexDirection::from_css)
      {
        direction = Some(value);
        continue;
      }

      if wrap.is_none()
        && let Ok(value) = input.try_parse(FlexWrap::from_css)
      {
        wrap = Some(value);
        continue;
      }

      return Err(unexpected_token!(
        input.current_source_location(),
        input.next()?,
      ));
    }

    Ok(Self {
      direction: direction.unwrap_or_default(),
      wrap: wrap.unwrap_or_default(),
    })
  }
}

impl<'i> FromCss<'i> for Flex {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    // https://developer.mozilla.org/en-US/docs/Web/CSS/flex#values
    if input
      .try_parse(|input| input.expect_ident_matching("none"))
      .is_ok()
    {
      return Ok(Flex::none());
    }

    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Ok(Flex::auto());
    }

    // https://developer.mozilla.org/en-US/docs/Web/CSS/flex#syntax
    let mut grow = None;
    let mut shrink = None;
    let mut basis = None;

    loop {
      if grow.is_none()
        && let Ok(val) = input.try_parse(Parser::expect_number)
      {
        grow = Some(val);
        shrink = input.try_parse(Parser::expect_number).ok();
        continue;
      }

      if basis.is_none()
        && let Ok(val) = input.try_parse(Length::from_css)
      {
        basis = Some(val);
        continue;
      }

      break;
    }

    Ok(Flex {
      grow: grow.unwrap_or(1.0),
      shrink: shrink.unwrap_or(1.0),
      basis: basis.unwrap_or(Length::zero()),
    })
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("none"),
    CssToken::Keyword("auto"),
    CssToken::Syntax(CssSyntaxKind::Number),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

impl MakeComputed for Flex {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.basis.make_computed(sizing);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_flex_three_values() {
    assert_eq!(
      Flex::from_str("1 1 auto"),
      Ok(Flex {
        grow: 1.0,
        shrink: 1.0,
        basis: Length::Auto
      })
    );
  }

  #[test]
  fn test_flex_single_number() {
    assert_eq!(
      Flex::from_str("2"),
      Ok(Flex {
        grow: 2.0,
        shrink: 1.0,
        basis: Length::zero()
      })
    );
  }

  #[test]
  fn test_flex_number_and_length() {
    assert_eq!(
      Flex::from_str("1 30px"),
      Ok(Flex {
        grow: 1.0,
        shrink: 1.0,
        basis: Length::Px(30.0)
      })
    );
  }

  #[test]
  fn test_flex_two_numbers() {
    assert_eq!(
      Flex::from_str("2 2"),
      Ok(Flex {
        grow: 2.0,
        shrink: 2.0,
        basis: Length::zero()
      })
    );
  }
}
