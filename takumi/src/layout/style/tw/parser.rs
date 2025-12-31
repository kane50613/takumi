use std::ops::Neg;

use cssparser::{Parser, match_ignore_ascii_case};

use crate::layout::style::{
  CssToken,
  Length::{self, *},
  tw::TailwindPropertyParser,
  *,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwFontSize {
  pub(crate) font_size: Length,
  pub(crate) line_height: Option<LineHeight>,
}

impl<'i> FromCss<'i> for TwFontSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self::new(Length::from_css(input)?, None))
  }

  fn valid_tokens() -> &'static [CssToken] {
    Length::<true>::valid_tokens()
  }
}

impl TailwindPropertyParser for TwFontSize {
  fn parse_tw(token: &str) -> Option<Self> {
    // text-xs/6 => font-size: 0.75rem, line-height: 1.5em
    if let Some((font_size, line_height)) = token.split_once('/') {
      return Some(Self::new(
        TwFontSize::parse_tw(font_size)?.font_size,
        Some(LineHeight::parse_tw(line_height)?),
      ));
    }

    match_ignore_ascii_case! {token,
      "xs" => Some(
        Self::new(Rem(0.75), Some(LineHeight(Em(1.0 / 0.75)))),
      ),
      "sm" => Some(
        Self::new(Rem(0.875), Some(LineHeight(Em(1.25 / 0.875)))),
      ),
      "base" => Some(
        Self::new(Rem(1.0), Some(LineHeight(Em(1.5 / 1.0)))),
      ),
      "lg" => Some(
        Self::new(Rem(1.125), Some(LineHeight(Em(1.75 / 1.125)))),
      ),
      "xl" => Some(
        Self::new(Rem(1.25), Some(LineHeight(Em(1.75 / 1.25)))),
      ),
      "2xl" => Some(
        Self::new(Rem(1.5), Some(LineHeight(Em(2.0 / 1.5)))),
      ),
      "3xl" => Some(
        Self::new(Rem(1.875), Some(LineHeight(Em(2.25 / 1.875)))),
      ),
      "4xl" => Some(
        Self::new(Rem(2.25), Some(LineHeight(Em(2.5 / 2.25)))),
      ),
      "5xl" => Some(
        Self::new(Rem(3.0), Some(LineHeight(Em(1.0)))),
      ),
      "6xl" => Some(
        Self::new(Rem(3.75), Some(LineHeight(Em(1.0)))),
      ),
      "7xl" => Some(
        Self::new(Rem(4.5), Some(LineHeight(Em(1.0)))),
      ),
      "8xl" => Some(
        Self::new(Rem(6.0), Some(LineHeight(Em(1.0)))),
      ),
      "9xl" => Some(
        Self::new(Rem(8.0), Some(LineHeight(Em(1.0)))),
      ),
      _ => None,
    }
  }
}

impl TwFontSize {
  pub const fn new(font_size: Length, line_height: Option<LineHeight>) -> Self {
    Self {
      font_size,
      line_height,
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TwGridTemplate(pub GridTemplateComponents);

impl<'i> FromCss<'i> for TwGridTemplate {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self(GridTemplateComponents::from_css(input)?))
  }

  fn valid_tokens() -> &'static [CssToken] {
    GridTemplateComponents::valid_tokens()
  }
}

impl TailwindPropertyParser for TwGridTemplate {
  fn parse_tw(token: &str) -> Option<Self> {
    let count = token.parse::<u32>().ok()?;

    // Create repeat(count, minmax(0, 1fr))
    let track_sizes = vec![
      GridTrackSize::MinMax(GridMinMaxSize {
        min: GridLength::Unit(Length::Px(0.0)),
        max: GridLength::Fr(1.0),
      });
      count as usize
    ];

    let template_components = track_sizes
      .into_iter()
      .map(GridTemplateComponent::Single)
      .collect();

    Some(TwGridTemplate(template_components))
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwLetterSpacing(pub Length);

impl<'i> FromCss<'i> for TwLetterSpacing {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self(Length::from_css(input)?))
  }

  fn valid_tokens() -> &'static [CssToken] {
    Length::<true>::valid_tokens()
  }
}

impl Neg for TwLetterSpacing {
  type Output = Self;

  fn neg(self) -> Self::Output {
    TwLetterSpacing(-self.0)
  }
}

impl TailwindPropertyParser for TwLetterSpacing {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "tighter" => Some(TwLetterSpacing(Length::Em(-0.05))),
      "tight" => Some(TwLetterSpacing(Length::Em(-0.025))),
      "normal" => Some(TwLetterSpacing(Length::Em(0.0))),
      "wide" => Some(TwLetterSpacing(Length::Em(0.025))),
      "wider" => Some(TwLetterSpacing(Length::Em(0.05))),
      "widest" => Some(TwLetterSpacing(Length::Em(0.1))),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwBorderWidth(pub Length);

impl<'i> FromCss<'i> for TwBorderWidth {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self(Length::from_css(input)?))
  }

  fn valid_tokens() -> &'static [CssToken] {
    Length::<true>::valid_tokens()
  }
}

impl TailwindPropertyParser for TwBorderWidth {
  fn parse_tw(token: &str) -> Option<Self> {
    let value = token.parse::<f32>().ok()?;

    Some(TwBorderWidth(Length::Px(value)))
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwRounded(pub(crate) Length<false>);

impl<'i> FromCss<'i> for TwRounded {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(TwRounded(Length::from_css(input)?))
  }

  fn valid_tokens() -> &'static [CssToken] {
    Length::<false>::valid_tokens()
  }
}

impl TailwindPropertyParser for TwRounded {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "full" => Some(TwRounded(Length::Px(9999.0))),
      "none" => Some(TwRounded(Length::Px(0.0))),
      "xs" => Some(TwRounded(Length::Rem(0.125))),
      "sm" => Some(TwRounded(Length::Rem(0.25))),
      "md" => Some(TwRounded(Length::Rem(0.375))),
      "lg" => Some(TwRounded(Length::Rem(0.5))),
      "xl" => Some(TwRounded(Length::Rem(0.75))),
      "2xl" => Some(TwRounded(Length::Rem(1.0))),
      "3xl" => Some(TwRounded(Length::Rem(1.5))),
      "4xl" => Some(TwRounded(Length::Rem(2.0))),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwBlur(pub(crate) Length);

impl<'i> FromCss<'i> for TwBlur {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(TwBlur(Length::from_css(input)?))
  }

  fn valid_tokens() -> &'static [CssToken] {
    Length::<true>::valid_tokens()
  }
}

impl TailwindPropertyParser for TwBlur {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "none" => Some(TwBlur(Length::Px(0.0))),
      "xs" => Some(TwBlur(Length::Px(4.0))),
      "sm" => Some(TwBlur(Length::Px(8.0))),
      "md" => Some(TwBlur(Length::Px(12.0))),
      "lg" => Some(TwBlur(Length::Px(16.0))),
      "xl" => Some(TwBlur(Length::Px(24.0))),
      "2xl" => Some(TwBlur(Length::Px(40.0))),
      "3xl" => Some(TwBlur(Length::Px(64.0))),
      _ => None,
    }
  }
}
