use std::{
  fmt,
  ops::{Deref, DerefMut, Mul, MulAssign},
};

use cssparser::{ParseError as CssParseError, Parser, Token, match_ignore_ascii_case};
use tiny_skia::Transform as TinyTransform;

use crate::style::{
  Angle, Animatable, Color, FromCss, Length, ListInterpolationStrategy, MakeComputed, ParseMode,
  ParseResult, PercentageNumber, SizingContext, ToCss, discrete, lerp, parse_nested_block,
  unexpected_token,
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
  fn make_computed(&mut self, sizing: &SizingContext) {
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
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self = match (*from, *to) {
      (Transform::Translate(from_x, from_y), Transform::Translate(to_x, to_y)) => {
        Transform::Translate(
          Animatable::interpolated(&from_x, &to_x, progress, sizing, current_color),
          Animatable::interpolated(&from_y, &to_y, progress, sizing, current_color),
        )
      }
      (Transform::Scale(from_x, from_y), Transform::Scale(to_x, to_y)) => {
        Transform::Scale(lerp(from_x, to_x, progress), lerp(from_y, to_y, progress))
      }
      (Transform::Rotate(from_angle), Transform::Rotate(to_angle)) => Transform::Rotate(
        Angle::interpolated(&from_angle, &to_angle, progress, sizing, current_color),
      ),
      (Transform::Skew(from_x, from_y), Transform::Skew(to_x, to_y)) => Transform::Skew(
        Animatable::interpolated(&from_x, &to_x, progress, sizing, current_color),
        Animatable::interpolated(&from_y, &to_y, progress, sizing, current_color),
      ),
      (Transform::Matrix(from_affine), Transform::Matrix(to_affine)) => Transform::Matrix(Affine {
        a: lerp(from_affine.a, to_affine.a, progress),
        b: lerp(from_affine.b, to_affine.b, progress),
        c: lerp(from_affine.c, to_affine.c, progress),
        d: lerp(from_affine.d, to_affine.d, progress),
        x: lerp(from_affine.x, to_affine.x, progress),
        y: lerp(from_affine.y, to_affine.y, progress),
      }),
      _ => discrete(from, to, progress),
    };
  }
}

// | a c x |
// | b d y |
// | 0 0 1 |
/// An affine transform matrix, representing a combination of translation, rotation, scaling, and skewing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
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

impl From<Affine> for TinyTransform {
  fn from(transform: Affine) -> Self {
    TinyTransform::from_row(
      transform.a,
      transform.b,
      transform.c,
      transform.d,
      transform.x,
      transform.y,
    )
  }
}

impl From<TinyTransform> for Affine {
  fn from(transform: TinyTransform) -> Self {
    Self {
      a: transform.sx,
      b: transform.ky,
      c: transform.kx,
      d: transform.sy,
      x: transform.tx,
      y: transform.ty,
    }
  }
}

impl Mul<Affine> for Affine {
  type Output = Affine;

