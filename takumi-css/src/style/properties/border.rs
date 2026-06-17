use std::fmt;

use crate::style::{ToCss, unexpected_token};
use cssparser::{Parser, Token, match_ignore_ascii_case};
use taffy::LengthPercentage;

use crate::style::{
  Animatable, BorderStyle, Color, ColorInput, CssSyntaxKind, CssToken, FromCss, MakeComputed,
  ParseResult, SizingContext, properties::Length, tw::TailwindPropertyParser,
};

/// CSSWG `<line-width>` keyword (`thin | medium | thick`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LineWidthKeyword {
  /// `thin`, resolves to 1px.
  Thin,
  /// `medium`, resolves to 3px (initial value of `border-width`/`outline-width`).
  #[default]
  Medium,
  /// `thick`, resolves to 5px.
  Thick,
}

impl LineWidthKeyword {
  /// Resolved width in pixels.
  pub const fn to_px(self) -> f32 {
    match self {
      Self::Thin => 1.0,
      Self::Medium => 3.0,
      Self::Thick => 5.0,
    }
  }
}

/// CSSWG `<line-width>`: a `thin | medium | thick` keyword or a `<length>`.
///
/// Used by `border-*-width` and `outline-width`. The initial value is `medium`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum LineWidth {
  /// A `thin | medium | thick` keyword.
  Keyword(LineWidthKeyword),
  /// An explicit length.
  Length(Length),
}

impl LineWidth {
  /// Zero width, the used value when the line's style is `none` or `hidden`.
  pub const ZERO: Self = Self::Length(Length::Px(0.0));

  /// Resolves the value to a [`Length`], mapping keywords to their pixel widths.
  pub fn to_length(self) -> Length {
    match self {
      Self::Keyword(keyword) => Length::Px(keyword.to_px()),
      Self::Length(length) => length,
    }
  }

  /// Resolves the value to absolute pixels.
  pub fn to_px(self, sizing: &SizingContext, percentage_full_px: f32) -> f32 {
    self.to_length().to_px(sizing, percentage_full_px)
  }

  /// Resolves the value to a taffy [`LengthPercentage`].
  pub fn resolve_to_length_percentage(self, sizing: &SizingContext) -> LengthPercentage {
    self.to_length().resolve_to_length_percentage(sizing)
  }
}

impl Default for LineWidth {
  fn default() -> Self {
    Self::Keyword(LineWidthKeyword::Medium)
  }
}

impl From<Length> for LineWidth {
  fn from(length: Length) -> Self {
    Self::Length(length)
  }
}

impl<'i> FromCss<'i> for LineWidth {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(keyword) = input.try_parse(|input| -> ParseResult<'i, LineWidthKeyword> {
      let location = input.current_source_location();
      let ident = input.expect_ident_cloned()?;
      match_ignore_ascii_case! { ident.as_ref(),
        "thin" => Ok(LineWidthKeyword::Thin),
        "medium" => Ok(LineWidthKeyword::Medium),
        "thick" => Ok(LineWidthKeyword::Thick),
        _ => Err(unexpected_token!(location, &Token::Ident(ident))),
      }
    }) {
      return Ok(Self::Keyword(keyword));
    }

    Ok(Self::Length(Length::from_css(input)?))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("thin"),
    CssToken::Keyword("medium"),
    CssToken::Keyword("thick"),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

impl MakeComputed for LineWidth {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let Self::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}

impl Animatable for LineWidth {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    let from_length = from.to_length();
    let to_length = to.to_length();
    let mut value = from_length;
    value.interpolate(&from_length, &to_length, progress, sizing, current_color);
    *self = Self::Length(value);
  }
}

impl TailwindPropertyParser for LineWidth {
  fn parse_tw(token: &str) -> Option<Self> {
    token
      .parse::<f32>()
      .ok()
      .map(|value| Self::Length(Length::Px(value)))
  }
}

impl ToCss for LineWidth {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Keyword(LineWidthKeyword::Thin) => dest.write_str("thin"),
      Self::Keyword(LineWidthKeyword::Medium) => dest.write_str("medium"),
      Self::Keyword(LineWidthKeyword::Thick) => dest.write_str("thick"),
      Self::Length(length) => length.to_css(dest),
    }
  }
}

/// Parsed `border` value.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Border {
  /// Border width.
  pub width: LineWidth,
  /// Border style.
  pub style: BorderStyle,
  /// Border color.
  pub color: ColorInput,
}

impl<'i> FromCss<'i> for Border {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut width = None;
    let mut style = None;
    let mut color = None;

    loop {
      if input.is_exhausted() {
        break;
      }

      if let Ok(value) = input.try_parse(LineWidth::from_css) {
        width = Some(value);
        continue;
      }

      if let Ok(value) = input.try_parse(BorderStyle::from_css) {
        style = Some(value);
        continue;
      }

      if let Ok(value) = input.try_parse(ColorInput::from_css) {
        color = Some(value);
        continue;
      }

      return Err(unexpected_token!(
        input.current_source_location(),
        input.next()?,
      ));
    }

    Ok(Border {
      width: width.unwrap_or_default(),
      style: style.unwrap_or_default(),
      color: color.unwrap_or_default(),
    })
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Length),
    CssToken::Syntax(CssSyntaxKind::BorderStyle),
    CssToken::Syntax(CssSyntaxKind::Color),
  ];
}

impl MakeComputed for Border {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.width.make_computed(sizing);
  }
}

#[cfg(test)]
mod tests {
  use crate::style::Color;

  use super::*;

  #[test]
  fn test_parse_border_style_solid() {
    assert_eq!(BorderStyle::from_str("solid"), Ok(BorderStyle::Solid));
  }

