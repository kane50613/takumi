use std::{fmt::Display, ops::Mul};

use cssparser::{Parser, Token, match_ignore_ascii_case};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use taffy::{Layout, Point, Size};
use ts_rs::TS;
use zeno::{Command, Vector};

use crate::{
  layout::style::{Angle, FromCss, LengthUnit, ParseResult, PercentageNumber},
  rendering::RenderContext,
};

const DEFAULT_SCALE: f32 = 1.0;

/// Represents a single CSS transform operation
#[derive(Debug, Clone, Deserialize, Serialize, Copy, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Transform {
  /// Translates an element along the X-axis and Y-axis by the specified lengths
  Translate(LengthUnit, LengthUnit),
  /// Scales an element by the specified factors
  Scale(f32, f32),
  /// Rotates an element (2D rotation) by angle in degrees
  Rotate(Angle),
  /// Skews an element by the specified angles
  Skew(Angle, Angle),
  /// Applies raw affine matrix values
  Matrix(Affine),
}

/// A collection of transform operations that can be applied together
#[derive(Debug, Clone, Deserialize, Serialize, TS, Default, PartialEq)]
#[ts(as = "TransformsValue")]
#[serde(try_from = "TransformsValue")]
pub struct Transforms(pub SmallVec<[Transform; 4]>);

impl Transforms {
  /// Converts the transforms to a [`Affine`] instance
  pub(crate) fn to_affine(
    &self,
    context: &RenderContext,
    layout: &Layout,
    center: Point<f32>,
  ) -> Affine {
    let mut instance = Affine::identity();

    for transform in self.0.iter().rev() {
      match *transform {
        Transform::Translate(x_length, y_length) => {
          instance = instance
            * Affine::translation(Size {
              width: x_length.resolve_to_px(context, layout.size.width),
              height: y_length.resolve_to_px(context, layout.size.height),
            });
        }
        Transform::Scale(x_scale, y_scale) => {
          instance = instance
            * Affine::scale(
              Size {
                width: x_scale,
                height: y_scale,
              },
              center,
            );
        }
        Transform::Rotate(angle) => {
          instance = instance * Affine::rotation(angle, center);
        }
        Transform::Skew(x_angle, y_angle) => {
          instance = instance
            * Affine::skew(
              Size {
                width: x_angle,
                height: y_angle,
              },
              center,
            );
        }
        Transform::Matrix(affine) => {
          instance = instance * affine;
        }
      }
    }

    instance
  }
}

/// Represents transform values that can be either a structured list or raw CSS
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(untagged)]
pub(crate) enum TransformsValue {
  /// A structured list of transform operations
  #[ts(as = "Vec<Transform>")]
  Transforms(SmallVec<[Transform; 4]>),
  /// Raw CSS transform string to be parsed
  Css(String),
}

impl<'i> FromCss<'i> for Transforms {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut transforms = SmallVec::new();

    while !input.is_exhausted() {
      let transform = Transform::from_css(input)?;
      transforms.push(transform);
    }

    Ok(Transforms(transforms))
  }
}

impl TryFrom<TransformsValue> for Transforms {
  type Error = String;

  fn try_from(value: TransformsValue) -> Result<Self, Self::Error> {
    match value {
      TransformsValue::Transforms(transforms) => Ok(Transforms(transforms)),
      TransformsValue::Css(css) => Transforms::from_str(&css).map_err(|e| e.to_string()),
    }
  }
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
        let x = LengthUnit::from_css(input)?;
        input.expect_comma()?;
        let y = LengthUnit::from_css(input)?;

        Ok(Transform::Translate(x, y))
      }),
      "translatex" => parser.parse_nested_block(|input| Ok(Transform::Translate(
        LengthUnit::from_css(input)?,
        LengthUnit::zero(),
      ))),
      "translatey" => parser.parse_nested_block(|input| Ok(Transform::Translate(
        LengthUnit::zero(),
        LengthUnit::from_css(input)?,
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
      _ => Err(location.new_basic_unexpected_token_error(token.clone()).into()),
    }
  }
}

/// Represents an affine transform matrix
#[derive(PartialEq, Deserialize, Serialize, Debug, Clone, Copy, TS)]
pub struct Affine {
  /// Horizontal scaling / cosine of rotation
  pub a: f32,
  /// Vertical shear / sine of rotation
  pub b: f32,
  /// Horizontal shear / negative sine of rotation
  pub c: f32,
  /// Vertical scaling / cosine of rotation
  pub d: f32,
  /// Horizontal translation (always orthogonal regardless of rotation)
  pub x: f32,
  /// Vertical translation (always orthogonal regardless of rotation)
  pub y: f32,
}

impl<'i> FromCss<'i> for Affine {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let a = input.expect_number()?;
    let b = input.expect_number()?;
    let c = input.expect_number()?;
    let d = input.expect_number()?;
    let x = input.expect_number()?;
    let y = input.expect_number()?;

    Ok(Self { a, b, c, d, x, y })
  }
}

impl Default for Affine {
  fn default() -> Self {
    Self::identity()
  }
}

impl Mul for Affine {
  type Output = Self;

  #[inline]
  fn mul(self, rhs: Self) -> Self {
    let lhs = self;

    Self {
      a: lhs.a * rhs.a + lhs.b * rhs.c,
      b: lhs.a * rhs.b + lhs.b * rhs.d,
      c: lhs.c * rhs.a + lhs.d * rhs.c,
      d: lhs.c * rhs.b + lhs.d * rhs.d,
      x: lhs.x * rhs.a + lhs.y * rhs.c + rhs.x,
      y: lhs.x * rhs.b + lhs.y * rhs.d + rhs.y,
    }
  }
}