  fn mul(self, rhs: Affine) -> Self::Output {
    // A factor that only rounds the linear part is skipped, but never a translation, which a
    // large scale on the other factor could make visible.
    let is_noop = |factor: Self| factor.x == 0.0 && factor.y == 0.0 && factor.is_identity();

    if is_noop(self) {
      return rhs;
    }

    if is_noop(rhs) {
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
  /// Whether every entry is finite and neither axis collapses to a point. A rank-1 matrix passes,
  /// which `is_invertible` rejects, so it still reaches the paint it transforms.
  #[cfg(feature = "svg")]
  pub(crate) fn is_renderable(self) -> bool {
    let (scale_x, scale_y) = self.row_scales();

    self.to_cols_array().iter().all(|entry| entry.is_finite())
      && scale_x > f32::EPSILON
      && scale_y > f32::EPSILON
  }

  /// The length of each row of the linear part, as tiny-skia's `Transform::get_scale` has them.
  #[cfg(feature = "svg")]
  pub fn row_scales(self) -> (f32, f32) {
    (self.a.hypot(self.c), self.b.hypot(self.d))
  }

  /// The scale Blink's `LayoutSVGShape::StrokeWidthForMarkerUnits` divides a non-scaling stroke by:
  /// the root mean square of the axis scales.
  #[cfg(feature = "svg")]
  pub(crate) fn root_mean_square_scale(self) -> f32 {
    let (x, y) = self.scale_components();

    ((x * x + y * y) / 2.0).sqrt()
  }

  /// Blink's `AffineTransform::Inverse` where `IsInvertible` holds: a normal determinant. It
  /// divides in double precision as Blink does. Approximate: the result rounds to `f32`, so a scale
  /// times its inverse can miss one by an ulp, and the determinant is tested after an `f32` cast
  /// where Blink tests the double.
  #[cfg(feature = "svg")]
  pub(crate) fn normal_inverse(self) -> Option<Self> {
    let [a, b, c, d, x, y] = [self.a, self.b, self.c, self.d, self.x, self.y].map(f64::from);
    let det = a * d - b * c;

    if !(det as f32).is_normal() {
      return None;
    }

    Some(Self {
      a: (d / det) as f32,
      b: (-b / det) as f32,
      c: (-c / det) as f32,
      d: (a / det) as f32,
      x: ((c * y - d * x) / det) as f32,
      y: ((b * x - a * y) / det) as f32,
    })
  }

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

  /// Geometric mean of the X and Y axis scales.
  pub fn uniform_scale(self) -> f32 {
    let sx = (self.a * self.a + self.b * self.b).sqrt();
    let sy = (self.c * self.c + self.d * self.d).sqrt();
    (sx * sy).sqrt()
  }

  /// Returns true if the transform is only a translation
  pub fn only_translation(self) -> bool {
    (self.a - 1.0).abs() < 1e-8
      && self.b.abs() < 1e-8
      && self.c.abs() < 1e-8
      && (self.d - 1.0).abs() < 1e-8
  }

  /// Creates a new rotation transform, as gfx's `Transform::Rotate` takes the angle's
  /// [`Angle::sin_cos_degrees`].
  pub fn rotation(angle: Angle) -> Self {
    let (sin, cos) = angle.sin_cos_degrees();

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
  pub fn transform_point(self, x: f32, y: f32) -> (f32, f32) {
    // Fast path: If the transform is only a translation, we can just add the translation to the point
    if self.only_translation() {
      return (x + self.x, y + self.y);
    }

    (
      self.a * x + self.c * y + self.x,
      self.b * x + self.d * y + self.y,
    )
  }

  /// Skia's `SkMatrix::mapVector`: `(x, y)` under the linear part alone, translation left out.
  pub fn transform_vector(self, x: f32, y: f32) -> (f32, f32) {
    (self.a * x + self.c * y, self.b * x + self.d * y)
  }

  /// cc's `MathUtil::ComputeTransform2dScaleComponents`: how far the transform stretches its x
  /// and y axes.
  pub fn scale_components(self) -> (f32, f32) {
    (self.a.hypot(self.b), self.c.hypot(self.d))
  }

  /// Creates a new skew transform, as gfx's `Transform::Skew` takes `TanDegrees` of each angle in
  /// double precision.
  pub(crate) fn skew(x: Angle, y: Angle) -> Self {
    let tan_degrees = |angle: Angle| f64::from(*angle).to_radians().tan() as f32;
    let (tanx, tany) = (tan_degrees(x), tan_degrees(y));

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
  pub(crate) fn determinant(self) -> f32 {
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

  /// `SkMatrix::invert`: the inverse through doubles, none where the determinant is
  /// nearly zero or the inverse leaves the finite range.
  pub fn skia_invert(self) -> Option<Self> {
    let Affine { a, b, c, d, x, y } = self;

    if self == Affine::IDENTITY {
      return Some(self);
    }

    if b == 0.0 && c == 0.0 {
      if a != 1.0 || d != 1.0 {
        let (inv_x, inv_y) = (1.0 / a, 1.0 / d);

        return Some(Affine {
          a: inv_x,
          b: 0.0,
          c: 0.0,
          d: inv_y,
          x: -x * inv_x,
          y: -y * inv_y,
        });
      }

      return Some(Affine::translation(-x, -y));
    }

    let det = f64::from(a) * f64::from(d) - f64::from(c) * f64::from(b);
    let nearly_zero = 1.0 / 4096.0_f32;

    if (det as f32).abs() <= nearly_zero * nearly_zero * nearly_zero {
      return None;
    }

    let inv_det = 1.0 / det;
    let cross_scale = |p: f32, q: f32, r: f32, s: f32| {
      ((f64::from(p) * f64::from(q) - f64::from(r) * f64::from(s)) * inv_det) as f32
    };
    let inverse = Affine {
      a: (f64::from(d) * inv_det) as f32,
      b: (-f64::from(b) * inv_det) as f32,
      c: (-f64::from(c) * inv_det) as f32,
      d: (f64::from(a) * inv_det) as f32,
      x: cross_scale(c, y, d, x),
      y: cross_scale(b, x, a, y),
    };

    inverse
      .to_cols_array()
      .iter()
      .all(|value| value.is_finite())
      .then_some(inverse)
  }

  /// The transforms composed left to right into an [`Affine`].
  pub(crate) fn from_transforms<'a, I: Iterator<Item = &'a Transform>>(
    transforms: I,
    sizing: &SizingContext,
    width: f32,
    height: f32,
  ) -> Affine {
    let mut instance = Affine::IDENTITY;

    for transform in transforms {
      instance *= match *transform {
        Transform::Translate(x_length, y_length) => Affine::translation(
          x_length.to_px(sizing, width),
          y_length.to_px(sizing, height),
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

impl Affine {
  fn parse<'i>(input: &mut Parser<'i>, mode: ParseMode) -> ParseResult<'i, Self> {
    let mut entries = [0.0; 6];

    for (index, entry) in entries.iter_mut().enumerate() {
      if index > 0 {
        mode.skip_separator(input)?;
      }

      *entry = input.expect_number()?;
    }

    let [a, b, c, d, x, y] = entries;

    Ok(Affine { a, b, c, d, x, y })
  }
}

impl<'i> FromCss<'i> for Affine {
  fn from_css_in(input: &mut Parser<'i>, mode: ParseMode) -> ParseResult<'i, Self> {
    Self::parse(input, mode)
  }

  const VALID_TOKENS: &'static [&'static str] = &["<number>"];
}

/// An SVG `transform` attribute takes bare numbers as px and degrees, and optional commas.
impl ParseMode {
  /// The comma between two arguments, which an SVG attribute may leave out.
  fn skip_separator<'i>(self, input: &mut Parser<'i>) -> ParseResult<'i, ()> {
    if self == Self::SvgAttribute {
      let _ = input.try_parse(Parser::expect_comma);
    } else {
      input.expect_comma()?;
    }

    Ok(())
  }

  fn length<'i>(self, input: &mut Parser<'i>) -> ParseResult<'i, Length> {
    if self == Self::SvgAttribute {
      Ok(Length::Px(input.expect_number()?))
    } else {
      Length::from_css(input)
    }
  }

  fn angle<'i>(self, input: &mut Parser<'i>) -> ParseResult<'i, Angle> {
    if self == Self::SvgAttribute {
      Ok(Angle::new(input.expect_number()?))
    } else {
      Angle::from_css(input)
    }
  }
}

/// A collection of transform operations that can be applied together
#[derive(Debug, Clone, PartialEq)]
pub struct Transforms(pub Box<[Transform]>);

impl<'i> FromCss<'i> for Transforms {
  fn from_css_in(input: &mut Parser<'i>, mode: ParseMode) -> ParseResult<'i, Self> {
    Self::parse(input, mode)
  }

  const VALID_TOKENS: &'static [&'static str] = Transform::VALID_TOKENS;
}

impl Transforms {
  fn parse<'i>(input: &mut Parser<'i>, mode: ParseMode) -> ParseResult<'i, Self> {
    let mut transforms = Vec::new();

    while !input.is_exhausted() {
      transforms.push(Transform::parse(input, mode)?);

      if mode == ParseMode::SvgAttribute {
        let _ = input.try_parse(Parser::expect_comma);
      }
    }

    Ok(Transforms(transforms.into_boxed_slice()))
  }

