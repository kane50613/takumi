use std::fmt;

use cssparser::{BasicParseErrorKind, Parser};
use typed_builder::TypedBuilder;

use super::box_shadow::parse_offsets_blur;
use crate::style::{
  Animatable, Color, ColorInput, CssSyntaxKind, CssToken, FromCss, Length,
  ListInterpolationStrategy, MakeComputed, ParseResult, SizingContext, ToCss,
  impl_comma_list_from_css, next_is_comma, tw::TailwindPropertyParser,
};

/// Represents a text shadow with all its properties.
#[derive(Debug, Clone, PartialEq, Copy, TypedBuilder)]
#[builder(field_defaults(default))]
#[non_exhaustive]
pub struct TextShadow {
  /// Horizontal offset of the shadow.
  #[builder(default = Length::zero())]
  pub offset_x: Length,
  /// Vertical offset of the shadow.
  #[builder(default = Length::zero())]
  pub offset_y: Length,
  /// Blur radius of the shadow. Higher values create a more blurred shadow.
  #[builder(default = Length::zero())]
  pub blur_radius: Length,
  /// Color of the shadow.
  pub color: ColorInput,
}

impl Default for TextShadow {
  fn default() -> Self {
    Self {
      offset_x: Length::zero(),
      offset_y: Length::zero(),
      blur_radius: Length::zero(),
      color: ColorInput::default(),
    }
  }
}

/// Represents a collection of text shadows; has custom `FromCss` implementation for comma-separated values.
pub(crate) type TextShadows = Box<[TextShadow]>;

impl_comma_list_from_css!(TextShadows, TextShadow);

impl<'i> FromCss<'i> for TextShadow {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, TextShadow> {
    let mut color = None;
    let mut lengths = None;

    while !input.is_exhausted() && !next_is_comma(input) {
      if lengths.is_none()
        && let Ok(value) = input.try_parse(parse_offsets_blur)
      {
        lengths = Some(value);
        continue;
      }

      if color.is_none()
        && let Ok(value) = input.try_parse(ColorInput::from_css)
      {
        color = Some(value);
        continue;
      }

      break;
    }

    let lengths = lengths.ok_or(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid))?;

    Ok(TextShadow {
      color: color.unwrap_or(ColorInput::CurrentColor),
      offset_x: lengths.0,
      offset_y: lengths.1,
      blur_radius: lengths.2,
    })
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Length),
    CssToken::Syntax(CssSyntaxKind::Color),
  ];
}

impl TailwindPropertyParser for TextShadow {}

impl MakeComputed for TextShadow {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.offset_x.make_computed(sizing);
    self.offset_y.make_computed(sizing);
    self.blur_radius.make_computed(sizing);
  }
}

impl Animatable for TextShadow {
  fn list_interpolation_strategy() -> ListInterpolationStrategy {
    ListInterpolationStrategy::PadToLongestWithNeutral
  }

  fn neutral_value_like(_other: &Self) -> Option<Self> {
    Some(Self {
      color: Color::transparent().into(),
      ..Self::default()
    })
  }

  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    self.offset_x.interpolate(
      &from.offset_x,
      &to.offset_x,
      progress,
      sizing,
      current_color,
    );
    self.offset_y.interpolate(
      &from.offset_y,
      &to.offset_y,
      progress,
      sizing,
      current_color,
    );
    self.blur_radius.interpolate(
      &from.blur_radius,
      &to.blur_radius,
      progress,
      sizing,
      current_color,
    );
    self
      .color
      .interpolate(&from.color, &to.color, progress, sizing, current_color);
  }
}

impl ToCss for TextShadow {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.offset_x.to_css(dest)?;
    dest.write_char(' ')?;
    self.offset_y.to_css(dest)?;
    if self.blur_radius != Length::zero() {
      dest.write_char(' ')?;
      self.blur_radius.to_css(dest)?;
    }
    if self.color != ColorInput::CurrentColor {
      dest.write_char(' ')?;
      self.color.to_css(dest)?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::{Color, FromCssStr, Length::Px};

  #[test]
  fn test_parse_text_shadow_no_blur_radius() {
    assert_eq!(
      TextShadows::from_css_str("5px 5px #558abb"),
      Ok(
        [TextShadow {
          offset_x: Px(5.0),
          offset_y: Px(5.0),
          blur_radius: Px(0.0),
          color: Color([85, 138, 187, 255]).into(),
        }]
        .into()
      )
    );
  }

  #[test]
  fn test_parse_text_shadow_multiple_values() {
    assert_eq!(
      TextShadows::from_css_str("5px 5px #558abb, 10px 10px #558abb"),
      Ok(
        [
          TextShadow {
            offset_x: Px(5.0),
            offset_y: Px(5.0),
            blur_radius: Px(0.0),
            color: Color([85, 138, 187, 255]).into(),
          },
          TextShadow {
            offset_x: Px(10.0),
            offset_y: Px(10.0),
            blur_radius: Px(0.0),
            color: Color([85, 138, 187, 255]).into(),
          }
        ]
        .into()
      )
    );
  }

  #[test]
  fn test_parse_text_shadow_multiple_rgba_values() {
    assert_eq!(
      TextShadows::from_css_str("5px 5px rgba(0, 0, 0, 0.5), 10px 10px rgba(255, 0, 0, 0.25)"),
      Ok(
        [
          TextShadow {
            offset_x: Px(5.0),
            offset_y: Px(5.0),
            blur_radius: Px(0.0),
            color: Color([0, 0, 0, 128]).into(),
          },
          TextShadow {
            offset_x: Px(10.0),
            offset_y: Px(10.0),
            blur_radius: Px(0.0),
            color: Color([255, 0, 0, 64]).into(),
          }
        ]
        .into()
      )
    );
  }
}
