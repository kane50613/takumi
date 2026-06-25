use std::fmt;

use cssparser::Parser;
use kurbo::{BezPath, ParamCurve, ParamCurveArclen, PathEl, PathSeg, Shape};
use taffy::{Point, Size};

use crate::style::{
  Angle, BasicShape, CssSyntaxKind, CssToken, FromCss, Length, MakeComputed, ParseResult,
  ShapeRadius, SizingContext, ToCss,
};

/// Flattening tolerance (CSS px) for arc-length integration and shape sampling.
const ARCLEN_ACCURACY: f64 = 0.25;

/// Represents the CSS `offset-rotate` property: the rotation applied to an
/// element as it travels along its `offset-path`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OffsetRotate {
  /// `auto [<angle>]`: face the direction of the path, plus an optional offset.
  Auto(Angle),
  /// `reverse [<angle>]`: face the reverse direction of the path, plus an optional offset.
  Reverse(Angle),
  /// `<angle>`: a fixed rotation that ignores the path direction.
  Fixed(Angle),
}

impl Default for OffsetRotate {
  fn default() -> Self {
    Self::Auto(Angle::zero())
  }
}

impl OffsetRotate {
  /// Resolves the final rotation, in radians, given the path tangent direction.
  pub fn resolve(self, tangent_radians: f32) -> f32 {
    match self {
      Self::Auto(angle) => tangent_radians + angle.to_radians(),
      Self::Reverse(angle) => tangent_radians + std::f32::consts::PI + angle.to_radians(),
      Self::Fixed(angle) => angle.to_radians(),
    }
  }
}

impl MakeComputed for OffsetRotate {}

impl<'i> FromCss<'i> for OffsetRotate {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      let angle = input.try_parse(Angle::from_css).unwrap_or(Angle::zero());
      return Ok(Self::Auto(angle));
    }

    if input
      .try_parse(|input| input.expect_ident_matching("reverse"))
      .is_ok()
    {
      let angle = input.try_parse(Angle::from_css).unwrap_or(Angle::zero());
      return Ok(Self::Reverse(angle));
    }

    let angle = Angle::from_css(input)?;

    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Ok(Self::Auto(angle));
    }
    if input
      .try_parse(|input| input.expect_ident_matching("reverse"))
      .is_ok()
    {
      return Ok(Self::Reverse(angle));
    }

    Ok(Self::Fixed(angle))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Angle),
    CssToken::Keyword("auto"),
    CssToken::Keyword("reverse"),
  ];
}

impl ToCss for OffsetRotate {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Auto(angle) => {
        dest.write_str("auto")?;
        if *angle != Angle::zero() {
          dest.write_char(' ')?;
          angle.to_css(dest)?;
        }
        Ok(())
      }
      Self::Reverse(angle) => {
        dest.write_str("reverse")?;
        if *angle != Angle::zero() {
          dest.write_char(' ')?;
          angle.to_css(dest)?;
        }
        Ok(())
      }
      Self::Fixed(angle) => angle.to_css(dest),
    }
  }
}

fn resolve_radius(
  radius: ShapeRadius,
  center: Size<f32>,
  sizing: &SizingContext,
  full: f32,
) -> f64 {
  f64::from(match radius {
    ShapeRadius::ClosestSide => center.width.min(center.height),
    ShapeRadius::FarthestSide => center.width.max(center.height),
    ShapeRadius::Length(length) => length.to_px(sizing, full),
  })
}

/// Converts an `offset-path` basic shape into a flattened path in the element's
/// box-local coordinate space. Mirrors the geometry used by clip-path masking.
fn basic_shape_to_bezpath(
  shape: &BasicShape,
  sizing: &SizingContext,
  size: Size<f32>,
) -> Option<BezPath> {
  let px = |length: Length, full: f32| f64::from(length.to_px(sizing, full));

  match shape {
    BasicShape::Path(path_shape) => BezPath::from_svg(&path_shape.path).ok(),
    BasicShape::Polygon(polygon) => {
      let mut coordinates = polygon.coordinates.iter();
      let first = coordinates.next()?;
      let mut path = BezPath::new();
      path.move_to((px(first.x, size.width), px(first.y, size.height)));
      for coordinate in coordinates {
        path.line_to((px(coordinate.x, size.width), px(coordinate.y, size.height)));
      }
      path.close_path();
      Some(path)
    }
    BasicShape::Ellipse(ellipse) => {
      let center = Size {
        width: ellipse.position.0.x.to_px(sizing, size.width),
        height: ellipse.position.0.y.to_px(sizing, size.height),
      };
      let radius_x = resolve_radius(ellipse.radius_x, center, sizing, size.width);
      let radius_y = resolve_radius(ellipse.radius_y, center, sizing, size.height);
      let ellipse = kurbo::Ellipse::new(
        (f64::from(center.width), f64::from(center.height)),
        (radius_x, radius_y),
        0.0,
      );
      Some(ellipse.to_path(ARCLEN_ACCURACY))
    }
    BasicShape::Inset(inset) => {
      // ponytail: rounded-inset corners ignored for offset-path sampling.
      let [top, right, bottom, left] = inset.inset.0;
      let rect = kurbo::Rect::new(
        px(left, size.width),
        px(top, size.height),
        f64::from(size.width) - px(right, size.width),
        f64::from(size.height) - px(bottom, size.height),
      );
      Some(rect.to_path(ARCLEN_ACCURACY))
    }
  }
}

