use std::{borrow::Cow, fmt::Debug};

use cssparser::{BasicParseErrorKind, ParseError, Parser};
use typed_builder::TypedBuilder;

use crate::{
  layout::style::{
    Animatable, Color, ColorInput, CssSyntaxKind, CssToken, FromCss, Length, LengthDefaultsToZero,
    ListInterpolationStrategy, MakeComputed, ParseResult, next_is_comma,
  },
  rendering::Sizing,
};

/// Represents a box shadow with all its properties.
/// Construct with [`BoxShadow::builder`].
#[derive(Debug, Clone, PartialEq, Copy, Default, TypedBuilder)]
#[non_exhaustive]
#[builder(field_defaults(default))]
pub struct BoxShadow {
  /// Whether the shadow is inset (inside the element) or outset (outside the element).
  #[builder(default = false)]
  pub inset: bool,
  /// Horizontal offset of the shadow.
  pub offset_x: LengthDefaultsToZero,
  /// Vertical offset of the shadow.
  pub offset_y: LengthDefaultsToZero,
  /// Blur radius of the shadow. Higher values create a more blurred shadow.
  pub blur_radius: LengthDefaultsToZero,
  /// Spread radius of the shadow. Positive values expand the shadow, negative values shrink it.
  pub spread_radius: LengthDefaultsToZero,
  /// Color of the shadow.
  pub color: ColorInput,
}

/// Represents a collection of box shadows, have custom `FromCss` implementation for comma-separated values.
pub type BoxShadows = Box<[BoxShadow]>;

impl<'i> FromCss<'i> for BoxShadows {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(
      input
        .parse_comma_separated(BoxShadow::from_css)?
        .into_boxed_slice(),
    )
  }

  const VALID_TOKENS: &'static [CssToken] = BoxShadow::VALID_TOKENS;
}

impl<'i> FromCss<'i> for BoxShadow {
  /// Parses a box-shadow value from CSS input.
  ///
  /// The box-shadow syntax allows for the following components in any order:
  /// - inset keyword (optional)
  /// - Two length values for horizontal and vertical offsets (required)
  /// - Two optional length values for blur radius and spread radius
  /// - A color value (optional)
  ///
  /// Examples:
  /// - `box-shadow: 2px 4px;`
  /// - `box-shadow: 2px 4px 6px;`
  /// - `box-shadow: 2px 4px 6px 8px;`
  /// - `box-shadow: 2px 4px red;`
  /// - `box-shadow: inset 2px 4px 6px red;`
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, BoxShadow> {
    let mut color = None;
    let mut lengths = None;
    let mut inset = false;

    while !input.is_exhausted() && !next_is_comma(input) {
      if !inset
        && input
          .try_parse(|input| input.expect_ident_matching("inset"))
          .is_ok()
      {
        inset = true;
        continue;
      }

      if lengths.is_none() {
        let value = input.try_parse::<_, _, ParseError<Cow<'i, str>>>(|input| {
          let horizontal = Length::from_css(input)?;
          let vertical = Length::from_css(input)?;

          let blur = input.try_parse(Length::from_css).unwrap_or(Length::zero());

          let spread = input.try_parse(Length::from_css).unwrap_or(Length::zero());

          Ok((horizontal, vertical, blur, spread))
        });

        if let Ok(value) = value {
          lengths = Some(value);
          continue;
        }
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

    Ok(BoxShadow {
      color: color.unwrap_or(ColorInput::Value(Color::transparent())),
      offset_x: lengths.0,
      offset_y: lengths.1,
      blur_radius: lengths.2,
      spread_radius: lengths.3,
      inset,
    })
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("inset"),
    CssToken::Syntax(CssSyntaxKind::Length),
    CssToken::Syntax(CssSyntaxKind::Color),
  ];
}

impl crate::layout::style::tw::TailwindPropertyParser for BoxShadow {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_str(token).ok()
  }
}

impl MakeComputed for BoxShadow {
  fn make_computed(&mut self, sizing: &Sizing) {
    self.offset_x.make_computed(sizing);
    self.offset_y.make_computed(sizing);
    self.blur_radius.make_computed(sizing);
    self.spread_radius.make_computed(sizing);
  }
}

impl Animatable for BoxShadow {
  fn list_interpolation_strategy() -> ListInterpolationStrategy {
    ListInterpolationStrategy::PadToLongestWithNeutral
  }

