use std::{fmt, mem};

use cssparser::{BasicParseErrorKind, Parser, Token, match_ignore_ascii_case};

use crate::style::{
  Animatable, AspectRatio, CssSyntaxKind, CssToken, FlexDirection, FromCss, FromCssStr, Length,
  MakeComputed, ParseResult, SizingContext, ToCss, tw::TailwindPropertyParser, unexpected_token,
};

/// Whether flex items wrap onto more than one line.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum FlexWrap {
  /// One line; items shrink to fit it.
  #[default]
  NoWrap,
  /// Many lines, stacked along the flex direction.
  Wrap,
  /// Many lines, stacked against the flex direction.
  WrapReverse,
  /// Like `Wrap`, with items spread so the longest line is as short as possible.
  Balance,
  /// Like `WrapReverse`, with items spread so the longest line is as short as possible.
  BalanceReverse,
}

impl MakeComputed for FlexWrap {}

impl Animatable for FlexWrap {}

impl FlexWrap {
  pub(crate) fn into_taffy(self) -> taffy::FlexWrap {
    match self {
      Self::NoWrap => taffy::FlexWrap::NoWrap,
      Self::Wrap => taffy::FlexWrap::Wrap,
      Self::WrapReverse => taffy::FlexWrap::WrapReverse,
      Self::Balance => taffy::FlexWrap::Balance,
      Self::BalanceReverse => taffy::FlexWrap::BalanceReverse,
    }
  }
}

impl<'i> FromCss<'i> for FlexWrap {
  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("nowrap"),
    CssToken::Keyword("wrap"),
    CssToken::Keyword("wrap-reverse"),
    CssToken::Keyword("balance"),
  ];

  // `[ nowrap | wrap | wrap-reverse ] || balance`, per CSS Flexbox Level 2.
  // <https://drafts.csswg.org/css-flexbox-2/#balance-values>
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut wrap = None;
    let mut balance = false;

    loop {
      let state = input.state();
      let location = input.current_source_location();
      let Ok(token) = input.next().cloned() else {
        break;
      };
      let Token::Ident(ident) = &token else {
        input.reset(&state);
        break;
      };

      let accepted = match_ignore_ascii_case! {ident,
        // `nowrap` stands alone: it names a single-line container, which no
        // line count can balance.
        "nowrap" => {
          if wrap.is_none() && !balance {
            return Ok(Self::NoWrap);
          }
          false
        },
        "wrap" => wrap.replace(Self::Wrap).is_none(),
        "wrap-reverse" => wrap.replace(Self::WrapReverse).is_none(),
        "balance" => !mem::replace(&mut balance, true),
        _ => {
          input.reset(&state);
          break;
        },
      };

      if !accepted {
        return Err(unexpected_token!(location, &token));
      }
    }

    match (wrap, balance) {
      (Some(Self::WrapReverse), true) => Ok(Self::BalanceReverse),
      (_, true) => Ok(Self::Balance),
      (Some(wrap), false) => Ok(wrap),
      (None, false) => Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid)),
    }
  }
}

impl ToCss for FlexWrap {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    dest.write_str(match self {
      Self::NoWrap => "nowrap",
      Self::Wrap => "wrap",
      Self::WrapReverse => "wrap-reverse",
      Self::Balance => "wrap balance",
      Self::BalanceReverse => "wrap-reverse balance",
    })
  }
}

/// The fewest flex lines a multi-line container is laid out into.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexLineCount(u16);

impl Default for FlexLineCount {
  fn default() -> Self {
    Self(1)
  }
}

impl FlexLineCount {
  pub(crate) fn get(self) -> u16 {
    self.0
  }
}

impl MakeComputed for FlexLineCount {}

impl Animatable for FlexLineCount {}

impl<'i> FromCss<'i> for FlexLineCount {
  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Syntax(CssSyntaxKind::Number)];

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let count = input.expect_integer()?;

    if count < 1 {
      return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
    }

    Ok(Self(count.min(u16::MAX as i32) as u16))
  }
}

impl ToCss for FlexLineCount {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "{}", self.0)
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Represents a flex shorthand property for flex-grow, flex-shrink, and flex-basis.
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

    let Ok(AspectRatio::Ratio(ratio)) = AspectRatio::from_css_str(token) else {
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
  pub(crate) const fn initial() -> Self {
    Self {
      grow: 0.0,
      shrink: 1.0,
      basis: Length::Auto,
    }
  }

  /// Create a new Flex from a number.
  pub(crate) const fn from_number(number: f32) -> Self {
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
      Flex::from_css_str("1 1 auto"),
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
      Flex::from_css_str("2"),
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
      Flex::from_css_str("1 30px"),
      Ok(Flex {
        grow: 1.0,
        shrink: 1.0,
        basis: Length::Px(30.0)
      })
    );
  }

  #[test]
  fn flex_wrap_takes_balance_in_either_order() {
    for (css, expected) in [
      ("nowrap", FlexWrap::NoWrap),
      ("wrap", FlexWrap::Wrap),
      ("wrap-reverse", FlexWrap::WrapReverse),
      ("balance", FlexWrap::Balance),
      ("wrap balance", FlexWrap::Balance),
      ("balance wrap", FlexWrap::Balance),
      ("wrap-reverse balance", FlexWrap::BalanceReverse),
      ("BALANCE WRAP-REVERSE", FlexWrap::BalanceReverse),
    ] {
      assert_eq!(FlexWrap::from_css_str(css), Ok(expected), "{css}");
    }

    assert_eq!(FlexWrap::Balance.to_css_string(), "wrap balance");
    assert_eq!(
      FlexWrap::BalanceReverse.to_css_string(),
      "wrap-reverse balance"
    );
    assert!(FlexWrap::from_css_str("balance balance").is_err());
    assert!(FlexWrap::from_css_str("wrap wrap-reverse").is_err());
    assert!(FlexWrap::from_css_str("balance nowrap").is_err());
    assert!(FlexWrap::from_css_str("10px").is_err());
  }

  /// `flex-flow` reads a wrap keyword out of a longer value, so an unknown
  /// keyword has to end the wrap value instead of failing it.
  #[test]
  fn flex_flow_pairs_balance_with_a_direction() {
    assert_eq!(
      FlexFlow::from_css_str("column wrap balance"),
      Ok(FlexFlow {
        direction: FlexDirection::Column,
        wrap: FlexWrap::Balance,
      })
    );
    assert_eq!(
      FlexFlow::from_css_str("wrap row"),
      Ok(FlexFlow {
        direction: FlexDirection::Row,
        wrap: FlexWrap::Wrap,
      })
    );
  }

  #[test]
  fn flex_line_count_takes_a_positive_integer() {
    assert_eq!(FlexLineCount::from_css_str("3"), Ok(FlexLineCount(3)));
    assert_eq!(FlexLineCount::default(), FlexLineCount(1));
    assert_eq!(FlexLineCount(4).to_css_string(), "4");
    assert!(FlexLineCount::from_css_str("0").is_err());
    assert!(FlexLineCount::from_css_str("-1").is_err());
    assert!(FlexLineCount::from_css_str("1.5").is_err());
  }

  #[test]
  fn test_flex_two_numbers() {
    assert_eq!(
      Flex::from_css_str("2 2"),
      Ok(Flex {
        grow: 2.0,
        shrink: 2.0,
        basis: Length::zero()
      })
    );
  }
}