  /// The list an SVG `transform` attribute writes, none when it is malformed.
  #[cfg(feature = "svg")]
  pub(crate) fn from_svg_attribute(text: &str) -> Option<Self> {
    Self::from_css_in(&mut Parser::new(text), ParseMode::SvgAttribute).ok()
  }
}

impl MakeComputed for Transforms {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.0.make_computed(sizing);
  }
}

impl Animatable for Transforms {
  fn missing_value() -> Option<Self> {
    <Box<[Transform]>>::missing_value().map(Self)
  }

  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    self
      .0
      .interpolate(&from.0, &to.0, progress, sizing, current_color);
  }
}

impl Deref for Transforms {
  type Target = Box<[Transform]>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for Transforms {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl From<Box<[Transform]>> for Transforms {
  fn from(box_slice: Box<[Transform]>) -> Self {
    Self(box_slice)
  }
}

impl From<Vec<Transform>> for Transforms {
  fn from(vec: Vec<Transform>) -> Self {
    Self(vec.into_boxed_slice())
  }
}

impl<const N: usize> From<[Transform; N]> for Transforms {
  fn from(arr: [Transform; N]) -> Self {
    Self(Box::from(arr))
  }
}

impl ToCss for Transform {
  const LIST_SEPARATOR: &'static str = " ";

  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Translate(x, y) => {
        dest.write_str("translate(")?;
        x.to_css(dest)?;
        dest.write_str(", ")?;
        y.to_css(dest)?;
        dest.write_char(')')
      }
      Self::Scale(x, y) => write!(dest, "scale({x}, {y})"),
      Self::Rotate(a) => {
        dest.write_str("rotate(")?;
        a.to_css(dest)?;
        dest.write_char(')')
      }
      Self::Skew(x, y) => {
        dest.write_str("skew(")?;
        x.to_css(dest)?;
        dest.write_str(", ")?;
        y.to_css(dest)?;
        dest.write_char(')')
      }
      Self::Matrix(affine) => affine.to_css(dest),
    }
  }
}