fn point_and_tangent(segment: PathSeg, t: f64) -> (Point<f32>, f32) {
  let point = segment.eval(t);
  let before = segment.eval((t - 1e-3).max(0.0));
  let after = segment.eval((t + 1e-3).min(1.0));
  let tangent = (after.y - before.y).atan2(after.x - before.x) as f32;

  (
    Point {
      x: point.x as f32,
      y: point.y as f32,
    },
    tangent,
  )
}

/// Samples an `offset-path` at `distance`, returning the point (box-local px)
/// and tangent direction (radians). Matches the CSS Motion Path distance model:
/// closed paths wrap, open paths clamp.
pub fn sample_offset_path(
  shape: &BasicShape,
  distance: Length,
  sizing: &SizingContext,
  border_box: Size<f32>,
) -> Option<(Point<f32>, f32)> {
  let path = basic_shape_to_bezpath(shape, sizing, border_box)?;

  let closed = path.elements().contains(&PathEl::ClosePath);
  let segments: Vec<PathSeg> = path.segments().collect();
  if segments.is_empty() {
    return None;
  }

  let lengths: Vec<f64> = segments
    .iter()
    .map(|segment| segment.arclen(ARCLEN_ACCURACY))
    .collect();
  let total: f64 = lengths.iter().sum();
  if total <= 0.0 {
    return None;
  }

  let raw = f64::from(distance.to_px(sizing, total as f32));
  let mut remaining = if closed {
    raw.rem_euclid(total)
  } else {
    raw.clamp(0.0, total)
  };

  for (segment, length) in segments.iter().zip(&lengths) {
    if *length <= 0.0 {
      continue;
    }
    if remaining <= *length {
      let t = segment.inv_arclen(remaining, ARCLEN_ACCURACY);
      return Some(point_and_tangent(*segment, t));
    }
    remaining -= *length;
  }

  segments
    .last()
    .map(|segment| point_and_tangent(*segment, 1.0))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_keywords_and_angles() {
    assert_eq!(
      OffsetRotate::from_str("auto"),
      Ok(OffsetRotate::Auto(Angle::zero()))
    );
    assert_eq!(
      OffsetRotate::from_str("reverse"),
      Ok(OffsetRotate::Reverse(Angle::zero()))
    );
    assert_eq!(
      OffsetRotate::from_str("45deg"),
      Ok(OffsetRotate::Fixed(Angle::new(45.0)))
    );
    assert_eq!(
      OffsetRotate::from_str("auto 90deg"),
      Ok(OffsetRotate::Auto(Angle::new(90.0)))
    );
    assert_eq!(
      OffsetRotate::from_str("90deg auto"),
      Ok(OffsetRotate::Auto(Angle::new(90.0)))
    );
  }

  #[test]
  fn resolve_uses_tangent() {
    assert_eq!(OffsetRotate::Auto(Angle::zero()).resolve(1.0), 1.0);
    assert_eq!(OffsetRotate::Fixed(Angle::zero()).resolve(1.0), 0.0);
  }

  #[test]
  fn samples_horizontal_line_midpoint() {
    let shape = BasicShape::from_str("path('M 0 0 L 100 0')").unwrap();
    let sizing = SizingContext::builder()
      .viewport(crate::Viewport::new((200, 200)))
      .build();
    let size = Size {
      width: 200.0,
      height: 200.0,
    };

    let (point, tangent) =
      sample_offset_path(&shape, Length::Percentage(50.0), &sizing, size).unwrap();

    assert!((point.x - 50.0).abs() < 0.5, "x = {}", point.x);
    assert!(point.y.abs() < 0.5, "y = {}", point.y);
    assert!(tangent.abs() < 1e-3, "tangent = {tangent}");
  }

  #[test]
  fn open_path_clamps_distance() {
    let shape = BasicShape::from_str("path('M 0 0 L 100 0')").unwrap();
    let sizing = SizingContext::builder()
      .viewport(crate::Viewport::new((200, 200)))
      .build();
    let size = Size {
      width: 200.0,
      height: 200.0,
    };

    let (point, _) = sample_offset_path(&shape, Length::Percentage(150.0), &sizing, size).unwrap();
    assert!((point.x - 100.0).abs() < 0.5, "x = {}", point.x);
  }
}
