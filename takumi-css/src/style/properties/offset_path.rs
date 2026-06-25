use std::f32::consts::FRAC_PI_2;
use std::fmt;

use cssparser::Parser;
use kurbo::{BezPath, ParamCurve, ParamCurveArclen, PathEl, PathSeg, Shape};
use taffy::{Point, Size};

use crate::{
  declare_enum_from_css_impl,
  style::{
    Angle, Animatable, BasicShape, Color, CssSyntaxKind, CssToken, FromCss, Length, MakeComputed,
    ParseResult, ShapePosition, ShapeRadius, SizingContext, ToCss,
  },
};

/// Flattening tolerance (CSS px) for arc-length integration and shape sampling.
const ARCLEN_ACCURACY: f64 = 0.25;

/// `<ray-size>`: how the ray's 100% length is resolved against the reference box.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum RaySize {
  /// Distance to the closest box side.
  #[default]
  ClosestSide,
  /// Distance to the closest box corner.
  ClosestCorner,
  /// Distance to the farthest box side.
  FarthestSide,
  /// Distance to the farthest box corner.
  FarthestCorner,
  /// Distance to the box side the ray points at.
  Sides,
}

crate::style::properties::declare_enum_from_css_impl!(
  RaySize,
  "closest-side" => RaySize::ClosestSide,
  "closest-corner" => RaySize::ClosestCorner,
  "farthest-side" => RaySize::FarthestSide,
  "farthest-corner" => RaySize::FarthestCorner,
  "sides" => RaySize::Sides,
);

/// The `ray()` offset-path function: a line from a start point at a given angle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayShape {
  /// Direction; `0deg` points up, increasing clockwise.
  pub angle: Angle,
  /// How the ray's length (the 100% offset-distance) is resolved.
  pub size: RaySize,
  /// Whether the ray is shortened to keep the element within the box.
  pub contain: bool,
  /// Explicit `at <position>` start; falls back to offset-position then center.
  pub position: Option<ShapePosition>,
}

fn parse_ray<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, RayShape> {
  input.expect_function_matching("ray")?;
  input.parse_nested_block(|input| {
    let mut angle = None;
    let mut size = None;
    let mut contain = false;
    let mut position = None;

    loop {
      if angle.is_none()
        && let Ok(value) = input.try_parse(Angle::from_css)
      {
        angle = Some(value);
        continue;
      }
      if size.is_none()
        && let Ok(value) = input.try_parse(RaySize::from_css)
      {
        size = Some(value);
        continue;
      }
      if !contain
        && input
          .try_parse(|input| input.expect_ident_matching("contain"))
          .is_ok()
      {
        contain = true;
        continue;
      }
      if position.is_none()
        && input
          .try_parse(|input| input.expect_ident_matching("at"))
          .is_ok()
      {
        position = Some(ShapePosition::from_css(input)?);
        continue;
      }
      break;
    }

    let Some(angle) = angle else {
      return Err(input.new_error_for_next_token());
    };

    Ok(RayShape {
      angle,
      size: size.unwrap_or_default(),
      contain,
      position,
    })
  })
}

/// `<coord-box>`: which box edge a bare coord-box offset-path traces.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CoordBox {
  /// `content-box`
  ContentBox,
  /// `padding-box`
  PaddingBox,
  /// `border-box` (the offset-path initial coord-box).
  #[default]
  BorderBox,
  /// `margin-box`
  MarginBox,
  /// `fill-box`
  FillBox,
  /// `stroke-box`
  StrokeBox,
  /// `view-box`
  ViewBox,
}

declare_enum_from_css_impl!(
  CoordBox,
  "content-box" => CoordBox::ContentBox,
  "padding-box" => CoordBox::PaddingBox,
  "border-box" => CoordBox::BorderBox,
  "margin-box" => CoordBox::MarginBox,
  "fill-box" => CoordBox::FillBox,
  "stroke-box" => CoordBox::StrokeBox,
  "view-box" => CoordBox::ViewBox,
);

/// The non-`none` value of the CSS `offset-path` property.
#[derive(Debug, Clone, PartialEq)]
pub enum OffsetPath {
  /// `ray()`
  Ray(RayShape),
  /// A basic shape: `path()`, `polygon()`, `circle()`, `ellipse()`, `inset()`.
  Shape(BasicShape),
  /// A bare `<coord-box>` (traces that box's edge).
  CoordBox(CoordBox),
}

impl MakeComputed for OffsetPath {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let OffsetPath::Shape(shape) = self {
      shape.make_computed(sizing);
    }
  }
}