impl ToCss for Affine {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    let Self { a, b, c, d, x, y } = self;
    write!(dest, "matrix({a}, {b}, {c}, {d}, {x}, {y})")
  }
}

impl ToCss for Transforms {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.0.to_css(dest)
  }
}

impl Transform {
  fn parse<'i>(parser: &mut Parser<'i>, mode: ParseMode) -> ParseResult<'i, Self> {
    let function = match parser.next()?.clone() {
      Token::Function(function) => function,
      // An SVG attribute may put spaces between a function's name and its parenthesis.
      Token::Ident(function) if mode == ParseMode::SvgAttribute => {
        parser.expect_parenthesis_block()?;

        function
      }
      _ => return Err(CssParseError::unexpected_token()),
    };

    let parse: fn(&mut Parser<'i>, ParseMode) -> ParseResult<'i, Self> = match_ignore_ascii_case! {&function,
      "translate" => |input, mode| {
        let x = mode.length(input)?;

        mode.skip_separator(input)?;

        let y = if mode == ParseMode::SvgAttribute {
          input
            .try_parse(|input| mode.length(input))
            .unwrap_or_else(|_| Length::zero())
        } else {
          mode.length(input)?
        };

        Ok(Self::Translate(x, y))
      },
      "translatex" => |input, mode| Ok(Self::Translate(
        mode.length(input)?,
        Length::zero(),
      )),
      "translatey" => |input, mode| Ok(Self::Translate(
        Length::zero(),
        mode.length(input)?,
      )),
      "scale" => |input, mode| {
        let PercentageNumber(x) = PercentageNumber::from_css(input)?;
        let has_comma = input.try_parse(Parser::expect_comma).is_ok();
        let has_y = if mode == ParseMode::SvgAttribute {
          !input.is_exhausted()
        } else {
          has_comma
        };

        if has_y {
          let PercentageNumber(y) = PercentageNumber::from_css(input)?;
          Ok(Self::Scale(x, y))
        } else {
          Ok(Self::Scale(x, x))
        }
      },
      "scalex" => |input, _| Ok(Self::Scale(
        PercentageNumber::from_css(input)?.0,
        DEFAULT_SCALE,
      )),
      "scaley" => |input, _| Ok(Self::Scale(
        DEFAULT_SCALE,
        PercentageNumber::from_css(input)?.0,
      )),
      "skew" => |input, mode| {
        let x = mode.angle(input)?;
        mode.skip_separator(input)?;
        let y = mode.angle(input)?;

        Ok(Self::Skew(x, y))
      },
      "skewx" => |input, mode| Ok(Self::Skew(
        mode.angle(input)?,
        Angle::default(),
      )),
      "skewy" => |input, mode| Ok(Self::Skew(
        Angle::default(),
        mode.angle(input)?,
      )),
      "rotate" => |input, mode| {
        let angle = mode.angle(input)?;

        if mode == ParseMode::SvgAttribute {
          mode.skip_separator(input)?;

          if let Ok(x) = input.try_parse(Parser::expect_number) {
            mode.skip_separator(input)?;

            let y = input.expect_number()?;
            let about = Affine::translation(x, y) * Affine::rotation(angle) * Affine::translation(-x, -y);

            return Ok(Self::Matrix(about));
          }
        }

        Ok(Self::Rotate(angle))
      },
      "matrix" => |input, mode| Ok(Self::Matrix(Affine::parse(input, mode)?)),
      _ => return Err(unexpected_token!(&Token::Function(function))),
    };

    parse_nested_block(parser, |input| parse(input, mode))
  }
}

