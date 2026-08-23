use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{
  Animatable, Color, CssDescriptorKind, CssToken, FromCss, MakeComputed, ParseResult,
  SizingContext, ToCss, unexpected_token,
};

/// Superellipse parameter for the CSS `corner-shape` property.
///
/// Stores the spec's `s` parameter; the curve exponent is `k = 2^s`
/// (clamped to `±16`). Positive values are convex, negative concave.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Superellipse(pub f32);

impl Superellipse {
  /// `notch`: fully concave square corner.
  pub const NOTCH: Self = Self(f32::NEG_INFINITY);
  /// `scoop`: concave quarter-circle.
  pub const SCOOP: Self = Self(-1.0);
  /// `bevel`: straight diagonal corner.
  pub const BEVEL: Self = Self(0.0);
  /// `round`: convex quarter-ellipse, the initial value.
  pub const ROUND: Self = Self(1.0);
  /// `squircle`: convex superellipse with exponent 4.
  pub const SQUIRCLE: Self = Self(2.0);
  /// `square`: no curvature regardless of `border-radius`.
  pub const SQUARE: Self = Self(f32::INFINITY);

  const DEGENERATE_PARAMETER: f32 = 16.0;

  /// The superellipse exponent `k = 2^s`, clamped to `2^±16`.
  pub(crate) fn exponent(self) -> f32 {
    Self::DEGENERATE_PARAMETER
      .min(self.0.max(-Self::DEGENERATE_PARAMETER))
      .exp2()
  }

  /// Treated as `square`: the corner renders with no curvature.
  pub(crate) fn is_degenerate(self) -> bool {
    self.0 >= Self::DEGENERATE_PARAMETER
  }

  /// Treated as `notch`: the corner cuts straight into the center point.
  pub(crate) fn is_fully_concave(self) -> bool {
    self.0 <= -Self::DEGENERATE_PARAMETER
  }

  /// True for `round`, which keeps the legacy quarter-ellipse path.
  pub(crate) fn is_round(self) -> bool {
    self.0 == 1.0
  }

  /// True for concave shapes (`scoop`, `notch`, negative parameters).
  pub(crate) fn is_concave(self) -> bool {
    self.0 < 0.0
  }

  fn to_interpolable(self) -> f32 {
    // https://drafts.csswg.org/css-borders-4/#corner-shape-interpolation
    let half_corner = 0.5f32.powf(0.5f32.powf(self.0.abs()));

    if self.0 >= 0.0 {
      half_corner
    } else {
      1.0 - half_corner
    }
  }

  fn from_interpolable(value: f32) -> Self {
    let (half_corner, sign) = if value >= 0.5 {
      (value, 1.0)
    } else {
      (1.0 - value, -1.0)
    };

    if half_corner >= 1.0 {
      return Self(sign * Self::DEGENERATE_PARAMETER);
    }

    let parameter = half_corner.log(0.5).log(0.5);

    Self(sign * parameter.clamp(0.0, Self::DEGENERATE_PARAMETER))
  }
}

impl Default for Superellipse {
  fn default() -> Self {
    Self::ROUND
  }
}

impl MakeComputed for Superellipse {}

impl Animatable for Superellipse {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &SizingContext,
    _current_color: Color,
  ) {
    let start = from.to_interpolable();
    let end = to.to_interpolable();

    *self = Self::from_interpolable(start + (end - start) * progress);
  }
}

impl<'i> FromCss<'i> for Superellipse {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    match token {
      Token::Ident(ident) => match_ignore_ascii_case! { ident,
        "notch" => Ok(Self::NOTCH),
        "scoop" => Ok(Self::SCOOP),
        "bevel" => Ok(Self::BEVEL),
        "round" => Ok(Self::ROUND),
        "squircle" => Ok(Self::SQUIRCLE),
        "square" => Ok(Self::SQUARE),
        _ => Err(unexpected_token!(location, token)),
      },
      Token::Function(function) => {
        if !function.eq_ignore_ascii_case("superellipse") {
          return Err(unexpected_token!(location, token));
        }

        input.parse_nested_block(|input| {
          let location = input.current_source_location();
          let token = input.next()?;

          match token {
            Token::Number { value, .. } => Ok(Self(*value)),
            Token::Ident(ident) => match_ignore_ascii_case! { ident,
              "infinity" => Ok(Self::SQUARE),
              "-infinity" => Ok(Self::NOTCH),
              _ => Err(unexpected_token!(Self, location, token)),
            },
            _ => Err(unexpected_token!(Self, location, token)),
          }
        })
      }
      _ => Err(unexpected_token!(location, token)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("notch"),
    CssToken::Keyword("scoop"),
    CssToken::Keyword("bevel"),
    CssToken::Keyword("round"),
    CssToken::Keyword("squircle"),
    CssToken::Keyword("square"),
    CssToken::Descriptor(CssDescriptorKind::SuperellipseFn),
  ];
}

impl ToCss for Superellipse {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    if self.is_degenerate() {
      return dest.write_str("square");
    }
    if self.is_fully_concave() {
      return dest.write_str("notch");
    }

