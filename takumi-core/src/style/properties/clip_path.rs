use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case, serialize_string};

use crate::style::{
  CssDescriptorKind, CssSyntaxKind, CssToken, FromCss, Length, MakeComputed, ParseResult, Sides,
  SizingContext, SpacePair, ToCss, unexpected_token,
};

/// Represents the fill rule used for determining the interior of shapes.
///
/// Corresponds to the SVG fill-rule attribute and is used in polygon(), path(), and shape() functions.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FillRule {
  /// The default rule - counts the number of times a ray from the point crosses the shape's edges
  #[default]
  NonZero,
  /// Counts the total number of crossings - if even, the point is outside
  EvenOdd,
}

/// Represents radius values for circle() and ellipse() functions.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ShapeRadius {
  /// Uses the length from the center to the closest side of the reference box
  #[default]
  ClosestSide,
  /// Uses the length from the center to the farthest side of the reference box
  FarthestSide,
  /// A specific length value
  Length(Length),
}

impl MakeComputed for ShapeRadius {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let ShapeRadius::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}

/// Represents a position for circle() and ellipse() functions.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ShapePosition(pub SpacePair<Length>);

impl MakeComputed for ShapePosition {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.0.make_computed(sizing);
  }
}

impl Default for ShapePosition {
  fn default() -> Self {
    Self(SpacePair::from_single(Length::Percentage(50.0)))
  }
}

/// Represents an inset() rectangle shape.
///
/// The inset() function creates an inset rectangle, with its size defined by the offset distance
/// of each of the four sides of its container and, optionally, rounded corners.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct InsetShape {
  /// Sides of the inset.
  pub inset: Sides<Length>,
  /// Optional border radius for rounded corners
  pub border_radius: Option<Sides<Length>>,
}

impl MakeComputed for InsetShape {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.inset.make_computed(sizing);
    self.border_radius.make_computed(sizing);
  }
}

/// Represents an ellipse() shape.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct EllipseShape {
  /// The horizontal radius
  pub radius_x: ShapeRadius,
  /// The vertical radius
  pub radius_y: ShapeRadius,
  /// The center position of the ellipse
  pub position: ShapePosition,
}

impl MakeComputed for EllipseShape {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.radius_x.make_computed(sizing);
    self.radius_y.make_computed(sizing);
    self.position.make_computed(sizing);
  }
}

/// Represents a single coordinate pair in a polygon.
pub(crate) type PolygonCoordinate = SpacePair<Length>;

/// Represents a polygon() shape.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PolygonShape {
  /// The fill rule to use
  pub fill_rule: Option<FillRule>,
  /// List of coordinate pairs defining the polygon vertices
  pub coordinates: Box<[PolygonCoordinate]>,
}

impl MakeComputed for PolygonShape {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.coordinates.make_computed(sizing);
  }
}

/// Represents a path() shape using an SVG path string.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PathShape {
  /// The fill rule to use
  pub fill_rule: Option<FillRule>,
  /// SVG path data string
  pub path: Box<str>,
}

/// Represents a basic shape function for clip-path.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BasicShape {
  /// inset() function
  Inset(Box<InsetShape>),
  /// ellipse() function
  Ellipse(Box<EllipseShape>),
  /// polygon() function
  Polygon(PolygonShape),
  /// path() function
  Path(PathShape),
}

impl MakeComputed for BasicShape {
  fn make_computed(&mut self, sizing: &SizingContext) {
    match self {
      BasicShape::Inset(shape) => shape.make_computed(sizing),
      BasicShape::Ellipse(shape) => shape.make_computed(sizing),
      BasicShape::Polygon(shape) => shape.make_computed(sizing),
      BasicShape::Path(_) => {}
    }
  }
}

impl BasicShape {
  /// The shape's fill rule, if it has one.
  pub fn fill_rule(&self) -> Option<FillRule> {
    match self {
      BasicShape::Polygon(shape) => shape.fill_rule,
      BasicShape::Path(shape) => shape.fill_rule,
      _ => None,
    }
  }
}

crate::style::properties::declare_enum_from_css_impl!(
  FillRule,
  "nonzero" => FillRule::NonZero,
  "evenodd" => FillRule::EvenOdd,
);