impl<'i> FromCss<'i> for OffsetPath {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(ray) = input.try_parse(parse_ray) {
      return Ok(OffsetPath::Ray(ray));
    }
    if let Ok(shape) = input.try_parse(BasicShape::from_css) {
      return Ok(OffsetPath::Shape(shape));
    }
    Ok(OffsetPath::CoordBox(CoordBox::from_css(input)?))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("ray()"),
    CssToken::Keyword("path()"),
    CssToken::Keyword("border-box"),
  ];
}

impl ToCss for OffsetPath {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      OffsetPath::Ray(ray) => {
        dest.write_str("ray(")?;
        ray.angle.to_css(dest)?;
        if ray.size != RaySize::default() {
          dest.write_char(' ')?;
          ray.size.to_css(dest)?;
        }
        if ray.contain {
          dest.write_str(" contain")?;
        }
        if let Some(position) = &ray.position {
          dest.write_str(" at ")?;
          position.to_css(dest)?;
        }
        dest.write_char(')')
      }
      OffsetPath::Shape(shape) => shape.to_css(dest),
      OffsetPath::CoordBox(coord_box) => coord_box.to_css(dest),
    }
  }
}

/// CSS `offset-anchor`: the box point that is placed on the path.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum OffsetAnchor {
  /// `auto`: use `transform-origin`.
  #[default]
  Auto,
  /// A fixed `<position>` within the box.
  Position(ShapePosition),
}

impl MakeComputed for OffsetAnchor {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let OffsetAnchor::Position(position) = self {
      position.make_computed(sizing);
    }
  }
}

impl OffsetAnchor {
  /// The explicit anchor point, or `None` when it defaults to transform-origin.
  pub fn resolve(self, sizing: &SizingContext, border_box: Size<f32>) -> Option<Point<f32>> {
    match self {
      OffsetAnchor::Auto => None,
      OffsetAnchor::Position(position) => Some(position_point(&position, sizing, border_box)),
    }
  }
}

impl<'i> FromCss<'i> for OffsetAnchor {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Ok(OffsetAnchor::Auto);
    }
    Ok(OffsetAnchor::Position(ShapePosition::from_css(input)?))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("auto"),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

impl ToCss for OffsetAnchor {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      OffsetAnchor::Auto => dest.write_str("auto"),
      OffsetAnchor::Position(position) => position.to_css(dest),
    }
  }
}

/// CSS `offset-position`: the path's starting position for `ray()`/shapes.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum OffsetPosition {
  /// `normal`: behaves as `at center` for the path's start.
  #[default]
  Normal,
  /// `auto`: use the element's static position (approximated as `normal`).
  Auto,
  /// A fixed `<position>`.
  Position(ShapePosition),
}

impl MakeComputed for OffsetPosition {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let OffsetPosition::Position(position) = self {
      position.make_computed(sizing);
    }
  }
}

impl OffsetPosition {
  fn resolve(self, sizing: &SizingContext, border_box: Size<f32>) -> Option<Point<f32>> {
    match self {
      OffsetPosition::Position(position) => Some(position_point(&position, sizing, border_box)),
      _ => None,
    }
  }
}

impl<'i> FromCss<'i> for OffsetPosition {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("normal"))
      .is_ok()
    {
      return Ok(OffsetPosition::Normal);
    }
    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Ok(OffsetPosition::Auto);
    }
    Ok(OffsetPosition::Position(ShapePosition::from_css(input)?))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Keyword("auto"),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

impl ToCss for OffsetPosition {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      OffsetPosition::Normal => dest.write_str("normal"),
      OffsetPosition::Auto => dest.write_str("auto"),
      OffsetPosition::Position(position) => position.to_css(dest),
    }
  }
}

/// The CSS `offset-rotate` property: rotation applied as the element travels
/// along its `offset-path`.
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

  fn angle_mut(&mut self) -> &mut Angle {
    match self {
      Self::Auto(angle) | Self::Reverse(angle) | Self::Fixed(angle) => angle,
    }
  }
}

impl MakeComputed for OffsetRotate {}

impl Animatable for OffsetRotate {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    // Interpolate the angle only when the auto/reverse/fixed kind matches.
    let same_kind = matches!(
      (from, to),
      (Self::Auto(_), Self::Auto(_))
        | (Self::Reverse(_), Self::Reverse(_))
        | (Self::Fixed(_), Self::Fixed(_))
    );

    if same_kind {
      *self = *from;
      let (Self::Auto(a) | Self::Reverse(a) | Self::Fixed(a)) = *from;
      let (Self::Auto(b) | Self::Reverse(b) | Self::Fixed(b)) = *to;
      let mut angle = a;
      angle.interpolate(&a, &b, progress, sizing, current_color);
      *self.angle_mut() = angle;
    } else {
      *self = if progress >= 0.5 { *to } else { *from };
    }
  }
}

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

