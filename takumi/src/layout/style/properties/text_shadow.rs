use std::{borrow::Cow, fmt::Debug};

use cssparser::{BasicParseErrorKind, ParseError, Parser};
use smallvec::SmallVec;

use crate::layout::style::{Color, ColorInput, FromCss, LengthUnit, ParseResult};

/// Represents a text shadow with all its properties.
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct TextShadow {
  /// Horizontal offset of the shadow.
  pub offset_x: LengthUnit,
  /// Vertical offset of the shadow.
  pub offset_y: LengthUnit,
  /// Blur radius of the shadow. Higher values create a more blurred shadow.
  pub blur_radius: LengthUnit,
  /// Color of the shadow.
  pub color: ColorInput,
}

/// Represents a collection of text shadows; has custom `FromCss` implementation for comma-separated values.
pub type TextShadows = SmallVec<[TextShadow; 4]>;

impl<'i> FromCss<'i> for TextShadows {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut shadows = SmallVec::new();

    loop {
      if input.is_exhausted() {
        break;
      }

      let shadow = TextShadow::from_css(input)?;
      shadows.push(shadow);

      if input.expect_comma().is_err() {
        break;
      }
    }

    Ok(shadows)
  }
}

impl<'i> FromCss<'i> for TextShadow {
  /// Parses a text-shadow value from CSS input.
  ///
  /// The text-shadow syntax supports the following components (in that order):
  /// - Two length values for horizontal and vertical offsets (required)
  /// - An optional length value for blur radius
  /// - An optional color value
  ///
  /// Examples:
  /// - `text-shadow: 2px 4px;`
  /// - `text-shadow: 2px 4px 6px;`
  /// - `text-shadow: 2px 4px red;`
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, TextShadow> {
    let mut color = None;
    let mut lengths = None;

    // Parse all components in a loop, as they can appear in any order
    loop {
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

          Ok((horizontal, vertical, blur))
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

    // Construct the TextShadow with parsed values or defaults
    Ok(TextShadow {
      // Use parsed color or default to transparent
      color: color.unwrap_or(ColorInput::Value(Color::transparent())),
      offset_x: lengths.0,
      offset_y: lengths.1,
      blur_radius: lengths.2,
    })
  }
}

#[cfg(test)]
mod tests {
  use crate::layout::style::LengthUnit::Px;

  use super::*;

  #[test]
  fn test_parse_text_shadow_no_blur_radius() {
    assert_eq!(
      TextShadows::from_str("5px 5px #558abb"),
      Ok(smallvec::smallvec![TextShadow {
        offset_x: Px(5.0),
        offset_y: Px(5.0),
        blur_radius: Px(0.0),
        color: Color([85, 138, 187, 255]).into(),
      }])
    );
  }

  #[test]
  fn test_parse_text_shadow_multiple_values() {
    assert_eq!(
      TextShadows::from_str("5px 5px #558abb, 10px 10px #558abb"),
      Ok(smallvec::smallvec![
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
      ])
    );
  }
}
