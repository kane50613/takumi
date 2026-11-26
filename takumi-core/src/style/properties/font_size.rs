use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{
  Animatable, Color, FromCss, Length, MakeComputed, ParseResult, SizingContext, ToCss,
  impl_css_enum, unexpected_token,
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

/// Blink's `kStrictFontSizeTable`, the keyword sizes of each `medium` size from 9px to 16px.
const FONT_SIZE_TABLE: [[u8; 8]; 8] = [
  [9, 9, 9, 9, 11, 14, 18, 27],
  [9, 9, 9, 10, 12, 15, 20, 30],
  [9, 9, 10, 11, 13, 17, 22, 33],
  [9, 9, 10, 12, 14, 18, 24, 36],
  [9, 10, 12, 13, 16, 20, 26, 39],
  [9, 10, 12, 14, 17, 21, 28, 42],
  [9, 10, 13, 15, 18, 23, 30, 45],
  [9, 10, 13, 16, 18, 24, 32, 48],
];

/// Blink's `kFontSizeFactors`, the keyword sizes as multiples of a `medium` size the table lacks.
const FONT_SIZE_FACTORS: [f32; 8] = [0.6, 0.75, 0.89, 1.0, 1.2, 1.5, 2.0, 3.0];

impl FontSizeKeyword {
  /// The keyword's CSS length when `medium` is `medium_size` CSS pixels, as Blink's
  /// `FontSizeFunctions::FontSizeForKeyword` sizes it in standards mode.
  ///
  /// Approximate: `medium` is the viewport's font size for every family, where Blink takes the
  /// smaller default fixed font size, 13px, for the generic `monospace` family, and a size outside
  /// the table is not raised to Blink's minimum logical font size.
  pub(crate) fn to_length(self, medium_size: f32) -> Length {
    let column = self as usize;
    let row = medium_size as usize;

    if medium_size.fract() == 0.0 && (9..=16).contains(&row) {
      return Length::Px(f32::from(FONT_SIZE_TABLE[row - 9][column]));
    }

    Length::Px(FONT_SIZE_FACTORS[column] * medium_size)
  }
}

impl_css_enum!(
  FontSizeKeyword,
  "xx-small" => FontSizeKeyword::XXSmall,
  "x-small" => FontSizeKeyword::XSmall,
  "small" => FontSizeKeyword::Small,
  "medium" => FontSizeKeyword::Medium,
  "large" => FontSizeKeyword::Large,
  "x-large" => FontSizeKeyword::XLarge,
  "xx-large" => FontSizeKeyword::XXLarge,
  "xxx-large" | "-webkit-xxx-large" => FontSizeKeyword::XXXLarge,
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
      Self::Keyword(keyword) => keyword
        .to_length(sizing.viewport.font_size)
        .to_px(sizing, inherited_font_size),
      Self::Length(length) => length.to_px(sizing, inherited_font_size),
      Self::Larger => inherited_font_size * RELATIVE_SIZE_RATIO,
      Self::Smaller => inherited_font_size / RELATIVE_SIZE_RATIO,
    }
  }

  /// The length this size animates as; `larger`/`smaller` scale `1em`.
  fn to_length(self, sizing: &SizingContext) -> Length {
    match self {
      Self::Keyword(keyword) => keyword.to_length(sizing.viewport.font_size),
      Self::Length(length) => length,
      Self::Larger => Length::Em(RELATIVE_SIZE_RATIO),
      Self::Smaller => Length::Em(1.0 / RELATIVE_SIZE_RATIO),
    }
  }

  /// Whether the size is authored as a negative length, which `font-size` rejects.
  pub(crate) fn is_negative(self) -> bool {
    matches!(self, Self::Length(length) if length.is_negative())
  }

  fn parse_relative<'i>(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let token = input.next()?;

    match token {
      Token::Ident(ident) => match_ignore_ascii_case! { ident,
        "larger" => Ok(Self::Larger),
        "smaller" => Ok(Self::Smaller),
        _ => Err(unexpected_token!(token)),
      },
      _ => Err(unexpected_token!(token)),
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
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    input
      .try_parse(FontSizeKeyword::from_css)
      .map(Self::Keyword)
      .or_else(|_| input.try_parse(Self::parse_relative))
      .or_else(|_| Length::parse_length_percentage(input).map(Self::Length))
  }

  const VALID_TOKENS: &'static [&'static str] = &[
    "xx-small",
    "x-small",
    "small",
    "medium",
    "large",
    "x-large",
    "xx-large",
    "xxx-large",
    "larger",
    "smaller",
    "<length>",
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
    *self = Self::Length(Length::interpolated(
      &from.to_length(sizing),
      &to.to_length(sizing),
      progress,
      sizing,
      current_color,
    ));
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

  use super::*;
  use crate::{
    style::{FromCssStr, SizingContext},
    viewport::Viewport,
  };

  #[test]
  fn relative_keywords_scale_inherited_size() {
    let sizing = SizingContext::builder()
      .viewport(Viewport::new((1200, 630)))
      .font_size(20.0)
      .line_height(0.0)
      .ch(20.0 / 2.0)
      .ex(20.0 / 2.0)
      .build();

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
    let sizing = SizingContext::builder()
      .viewport(Viewport::new((1200, 630)))
      .font_size(16.0)
      .line_height(0.0)
      .ch(16.0 / 2.0)
      .ex(16.0 / 2.0)
      .build();

    assert_eq!(FontSize::default().to_px(&sizing, sizing.font_size), 16.0);
  }

  /// <https://drafts.csswg.org/css-fonts-4/#absolute-size-mapping>
  #[test]
  fn keywords_size_from_the_default_font_size_not_the_root() {
    let sizing = SizingContext::builder()
      .viewport(Viewport::new((1200, 630)))
      .font_size(20.0)
      .root_font_size(Some(20.0))
      .line_height(0.0)
      .ch(10.0)
      .ex(10.0)
      .build();
    let px = |css| FontSize::from_css_str(css).unwrap().to_px(&sizing, 20.0);

    assert_eq!(px("medium"), 16.0);
    assert_eq!(px("small"), 13.0);
    assert_eq!(px("xx-large"), 32.0);
  }

  #[test]
  fn rem_font_size_in_descendant_does_not_double_apply_dpr() {
    use crate::viewport::DEFAULT_FONT_SIZE;

    let viewport = Viewport::new((1200, 630)).with_device_pixel_ratio(2.0);
    let root_font_size_device_px = DEFAULT_FONT_SIZE * viewport.device_pixel_ratio;
    let sizing = SizingContext::builder()
      .viewport(viewport)
      .font_size(root_font_size_device_px)
      .root_font_size(Some(root_font_size_device_px))
      .line_height(0.0)
      .ch(root_font_size_device_px / 2.0)
      .ex(root_font_size_device_px / 2.0)
      .build();

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