/// The CSS `offset` shorthand.
#[derive(Debug, Clone, PartialEq)]
pub struct OffsetShorthand {
  /// `offset-position`
  pub position: OffsetPosition,
  /// `offset-path` (`None` = `none`).
  pub path: Option<OffsetPath>,
  /// `offset-distance`
  pub distance: Length,
  /// `offset-rotate`
  pub rotate: OffsetRotate,
  /// `offset-anchor`
  pub anchor: OffsetAnchor,
}

impl<'i> FromCss<'i> for OffsetShorthand {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let position = input
      .try_parse(OffsetPosition::from_css)
      .unwrap_or_default();

    let path = input
      .try_parse(<Option<OffsetPath>>::from_css)
      .ok()
      .flatten();

    let mut distance = Length::default();
    let mut rotate = OffsetRotate::default();
    if path.is_some() {
      let mut got_distance = false;
      let mut got_rotate = false;
      loop {
        if !got_distance && let Ok(value) = input.try_parse(Length::from_css) {
          distance = value;
          got_distance = true;
          continue;
        }
        if !got_rotate && let Ok(value) = input.try_parse(OffsetRotate::from_css) {
          rotate = value;
          got_rotate = true;
          continue;
        }
        break;
      }
    }

    let anchor = if input.try_parse(|input| input.expect_delim('/')).is_ok() {
      OffsetAnchor::from_css(input)?
    } else {
      OffsetAnchor::default()
    };

    Ok(OffsetShorthand {
      position,
      path,
      distance,
      rotate,
      anchor,
    })
  }

  const VALID_TOKENS: &'static [CssToken] = OffsetPath::VALID_TOKENS;
}

