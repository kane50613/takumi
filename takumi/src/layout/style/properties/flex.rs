use cssparser::{Parser, match_ignore_ascii_case};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::layout::style::{
  AspectRatio, FromCss, LengthUnit, ParseResult, tw::TailwindPropertyParser,
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq)]
#[serde(try_from = "FlexValue")]
#[ts(as = "FlexValue")]
/// Represents a flex shorthand property for flex-grow, flex-shrink, and flex-basis.
pub struct Flex {
  /// The flex-grow value.
  pub grow: f32,
  /// The flex-shrink value.
  pub shrink: f32,
  /// The flex-basis value.
  pub basis: LengthUnit,
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
      basis: LengthUnit::Auto,
    }
  }

  /// The flex-grow value is 0.
  pub const fn none() -> Self {
    Self {
      grow: 0.0,
      shrink: 0.0,
      basis: LengthUnit::Auto,
    }
  }

  /// The flex-grow value is 0 and the flex-shrink value is 1.
  pub const fn initial() -> Self {
    Self {
      grow: 0.0,
      shrink: 1.0,
      basis: LengthUnit::Auto,
    }
  }

  /// Create a new Flex from a number.
  pub const fn from_number(number: f32) -> Self {
    Self {
      grow: number,
      shrink: 1.0,
      basis: LengthUnit::zero(),
    }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(untagged)]
pub(crate) enum FlexValue {
  Structured {
    grow: Option<f32>,
    shrink: Option<f32>,
    basis: Option<LengthUnit>,
  },
  Number(f32),
  #[ts(type = "'auto' | 'none' | string")]
  Css(String),
}

impl TryFrom<FlexValue> for Flex {
  type Error = String;

  fn try_from(value: FlexValue) -> Result<Self, Self::Error> {
    match value {
      FlexValue::Structured {
        grow,
        shrink,
        basis,
      } => Ok(Flex {
        grow: grow.unwrap_or(0.0),
        shrink: shrink.unwrap_or(1.0),
        basis: basis.unwrap_or(LengthUnit::Auto),
      }),
      FlexValue::Number(grow) => Ok(Flex::from_number(grow)),
      FlexValue::Css(css) => Flex::from_str(&css).map_err(|e| e.to_string()),
    }
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
      if grow.is_none() {
        if let Ok(val) = input.try_parse(Parser::expect_number) {
          grow = Some(val);
          shrink = input.try_parse(Parser::expect_number).ok();
          continue;
        }
      }

      if basis.is_none() {
        if let Ok(val) = input.try_parse(LengthUnit::from_css) {
          basis = Some(val);
          continue;
        }
      }

      break;
    }

    Ok(Flex {
      grow: grow.unwrap_or(1.0),
      shrink: shrink.unwrap_or(1.0),
      basis: basis.unwrap_or(LengthUnit::zero()),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_flex_three_values() {
    let flex = Flex::from_str("1 1 auto").unwrap();

    assert_eq!(
      flex,
      Flex {
        grow: 1.0,
        shrink: 1.0,
        basis: LengthUnit::Auto
      }
    );
  }

  #[test]
  fn test_flex_single_number() {
    let flex = Flex::from_str("2").unwrap();

    assert_eq!(
      flex,
      Flex {
        grow: 2.0,
        shrink: 1.0,
        basis: LengthUnit::zero()
      }
    );
  }

  #[test]
  fn test_flex_number_and_length() {
    let flex = Flex::from_str("1 30px").unwrap();

    assert_eq!(
      flex,
      Flex {
        grow: 1.0,
        shrink: 1.0,
        basis: LengthUnit::Px(30.0)
      }
    );
  }

  #[test]
  fn test_flex_two_numbers() {
    let flex = Flex::from_str("2 2").unwrap();

    assert_eq!(
      flex,
      Flex {
        grow: 2.0,
        shrink: 2.0,
        basis: LengthUnit::zero()
      }
    );
  }
}
