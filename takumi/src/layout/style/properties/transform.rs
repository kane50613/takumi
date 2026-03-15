use std::ops::{Mul, MulAssign};

use cssparser::{Parser, Token, match_ignore_ascii_case};
use taffy::{Point, Size};

use crate::{
  layout::style::{
    Angle, Animatable, Color, CssSyntaxKind, CssToken, FromCss, Length, ListInterpolationStrategy,
    MakeComputed, ParseResult, PercentageNumber, lerp,
  },
  rendering::Sizing,
};

const DEFAULT_SCALE: f32 = 1.0;

/// Represents a single CSS transform operation
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Transform {
  /// Translates an element along the X-axis and Y-axis by the specified lengths
  Translate(Length, Length),
  /// Scales an element by the specified factors
  Scale(f32, f32),
  /// Rotates an element (2D rotation) by angle in degrees
  Rotate(Angle),
  /// Skews an element by the specified angles
  Skew(Angle, Angle),
  /// Applies raw affine matrix values
  Matrix(Affine),
}

impl MakeComputed for Transform {
  fn make_computed(&mut self, sizing: &Sizing) {
    if let Transform::Translate(x, y) = self {
      x.make_computed(sizing);
      y.make_computed(sizing);
    }
  }
}

impl Animatable for Transform {
  fn list_interpolation_strategy() -> ListInterpolationStrategy {
    ListInterpolationStrategy::PadToLongestWithNeutral
  }