fn position_point(position: &ShapePosition, sizing: &SizingContext, size: Size<f32>) -> Point<f32> {
  Point {
    x: position.0.x.to_px(sizing, size.width),
    y: position.0.y.to_px(sizing, size.height),
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
    BasicShape::Path(path_shape) => {
      // path() coords are CSS px; scale to device space like the to_px shapes.
      let mut path = BezPath::from_svg(&path_shape.path).ok()?;
      path.apply_affine(kurbo::Affine::scale(f64::from(sizing.to_device(1.0))));
      Some(path)
    }
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

fn sample_bezpath(
  path: &BezPath,
  distance: Length,
  sizing: &SizingContext,
) -> Option<(Point<f32>, f32)> {
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

/// The ray's 100% length, resolved against the box (Blink `CalculateRayPathLength`).
fn ray_length(start: Point<f32>, ray: &RayShape, size: Size<f32>) -> f32 {
  let perpendicular = |reduce: fn(f32, f32) -> f32| {
    reduce(
      reduce(start.x.abs(), (start.x - size.width).abs()),
      reduce(start.y.abs(), (start.y - size.height).abs()),
    )
  };
  let corner = |reduce: fn(f32, f32) -> f32| {
    let distance = |x: f32, y: f32| ((start.x - x).powi(2) + (start.y - y).powi(2)).sqrt();
    reduce(
      reduce(distance(0.0, 0.0), distance(size.width, 0.0)),
      reduce(
        distance(size.width, size.height),
        distance(0.0, size.height),
      ),
    )
  };

  match ray.size {
    RaySize::ClosestSide => perpendicular(f32::min),
    RaySize::FarthestSide => perpendicular(f32::max),
    RaySize::ClosestCorner => corner(f32::min),
    RaySize::FarthestCorner => corner(f32::max),
    RaySize::Sides => {
      if start.x < 0.0 || start.x > size.width || start.y < 0.0 || start.y > size.height {
        return 0.0;
      }
      let theta = ray.angle.to_radians();
      let (mut sin_t, mut cos_t) = (theta.sin(), theta.cos());
      let vertical = if cos_t >= 0.0 {
        start.y
      } else {
        size.height - start.y
      };
      let horizontal = if sin_t >= 0.0 {
        size.width - start.x
      } else {
        start.x
      };
      cos_t = cos_t.abs();
      sin_t = sin_t.abs();
      if vertical * sin_t > horizontal * cos_t {
        horizontal / sin_t
      } else {
        vertical / cos_t
      }
    }
  }
}

fn sample_ray(
  ray: &RayShape,
  distance: Length,
  offset_position: &OffsetPosition,
  sizing: &SizingContext,
  border_box: Size<f32>,
) -> (Point<f32>, f32) {
  let start = ray
    .position
    .as_ref()
    .map(|position| position_point(position, sizing, border_box))
    .or_else(|| offset_position.resolve(sizing, border_box))
    .unwrap_or_else(|| position_point(&ShapePosition::default(), sizing, border_box));

  let length = ray_length(start, ray, border_box);
  let traveled = distance.to_px(sizing, length);
  // 0deg points up; the direction (and tangent) is the angle minus a quarter turn.
  let direction = ray.angle.to_radians() - FRAC_PI_2;

  (
    Point {
      x: start.x + traveled * direction.cos(),
      y: start.y + traveled * direction.sin(),
    },
    direction,
  )
}

/// Samples an `offset-path` at `distance`, returning the point (box-local px)
/// and tangent direction (radians).
pub fn sample_offset_path(
  path: &OffsetPath,
  distance: Length,
  offset_position: &OffsetPosition,
  sizing: &SizingContext,
  border_box: Size<f32>,
) -> Option<(Point<f32>, f32)> {
  match path {
    OffsetPath::Ray(ray) => Some(sample_ray(
      ray,
      distance,
      offset_position,
      sizing,
      border_box,
    )),
    OffsetPath::Shape(shape) => sample_bezpath(
      &basic_shape_to_bezpath(shape, sizing, border_box)?,
      distance,
      sizing,
    ),
    OffsetPath::CoordBox(_) => {
      // Every coord box falls back to the available border box.
      let rect = kurbo::Rect::new(
        0.0,
        0.0,
        f64::from(border_box.width),
        f64::from(border_box.height),
      );
      sample_bezpath(&rect.to_path(ARCLEN_ACCURACY), distance, sizing)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_sizing() -> SizingContext {
    SizingContext::builder()
      .viewport(crate::Viewport::new((200, 200)))
      .build()
  }

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
    let path = OffsetPath::from_str("path('M 0 0 L 100 0')").unwrap();
    let size = Size {
      width: 200.0,
      height: 200.0,
    };

    let (point, tangent) = sample_offset_path(
      &path,
      Length::Percentage(50.0),
      &OffsetPosition::Normal,
      &test_sizing(),
      size,
    )
    .unwrap();

    assert!((point.x - 50.0).abs() < 0.5, "x = {}", point.x);
    assert!(point.y.abs() < 0.5, "y = {}", point.y);
    assert!(tangent.abs() < 1e-3, "tangent = {tangent}");
  }

  #[test]
  fn open_path_clamps_distance() {
    let path = OffsetPath::from_str("path('M 0 0 L 100 0')").unwrap();
    let size = Size {
      width: 200.0,
      height: 200.0,
    };

    let (point, _) = sample_offset_path(
      &path,
      Length::Percentage(150.0),
      &OffsetPosition::Normal,
      &test_sizing(),
      size,
    )
    .unwrap();
    assert!((point.x - 100.0).abs() < 0.5, "x = {}", point.x);
  }

  #[test]
  fn path_coordinates_scale_with_device_pixel_ratio() {
    let path = OffsetPath::from_str("path('M 0 0 L 100 0')").unwrap();
    let size = Size {
      width: 400.0,
      height: 400.0,
    };
    let sizing = SizingContext::builder()
      .viewport(crate::Viewport::new((400, 400)).with_device_pixel_ratio(2.0))
      .build();

    let (point, _) = sample_offset_path(
      &path,
      Length::Percentage(50.0),
      &OffsetPosition::Normal,
      &sizing,
      size,
    )
    .unwrap();

    // The authored 100px line is CSS px; at dpr 2 its midpoint sits at 100 device px.
    assert!((point.x - 100.0).abs() < 0.5, "x = {}", point.x);
  }

  #[test]
  fn ray_points_up_at_zero_degrees() {
    let path = OffsetPath::from_str("ray(0deg)").unwrap();
    let size = Size {
      width: 200.0,
      height: 200.0,
    };

    // Start at center (100, 100); 0deg points up, so distance moves -y.
    let (point, _) = sample_offset_path(
      &path,
      Length::Px(50.0),
      &OffsetPosition::Normal,
      &test_sizing(),
      size,
    )
    .unwrap();

    assert!((point.x - 100.0).abs() < 0.5, "x = {}", point.x);
    assert!((point.y - 50.0).abs() < 0.5, "y = {}", point.y);
  }

  #[test]
  fn ray_at_position_overrides_start() {
    let path = OffsetPath::from_str("ray(90deg at 0% 0%)").unwrap();
    let size = Size {
      width: 200.0,
      height: 200.0,
    };

    // Start at (0,0); 90deg points right, so distance moves +x.
    let (point, _) = sample_offset_path(
      &path,
      Length::Px(30.0),
      &OffsetPosition::Normal,
      &test_sizing(),
      size,
    )
    .unwrap();

    assert!((point.x - 30.0).abs() < 0.5, "x = {}", point.x);
    assert!(point.y.abs() < 0.5, "y = {}", point.y);
  }
}
