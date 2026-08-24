use std::{borrow::Cow, fmt, fmt::Debug};

use cssparser::{BasicParseErrorKind, ParseError, Parser};
use typed_builder::TypedBuilder;

use crate::style::TwNamespace;
use crate::style::{
  Animatable, Color, ColorInput, CssSyntaxKind, CssToken, FromCss, FromCssStr, Length,
  ListInterpolationStrategy, MakeComputed, ParseResult, SizingContext, ToCss, next_is_comma,
};

/// Represents a box shadow with all its properties.
/// Construct with [`BoxShadow::builder`].
#[derive(Debug, Clone, PartialEq, Copy, TypedBuilder)]
#[builder(field_defaults(default))]
#[non_exhaustive]
pub struct BoxShadow {
  /// Whether the shadow is inset (inside the element) or outset (outside the element).
  #[builder(default = false)]
  pub inset: bool,
  /// Horizontal offset of the shadow.
  #[builder(default = Length::zero())]
  pub offset_x: Length,
  /// Vertical offset of the shadow.
  #[builder(default = Length::zero())]
  pub offset_y: Length,
  /// Blur radius of the shadow. Higher values create a more blurred shadow.
  #[builder(default = Length::zero())]
  pub blur_radius: Length,
  /// Spread radius of the shadow. Positive values expand the shadow, negative values shrink it.
  #[builder(default = Length::zero())]
  pub spread_radius: Length,
  /// Color of the shadow.
  pub color: ColorInput,
}

impl Default for BoxShadow {
  fn default() -> Self {
    Self {
      inset: false,
      offset_x: Length::zero(),
      offset_y: Length::zero(),
      blur_radius: Length::zero(),
      spread_radius: Length::zero(),
      color: ColorInput::default(),
    }
  }
}

/// Represents a collection of box shadows, have custom `FromCss` implementation for comma-separated values.
pub(crate) type BoxShadows = Box<[BoxShadow]>;

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

/// Parses a `<length>` rejecting percentages, which are invalid for shadow blur/spread radii.
fn parse_non_percentage_length<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, Length> {
  let length = Length::from_css(input)?;
  if matches!(length, Length::Percentage(_)) {
    return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
  }
  Ok(length)
}

pub(super) fn parse_offsets_blur<'i>(
  input: &mut Parser<'i, '_>,
) -> ParseResult<'i, (Length, Length, Length)> {
  let horizontal = Length::from_css(input)?;
  let vertical = Length::from_css(input)?;
  let blur = input
    .try_parse(parse_non_percentage_length)
    .unwrap_or(Length::zero());
  Ok((horizontal, vertical, blur))
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
          let (horizontal, vertical, blur) = parse_offsets_blur(input)?;
          let spread = input
            .try_parse(parse_non_percentage_length)
            .unwrap_or(Length::zero());
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

impl crate::style::tw::TailwindPropertyParser for BoxShadow {
  const NAMESPACES: &'static [TwNamespace] = &[TwNamespace::Shadow];

  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_css_str(token).ok()
  }
}

impl MakeComputed for BoxShadow {
  fn make_computed(&mut self, sizing: &SizingContext) {
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
    sizing: &SizingContext,
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

impl ToCss for BoxShadow {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    if self.inset {
      dest.write_str("inset ")?;
    }
    self.offset_x.to_css(dest)?;
    dest.write_char(' ')?;
    self.offset_y.to_css(dest)?;

    let blur_zero = self.blur_radius == Length::zero();
    let spread_zero = self.spread_radius == Length::zero();
    if !spread_zero {
      dest.write_char(' ')?;
      self.blur_radius.to_css(dest)?;
      dest.write_char(' ')?;
      self.spread_radius.to_css(dest)?;
    } else if !blur_zero {
      dest.write_char(' ')?;
      self.blur_radius.to_css(dest)?;
    }

    dest.write_char(' ')?;
    self.color.to_css(dest)
  }
}
#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::{
    Color,
    Length::{self, Px},
  };

  fn red() -> ColorInput {
    ColorInput::Value(Color([255, 0, 0, 255]))
  }

  fn transparent() -> ColorInput {
    ColorInput::Value(Color::transparent())
  }

  #[test]
  fn test_parse_box_shadow() {
    let cases: &[(&str, BoxShadow)] = &[
      (
        "2px 4px",
        BoxShadow {
          offset_x: Px(2.0),
          offset_y: Px(4.0),
          color: transparent(),
          ..Default::default()
        },
      ),
      (
        "2px 4px 6px",
        BoxShadow {
          offset_x: Px(2.0),
          offset_y: Px(4.0),
          blur_radius: Px(6.0),
          color: transparent(),
          ..Default::default()
        },
      ),
      (
        "2px 4px 6px 8px",
        BoxShadow {
          offset_x: Px(2.0),
          offset_y: Px(4.0),
          blur_radius: Px(6.0),
          spread_radius: Px(8.0),
          color: transparent(),
          ..Default::default()
        },
      ),
      (
        "2px 4px red",
        BoxShadow {
          offset_x: Px(2.0),
          offset_y: Px(4.0),
          color: red(),
          ..Default::default()
        },
      ),
      (
        "inset 2px 4px",
        BoxShadow {
          offset_x: Px(2.0),
          offset_y: Px(4.0),
          color: transparent(),
          inset: true,
          ..Default::default()
        },
      ),
      (
        "red 2px 4px",
        BoxShadow {
          offset_x: Px(2.0),
          offset_y: Px(4.0),
          color: red(),
          ..Default::default()
        },
      ),
      (
        "2px 4px inset red",
        BoxShadow {
          offset_x: Px(2.0),
          offset_y: Px(4.0),
          color: red(),
          inset: true,
          ..Default::default()
        },
      ),
      (
        "2px 4px #ff0000",
        BoxShadow {
          offset_x: Px(2.0),
          offset_y: Px(4.0),
          color: red(),
          ..Default::default()
        },
      ),
      (
        "2px 4px rgba(255, 0, 0, 0.5)",
        BoxShadow {
          offset_x: Px(2.0),
          offset_y: Px(4.0),
          color: ColorInput::Value(Color([255, 0, 0, 128])),
          ..Default::default()
        },
      ),
    ];

    for (css, expected) in cases {
      assert_eq!(BoxShadow::from_css_str(css), Ok(*expected), "css: {css}");
    }
  }

  #[test]
  fn test_parse_box_shadow_invalid() {
    assert!(BoxShadow::from_css_str("2px").is_err());
    assert!(BoxShadow::from_css_str("").is_err());
  }

  #[test]
  fn test_box_shadow_rejects_percentage_radii() {
    // Percentages are not valid <length> for blur/spread; loose parsing ignores
    // the trailing token rather than capturing it as a Percentage length.
    assert_eq!(
      BoxShadow::from_css_str("2px 4px 50%"),
      Ok(BoxShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        color: transparent(),
        ..Default::default()
      })
    );
    assert_eq!(
      BoxShadow::from_css_str("2px 4px 6px 50%"),
      Ok(BoxShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Px(6.0),
        color: transparent(),
        ..Default::default()
      })
    );
  }

  #[test]
  fn test_parse_multiple_box_shadows_with_rgba() {
    assert_eq!(
      BoxShadows::from_css_str("2px 4px rgba(0, 0, 0, 0.5), 1px 2px 3px rgba(255, 0, 0, 0.25)"),
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