impl<'i> FromCss<'i> for Transform {
  fn from_css_in(parser: &mut Parser<'i>, mode: ParseMode) -> ParseResult<'i, Self> {
    Self::parse(parser, mode)
  }

  const VALID_TOKENS: &'static [&'static str] = &["<transform-function>"];
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;
  #[cfg(feature = "svg")]
  use crate::viewport::Viewport;

  #[cfg(feature = "svg")]
  #[test]
  fn a_scale_and_its_normal_inverse_multiply_back_to_identity() {
    let scale = Affine::scale(0.2, 0.2);
    let inverse = scale.normal_inverse().expect("a scale inverts");

    assert_eq!(
      (scale * inverse).to_cols_array(),
      Affine::IDENTITY.to_cols_array()
    );
  }

  #[cfg(feature = "svg")]
  fn svg_matrix(text: &str) -> Option<[f32; 6]> {
    let sizing = SizingContext::builder()
      .viewport(Viewport::new((100, Some(100))))
      .build();
    let transforms = Transforms::from_svg_attribute(text)?;

    Some(Affine::from_transforms(transforms.iter(), &sizing, 0.0, 0.0).to_cols_array())
  }

  #[cfg(feature = "svg")]
  #[test]
  fn an_svg_attribute_writes_its_lists_without_units() {
    assert_eq!(
      svg_matrix("translate(10 15) translate(0 5)"),
      Some([1.0, 0.0, 0.0, 1.0, 10.0, 20.0])
    );
    assert_eq!(svg_matrix("scale(2)"), Some([2.0, 0.0, 0.0, 2.0, 0.0, 0.0]));
    assert_eq!(
      svg_matrix("translate(5), scale(2 3)"),
      Some([2.0, 0.0, 0.0, 3.0, 5.0, 0.0])
    );
    assert_eq!(
      svg_matrix("translate(10-5)"),
      Some([1.0, 0.0, 0.0, 1.0, 10.0, -5.0])
    );
    assert_eq!(svg_matrix(""), Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
    assert_eq!(
      svg_matrix("rotate(90 10 10)").map(|matrix| matrix.map(f32::round)),
      Some([0.0, 1.0, -1.0, 0.0, 20.0, 0.0])
    );
    assert_eq!(
      svg_matrix("matrix(1 0 0 1 3,4)"),
      Some([1.0, 0.0, 0.0, 1.0, 3.0, 4.0])
    );
    assert_eq!(
      svg_matrix("  matrix  (  1 0 0 1 3 4  )  "),
      Some([1.0, 0.0, 0.0, 1.0, 3.0, 4.0])
    );
  }

  #[cfg(feature = "svg")]
  #[test]
  fn an_svg_attribute_with_a_bad_item_is_invalid() {
    assert_eq!(svg_matrix("translate(10) bogus(1)"), None);
    assert_eq!(svg_matrix("scale()"), None);
  }

  #[test]
  fn css_still_wants_commas_and_units() {
    assert!(Transform::from_css_str("translate(10 20)").is_err());
    assert!(Transform::from_css_str("rotate(10 1 1)").is_err());
  }

  #[test]
  fn a_small_translation_survives_a_product_with_a_large_scale() {
    let product = Affine::scale(1e6, 1e6) * Affine::translation(9e-7, 0.0);

    assert!((product.x - 0.9).abs() < 1e-4, "{product:?}");
  }

  #[test]
  fn test_transform_from_str() {
    assert_eq!(
      Transform::from_css_str("translate(10px, 20px)"),
      Ok(Transform::Translate(Length::Px(10.0), Length::Px(20.0)))
    );
    assert!(Transform::from_css_str("translate(10, 20px)").is_err());
  }

  #[test]
  fn test_transform_scale_from_str() {
    assert_eq!(
      Transform::from_css_str("scale(10)"),
      Ok(Transform::Scale(10.0, 10.0))
    );
  }

  #[test]
  fn transform_functions_keep_their_individual_grammars() {
    for (css, expected) in [
      (
        "translate(1px, 2px)",
        Transform::Translate(Length::Px(1.0), Length::Px(2.0)),
      ),
      (
        "translateX(1px)",
        Transform::Translate(Length::Px(1.0), Length::zero()),
      ),
      (
        "translateY(2px)",
        Transform::Translate(Length::zero(), Length::Px(2.0)),
      ),
      ("scale(2, 3)", Transform::Scale(2.0, 3.0)),
      ("scaleX(2)", Transform::Scale(2.0, DEFAULT_SCALE)),
      ("scaleY(3)", Transform::Scale(DEFAULT_SCALE, 3.0)),
      (
        "skew(1deg, 2deg)",
        Transform::Skew(Angle::new(1.0), Angle::new(2.0)),
      ),
      (
        "skewX(1deg)",
        Transform::Skew(Angle::new(1.0), Angle::default()),
      ),
      (
        "skewY(2deg)",
        Transform::Skew(Angle::default(), Angle::new(2.0)),
      ),
      ("rotate(3deg)", Transform::Rotate(Angle::new(3.0))),
      (
        "matrix(1, 0, 0, 1, 2, 3)",
        Transform::Matrix(Affine::translation(2.0, 3.0)),
      ),
    ] {
      assert_eq!(Transform::from_css_str(css), Ok(expected), "{css}");
    }
  }

  #[test]
  fn transform_errors_reject_missing_trailing_and_unknown_tokens() {
    assert!(Transform::from_css_str("translate(1px)").is_err());
    assert!(Transforms::from_css_str("translateX(1px) trailing").is_err());
    assert!(Transforms::from_css_str("spin(1deg)").is_err());
  }

  #[test]
  fn test_transform_negative_scale_reflects() {
    assert_eq!(
      Transform::from_css_str("scale(-1)"),
      Ok(Transform::Scale(-1.0, -1.0))
    );
    assert_eq!(
      Transform::from_css_str("scaleX(-1)"),
      Ok(Transform::Scale(-1.0, DEFAULT_SCALE))
    );
    assert_eq!(
      Transform::from_css_str("scaleY(-2)"),
      Ok(Transform::Scale(DEFAULT_SCALE, -2.0))
    );
  }

  #[test]
  fn quarter_turns_rotate_exactly() {
    for (degrees, (sin, cos)) in [
      (0.0, (0.0, 1.0)),
      (90.0, (1.0, 0.0)),
      (180.0, (0.0, -1.0)),
      (270.0, (-1.0, 0.0)),
      (-90.0, (-1.0, 0.0)),
      (450.0, (1.0, 0.0)),
    ] {
      let rotation = Affine::rotation(Angle::new(degrees));

      assert_eq!(
        (rotation.a, rotation.b, rotation.c, rotation.d),
        (cos, sin, -sin, cos),
        "{degrees}deg"
      );
    }
  }

  /// gfx's `SinCosDegrees` and `TanDegrees` read the specified degrees in double precision.
  #[test]
  fn rotation_and_skew_read_the_specified_degrees() {
    for degrees in [30.0_f32, 400.0, -330.0, 1000.5, 98765.4] {
      let radians = f64::from(degrees).to_radians();
      let rotation = Affine::rotation(Angle::new(degrees));
      let skew = Affine::skew(Angle::new(degrees), Angle::zero());

      assert_eq!(
        (rotation.a, rotation.b),
        (radians.cos() as f32, radians.sin() as f32),
        "{degrees}deg"
      );
      assert_eq!(skew.c, radians.tan() as f32, "{degrees}deg");
    }
  }

  #[test]
  fn test_transform_invert() {
    let transform = Affine::rotation(Angle::new(45.0));

    assert!(transform.invert().is_some_and(|inverse| {
      let (x, y) = (1234.0, -5678.0);

      let (transformed_x, transformed_y) = transform.transform_point(x, y);
      let (processed_x, processed_y) = inverse.transform_point(transformed_x, transformed_y);

      (x - processed_x).abs() < 1.0 && (y - processed_y).abs() < 1.0
    }));
  }
}