    match *self {
      Self::SCOOP => dest.write_str("scoop"),
      Self::BEVEL => dest.write_str("bevel"),
      Self::ROUND => dest.write_str("round"),
      Self::SQUIRCLE => dest.write_str("squircle"),
      Self(value) => write!(dest, "superellipse({})", value),
    }
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::*;
  use crate::{
    geometry::Size,
    style::{CalcArena, FromCssStr},
    viewport::Viewport,
  };

  fn sizing() -> SizingContext {
    SizingContext {
      viewport: Viewport::new((200, 100)),
      container_size: Size::NONE,
      container_read: Default::default(),
      font_size: 16.0,
      root_font_size: None,
      line_height: 0.0,
      root_line_height: None,
      calc_arena: Rc::new(CalcArena::default()),
    }
  }

  #[test]
  fn parses_keywords() {
    assert_eq!(Superellipse::from_css_str("round"), Ok(Superellipse::ROUND));
    assert_eq!(
      Superellipse::from_css_str("squircle"),
      Ok(Superellipse::SQUIRCLE)
    );
    assert_eq!(Superellipse::from_css_str("bevel"), Ok(Superellipse::BEVEL));
    assert_eq!(Superellipse::from_css_str("scoop"), Ok(Superellipse::SCOOP));
    assert_eq!(Superellipse::from_css_str("notch"), Ok(Superellipse::NOTCH));
    assert_eq!(
      Superellipse::from_css_str("square"),
      Ok(Superellipse::SQUARE)
    );
  }

  #[test]
  fn parses_superellipse_function() {
    assert_eq!(
      Superellipse::from_css_str("superellipse(2)"),
      Ok(Superellipse(2.0))
    );
    assert_eq!(
      Superellipse::from_css_str("superellipse(-1.5)"),
      Ok(Superellipse(-1.5))
    );
    assert_eq!(
      Superellipse::from_css_str("superellipse(infinity)"),
      Ok(Superellipse::SQUARE)
    );
    assert_eq!(
      Superellipse::from_css_str("superellipse(-infinity)"),
      Ok(Superellipse::NOTCH)
    );
  }

  #[test]
  fn rejects_invalid_values() {
    assert!(Superellipse::from_css_str("rounded").is_err());
    assert!(Superellipse::from_css_str("superellipse(auto)").is_err());
    assert!(Superellipse::from_css_str("superellipse()").is_err());
    assert!(Superellipse::from_css_str("superellipse(2 3)").is_err());
  }

  #[test]
  fn serializes_canonical_keywords() {
    let mut out = String::new();

    Superellipse::SQUIRCLE.to_css(&mut out).unwrap();
    assert_eq!(out, "squircle");

    out.clear();
    Superellipse(3.0).to_css(&mut out).unwrap();
    assert_eq!(out, "superellipse(3)");

    out.clear();
    Superellipse::SQUARE.to_css(&mut out).unwrap();
    assert_eq!(out, "square");
  }

  #[test]
  fn exponent_maps_keywords() {
    assert_eq!(Superellipse::ROUND.exponent(), 2.0);
    assert_eq!(Superellipse::SQUIRCLE.exponent(), 4.0);
    assert_eq!(Superellipse::BEVEL.exponent(), 1.0);
    assert_eq!(Superellipse::SCOOP.exponent(), 0.5);
    assert_eq!(Superellipse::SQUARE.exponent(), 65536.0);
    assert_eq!(Superellipse::NOTCH.exponent(), 65536.0f32.recip());
  }

  #[test]
  fn interpolable_mapping_round_trips() {
    for parameter in [-8.0, -1.5, -1.0, 0.0, 0.7, 1.0, 2.0, 8.0] {
      let shape = Superellipse(parameter);
      let round_tripped = Superellipse::from_interpolable(shape.to_interpolable());

      assert!(
        (round_tripped.0 - parameter).abs() < 1e-3,
        "{parameter} round-tripped to {}",
        round_tripped.0
      );
    }
  }

  #[test]
  fn interpolation_round_trips_endpoints() {
    let sizing = sizing();
    let mut value = Superellipse::ROUND;

    value.interpolate(
      &Superellipse::ROUND,
      &Superellipse::BEVEL,
      0.0,
      &sizing,
      Color::black(),
    );
    assert!((value.0 - 1.0).abs() < 1e-4);

    value.interpolate(
      &Superellipse::ROUND,
      &Superellipse::BEVEL,
      1.0,
      &sizing,
      Color::black(),
    );
    assert!(value.0.abs() < 1e-4);

    value.interpolate(
      &Superellipse::NOTCH,
      &Superellipse::SQUARE,
      0.5,
      &sizing,
      Color::black(),
    );
    assert!(value.0.abs() < 1e-4);
  }
}
