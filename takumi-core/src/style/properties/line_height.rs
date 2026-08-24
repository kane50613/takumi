use cssparser::{Parser, match_ignore_ascii_case};

use crate::style::TwNamespace;
use crate::style::{
  CssSyntaxKind, CssToken, FromCss, Length, MakeComputed, ParseResult, SizingContext, ToCss,
  parse_calc_number_expression, tw::TailwindPropertyParser,
};

/// Represents a line height value.
#[derive(Debug, Clone, PartialEq, Copy, Default)]
#[non_exhaustive]
pub enum LineHeight {
  /// Normal line height.
  #[default]
  Normal,
  /// A unitless line height which is relative to the font size.
  Unitless(f32),
  /// A specific line height.
  Length(Length),
}

impl From<Length> for LineHeight {
  fn from(value: Length) -> Self {
    Self::Length(value)
  }
}

impl TailwindPropertyParser for LineHeight {
  const NAMESPACES: &'static [TwNamespace] = &[TwNamespace::Leading];

  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {&token,
      "none" => Some(LineHeight::Unitless(1.0)),
      "tight" => Some(LineHeight::Unitless(1.25)),
      "snug" => Some(LineHeight::Unitless(1.375)),
      "normal" => Some(LineHeight::Unitless(1.5)),
      "relaxed" => Some(LineHeight::Unitless(1.625)),
      "loose" => Some(LineHeight::Unitless(2.0)),
      _ => {
        let Ok(value) = token.parse::<f32>() else {
          return None;
        };

        Some(LineHeight::Length(Length::from_spacing(value)))
      }
    }
  }
}

impl<'i> FromCss<'i> for LineHeight {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("normal"))
      .is_ok()
    {
      return Ok(Self::Normal);
    }

    if let Ok(number) = input.try_parse(parse_calc_number_expression) {
      return Ok(LineHeight::Unitless(number));
    }

    if let Ok(percent) = input.try_parse(Parser::expect_percentage) {
      return Ok(LineHeight::Length(Length::Percentage(percent * 100.0)));
    }

    let Ok(number) = input.try_parse(Parser::expect_number) else {
      return Length::from_css(input).map(Into::into);
    };

    Ok(LineHeight::Unitless(number))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Number),
    CssToken::Syntax(CssSyntaxKind::Length),
    CssToken::Syntax(CssSyntaxKind::Percentage),
  ];
}

impl LineHeight {
  // Match Blink text-fit line-height scaling: non-fixed line heights scale, fixed and percentage line heights do not.
  // Reference: https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_box_state.cc;l=137
  /// Whether the line height scales under text-fit (non-fixed values).
  pub(crate) const fn scales_with_text_fit(self) -> bool {
    matches!(self, Self::Normal | Self::Unitless(_))
  }

  /// Converts to parley's line-height representation.
  pub(crate) fn into_parley(self, sizing: &SizingContext) -> parley::LineHeight {
    match self {
      Self::Normal => parley::LineHeight::MetricsRelative(1.0),
      Self::Length(length) => parley::LineHeight::Absolute(length.to_px(sizing, sizing.font_size)),
      Self::Unitless(value) => parley::LineHeight::FontSizeRelative(value),
    }
  }

  /// Resolves to pixels, using `normal_basis` for `normal`.
  pub fn to_px(self, sizing: &SizingContext, normal_basis: f32) -> f32 {
    match self {
      Self::Normal => normal_basis,
      Self::Unitless(value) => value * sizing.font_size,
      Self::Length(length) => length.to_px(sizing, sizing.font_size),
    }
  }
}

impl MakeComputed for LineHeight {
  fn make_computed(&mut self, sizing: &SizingContext) {
    match self {
      Self::Length(Length::Percentage(value)) => {
        let font_size = sizing.to_css(sizing.font_size);

        *self = Self::Length(Length::Px((*value / 100.0) * font_size));
      }
      Self::Length(length) => length.make_computed(sizing),
      Self::Normal | Self::Unitless(_) => {}
    }
  }
}

impl ToCss for LineHeight {
  fn to_css<W: std::fmt::Write>(&self, dest: &mut W) -> std::fmt::Result {
    match self {
      Self::Normal => dest.write_str("normal"),
      Self::Unitless(v) => write!(dest, "{}", v),
      Self::Length(l) => l.to_css(dest),
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::style::{FromCssStr, Length, LineHeight, Theme, tw::TailwindPropertyParser};

  #[test]
  fn parses_unitless_calc_expression() {
    assert_eq!(
      LineHeight::from_css_str("calc(1.75 / 1.125)"),
      Ok(LineHeight::Unitless(1.75 / 1.125))
    );
  }

  #[test]
  fn parses_percentage_as_font_size_relative() {
    assert_eq!(
      LineHeight::from_css_str("90%"),
      Ok(LineHeight::Length(Length::Percentage(90.0)))
    );
  }

  #[test]
  fn tailwind_spacing_scale_uses_absolute_length() {
    assert_eq!(
      LineHeight::parse_tw("7"),
      Some(LineHeight::Length(Length::Rem(1.75)))
    );
  }

  #[test]
  fn tailwind_arbitrary_percentage_is_supported() {
    assert_eq!(
      LineHeight::parse_tw_with_arbitrary("[90%]", &Theme::default(), LineHeight::NAMESPACES),
      Some(LineHeight::Length(Length::Percentage(90.0)))
    );
  }
}
