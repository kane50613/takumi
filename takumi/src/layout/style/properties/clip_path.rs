use cssparser::{Parser, Token, match_ignore_ascii_case};
use taffy::{AbsoluteAxis, Point, Rect, Size};
use zeno::{Fill, PathBuilder, PathData, Placement};

use crate::{
  layout::style::{Axis, Color, CssToken, FromCss, Length, ParseResult, Sides, SpacePair},
  rendering::{BorderProperties, MaskMemory, RenderContext, Sizing},
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

impl From<FillRule> for Fill {
  fn from(value: FillRule) -> Self {
    match value {
      FillRule::EvenOdd => Fill::EvenOdd,
      FillRule::NonZero => Fill::NonZero,
    }
  }
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

/// Represents a position for circle() and ellipse() functions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapePosition(pub SpacePair<Length>);

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
pub struct InsetShape {
  /// Sides of the inset.
  pub inset: Sides<Length>,
  /// Optional border radius for rounded corners
  pub border_radius: Option<Sides<Length>>,
}

/// Represents a circle() shape.
#[derive(Debug, Clone, PartialEq)]
pub struct CircleShape {
  /// The radius of the circle
  pub radius: ShapeRadius,
  /// The center position of the circle
  pub position: ShapePosition,
}

/// Represents an ellipse() shape.
#[derive(Debug, Clone, PartialEq)]
pub struct EllipseShape {
  /// The horizontal radius
  pub radius_x: ShapeRadius,
  /// The vertical radius
  pub radius_y: ShapeRadius,
  /// The center position of the ellipse
  pub position: ShapePosition,
}

/// Represents a single coordinate pair in a polygon.
pub type PolygonCoordinate = SpacePair<Length>;

/// Represents a polygon() shape.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonShape {
  /// The fill rule to use
  pub fill_rule: Option<FillRule>,
  /// List of coordinate pairs defining the polygon vertices
  pub coordinates: Vec<PolygonCoordinate>,
}

/// Represents a path() shape using an SVG path string.
#[derive(Debug, Clone, PartialEq)]
pub struct PathShape {
  /// The fill rule to use
  pub fill_rule: Option<FillRule>,
  /// SVG path data string
  pub path: String,
}

/// Represents a basic shape function for clip-path.
#[derive(Debug, Clone, PartialEq)]
pub enum BasicShape {
  /// inset() function
  Inset(InsetShape),
  /// ellipse() function
  Ellipse(EllipseShape),
  /// polygon() function
  Polygon(PolygonShape),
  /// path() function
  Path(PathShape),
}

fn resolve_radius(radius: ShapeRadius, distance: Size<f32>, sizing: &Sizing, full: f32) -> f32 {
  match radius {
    ShapeRadius::ClosestSide => distance.width.min(distance.height),
    ShapeRadius::FarthestSide => distance.width.max(distance.height),
    ShapeRadius::Length(length) => length.to_px(sizing, full),
  }
}

impl BasicShape {
  pub(crate) fn fill_rule(&self) -> Option<FillRule> {
    match self {
      BasicShape::Polygon(shape) => shape.fill_rule,
      BasicShape::Path(shape) => shape.fill_rule,
      _ => None,
    }
  }

  pub(crate) fn render_mask<'m>(
    &self,
    context: &RenderContext,
    size: Size<f32>,
    mask_memory: &'m mut MaskMemory,
  ) -> (&'m [u8], Placement) {
    let mut paths = Vec::new();

    match self {
      BasicShape::Inset(shape) => {
        let inset: Rect<f32> = shape
          .inset
          .map_axis(|value, axis| {
            value.to_px(
              &context.sizing,
              match axis {
                Axis::Horizontal => size.width,
                Axis::Vertical => size.height,
              },
            )
          })
          .into();

        let border = BorderProperties {
          width: Rect::zero(),
          color: Color::transparent(),
          radius: shape
            .border_radius
            .map(|radius| {
              Sides(
                radius
                  .0
                  .map(|corner| SpacePair::from_single(corner.to_px(&context.sizing, size.width))),
              )
            })
            .unwrap_or_default(),
        };

        border.append_mask_commands(
          &mut paths,
          Size {
            width: size.width - inset.grid_axis_sum(AbsoluteAxis::Horizontal),
            height: size.height - inset.grid_axis_sum(AbsoluteAxis::Vertical),
          },
          Point {
            x: inset.left,
            y: inset.top,
          },
        );
      }
      BasicShape::Ellipse(shape) => {
        let distance = Size {
          width: shape.position.0.x.to_px(&context.sizing, size.width),
          height: shape.position.0.y.to_px(&context.sizing, size.height),
        };

        paths.add_ellipse(
          (distance.width, distance.height),
          resolve_radius(shape.radius_x, distance, &context.sizing, size.width),
          resolve_radius(shape.radius_y, distance, &context.sizing, size.height),
        );
      }
      BasicShape::Polygon(shape) => {
        if !shape.coordinates.is_empty() {
          // Start the path at the first coordinate
          let first = &shape.coordinates[0];
          let first_x = first.x.to_px(&context.sizing, size.width);
          let first_y = first.y.to_px(&context.sizing, size.height);

          paths.move_to((first_x, first_y));

          // Add lines to each subsequent coordinate
          for coord in &shape.coordinates[1..] {
            let x = coord.x.to_px(&context.sizing, size.width);
            let y = coord.y.to_px(&context.sizing, size.height);
            paths.line_to((x, y));
          }

          // Close the path to complete the polygon
          paths.close();
        }
      }
      BasicShape::Path(shape) => {
        paths.extend(shape.path.as_str().commands());
      }
    }

    mask_memory.render(
      &paths,
      Some(context.transform),
      Some(Fill::from(self.fill_rule().unwrap_or(context.style.clip_rule)).into()),
    )
  }
}