  #[test]
  fn test_parse_border_style_dashed() {
    assert_eq!(BorderStyle::from_str("dashed"), Ok(BorderStyle::Dashed));
  }

  #[test]
  fn test_parse_border_width_only() {
    assert_eq!(
      Border::from_str("10px"),
      Ok(Border {
        width: LineWidth::Length(Length::Px(10.0)),
        style: BorderStyle::None,
        color: ColorInput::CurrentColor,
      })
    );
  }

  #[test]
  fn test_parse_border_style_only() {
    assert_eq!(
      Border::from_str("solid"),
      Ok(Border {
        width: LineWidth::default(),
        style: BorderStyle::Solid,
        color: ColorInput::CurrentColor,
      })
    );
  }

  #[test]
  fn test_parse_border_color_only() {
    assert_eq!(
      Border::from_str("red"),
      Ok(Border {
        width: LineWidth::default(),
        style: BorderStyle::None,
        color: ColorInput::Value(Color([255, 0, 0, 255])),
      })
    );
  }

  #[test]
  fn test_parse_border_width_and_style() {
    assert_eq!(
      Border::from_str("2px solid"),
      Ok(Border {
        width: LineWidth::Length(Length::Px(2.0)),
        style: BorderStyle::Solid,
        color: ColorInput::CurrentColor,
      })
    );
  }

  #[test]
  fn test_parse_border_width_style_color() {
    assert_eq!(
      Border::from_str("2px solid red"),
      Ok(Border {
        width: LineWidth::Length(Length::Px(2.0)),
        style: BorderStyle::Solid,
        color: ColorInput::Value(Color([255, 0, 0, 255])),
      })
    );
  }

  #[test]
  fn test_parse_border_style_width_color() {
    assert_eq!(
      Border::from_str("solid 2px red"),
      Ok(Border {
        width: LineWidth::Length(Length::Px(2.0)),
        style: BorderStyle::Solid,
        color: ColorInput::Value(Color([255, 0, 0, 255])),
      })
    );
  }

  #[test]
  fn test_parse_border_color_style_width() {
    assert_eq!(
      Border::from_str("red solid 2px"),
      Ok(Border {
        width: LineWidth::Length(Length::Px(2.0)),
        style: BorderStyle::Solid,
        color: ColorInput::Value(Color([255, 0, 0, 255])),
      })
    );
  }

  #[test]
  fn test_parse_border_rem_units() {
    assert_eq!(
      Border::from_str("1.5rem solid blue"),
      Ok(Border {
        width: LineWidth::Length(Length::Rem(1.5)),
        style: BorderStyle::Solid,
        color: ColorInput::Value(Color([0, 0, 255, 255])),
      })
    );
  }

  #[test]
  fn test_parse_border_hex_color() {
    assert_eq!(
      Border::from_str("3px solid #ff0000"),
      Ok(Border {
        width: LineWidth::Length(Length::Px(3.0)),
        style: BorderStyle::Solid,
        color: ColorInput::Value(Color([255, 0, 0, 255])),
      })
    );
  }

  #[test]
  fn test_parse_border_rgb_color() {
    assert_eq!(
      Border::from_str("4px solid rgb(0, 255, 0)"),
      Ok(Border {
        width: LineWidth::Length(Length::Px(4.0)),
        style: BorderStyle::Solid,
        color: ColorInput::Value(Color([0, 255, 0, 255])),
      })
    );
  }

  #[test]
  fn test_parse_border_dashed() {
    assert_eq!(
      Border::from_str("2px dashed red"),
      Ok(Border {
        width: LineWidth::Length(Length::Px(2.0)),
        style: BorderStyle::Dashed,
        color: ColorInput::Value(Color([255, 0, 0, 255])),
      })
    );
  }

  #[test]
  fn test_parse_border_invalid_color() {
    assert!(Border::from_str("2px solid invalid-color").is_err());
  }

  #[test]
  fn test_parse_border_empty() {
    assert_eq!(Border::from_str(""), Ok(Border::default()));
  }

  #[test]
  fn test_border_value_from_css() {
    assert_eq!(
      Border::from_str("3px solid blue"),
      Ok(Border {
        width: LineWidth::Length(Length::Px(3.0)),
        style: BorderStyle::Solid,
        color: ColorInput::Value(Color([0, 0, 255, 255])),
      })
    );
  }

  #[test]
  fn test_border_value_from_invalid_css() {
    assert!(Border::from_str("invalid border").is_err());
  }

  #[test]
  fn test_line_width_default_is_medium() {
    assert_eq!(
      LineWidth::default(),
      LineWidth::Keyword(LineWidthKeyword::Medium)
    );
    assert_eq!(LineWidth::default().to_length(), Length::Px(3.0));
    assert_eq!(LineWidthKeyword::Thin.to_px(), 1.0);
    assert_eq!(LineWidthKeyword::Thick.to_px(), 5.0);
  }

  #[test]
  fn test_line_width_keywords() {
    assert_eq!(
      LineWidth::from_str("thin"),
      Ok(LineWidth::Keyword(LineWidthKeyword::Thin))
    );
    assert_eq!(
      LineWidth::from_str("medium"),
      Ok(LineWidth::Keyword(LineWidthKeyword::Medium))
    );
    assert_eq!(
      LineWidth::from_str("thick"),
      Ok(LineWidth::Keyword(LineWidthKeyword::Thick))
    );
    assert_eq!(
      LineWidth::from_str("2px"),
      Ok(LineWidth::Length(Length::Px(2.0)))
    );
  }

  #[test]
  fn test_border_keyword_width() {
    assert_eq!(
      Border::from_str("thick solid"),
      Ok(Border {
        width: LineWidth::Keyword(LineWidthKeyword::Thick),
        style: BorderStyle::Solid,
        color: ColorInput::CurrentColor,
      })
    );
  }
}