impl<'i> FromCss<'i> for ShapeRadius {
  fn from_css(parser: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = parser.current_source_location();

    // Try parsing as length first
    if let Ok(length) = parser.try_parse(Length::from_css) {
      return Ok(ShapeRadius::Length(length));
    }

    // Try parsing keywords
    let ident = parser.expect_ident()?;
    match_ignore_ascii_case! { &ident,
      "closest-side" => Ok(ShapeRadius::ClosestSide),
      "farthest-side" => Ok(ShapeRadius::FarthestSide),
      _ => Err(unexpected_token!(location, &Token::Ident(ident.clone()))),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("closest-side"),
    CssToken::Keyword("farthest-side"),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

impl<'i> FromCss<'i> for ShapePosition {
  fn from_css(parser: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let first = Length::from_css(parser)?;

    // If there's a second value, parse it; otherwise default to 50%
    let second = parser
      .try_parse(Length::from_css)
      .unwrap_or(Length::Percentage(50.0));

    Ok(ShapePosition(SpacePair::from_pair(first, second)))
  }

  const VALID_TOKENS: &'static [CssToken] = Length::VALID_TOKENS;
}

impl<'i> FromCss<'i> for BasicShape {
  fn from_css(parser: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = parser.current_source_location();
    let token = parser.next()?;

    match token {
      Token::Function(function) => {
        match_ignore_ascii_case! { &function,
          "inset" => parser.parse_nested_block(|input| {
            let inset = Sides::from_css(input)?;

            // Parse border radius with "round" keyword
            let border_radius = if input.try_parse(|input| input.expect_ident_matching("round")).is_ok() {
              Some(Sides::from_css(input)?)
            } else {
              None
            };

            Ok(BasicShape::Inset(Box::new(InsetShape {
              inset,
              border_radius,
            })))
          }),
          "circle" => parser.parse_nested_block(|input| {
            let radius = input.try_parse(ShapeRadius::from_css).unwrap_or_default();

            let position = if input.try_parse(|input| input.expect_ident_matching("at")).is_ok() {
              ShapePosition::from_css(input)?
            } else {
              ShapePosition::default()
            };

            Ok(BasicShape::Ellipse(Box::new(EllipseShape { radius_x: radius, radius_y: radius, position })))
          }),
          "ellipse" => parser.parse_nested_block(|input| {
            let radius_x = ShapeRadius::from_css(input)?;
            let radius_y = input.try_parse(ShapeRadius::from_css).unwrap_or_default();

            let position = if input.try_parse(|input| input.expect_ident_matching("at")).is_ok() {
              ShapePosition::from_css(input)?
            } else {
              ShapePosition::default()
            };

            Ok(BasicShape::Ellipse(Box::new(EllipseShape { radius_x, radius_y, position })))
          }),
          "polygon" => parser.parse_nested_block(|input| {
            let fill_rule = input.try_parse(FillRule::from_css).ok();
            if fill_rule.is_some() {
              input.expect_comma()?;
            }

            Ok(BasicShape::Polygon(PolygonShape {
              fill_rule,
              coordinates: input
                .parse_comma_separated(PolygonCoordinate::from_css)?
                .into_boxed_slice(),
            }))
          }),
          "path" => parser.parse_nested_block(|input| {
            let fill_rule = input.try_parse(FillRule::from_css).ok();
            if fill_rule.is_some() {
              input.expect_comma()?;
            }

            let path = input.expect_string()?.as_ref().into();

            Ok(BasicShape::Path(PathShape {
              fill_rule,
              path,
            }))
          }),
          _ => Err(unexpected_token!(location, token)),
        }
      }
      _ => Err(unexpected_token!(location, token)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Descriptor(CssDescriptorKind::InsetFn),
    CssToken::Descriptor(CssDescriptorKind::CircleFn),
    CssToken::Descriptor(CssDescriptorKind::EllipseFn),
    CssToken::Descriptor(CssDescriptorKind::PolygonFn),
    CssToken::Descriptor(CssDescriptorKind::PathFn),
  ];
}

impl ToCss for ShapeRadius {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::ClosestSide => dest.write_str("closest-side"),
      Self::FarthestSide => dest.write_str("farthest-side"),
      Self::Length(l) => l.to_css(dest),
    }
  }
}

impl ToCss for ShapePosition {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.0.to_css(dest)
  }
}

impl ToCss for BasicShape {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Inset(shape) => {
        dest.write_str("inset(")?;
        shape.inset.to_css(dest)?;
        if let Some(radius) = &shape.border_radius {
          dest.write_str(" round ")?;
          radius.to_css(dest)?;
        }
        dest.write_char(')')
      }
      Self::Ellipse(shape) => {
        if shape.radius_x == shape.radius_y {
          dest.write_str("circle(")?;
          let mut has_radius = false;
          if shape.radius_x != ShapeRadius::ClosestSide {
            shape.radius_x.to_css(dest)?;
            has_radius = true;
          }
          if shape.position != ShapePosition::default() {
            if has_radius {
              dest.write_char(' ')?;
            }
            dest.write_str("at ")?;
            shape.position.to_css(dest)?;
          }
          dest.write_char(')')
        } else {
          dest.write_str("ellipse(")?;
          shape.radius_x.to_css(dest)?;
          dest.write_char(' ')?;
          shape.radius_y.to_css(dest)?;
          if shape.position != ShapePosition::default() {
            dest.write_str(" at ")?;
            shape.position.to_css(dest)?;
          }
          dest.write_char(')')
        }
      }
      Self::Polygon(shape) => {
        dest.write_str("polygon(")?;
        if let Some(rule) = shape.fill_rule {
          rule.to_css(dest)?;
          dest.write_str(", ")?;
        }
        let mut first = true;
        for coord in shape.coordinates.iter() {
          if !first {
            dest.write_str(", ")?;
          }
          coord.to_css(dest)?;
          first = false;
        }
        dest.write_char(')')
      }
      Self::Path(shape) => {
        dest.write_str("path(")?;
        if let Some(rule) = shape.fill_rule {
          rule.to_css(dest)?;
          dest.write_str(", ")?;
        }
        serialize_string(&shape.path, dest)?;
        dest.write_char(')')
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::assert_matches;

  use Length::*;

  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_inset_simple() {
    assert_eq!(
      BasicShape::from_css_str("inset(10px)"),
      Ok(BasicShape::Inset(Box::new(InsetShape {
        inset: Sides([Px(10.0); 4]),
        border_radius: None,
      })))
    );
  }

  #[test]
  fn test_parse_inset_four_values() {
    assert_eq!(
      BasicShape::from_css_str("inset(10px 20px 30px 40px)"),
      Ok(BasicShape::Inset(Box::new(InsetShape {
        inset: Sides([Px(10.0), Px(20.0), Px(30.0), Px(40.0)]),
        border_radius: None,
      })))
    );
  }

  #[test]
  fn test_parse_inset_with_border_radius() {
    assert_eq!(
      BasicShape::from_css_str("inset(10px round 5px)"),
      Ok(BasicShape::Inset(Box::new(InsetShape {
        inset: Sides::from(Px(10.0)),
        border_radius: Some(Sides::from(Px(5.0))),
      })))
    );
  }

  #[test]
  fn test_parse_inset_with_complex_border_radius() {
    assert_eq!(
      BasicShape::from_css_str("inset(10px 20px 30px 40px round 5px 10px 15px 20px)"),
      Ok(BasicShape::Inset(Box::new(InsetShape {
        inset: Sides([Px(10.0), Px(20.0), Px(30.0), Px(40.0)]),
        border_radius: Some(Sides([Px(5.0), Px(10.0), Px(15.0), Px(20.0)])),
      })))
    );
  }

  #[test]
  fn test_parse_circle_simple() {
    assert_eq!(
      BasicShape::from_css_str("circle(50px)"),
      Ok(BasicShape::Ellipse(Box::new(EllipseShape {
        radius_x: ShapeRadius::Length(Px(50.0)),
        radius_y: ShapeRadius::Length(Px(50.0)),
        position: ShapePosition::default(),
      })))
    );
  }

  #[test]
  fn test_parse_circle_with_position() {
    assert_eq!(
      BasicShape::from_css_str("circle(50px at 25% 75%)"),
      Ok(BasicShape::Ellipse(Box::new(EllipseShape {
        radius_x: ShapeRadius::Length(Px(50.0)),
        radius_y: ShapeRadius::Length(Px(50.0)),
        position: ShapePosition(SpacePair {
          x: Length::Percentage(25.0),
          y: Length::Percentage(75.0),
        }),
      })))
    );
  }

  #[test]
  fn test_parse_circle_default_radius() {
    assert_eq!(
      BasicShape::from_css_str("circle(at 25% 75%)"),
      Ok(BasicShape::Ellipse(Box::new(EllipseShape {
        radius_x: ShapeRadius::ClosestSide,
        radius_y: ShapeRadius::ClosestSide,
        position: ShapePosition(SpacePair {
          x: Length::Percentage(25.0),
          y: Length::Percentage(75.0),
        }),
      })))
    );
  }

  #[test]
  fn test_parse_ellipse_simple() {
    assert_eq!(
      BasicShape::from_css_str("ellipse(50px 30px)"),
      Ok(BasicShape::Ellipse(Box::new(EllipseShape {
        radius_x: ShapeRadius::Length(Px(50.0)),
        radius_y: ShapeRadius::Length(Px(30.0)),
        position: ShapePosition::default(),
      })))
    );
  }

  #[test]
  fn test_parse_ellipse_with_position() {
    assert_eq!(
      BasicShape::from_css_str("ellipse(50px 30px at 25% 75%)"),
      Ok(BasicShape::Ellipse(Box::new(EllipseShape {
        radius_x: ShapeRadius::Length(Px(50.0)),
        radius_y: ShapeRadius::Length(Px(30.0)),
        position: ShapePosition(SpacePair {
          x: Length::Percentage(25.0),
          y: Length::Percentage(75.0),
        }),
      })))
    );
  }

  #[test]
  fn test_parse_polygon_triangle() {
    assert_matches!(
      BasicShape::from_css_str("polygon(50% 0%, 0% 100%, 100% 100%)"),
      Ok(BasicShape::Polygon(PolygonShape {
        fill_rule: None,
        coordinates: coords,
      })) if coords.len() == 3 &&
            coords[0] == SpacePair { x: Length::Percentage(50.0), y: Length::Percentage(0.0) } &&
            coords[1] == SpacePair { x: Length::Percentage(0.0), y: Length::Percentage(100.0) } &&
            coords[2] == SpacePair { x: Length::Percentage(100.0), y: Length::Percentage(100.0) }
    );
  }

  #[test]
  fn test_parse_polygon_with_fill_rule() {
    assert_matches!(
      BasicShape::from_css_str("polygon(evenodd, 50% 0%, 0% 100%, 100% 100%)"),
      Ok(BasicShape::Polygon(PolygonShape {
        fill_rule: Some(FillRule::EvenOdd),
        coordinates: coords,
      })) if coords.len() == 3
    );
  }

  #[test]
  fn test_parse_path() {
    assert_eq!(
      BasicShape::from_css_str("path('M 10 10 L 90 90')"),
      Ok(BasicShape::Path(PathShape {
        fill_rule: None,
        path: "M 10 10 L 90 90".into(),
      }))
    );
  }

  #[test]
  fn test_parse_path_with_fill_rule() {
    assert_eq!(
      BasicShape::from_css_str("path(evenodd, 'M 10 10 L 90 90')"),
      Ok(BasicShape::Path(PathShape {
        fill_rule: Some(FillRule::EvenOdd),
        path: "M 10 10 L 90 90".into(),
      }))
    );
  }

  #[test]
  fn test_parse_circle_percentage_radius() {
    assert_eq!(
      BasicShape::from_css_str("circle(50%)"),
      Ok(BasicShape::Ellipse(Box::new(EllipseShape {
        radius_x: ShapeRadius::Length(Length::Percentage(50.0)),
        radius_y: ShapeRadius::Length(Length::Percentage(50.0)),
        position: ShapePosition::default(),
      })))
    );
  }

  #[test]
  fn test_parse_circle_closest_side() {
    assert_eq!(
      BasicShape::from_css_str("circle(closest-side)"),
      Ok(BasicShape::Ellipse(Box::new(EllipseShape {
        radius_x: ShapeRadius::ClosestSide,
        radius_y: ShapeRadius::ClosestSide,
        position: ShapePosition::default(),
      })))
    );
  }

  #[test]
  fn test_parse_circle_farthest_side() {
    assert_eq!(
      BasicShape::from_css_str("circle(farthest-side)"),
      Ok(BasicShape::Ellipse(Box::new(EllipseShape {
        radius_x: ShapeRadius::FarthestSide,
        radius_y: ShapeRadius::FarthestSide,
        position: ShapePosition::default(),
      })))
    );
  }
}