impl<'i> FromCss<'i> for FillRule {
  fn from_css(parser: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = parser.current_source_location();
    let ident = parser.expect_ident()?;

    match_ignore_ascii_case! { &ident,
      "nonzero" => Ok(FillRule::NonZero),
      "evenodd" => Ok(FillRule::EvenOdd),
      _ => Err(Self::unexpected_token_error(location, &Token::Ident(ident.clone()))),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Keyword("nonzero"), CssToken::Keyword("evenodd")]
  }
}

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
      _ => Err(Self::unexpected_token_error(location, &Token::Ident(ident.clone()))),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("closest-side"),
      CssToken::Keyword("farthest-side"),
      CssToken::Token("length"),
    ]
  }
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

  fn valid_tokens() -> &'static [CssToken] {
    Length::<true>::valid_tokens()
  }
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

            Ok(BasicShape::Inset(InsetShape {
              inset,
              border_radius,
            }))
          }),
          "circle" => parser.parse_nested_block(|input| {
            let radius = input.try_parse(ShapeRadius::from_css).unwrap_or_default();

            let position = if input.try_parse(|input| input.expect_ident_matching("at")).is_ok() {
              ShapePosition::from_css(input)?
            } else {
              ShapePosition::default()
            };

            Ok(BasicShape::Ellipse(EllipseShape { radius_x: radius, radius_y: radius, position }))
          }),
          "ellipse" => parser.parse_nested_block(|input| {
            let radius_x = ShapeRadius::from_css(input)?;
            let radius_y = input.try_parse(ShapeRadius::from_css).unwrap_or_default();

            let position = if input.try_parse(|input| input.expect_ident_matching("at")).is_ok() {
              ShapePosition::from_css(input)?
            } else {
              ShapePosition::default()
            };

            let ellipse_shape = EllipseShape { radius_x, radius_y, position };
            Ok(BasicShape::Ellipse(ellipse_shape))
          }),
          "polygon" => parser.parse_nested_block(|input| {
            let fill_rule = input.try_parse(FillRule::from_css).ok();
            if fill_rule.is_some() {
              input.expect_comma()?;
            }

            let mut coordinates = Vec::new();

            // Parse first coordinate pair
            coordinates.push(PolygonCoordinate::from_css(input)?);

            // Parse remaining coordinate pairs
            while input.try_parse(Parser::expect_comma).is_ok() {
              coordinates.push(PolygonCoordinate::from_css(input)?);
            }

            Ok(BasicShape::Polygon(PolygonShape {
              fill_rule,
              coordinates,
            }))
          }),
          "path" => parser.parse_nested_block(|input| {
            let fill_rule = input.try_parse(FillRule::from_css).ok();
            if fill_rule.is_some() {
              input.expect_comma()?;
            }

            let path = input.expect_string()?.to_string();

            Ok(BasicShape::Path(PathShape {
              fill_rule,
              path,
            }))
          }),
          _ => Err(Self::unexpected_token_error(location, token)),
        }
      }
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Token("inset()"),
      CssToken::Token("circle()"),
      CssToken::Token("ellipse()"),
      CssToken::Token("polygon()"),
      CssToken::Token("path()"),
    ]
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use Length::*;

  #[test]
  fn test_parse_inset_simple() {
    assert_eq!(
      BasicShape::from_str("inset(10px)"),
      Ok(BasicShape::Inset(InsetShape {
        inset: Sides([Px(10.0); 4]),
        border_radius: None,
      }))
    );
  }

  #[test]
  fn test_parse_inset_four_values() {
    assert_eq!(
      BasicShape::from_str("inset(10px 20px 30px 40px)"),
      Ok(BasicShape::Inset(InsetShape {
        inset: Sides([Px(10.0), Px(20.0), Px(30.0), Px(40.0)]),
        border_radius: None,
      }))
    );
  }

  #[test]
  fn test_parse_inset_with_border_radius() {
    assert_eq!(
      BasicShape::from_str("inset(10px round 5px)"),
      Ok(BasicShape::Inset(InsetShape {
        inset: Sides::from(Px(10.0)),
        border_radius: Some(Sides::from(Px(5.0))),
      }))
    );
  }

  #[test]
  fn test_parse_inset_with_complex_border_radius() {
    assert_eq!(
      BasicShape::from_str("inset(10px 20px 30px 40px round 5px 10px 15px 20px)"),
      Ok(BasicShape::Inset(InsetShape {
        inset: Sides([Px(10.0), Px(20.0), Px(30.0), Px(40.0)]),
        border_radius: Some(Sides([Px(5.0), Px(10.0), Px(15.0), Px(20.0)])),
      }))
    );
  }

  #[test]
  fn test_parse_circle_simple() {
    assert_eq!(
      BasicShape::from_str("circle(50px)"),
      Ok(BasicShape::Ellipse(EllipseShape {
        radius_x: ShapeRadius::Length(Px(50.0)),
        radius_y: ShapeRadius::Length(Px(50.0)),
        position: ShapePosition::default(),
      }))
    );
  }

  #[test]
  fn test_parse_circle_with_position() {
    assert_eq!(
      BasicShape::from_str("circle(50px at 25% 75%)"),
      Ok(BasicShape::Ellipse(EllipseShape {
        radius_x: ShapeRadius::Length(Px(50.0)),
        radius_y: ShapeRadius::Length(Px(50.0)),
        position: ShapePosition(SpacePair {
          x: Length::Percentage(25.0),
          y: Length::Percentage(75.0),
        }),
      }))
    );
  }

  #[test]
  fn test_parse_circle_default_radius() {
    assert_eq!(
      BasicShape::from_str("circle(at 25% 75%)"),
      Ok(BasicShape::Ellipse(EllipseShape {
        radius_x: ShapeRadius::ClosestSide,
        radius_y: ShapeRadius::ClosestSide,
        position: ShapePosition(SpacePair {
          x: Length::Percentage(25.0),
          y: Length::Percentage(75.0),
        }),
      }))
    );
  }

  #[test]
  fn test_parse_ellipse_simple() {
    assert_eq!(
      BasicShape::from_str("ellipse(50px 30px)"),
      Ok(BasicShape::Ellipse(EllipseShape {
        radius_x: ShapeRadius::Length(Px(50.0)),
        radius_y: ShapeRadius::Length(Px(30.0)),
        position: ShapePosition::default(),
      }))
    );
  }

  #[test]
  fn test_parse_ellipse_with_position() {
    assert_eq!(
      BasicShape::from_str("ellipse(50px 30px at 25% 75%)"),
      Ok(BasicShape::Ellipse(EllipseShape {
        radius_x: ShapeRadius::Length(Px(50.0)),
        radius_y: ShapeRadius::Length(Px(30.0)),
        position: ShapePosition(SpacePair {
          x: Length::Percentage(25.0),
          y: Length::Percentage(75.0),
        }),
      }))
    );
  }

  #[test]
  fn test_parse_polygon_triangle() {
    assert!(matches!(
      BasicShape::from_str("polygon(50% 0%, 0% 100%, 100% 100%)"),
      Ok(BasicShape::Polygon(PolygonShape {
        fill_rule: None,
        coordinates: coords,
      })) if coords.len() == 3 &&
            coords[0] == SpacePair { x: Length::Percentage(50.0), y: Length::Percentage(0.0) } &&
            coords[1] == SpacePair { x: Length::Percentage(0.0), y: Length::Percentage(100.0) } &&
            coords[2] == SpacePair { x: Length::Percentage(100.0), y: Length::Percentage(100.0) }
    ));
  }

  #[test]
  fn test_parse_polygon_with_fill_rule() {
    assert!(matches!(
      BasicShape::from_str("polygon(evenodd, 50% 0%, 0% 100%, 100% 100%)"),
      Ok(BasicShape::Polygon(PolygonShape {
        fill_rule: Some(FillRule::EvenOdd),
        coordinates: coords,
      })) if coords.len() == 3
    ));
  }

  #[test]
  fn test_parse_path() {
    assert_eq!(
      BasicShape::from_str("path('M 10 10 L 90 90')"),
      Ok(BasicShape::Path(PathShape {
        fill_rule: None,
        path: "M 10 10 L 90 90".to_string(),
      }))
    );
  }

  #[test]
  fn test_parse_path_with_fill_rule() {
    assert_eq!(
      BasicShape::from_str("path(evenodd, 'M 10 10 L 90 90')"),
      Ok(BasicShape::Path(PathShape {
        fill_rule: Some(FillRule::EvenOdd),
        path: "M 10 10 L 90 90".to_string(),
      }))
    );
  }

  #[test]
  fn test_parse_circle_percentage_radius() {
    assert_eq!(
      BasicShape::from_str("circle(50%)"),
      Ok(BasicShape::Ellipse(EllipseShape {
        radius_x: ShapeRadius::Length(Length::Percentage(50.0)),
        radius_y: ShapeRadius::Length(Length::Percentage(50.0)),
        position: ShapePosition::default(),
      }))
    );
  }

  #[test]
  fn test_parse_circle_closest_side() {
    assert_eq!(
      BasicShape::from_str("circle(closest-side)"),
      Ok(BasicShape::Ellipse(EllipseShape {
        radius_x: ShapeRadius::ClosestSide,
        radius_y: ShapeRadius::ClosestSide,
        position: ShapePosition::default(),
      }))
    );
  }

  #[test]
  fn test_parse_circle_farthest_side() {
    assert_eq!(
      BasicShape::from_str("circle(farthest-side)"),
      Ok(BasicShape::Ellipse(EllipseShape {
        radius_x: ShapeRadius::FarthestSide,
        radius_y: ShapeRadius::FarthestSide,
        position: ShapePosition::default(),
      }))
    );
  }
}
