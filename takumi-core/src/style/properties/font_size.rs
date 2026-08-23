use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, Length, MakeComputed, ParseResult,
  SizingContext, ToCss, declare_enum_from_css_impl, unexpected_token,
};

/// Absolute `font-size` keywords.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
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
  pub(crate) const fn to_length(self) -> Length {
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

declare_enum_from_css_impl!(
  FontSizeKeyword,
  "xx-small" => FontSizeKeyword::XXSmall,
  "x-small" => FontSizeKeyword::XSmall,
  "small" => FontSizeKeyword::Small,
  "medium" => FontSizeKeyword::Medium,
  "large" => FontSizeKeyword::Large,
  "x-large" => FontSizeKeyword::XLarge,
  "xx-large" => FontSizeKeyword::XXLarge,
  "xxx-large" => FontSizeKeyword::XXXLarge,
);

// https://drafts.csswg.org/css-fonts-4/#valdef-font-size-larger
const RELATIVE_SIZE_RATIO: f32 = 1.2;

/// A `font-size` value: a keyword, a relative keyword, or an explicit length.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum FontSize {
  /// A CSS absolute-size keyword such as `medium`.
  Keyword(FontSizeKeyword),
  /// A concrete CSS length such as `16px` or `1rem`.
  Length(Length),
  /// `larger` — the inherited font size scaled up by 1.2.
  Larger,
  /// `smaller` — the inherited font size scaled down by 1.2.
  Smaller,
}

impl FontSize {
  /// Resolves to pixels against the inherited font size.
  pub fn to_px(self, sizing: &SizingContext, inherited_font_size: f32) -> f32 {
    match self {
      Self::Keyword(keyword) => keyword.to_length().to_px(sizing, inherited_font_size),
      Self::Length(length) => length.to_px(sizing, inherited_font_size),
      Self::Larger => inherited_font_size * RELATIVE_SIZE_RATIO,
      Self::Smaller => inherited_font_size / RELATIVE_SIZE_RATIO,
    }
  }

  fn parse_relative<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    match token {
      Token::Ident(ident) => match_ignore_ascii_case! { ident,
        "larger" => Ok(Self::Larger),
        "smaller" => Ok(Self::Smaller),
        _ => Err(unexpected_token!(location, token)),
      },
      _ => Err(unexpected_token!(location, token)),
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
      .or_else(|_| input.try_parse(Self::parse_relative))
      .or_else(|_| Length::from_css(input).map(Self::Length))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("xx-small"),
    CssToken::Keyword("x-small"),
    CssToken::Keyword("small"),
    CssToken::Keyword("medium"),
    CssToken::Keyword("large"),
    CssToken::Keyword("x-large"),
    CssToken::Keyword("xx-large"),
    CssToken::Keyword("xxx-large"),
    CssToken::Keyword("larger"),
    CssToken::Keyword("smaller"),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

impl MakeComputed for FontSize {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let Self::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}

impl Animatable for FontSize {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    let to_length = |size: Self| match size {
      Self::Keyword(keyword) => keyword.to_length(),
      Self::Length(length) => length,
      Self::Larger => Length::Em(RELATIVE_SIZE_RATIO),
      Self::Smaller => Length::Em(1.0 / RELATIVE_SIZE_RATIO),
    };

    let from_length = to_length(*from);
    let to_length = to_length(*to);

    let mut value = from_length;
    value.interpolate(&from_length, &to_length, progress, sizing, current_color);
    *self = Self::Length(value);
  }
}

impl ToCss for FontSize {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Keyword(k) => k.to_css(dest),
      Self::Length(l) => l.to_css(dest),
      Self::Larger => dest.write_str("larger"),
      Self::Smaller => dest.write_str("smaller"),
    }
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::*;
  use crate::{
    geometry::Size,
    style::{CalcArena, FromCssStr, SizingContext},
    viewport::Viewport,
  };

  #[test]
  fn relative_keywords_scale_inherited_size() {
    let sizing = SizingContext {
      viewport: Viewport::new((1200, 630)),
      container_size: Size::NONE,
      container_read: Default::default(),
      font_size: 20.0,
      root_font_size: None,
      line_height: 0.0,
      root_line_height: None,
      calc_arena: Rc::new(CalcArena::default()),
    };

    assert_eq!(FontSize::from_css_str("larger"), Ok(FontSize::Larger));
    assert_eq!(FontSize::from_css_str("smaller"), Ok(FontSize::Smaller));
    assert_eq!(FontSize::Larger.to_px(&sizing, 20.0), 24.0);
    assert_eq!(FontSize::Smaller.to_px(&sizing, 20.0), 20.0 / 1.2);
  }

  #[test]
  fn defaults_to_medium_keyword() {
    assert_eq!(
      FontSize::default(),
      FontSize::Keyword(FontSizeKeyword::Medium)
    );
  }

  #[test]
  fn resolves_medium_keyword_to_default_font_size() {
    let sizing = SizingContext {
      viewport: Viewport::new((1200, 630)),
      container_size: Size::NONE,
      container_read: Default::default(),
      font_size: 16.0,
      root_font_size: None,
      line_height: 0.0,
      root_line_height: None,
      calc_arena: Rc::new(CalcArena::default()),
    };

    assert_eq!(FontSize::default().to_px(&sizing, sizing.font_size), 16.0);
  }

  #[test]
  fn rem_font_size_in_descendant_does_not_double_apply_dpr() {
    use crate::viewport::DEFAULT_FONT_SIZE;

    let viewport = Viewport::new((1200, 630)).with_device_pixel_ratio(2.0);
    let root_font_size_device_px = DEFAULT_FONT_SIZE * viewport.device_pixel_ratio;
    let sizing = SizingContext {
      viewport,
      container_size: Size::NONE,
      container_read: Default::default(),
      font_size: root_font_size_device_px,
      root_font_size: Some(root_font_size_device_px),
      line_height: 0.0,
      root_line_height: None,
      calc_arena: Rc::new(CalcArena::default()),
    };

    assert_eq!(
      FontSize::Length(Length::Rem(0.5)).to_px(&sizing, sizing.font_size),
      16.0
    );
    assert_eq!(
      FontSize::Length(Length::Rem(1.0)).to_px(&sizing, sizing.font_size),
      32.0
    );
  }
}
