use std::fmt;

use cssparser::Parser;

use crate::style::{
  Animatable, BorderStyle, Color, ColorInput, CssSyntaxKind, CssToken, FromCss, MakeComputed,
  ParseResult, SizingContext, ToCss, declare_enum_from_css_impl, properties::Length,
  tw::TailwindPropertyParser, unexpected_token,
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

declare_enum_from_css_impl!(
  LineWidthKeyword,
  "thin" => LineWidthKeyword::Thin,
  "medium" => LineWidthKeyword::Medium,
  "thick" => LineWidthKeyword::Thick,
);

impl From<LineWidthKeyword> for Length {
  fn from(keyword: LineWidthKeyword) -> Self {
    Length::Px(match keyword {
      LineWidthKeyword::Thin => 1.0,
      LineWidthKeyword::Medium => 3.0,
      LineWidthKeyword::Thick => 5.0,
    })
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

impl From<LineWidth> for Length {
  fn from(width: LineWidth) -> Self {
    match width {
      LineWidth::Keyword(keyword) => keyword.into(),
      LineWidth::Length(length) => length,
    }
  }
}

impl<'i> FromCss<'i> for LineWidth {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(keyword) = input.try_parse(LineWidthKeyword::from_css) {
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

impl LineWidth {
  /// The used width in device pixels, snapped as a border width.
  pub(crate) fn to_used_px(self, sizing: &SizingContext) -> f32 {
    Length::from(self).to_border_px(sizing, 0.0)
  }
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
    let from_length = Length::from(*from);
    let to_length = Length::from(*to);
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
  use super::*;
  use crate::{
    style::{Color, FromCssStr},
    viewport::Viewport,
  };

  fn used_px(css: &str, dpr: f32) -> f32 {
    let sizing = SizingContext::builder()
      .viewport(Viewport::new((100, Some(100))).with_device_pixel_ratio(dpr))
      .build();

    LineWidth::from_css_str(css).unwrap().to_used_px(&sizing)
  }

  #[test]
  fn test_used_line_width_snaps_to_device_pixels() {
    // A width that already lands on whole device pixels stays put, so the
    // half-pixel hairline stays a hairline once the scale makes it whole.
    assert_eq!(used_px("0.5px", 2.0), 1.0);
    assert_eq!(used_px("0.25px", 4.0), 1.0);
    // Thinner than one device pixel still rounds up to one.
    assert_eq!(used_px("0.2px", 2.0), 1.0);
    // Thicker rounds toward zero in device pixels, not CSS pixels.
    assert_eq!(used_px("1.75px", 2.0), 3.0);
    assert_eq!(used_px("2.5px", 0.5), 1.0);
  }

  #[test]
  fn test_used_line_width_rounds_down_and_keeps_hairlines() {
    let used = |css: &str| used_px(css, 1.0);

    assert_eq!(used("0"), 0.0);
    assert_eq!(used("0.3px"), 1.0);
    assert_eq!(used("0.75px"), 1.0);
    assert_eq!(used("1px"), 1.0);
    assert_eq!(used("1.5px"), 1.0);
    assert_eq!(used("2.5px"), 2.0);
    assert_eq!(used("thin"), 1.0);
    assert_eq!(used("medium"), 3.0);
    assert_eq!(used("thick"), 5.0);
  }

  #[test]
  fn test_parse_border_style_solid() {
    assert_eq!(BorderStyle::from_css_str("solid"), Ok(BorderStyle::Solid));
  }

  #[test]
  fn test_parse_border_style_dashed() {
    assert_eq!(BorderStyle::from_css_str("dashed"), Ok(BorderStyle::Dashed));
  }

  #[test]
  fn test_parse_border_width_only() {
    assert_eq!(
      Border::from_css_str("10px"),
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
      Border::from_css_str("solid"),
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
      Border::from_css_str("red"),
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
      Border::from_css_str("2px solid"),
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
      Border::from_css_str("2px solid red"),
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
      Border::from_css_str("solid 2px red"),
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
      Border::from_css_str("red solid 2px"),
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
      Border::from_css_str("1.5rem solid blue"),
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
      Border::from_css_str("3px solid #ff0000"),
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
      Border::from_css_str("4px solid rgb(0, 255, 0)"),
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
      Border::from_css_str("2px dashed red"),
      Ok(Border {
        width: LineWidth::Length(Length::Px(2.0)),
        style: BorderStyle::Dashed,
        color: ColorInput::Value(Color([255, 0, 0, 255])),
      })
    );
  }

  #[test]
  fn test_parse_border_invalid_color() {
    assert!(Border::from_css_str("2px solid invalid-color").is_err());
  }

  #[test]
  fn test_parse_border_empty() {
    assert_eq!(Border::from_css_str(""), Ok(Border::default()));
  }

  #[test]
  fn test_border_value_from_css() {
    assert_eq!(
      Border::from_css_str("3px solid blue"),
      Ok(Border {
        width: LineWidth::Length(Length::Px(3.0)),
        style: BorderStyle::Solid,
        color: ColorInput::Value(Color([0, 0, 255, 255])),
      })
    );
  }

  #[test]
  fn test_border_value_from_invalid_css() {
    assert!(Border::from_css_str("invalid border").is_err());
  }

  #[test]
  fn test_line_width_default_is_medium() {
    assert_eq!(
      LineWidth::default(),
      LineWidth::Keyword(LineWidthKeyword::Medium)
    );
    assert_eq!(Length::from(LineWidth::default()), Length::Px(3.0));
    assert_eq!(Length::from(LineWidthKeyword::Thin), Length::Px(1.0));
    assert_eq!(Length::from(LineWidthKeyword::Thick), Length::Px(5.0));
  }

  #[test]
  fn test_line_width_keywords() {
    assert_eq!(
      LineWidth::from_css_str("thin"),
      Ok(LineWidth::Keyword(LineWidthKeyword::Thin))
    );
    assert_eq!(
      LineWidth::from_css_str("medium"),
      Ok(LineWidth::Keyword(LineWidthKeyword::Medium))
    );
    assert_eq!(
      LineWidth::from_css_str("thick"),
      Ok(LineWidth::Keyword(LineWidthKeyword::Thick))
    );
    assert_eq!(
      LineWidth::from_css_str("2px"),
      Ok(LineWidth::Length(Length::Px(2.0)))
    );
  }

  #[test]
  fn test_border_keyword_width() {
    assert_eq!(
      Border::from_css_str("thick solid"),
      Ok(Border {
        width: LineWidth::Keyword(LineWidthKeyword::Thick),
        style: BorderStyle::Solid,
        color: ColorInput::CurrentColor,
      })
    );
  }
}
