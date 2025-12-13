use std::{borrow::Cow, fmt::Debug};

use cssparser::{BasicParseErrorKind, ParseError, Parser};
use smallvec::SmallVec;

use crate::layout::style::{Color, ColorInput, FromCss, LengthUnit, ParseResult};

/// Represents a box shadow with all its properties.
///
/// Box shadows can be either outset (default) or inset, and consist of:
/// - Horizontal and vertical offsets
/// - Blur radius (optional, defaults to 0)
/// - Spread radius (optional, defaults to 0)
/// - Color (optional, defaults to transparent)
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct BoxShadow {
  /// Whether the shadow is inset (inside the element) or outset (outside the element).
  pub inset: bool,
  /// Horizontal offset of the shadow.
  pub offset_x: LengthUnit,
  /// Vertical offset of the shadow.
  pub offset_y: LengthUnit,
  /// Blur radius of the shadow. Higher values create a more blurred shadow.
  pub blur_radius: LengthUnit,
  /// Spread radius of the shadow. Positive values expand the shadow, negative values shrink it.
  pub spread_radius: LengthUnit,
  /// Color of the shadow.
  pub color: ColorInput,
}

/// Represents a collection of box shadows, have custom `FromCss` implementation for comma-separated values.
pub type BoxShadows = SmallVec<[BoxShadow; 4]>;

impl<'i> FromCss<'i> for BoxShadows {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut shadows = SmallVec::new();

    loop {
      if input.is_exhausted() {
        break;
      }

      let shadow = BoxShadow::from_css(input)?;
      shadows.push(shadow);

      if input.expect_comma().is_err() {
        break;
      }
    }

    Ok(shadows)
  }
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

    // Parse all components in a loop, as they can appear in any order
    loop {
      // Try to parse the "inset" keyword if not already found
      if !inset
        && input
          .try_parse(|input| input.expect_ident_matching("inset"))
          .is_ok()
      {
        inset = true;
        continue;
      }

      // Try to parse length values (offsets, blur radius, spread radius)
      if lengths.is_none() {
        let value = input.try_parse::<_, _, ParseError<Cow<'i, str>>>(|input| {
          // Parse the required horizontal and vertical offsets
          let horizontal = LengthUnit::from_css(input)?;
          let vertical = LengthUnit::from_css(input)?;

          // Parse optional blur radius (defaults to 0)
          let blur = input
            .try_parse(LengthUnit::from_css)
            .unwrap_or(LengthUnit::zero());

          // Parse optional spread radius (defaults to 0)
          let spread = input
            .try_parse(LengthUnit::from_css)
            .unwrap_or(LengthUnit::zero());

          Ok((horizontal, vertical, blur, spread))
        });

        if let Ok(value) = value {
          lengths = Some(value);
          continue;
        }
      }

      // Try to parse a color value if not already found
      if color.is_none()
        && let Ok(value) = input.try_parse(ColorInput::from_css)
      {
        color = Some(value);
        continue;
      }

      // If we can't parse anything else, break out of the loop
      break;
    }

    // At minimum, we need the two required length values (offsets)
    let lengths = lengths.ok_or(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid))?;

    // Construct the BoxShadow with parsed values or defaults
    Ok(BoxShadow {
      // Use parsed color or default to transparent
      color: color.unwrap_or(ColorInput::Value(Color::transparent())),
      offset_x: lengths.0,
      offset_y: lengths.1,
      blur_radius: lengths.2,
      spread_radius: lengths.3,
      inset,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::style::{
    Color,
    LengthUnit::{self, Px},
  };

  #[test]
  fn test_parse_simple_box_shadow() {
    // Test parsing a simple box-shadow with just offsets
    assert_eq!(
      BoxShadow::from_str("2px 4px"),
      Ok(BoxShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: LengthUnit::zero(),
        spread_radius: LengthUnit::zero(),
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
        spread_radius: LengthUnit::zero(),
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
        blur_radius: LengthUnit::zero(),
        spread_radius: LengthUnit::zero(),
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
        blur_radius: LengthUnit::zero(),
        spread_radius: LengthUnit::zero(),
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
        blur_radius: LengthUnit::zero(),
        spread_radius: LengthUnit::zero(),
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
        blur_radius: LengthUnit::zero(),
        spread_radius: LengthUnit::zero(),
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
        blur_radius: LengthUnit::zero(),
        spread_radius: LengthUnit::zero(),
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
        blur_radius: LengthUnit::zero(),
        spread_radius: LengthUnit::zero(),
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
}