impl Mul<Affine> for Point<f32> {
  type Output = Point<f32>;

  #[inline]
  fn mul(self, m: Affine) -> Point<f32> {
    Point {
      x: self.x * m.a + self.y * m.c + m.x,
      y: self.x * m.b + self.y * m.d + m.y,
    }
  }
}

impl Mul<Affine> for Vector {
  type Output = Vector;

  #[inline]
  fn mul(self, m: Affine) -> Vector {
    Vector {
      x: self.x * m.a + self.y * m.c + m.x,
      y: self.x * m.b + self.y * m.d + m.y,
    }
  }
}

impl Affine {
  /// Checks if the transform is the identity transform
  pub fn is_identity(self) -> bool {
    self == Self::identity()
  }

  /// Creates a new identity transform
  pub const fn identity() -> Self {
    Self {
      a: 1.0,
      b: 0.0,
      c: 0.0,
      d: 1.0,
      x: 0.0,
      y: 0.0,
    }
  }

  /// Applies the transform on the paths
  pub fn apply_on_paths(self, mask: &mut [Command]) {
    if self.is_identity() {
      return;
    }

    for command in mask {
      match command {
        Command::MoveTo(target) => {
          let point = (*target) * self;

          *command = Command::MoveTo(point);
        }
        Command::LineTo(target) => {
          let point = (*target) * self;

          *command = Command::LineTo(point);
        }
        Command::CurveTo(target1, target2, target3) => {
          let point1 = (*target1) * self;
          let point2 = (*target2) * self;
          let point3 = (*target3) * self;

          *command = Command::CurveTo(point1, point2, point3);
        }
        Command::QuadTo(target1, target2) => {
          let point1 = (*target1) * self;
          let point2 = (*target2) * self;

          *command = Command::QuadTo(point1, point2);
        }
        Command::Close => {}
      }
    }
  }

  /// Creates a new rotation transform
  pub fn rotation(angle: Angle, center: Point<f32>) -> Self {
    let angle = angle.to_radians();
    let cos = angle.cos();
    let sin = angle.sin();

    Self {
      a: cos,
      b: sin,
      c: -sin,
      d: cos,
      x: center.x - cos * center.x + sin * center.y,
      y: center.y - cos * center.y - sin * center.x,
    }
  }

  /// Creates a new translation transform
  pub const fn translation(size: Size<f32>) -> Self {
    Self {
      x: size.width,
      y: size.height,
      ..Self::identity()
    }
  }

  /// Creates a new scale transform
  pub const fn scale(scale: Size<f32>, center: Point<f32>) -> Self {
    Self {
      a: scale.width,
      b: 0.0,
      c: 0.0,
      d: scale.height,
      x: center.x - scale.width * center.x,
      y: center.y - scale.height * center.y,
    }
  }

  /// Creates a new skew transform
  pub fn skew(angle: Size<Angle>, center: Point<f32>) -> Self {
    let tanx = angle.width.to_radians().tan();
    let tany = angle.height.to_radians().tan();

    Self {
      a: 1.0,
      b: tany,
      c: tanx,
      d: 1.0,
      x: -center.y * tany,
      y: -center.x * tanx,
    }
  }

  /// Calculates the determinant of the transform
  pub fn determinant(self) -> f32 {
    self.a * self.d - self.b * self.c
  }

  /// Inverts the transform, returns `None` if the transform is not invertible
  pub fn invert(self) -> Option<Self> {
    let det = self.determinant();
    if det.abs() < f32::EPSILON {
      return None;
    }

    Some(Self {
      a: self.d / det,
      b: self.b / -det,
      c: self.c / -det,
      d: self.a / det,
      x: (self.d * self.x - self.c * self.y) / -det,
      y: (self.b * self.x - self.a * self.y) / det,
    })
  }

  /// Zero the translation
  pub fn zero_translation(&mut self) {
    self.x = 0.0;
    self.y = 0.0;
  }

  /// Decomposes the transform into a scale, rotation, and translation
  pub(crate) fn decompose(self) -> DecomposedTransform {
    DecomposedTransform {
      scale: Size {
        width: (self.a * self.a + self.c * self.c).sqrt(),
        height: (self.b * self.b + self.d * self.d).sqrt(),
      },
      rotation: Angle::new(self.b.atan2(self.d).to_degrees()),
      translation: Size {
        width: self.x,
        height: self.y,
      },
    }
  }
}

pub(crate) struct DecomposedTransform {
  pub scale: Size<f32>,
  pub rotation: Angle,
  pub translation: Size<f32>,
}

impl Display for DecomposedTransform {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "DecomposedTransform(scale={:?}, rotation={:?}, translation={:?})",
      self.scale, self.rotation, self.translation
    )
  }
}

impl DecomposedTransform {
  /// Checks if the transform is rotated
  pub fn is_rotated(&self) -> bool {
    self.rotation != Angle::zero()
  }

  /// Checks if the transform is scaled
  pub fn is_scaled(&self) -> bool {
    self.scale
      != Size {
        width: 1.0,
        height: 1.0,
      }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_transform_from_str() {
    let transform = Transform::from_str("translate(10, 20px)").unwrap();

    assert_eq!(
      transform,
      Transform::Translate(LengthUnit::Px(10.0), LengthUnit::Px(20.0))
    );
  }

  #[test]
  fn test_transform_scale_from_str() {
    let transform = Transform::from_str("scale(10)").unwrap();

    assert_eq!(transform, Transform::Scale(10.0, 10.0));
  }
}
