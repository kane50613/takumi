use cssparser::match_ignore_ascii_case;

use crate::layout::style::{
  LengthUnit::{self, Em, Rem},
  tw::TailwindPropertyParser,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwFontSize {
  pub(crate) font_size: LengthUnit,
  pub(crate) line_height: Option<LengthUnit>,
}

impl TailwindPropertyParser for TwFontSize {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "xs" => Some(
        Self::new(Rem(0.75), Some(Em(1.0 / 0.75))),
      ),
      "sm" => Some(
        Self::new(Rem(0.875), Some(Em(1.25 / 0.875))),
      ),
      "base" => Some(
        Self::new(Rem(1.0), Some(Em(1.5 / 1.0))),
      ),
      "lg" => Some(
        Self::new(Rem(1.125), Some(Em(1.75 / 1.125))),
      ),
      "xl" => Some(
        Self::new(Rem(1.25), Some(Em(1.75 / 1.25))),
      ),
      "2xl" => Some(
        Self::new(Rem(1.5), Some(Em(2.0 / 1.5))),
      ),
      "3xl" => Some(
        Self::new(Rem(1.875), Some(Em(2.25 / 1.875))),
      ),
      "4xl" => Some(
        Self::new(Rem(2.25), Some(Em(2.5 / 2.25))),
      ),
      "5xl" => Some(
        Self::new(Rem(3.0), Some(Em(1.0))),
      ),
      "6xl" => Some(
        Self::new(Rem(3.75), Some(Em(1.0))),
      ),
      "7xl" => Some(
        Self::new(Rem(4.5), Some(Em(1.0))),
      ),
      "8xl" => Some(
        Self::new(Rem(6.0), Some(Em(1.0))),
      ),
      "9xl" => Some(
        Self::new(Rem(8.0), Some(Em(1.0))),
      ),
      _ => None,
    }
  }
}

impl TwFontSize {
  pub const fn new(font_size: LengthUnit, line_height: Option<LengthUnit>) -> Self {
    Self {
      font_size,
      line_height,
    }
  }
}