  fn neutral_value_like(other: &Self) -> Option<Self> {
    Some(match *other {
      Transform::Translate(_, _) => Transform::Translate(Length::zero(), Length::zero()),
      Transform::Scale(_, _) => Transform::Scale(1.0, 1.0),
      Transform::Rotate(_) => Transform::Rotate(Angle::zero()),
      Transform::Skew(_, _) => Transform::Skew(Angle::zero(), Angle::zero()),
      Transform::Matrix(_) => Transform::Matrix(Affine::IDENTITY),
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
    *self = match (*from, *to) {
      (Transform::Translate(from_x, from_y), Transform::Translate(to_x, to_y)) => {
        let mut x = from_x;
        x.interpolate(&from_x, &to_x, progress, sizing, current_color);
        let mut y = from_y;
        y.interpolate(&from_y, &to_y, progress, sizing, current_color);
        Transform::Translate(x, y)
      }
      (Transform::Scale(from_x, from_y), Transform::Scale(to_x, to_y)) => {
        Transform::Scale(lerp(from_x, to_x, progress), lerp(from_y, to_y, progress))
      }
      (Transform::Rotate(from_angle), Transform::Rotate(to_angle)) => {
        let mut angle = from_angle;
        angle.interpolate(&from_angle, &to_angle, progress, sizing, current_color);
        Transform::Rotate(angle)
      }
      (Transform::Skew(from_x, from_y), Transform::Skew(to_x, to_y)) => {
        let mut x = from_x;
        x.interpolate(&from_x, &to_x, progress, sizing, current_color);
        let mut y = from_y;
        y.interpolate(&from_y, &to_y, progress, sizing, current_color);
        Transform::Skew(x, y)
      }
      (Transform::Matrix(from_affine), Transform::Matrix(to_affine)) => Transform::Matrix(Affine {
        a: lerp(from_affine.a, to_affine.a, progress),
        b: lerp(from_affine.b, to_affine.b, progress),
        c: lerp(from_affine.c, to_affine.c, progress),
        d: lerp(from_affine.d, to_affine.d, progress),
        x: lerp(from_affine.x, to_affine.x, progress),
        y: lerp(from_affine.y, to_affine.y, progress),
      }),
      _ => {
        if progress >= 0.5 {
          *to
        } else {
          *from
        }
      }
    };
  }
}

/// | a c x |
/// | b d y |
/// | 0 0 1 |
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[non_exhaustive]
pub struct Affine {
  /// Horizontal scaling / cosine of rotation
  pub a: f32,
  /// Horizontal shear / sine of rotation
  pub b: f32,
  /// Vertical shear / negative sine of rotation
  pub c: f32,
  /// Vertical scaling / cosine of rotation
  pub d: f32,
  /// Horizontal translation (always orthogonal regardless of rotation)
  pub x: f32,
  /// Vertical translation (always orthogonal regardless of rotation)
  pub y: f32,
}

impl Mul<Affine> for Affine {
  type Output = Affine;

  fn mul(self, rhs: Affine) -> Self::Output {
    if self.is_identity() {
      return rhs;
    }

    if rhs.is_identity() {
      return self;
    }

    Affine {
      a: self.a * rhs.a + self.c * rhs.b,
      b: self.b * rhs.a + self.d * rhs.b,
      c: self.a * rhs.c + self.c * rhs.d,
      d: self.b * rhs.c + self.d * rhs.d,
      x: self.a * rhs.x + self.c * rhs.y + self.x,
      y: self.b * rhs.x + self.d * rhs.y + self.y,
    }
  }
}

impl MulAssign<Affine> for Affine {
  fn mul_assign(&mut self, rhs: Affine) {
    *self = *self * rhs;
  }
}

impl Affine {
  /// Converts the affine transform to a column-major array.
  pub fn to_cols_array(&self) -> [f32; 6] {
    [self.a, self.b, self.c, self.d, self.x, self.y]
  }

  /// Returns the identity transform
  pub const IDENTITY: Self = Self {
    a: 1.0,
    b: 0.0,
    c: 0.0,
    d: 1.0,
    x: 0.0,
    y: 0.0,
  };

  /// Returns true if the transform is the identity transform
  pub fn is_identity(self) -> bool {
    (self.a - 1.0).abs() < 1e-6
      && self.b.abs() < 1e-6
      && self.c.abs() < 1e-6
      && (self.d - 1.0).abs() < 1e-6
      && self.x.abs() < 1e-6
      && self.y.abs() < 1e-6
  }

  /// Decomposes the translation part of the transform
  pub fn decompose_translation(self) -> Point<f32> {
    Point {
      x: self.x,
      y: self.y,
    }
  }

  /// Returns true if the transform is only a translation
  pub(crate) fn only_translation(self) -> bool {
    (self.a - 1.0).abs() < 1e-8
      && self.b.abs() < 1e-8
      && self.c.abs() < 1e-8
      && (self.d - 1.0).abs() < 1e-8
  }

  /// Creates a new rotation transform
  pub fn rotation(angle: Angle) -> Self {
    let (sin, cos) = angle.to_radians().sin_cos();

    Self {
      a: cos,
      b: sin,
      c: -sin,
      d: cos,
      x: 0.0,
      y: 0.0,
    }
  }

  /// Creates a new translation transform
  pub const fn translation(x: f32, y: f32) -> Self {
    Self {
      x,
      y,
      ..Self::IDENTITY
    }
  }

  /// Creates a new scale transform
  pub const fn scale(x: f32, y: f32) -> Self {
    Self {
      a: x,
      b: 0.0,
      c: 0.0,
      d: y,
      x: 0.0,
      y: 0.0,
    }
  }

  /// Transforms a point by the transform
  #[inline(always)]
  pub fn transform_point(self, point: Point<f32>) -> Point<f32> {
    // Fast path: If the transform is only a translation, we can just add the translation to the point
    if self.only_translation() {
      return Point {
        x: point.x + self.x,
        y: point.y + self.y,
      };
    }

    Point {
      x: self.a * point.x + self.c * point.y + self.x,
      y: self.b * point.x + self.d * point.y + self.y,
    }
  }

  /// Creates a new skew transform
  pub fn skew(x: Angle, y: Angle) -> Self {
    let tanx = x.to_radians().tan();
    let tany = y.to_radians().tan();

    Self {
      a: 1.0,
      b: tany,
      c: tanx,
      d: 1.0,
      x: 0.0,
      y: 0.0,
    }
  }

  /// Calculates the determinant of the transform
  #[inline(always)]
  pub fn determinant(self) -> f32 {
    self.a * self.d - self.b * self.c
  }

  /// Returns true if the transform is invertible
  #[inline(always)]
  pub fn is_invertible(self) -> bool {
    self.determinant().abs() > f32::EPSILON
  }

  /// Inverts the transform, returns `None` if the transform is not invertible
  pub fn invert(self) -> Option<Self> {
    let det = self.determinant();
    if det.abs() < f32::EPSILON {
      return None;
    }

    let inv_det = 1.0 / det;

    Some(Self {
      a: self.d * inv_det,
      b: self.b * -inv_det,
      c: self.c * -inv_det,
      d: self.a * inv_det,
      x: (self.d * self.x - self.c * self.y) * -inv_det,
      y: (self.b * self.x - self.a * self.y) * inv_det,
    })
  }

  /// Converts the transforms to a [`Affine`] instance
  ///
  /// CSS transform property applies transformations from left to right.
  /// For `transform: translate() rotate()`, the resulting matrix is translate * rotate.
  /// When applied to point p: translate * rotate * p, rotate is applied first.
  pub(crate) fn from_transforms<'a, I: Iterator<Item = &'a Transform>>(
    transforms: I,
    sizing: &Sizing,
    border_box: Size<f32>,
  ) -> Affine {
    let mut instance = Affine::IDENTITY;

    for transform in transforms {
      instance *= match *transform {
        Transform::Translate(x_length, y_length) => Affine::translation(
          x_length.to_px(sizing, border_box.width),
          y_length.to_px(sizing, border_box.height),
        ),
        Transform::Scale(x_scale, y_scale) => Affine::scale(x_scale, y_scale),
        Transform::Rotate(angle) => Affine::rotation(angle),
        Transform::Skew(x_angle, y_angle) => Affine::skew(x_angle, y_angle),
        Transform::Matrix(affine) => affine,
      };
    }

    instance
  }
}

impl From<Affine> for zeno::Transform {
  fn from(affine: Affine) -> Self {
    if affine.is_identity() {
      zeno::Transform::IDENTITY
    } else if affine.only_translation() {
      zeno::Transform::translation(affine.x, affine.y)
    } else {
      zeno::Transform::new(affine.a, affine.b, affine.c, affine.d, affine.x, affine.y)
    }
  }
}

impl<'i> FromCss<'i> for Affine {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let a = input.expect_number()?;
    input.expect_comma()?;
    let b = input.expect_number()?;
    input.expect_comma()?;
    let c = input.expect_number()?;
    input.expect_comma()?;
    let d = input.expect_number()?;
    input.expect_comma()?;
    let x = input.expect_number()?;
    input.expect_comma()?;
    let y = input.expect_number()?;

    Ok(Affine { a, b, c, d, x, y })
  }

  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Syntax(CssSyntaxKind::Number)];
}

