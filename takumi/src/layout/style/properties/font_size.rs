use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::{
  layout::style::{Animatable, CssToken, FromCss, Length, MakeComputed, ParseResult},
  rendering::Sizing,
};

/// Absolute `font-size` keywords.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontSizeKeyword {
  /// Maps to the `xx-small` keyword.
  XXSmall,
  /// Maps to the `x-small` keyword.
  XSmall,
  /// Maps to the `small` keyword.
  Small,
  /// Maps to the `medium` keyword.
  #[default]
  Medium,
  /// Maps to the `large` keyword.
  Large,
  /// Maps to the `x-large` keyword.
  XLarge,
  /// Maps to the `xx-large` keyword.
  XXLarge,
  /// Maps to the `xxx-large` keyword.
  XXXLarge,
}

impl FontSizeKeyword {
  /// Resolves the keyword to its root-relative CSS length.
  pub const fn to_length(self) -> Length {
    match self {
      Self::XXSmall => Length::Rem(0.6),
      Self::XSmall => Length::Rem(0.75),
      Self::Small => Length::Rem(8.0 / 9.0),
      Self::Medium => Length::Rem(1.0),
      Self::Large => Length::Rem(1.2),
      Self::XLarge => Length::Rem(1.5),
      Self::XXLarge => Length::Rem(2.0),
      Self::XXXLarge => Length::Rem(3.0),
    }
  }
}

impl<'i> FromCss<'i> for FontSizeKeyword {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    match token {
      Token::Ident(ident) => match_ignore_ascii_case! { ident,
        "xx-small" => Ok(Self::XXSmall),
        "x-small" => Ok(Self::XSmall),
        "small" => Ok(Self::Small),
        "medium" => Ok(Self::Medium),
        "large" => Ok(Self::Large),
        "x-large" => Ok(Self::XLarge),
        "xx-large" => Ok(Self::XXLarge),
        "xxx-large" => Ok(Self::XXXLarge),
        _ => Err(Self::unexpected_token_error(location, token)),
      },
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("xx-small"),
      CssToken::Keyword("x-small"),
      CssToken::Keyword("small"),
      CssToken::Keyword("medium"),
      CssToken::Keyword("large"),
      CssToken::Keyword("x-large"),
      CssToken::Keyword("xx-large"),
      CssToken::Keyword("xxx-large"),
    ]
  }
}

/// A `font-size` value, either a keyword or an explicit length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontSize {
  /// A CSS absolute-size keyword such as `medium`.
  Keyword(FontSizeKeyword),
  /// A concrete CSS length such as `16px` or `1rem`.
  Length(Length),
}

impl FontSize {
  pub(crate) fn to_px(self, sizing: &Sizing, inherited_font_size: f32) -> f32 {
    match self {
      Self::Keyword(keyword) => keyword.to_length().to_px(sizing, inherited_font_size),
      Self::Length(length) => length.to_px(sizing, inherited_font_size),
    }
  }
}

impl Default for FontSize {
  fn default() -> Self {
    Self::Keyword(FontSizeKeyword::Medium)
  }
}

impl From<Length> for FontSize {
  fn from(value: Length) -> Self {
    Self::Length(value)
  }
}

impl From<FontSizeKeyword> for FontSize {
  fn from(value: FontSizeKeyword) -> Self {
    Self::Keyword(value)
  }
}

impl<'i> FromCss<'i> for FontSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    input
      .try_parse(FontSizeKeyword::from_css)
      .map(Self::Keyword)
      .or_else(|_| Length::from_css(input).map(Self::Length))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("xx-small"),
      CssToken::Keyword("x-small"),
      CssToken::Keyword("small"),
      CssToken::Keyword("medium"),
      CssToken::Keyword("large"),
      CssToken::Keyword("x-large"),
      CssToken::Keyword("xx-large"),
      CssToken::Keyword("xxx-large"),
      CssToken::Token("length"),
    ]
  }
}

impl MakeComputed for FontSize {
  fn make_computed(&mut self, sizing: &Sizing) {
    if let Self::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}

impl Animatable for FontSize {}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use taffy::Size;

  use super::*;
  use crate::{
    layout::{style::CalcArena, viewport::Viewport},
    rendering::Sizing,
  };

  #[test]
  fn defaults_to_medium_keyword() {
    assert_eq!(
      FontSize::default(),
      FontSize::Keyword(FontSizeKeyword::Medium)
    );
  }

  #[test]
  fn resolves_medium_keyword_to_default_font_size() {
    let sizing = Sizing {
      viewport: Viewport::new(Some(1200), Some(630)),
      container_size: Size::NONE,
      font_size: 16.0,
      calc_arena: Rc::new(CalcArena::default()),
    };

    assert_eq!(FontSize::default().to_px(&sizing, sizing.font_size), 16.0);
  }
}