  fn neutral_value_like(other: &Self) -> Option<Self> {
    Some(Self {
      inset: other.inset,
      offset_x: Length::zero(),
      offset_y: Length::zero(),
      blur_radius: Length::zero(),
      spread_radius: Length::zero(),
      color: Color::transparent().into(),
    })
  }

  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &Sizing,
    current_color: Color,
  ) {
    if from.inset != to.inset {
      *self = if progress >= 0.5 { *to } else { *from };
      return;
    }

    self.inset = from.inset;
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
    self.spread_radius.interpolate(
      &from.spread_radius,
      &to.spread_radius,
      progress,
      sizing,
      current_color,
    );
    self
      .color
      .interpolate(&from.color, &to.color, progress, sizing, current_color);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::style::{
    Color,
    Length::{self, Px},
  };

  #[test]
  fn test_parse_simple_box_shadow() {
    // Test parsing a simple box-shadow with just offsets
    assert_eq!(
      BoxShadow::from_str("2px 4px"),
      Ok(BoxShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Length::zero(),
        spread_radius: Length::zero(),
        color: ColorInput::Value(Color::transparent()),
        inset: false,
      })
    );
  }

  #[test]
  fn test_parse_box_shadow_with_blur() {
    // Test parsing box-shadow with blur radius
    assert_eq!(
      BoxShadow::from_str("2px 4px 6px"),
      Ok(BoxShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Px(6.0),
        spread_radius: Length::zero(),
        color: ColorInput::Value(Color::transparent()),
        inset: false,
      })
    );
  }

  #[test]
  fn test_parse_box_shadow_with_spread() {
    // Test parsing box-shadow with blur and spread radius
    assert_eq!(
      BoxShadow::from_str("2px 4px 6px 8px"),
      Ok(BoxShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Px(6.0),
        spread_radius: Px(8.0),
        color: ColorInput::Value(Color::transparent()),
        inset: false,
      })
    );
  }

  #[test]
  fn test_parse_box_shadow_with_color() {
    // Test parsing box-shadow with color
    assert_eq!(
      BoxShadow::from_str("2px 4px red"),
      Ok(BoxShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Length::zero(),
        spread_radius: Length::zero(),
        color: ColorInput::Value(Color([255, 0, 0, 255])),
        inset: false,
      })
    );
  }

  #[test]
  fn test_parse_inset_box_shadow() {
    // Test parsing inset box-shadow
    assert_eq!(
      BoxShadow::from_str("inset 2px 4px"),
      Ok(BoxShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Length::zero(),
        spread_radius: Length::zero(),
        color: ColorInput::Value(Color::transparent()),
        inset: true,
      })
    );
  }

  #[test]
  fn test_parse_box_shadow_color_first() {
    // Test parsing box-shadow with color before offsets
    assert_eq!(
      BoxShadow::from_str("red 2px 4px"),
      Ok(BoxShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Length::zero(),
        spread_radius: Length::zero(),
        color: ColorInput::Value(Color([255, 0, 0, 255])),
        inset: false,
      })
    );
  }

  #[test]
  fn test_parse_box_shadow_inset_after_offsets() {
    // Test parsing box-shadow with inset keyword after offsets
    assert_eq!(
      BoxShadow::from_str("2px 4px inset red"),
      Ok(BoxShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Length::zero(),
        spread_radius: Length::zero(),
        color: ColorInput::Value(Color([255, 0, 0, 255])),
        inset: true,
      })
    );
  }

  #[test]
  fn test_parse_box_shadow_hex_color() {
    // Test parsing box-shadow with hex color
    assert_eq!(
      BoxShadow::from_str("2px 4px #ff0000"),
      Ok(BoxShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Length::zero(),
        spread_radius: Length::zero(),
        color: ColorInput::Value(Color([255, 0, 0, 255])),
        inset: false,
      })
    );
  }

  #[test]
  fn test_parse_box_shadow_rgba_color() {
    // Test parsing box-shadow with rgba color
    assert_eq!(
      BoxShadow::from_str("2px 4px rgba(255, 0, 0, 0.5)"),
      Ok(BoxShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Length::zero(),
        spread_radius: Length::zero(),
        color: ColorInput::Value(Color([255, 0, 0, 128])), // 0.5 * 255 = 128
        inset: false,
      })
    );
  }

  #[test]
  fn test_parse_box_shadow_invalid() {
    // Test parsing invalid box-shadow (missing required offsets)
    assert!(BoxShadow::from_str("2px").is_err());

    // Test parsing invalid box-shadow (no values)
    assert!(BoxShadow::from_str("").is_err());
  }

  #[test]
  fn test_parse_multiple_box_shadows_with_rgba() {
    assert_eq!(
      BoxShadows::from_str("2px 4px rgba(0, 0, 0, 0.5), 1px 2px 3px rgba(255, 0, 0, 0.25)"),
      Ok(
        [
          BoxShadow {
            offset_x: Px(2.0),
            offset_y: Px(4.0),
            blur_radius: Length::zero(),
            spread_radius: Length::zero(),
            color: ColorInput::Value(Color([0, 0, 0, 128])),
            inset: false,
          },
          BoxShadow {
            offset_x: Px(1.0),
            offset_y: Px(2.0),
            blur_radius: Px(3.0),
            spread_radius: Length::zero(),
            color: ColorInput::Value(Color([255, 0, 0, 64])),
            inset: false,
          }
        ]
        .into()
      )
    );
  }
}