/// A collection of transform operations that can be applied together
pub type Transforms = Box<[Transform]>;

impl<'i> FromCss<'i> for Transforms {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut transforms = Vec::new();

    while !input.is_exhausted() {
      let transform = Transform::from_css(input)?;
      transforms.push(transform);
    }

    Ok(transforms.into_boxed_slice())
  }

  const VALID_TOKENS: &'static [CssToken] = Transform::VALID_TOKENS;
}

impl<'i> FromCss<'i> for Transform {
  fn from_css(parser: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = parser.current_source_location();
    let token = parser.next()?;

    let Token::Function(function) = token else {
      return Err(
        location
          .new_basic_unexpected_token_error(token.clone())
          .into(),
      );
    };

    match_ignore_ascii_case! {function,
      "translate" => parser.parse_nested_block(|input| {
        let x = Length::from_css(input)?;
        input.expect_comma()?;
        let y = Length::from_css(input)?;

        Ok(Transform::Translate(x, y))
      }),
      "translatex" => parser.parse_nested_block(|input| Ok(Transform::Translate(
        Length::from_css(input)?,
        Length::zero(),
      ))),
      "translatey" => parser.parse_nested_block(|input| Ok(Transform::Translate(
        Length::zero(),
        Length::from_css(input)?,
      ))),
      "scale" => parser.parse_nested_block(|input| {
        let PercentageNumber(x) = PercentageNumber::from_css(input)?;
        if input.try_parse(Parser::expect_comma).is_ok() {
          let PercentageNumber(y) = PercentageNumber::from_css(input)?;
          Ok(Transform::Scale(x, y))
        } else {
          Ok(Transform::Scale(x, x))
        }
      }),
      "scalex" => parser.parse_nested_block(|input| Ok(Transform::Scale(
        PercentageNumber::from_css(input)?.0,
        DEFAULT_SCALE,
      ))),
      "scaley" => parser.parse_nested_block(|input| Ok(Transform::Scale(
        DEFAULT_SCALE,
        PercentageNumber::from_css(input)?.0,
      ))),
      "skew" => parser.parse_nested_block(|input| {
        let x = Angle::from_css(input)?;
        input.expect_comma()?;
        let y = Angle::from_css(input)?;

        Ok(Transform::Skew(x, y))
      }),
      "skewx" => parser.parse_nested_block(|input| Ok(Transform::Skew(
        Angle::from_css(input)?,
        Angle::default(),
      ))),
      "skewy" => parser.parse_nested_block(|input| Ok(Transform::Skew(
        Angle::default(),
        Angle::from_css(input)?,
      ))),
      "rotate" => parser.parse_nested_block(|input| Ok(Transform::Rotate(
        Angle::from_css(input)?,
      ))),
      "matrix" => parser.parse_nested_block(|input| Ok(Transform::Matrix(
        Affine::from_css(input)?,
      ))),
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Syntax(CssSyntaxKind::TransformFunction)];
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_transform_from_str() {
    assert_eq!(
      Transform::from_str("translate(10, 20px)"),
      Ok(Transform::Translate(Length::Px(10.0), Length::Px(20.0)))
    );
  }

  #[test]
  fn test_transform_scale_from_str() {
    assert_eq!(
      Transform::from_str("scale(10)"),
      Ok(Transform::Scale(10.0, 10.0))
    );
  }

  #[test]
  fn test_transform_invert() {
    let transform = Affine::rotation(Angle::new(45.0));

    assert!(transform.invert().is_some_and(|inverse| {
      let random_point = Point {
        x: 1234.0,
        y: -5678.0,
      };

      let processed_point = inverse.transform_point(transform.transform_point(random_point));

      (random_point.x - processed_point.x).abs() < 1.0
        && (random_point.y - processed_point.y).abs() < 1.0
    }));
  }
}
