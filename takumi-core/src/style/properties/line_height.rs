use std::fmt;

use cssparser::{Parser, match_ignore_ascii_case};

use crate::style::{
  FromCss, Length, MakeComputed, ParseResult, SizingContext, ToCss, parse_calc_number_expression,
  tw::Namespace, tw::TailwindPropertyParser,
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
  // Spacing second: `leading-7` multiplies `--spacing`, `leading-tight` reads
  // `--leading-tight`.
  const NAMESPACES: &'static [Namespace] = &[Namespace::Leading, Namespace::Spacing];

  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {&token,
      "none" => Some(LineHeight::Unitless(1.0)),
      "tight" => Some(LineHeight::Unitless(1.25)),
      "snug" => Some(LineHeight::Unitless(1.375)),
      "normal" => Some(LineHeight::Unitless(1.5)),
      "relaxed" => Some(LineHeight::Unitless(1.625)),
      "loose" => Some(LineHeight::Unitless(2.0)),
      _ => token
        .parse()
        .ok()
        .map(|value| LineHeight::Length(Length::from_spacing(value))),
    }
  }
}

impl<'i> FromCss<'i> for LineHeight {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("normal"))
      .is_ok()
    {
      return Ok(Self::Normal);
    }

    // A calculation clamps to the property's non-negative range.
    // <https://drafts.csswg.org/css-values-4/#calc-range>
    if let Ok(number) = input.try_parse(parse_calc_number_expression) {
      return Ok(LineHeight::Unitless(number.max(0.0)));
    }

    if let Ok(percent) = input.try_parse(Parser::expect_percentage) {
      return Ok(LineHeight::Length(Length::Percentage(percent * 100.0)));
    }

    let Ok(number) = input.try_parse(Parser::expect_number) else {
      return Length::from_css(input).map(Into::into);
    };

    Ok(LineHeight::Unitless(number))
  }

  const VALID_TOKENS: &'static [&'static str] = &["<number>", "<length>", "<percentage>"];
}

impl LineHeight {
  /// Whether the line height is authored as a negative number or length, which Blink's
  /// `ConsumeLineHeight` rejects.
  pub(crate) fn is_negative(self) -> bool {
    match self {
      Self::Normal => false,
      Self::Unitless(value) => value < 0.0,
      Self::Length(length) => length.is_negative(),
    }
  }

  /// Resolves to pixels, using `normal_basis` for `normal`, a negative calculation clamped to zero.
  pub fn to_px(self, sizing: &SizingContext, normal_basis: f32) -> f32 {
    match self {
      Self::Normal => normal_basis,
      Self::Unitless(value) => value * sizing.font_size,
      Self::Length(length) => length.to_px(sizing, sizing.font_size).max(0.0),
    }
  }
}

impl MakeComputed for LineHeight {
  fn make_computed(&mut self, sizing: &SizingContext) {
    // Blink's `StyleBuilderConverter::ConvertLineHeight` computes a length, a percentage, or a
    // calculation to pixels against the element's own font size.
    if let Self::Length(length) = self {
      *length = Length::Px(sizing.to_css(length.to_px(sizing, sizing.font_size).max(0.0)));
    }
  }
}

impl ToCss for LineHeight {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Normal => dest.write_str("normal"),
      Self::Unitless(v) => write!(dest, "{v}"),
      Self::Length(l) => l.to_css(dest),
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    style::{
      FromCssStr, Length, LineHeight, MakeComputed, SizingContext, tw::TailwindPropertyParser,
    },
    viewport::Viewport,
  };

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
  fn computes_a_percentage_calculation_against_the_font_size() {
    let sizing = SizingContext::builder()
      .viewport(Viewport::new((1200, 630)))
      .font_size(20.0)
      .line_height(0.0)
      .ch(10.0)
      .ex(10.0)
      .build();
    let mut line_height = LineHeight::from_css_str("calc(100% + 4px)").unwrap();

    line_height.make_computed(&sizing);
    assert_eq!(line_height, LineHeight::Length(Length::Px(24.0)));
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
      LineHeight::parse_tw_with_arbitrary("[90%]"),
      Some(LineHeight::Length(Length::Percentage(90.0)))
    );
  }
}
