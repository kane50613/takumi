use cssparser::{Parser, Token, match_ignore_ascii_case};
use serde::{Deserialize, Serialize};
use taffy::{AbsoluteAxis, Point, Rect, Size};
use ts_rs::TS;
use zeno::{Fill, Mask, PathBuilder, PathData, Placement};

use crate::{
  layout::style::{Affine, Axis, Color, FromCss, LengthUnit, ParseResult, Sides, SpacePair},
  rendering::{BorderProperties, RenderContext},
};

/// Represents the fill rule used for determining the interior of shapes.
///
/// Corresponds to the SVG fill-rule attribute and is used in polygon(), path(), and shape() functions.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
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
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum ShapeRadius {
  /// Uses the length from the center to the closest side of the reference box
  #[default]
  ClosestSide,
  /// Uses the length from the center to the farthest side of the reference box
  FarthestSide,
  /// A specific length value
  #[serde(untagged)]
  Length(LengthUnit),
}

/// Represents a position for circle() and ellipse() functions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
pub struct ShapePosition(pub SpacePair<LengthUnit>);

impl Default for ShapePosition {
  fn default() -> Self {
    Self(SpacePair::from_single(LengthUnit::Percentage(50.0)))
  }
}

/// Represents an inset() rectangle shape.
///
/// The inset() function creates an inset rectangle, with its size defined by the offset distance
/// of each of the four sides of its container and, optionally, rounded corners.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InsetShape {
  /// Sides of the inset.
  pub inset: Sides<LengthUnit>,
  /// Optional border radius for rounded corners
  pub border_radius: Option<Sides<LengthUnit>>,
}

/// Represents a circle() shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CircleShape {
  /// The radius of the circle
  pub radius: ShapeRadius,
  /// The center position of the circle
  pub position: ShapePosition,
}

/// Represents an ellipse() shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EllipseShape {
  /// The horizontal radius
  pub radius_x: ShapeRadius,
  /// The vertical radius
  pub radius_y: ShapeRadius,
  /// The center position of the ellipse
  pub position: ShapePosition,
}

/// Represents a single coordinate pair in a polygon.
pub type PolygonCoordinate = SpacePair<LengthUnit>;

/// Represents a polygon() shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PolygonShape {
  /// The fill rule to use
  pub fill_rule: Option<FillRule>,
  /// List of coordinate pairs defining the polygon vertices
  pub coordinates: Vec<PolygonCoordinate>,
}

/// Represents a path() shape using an SVG path string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PathShape {
  /// The fill rule to use
  pub fill_rule: Option<FillRule>,
  /// SVG path data string
  pub path: String,
}

/// Represents a basic shape function for clip-path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(as = "BasicShapeValue")]
#[serde(try_from = "BasicShapeValue")]
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

/// Proxy type for CSS deserialization that accepts either structured data or CSS strings.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum BasicShapeValue {
  /// inset() function
  Inset(InsetShape),
  /// circle() function
  Circle(CircleShape),
  /// ellipse() function
  Ellipse(EllipseShape),
  /// polygon() function
  Polygon(PolygonShape),
  /// path() function
  Path(PathShape),
  #[serde(untagged)]
  /// Raw CSS string to be parsed
  Css(String),
}

impl TryFrom<BasicShapeValue> for BasicShape {
  type Error = String;

  fn try_from(value: BasicShapeValue) -> Result<Self, Self::Error> {
    match value {
      BasicShapeValue::Inset(shape) => Ok(BasicShape::Inset(shape)),
      BasicShapeValue::Circle(shape) => Ok(BasicShape::Ellipse(EllipseShape {
        radius_x: shape.radius,
        radius_y: shape.radius,
        position: shape.position,
      })),
      BasicShapeValue::Ellipse(shape) => Ok(BasicShape::Ellipse(shape)),
      BasicShapeValue::Polygon(shape) => Ok(BasicShape::Polygon(shape)),
      BasicShapeValue::Path(shape) => Ok(BasicShape::Path(shape)),
      BasicShapeValue::Css(css) => BasicShape::from_str(&css).map_err(|e| e.to_string()),
    }
  }
}

fn resolve_radius(
  radius: ShapeRadius,
  distance: Size<f32>,
  context: &RenderContext,
  full: f32,
) -> f32 {
  match radius {
    ShapeRadius::ClosestSide => distance.width.min(distance.height),
    ShapeRadius::FarthestSide => distance.width.max(distance.height),
    ShapeRadius::Length(length) => length.resolve_to_px(context, full),
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

  pub(crate) fn render_mask(
    &self,
    context: &RenderContext,
    size: Size<f32>,
  ) -> (Vec<u8>, Placement) {
    let mut paths = Vec::new();

    match self {
      BasicShape::Inset(shape) => {
        let inset: Rect<f32> = shape
          .inset
          .map_axis(|value, axis| {
            value.resolve_to_px(
              context,
              match axis {
                Axis::Horizontal => size.width,
                Axis::Vertical => size.height,
              },
            )
          })
          .into();

        let border = BorderProperties {
          width: Rect::zero(),
          offset: Point {
            x: inset.left,
            y: inset.top,
          },
          size: Size {
            width: size.width - inset.grid_axis_sum(AbsoluteAxis::Horizontal),
            height: size.height - inset.grid_axis_sum(AbsoluteAxis::Vertical),
          },
          color: Color::transparent(),
          radius: shape
            .border_radius
            .map(|radius| {
              Sides(
                radius
                  .0
                  .map(|corner| corner.resolve_to_px(context, size.width)),
              )
            })
            .unwrap_or_default(),
          transform: Affine::identity(),
        };

        border.append_mask_commands(&mut paths);
      }
      BasicShape::Ellipse(shape) => {
        let distance = Size {
          width: shape.position.0.x.resolve_to_px(context, size.width),
          height: shape.position.0.y.resolve_to_px(context, size.height),
        };

        paths.add_ellipse(
          (distance.width, distance.height),
          resolve_radius(shape.radius_x, distance, context, size.width),
          resolve_radius(shape.radius_y, distance, context, size.height),
        );
      }
      BasicShape::Polygon(shape) => {
        if !shape.coordinates.is_empty() {
          // Start the path at the first coordinate
          let first = &shape.coordinates[0];
          let first_x = first.x.resolve_to_px(context, size.width);
          let first_y = first.y.resolve_to_px(context, size.height);

          paths.move_to((first_x, first_y));

          // Add lines to each subsequent coordinate
          for coord in &shape.coordinates[1..] {
            let x = coord.x.resolve_to_px(context, size.width);
            let y = coord.y.resolve_to_px(context, size.height);
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

    context.transform.apply_on_paths(&mut paths);

    Mask::new(&paths)
      .style(Fill::from(
        self.fill_rule().unwrap_or(context.style.clip_rule),
      ))
      .render()
  }
}

impl<'i> FromCss<'i> for FillRule {
  fn from_css(parser: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = parser.current_source_location();
    let ident = parser.expect_ident()?;

    match_ignore_ascii_case! { &ident,
      "nonzero" => Ok(FillRule::NonZero),
      "evenodd" => Ok(FillRule::EvenOdd),
      _ => Err(location.new_basic_unexpected_token_error(Token::Ident(ident.clone())).into())
    }
  }
}

impl<'i> FromCss<'i> for ShapeRadius {
  fn from_css(parser: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = parser.current_source_location();

    // Try parsing as length first
    if let Ok(length) = parser.try_parse(LengthUnit::from_css) {
      return Ok(ShapeRadius::Length(length));
    }

    // Try parsing keywords
    let ident = parser.expect_ident()?;
    match_ignore_ascii_case! { &ident,
      "closest-side" => Ok(ShapeRadius::ClosestSide),
      "farthest-side" => Ok(ShapeRadius::FarthestSide),
      _ => Err(location.new_basic_unexpected_token_error(Token::Ident(ident.clone())).into())
    }
  }
}

impl<'i> FromCss<'i> for ShapePosition {
  fn from_css(parser: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let first = LengthUnit::from_css(parser)?;

    // If there's a second value, parse it; otherwise default to 50%
    let second = parser
      .try_parse(LengthUnit::from_css)
      .unwrap_or(LengthUnit::Percentage(50.0));

    Ok(ShapePosition(SpacePair::from_pair(first, second)))
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
          _ => Err(location.new_basic_unexpected_token_error(token.clone()).into())
        }
      }
      _ => Err(
        location
          .new_basic_unexpected_token_error(token.clone())
          .into(),
      ),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use LengthUnit::*;

  #[test]
  fn test_parse_inset_simple() {
    let result = BasicShape::from_str("inset(10px)").unwrap();
    if let BasicShape::Inset(inset) = result {
      assert_eq!(inset.inset.0, [Px(10.0); 4]);
    } else {
      panic!("Expected inset shape");
    }
  }

  #[test]
  fn test_parse_inset_four_values() {
    let result = BasicShape::from_str("inset(10px 20px 30px 40px)").unwrap();
    if let BasicShape::Inset(inset) = result {
      assert_eq!(inset.inset.0[0], LengthUnit::Px(10.0));
      assert_eq!(inset.inset.0[1], LengthUnit::Px(20.0));
      assert_eq!(inset.inset.0[2], LengthUnit::Px(30.0));
      assert_eq!(inset.inset.0[3], LengthUnit::Px(40.0));
      assert_eq!(inset.border_radius, None);
    } else {
      panic!("Expected inset shape");
    }
  }

  #[test]
  fn test_parse_inset_with_border_radius() {
    let result = BasicShape::from_str("inset(10px round 5px)").unwrap();
    if let BasicShape::Inset(inset) = result {
      assert_eq!(inset.inset.0[0], LengthUnit::Px(10.0));
      assert_eq!(inset.inset.0[1], LengthUnit::Px(10.0));
      assert_eq!(inset.inset.0[2], LengthUnit::Px(10.0));
      assert_eq!(inset.inset.0[3], LengthUnit::Px(10.0));

      let border_radius = inset.border_radius.expect("Should have border radius");
      assert_eq!(border_radius.0[0], LengthUnit::Px(5.0)); // top-left
      assert_eq!(border_radius.0[1], LengthUnit::Px(5.0)); // top-right
      assert_eq!(border_radius.0[2], LengthUnit::Px(5.0)); // bottom-right
      assert_eq!(border_radius.0[3], LengthUnit::Px(5.0)); // bottom-left
    } else {
      panic!("Expected inset shape");
    }
  }

  #[test]
  fn test_parse_inset_with_complex_border_radius() {
    let result =
      BasicShape::from_str("inset(10px 20px 30px 40px round 5px 10px 15px 20px)").unwrap();
    if let BasicShape::Inset(inset) = result {
      assert_eq!(inset.inset.0[0], LengthUnit::Px(10.0));
      assert_eq!(inset.inset.0[1], LengthUnit::Px(20.0));
      assert_eq!(inset.inset.0[2], LengthUnit::Px(30.0));
      assert_eq!(inset.inset.0[3], LengthUnit::Px(40.0));

      let border_radius = inset.border_radius.expect("Should have border radius");
      assert_eq!(border_radius.0[0], LengthUnit::Px(5.0)); // top-left
      assert_eq!(border_radius.0[1], LengthUnit::Px(10.0)); // top-right
      assert_eq!(border_radius.0[2], LengthUnit::Px(15.0)); // bottom-right
      assert_eq!(border_radius.0[3], LengthUnit::Px(20.0)); // bottom-left
    } else {
      panic!("Expected inset shape");
    }
  }

  #[test]
  fn test_parse_circle_simple() {
    let result = BasicShape::from_str("circle(50px)").unwrap();
    if let BasicShape::Ellipse(circle) = result {
      assert_eq!(circle.radius_x, circle.radius_y);
      assert_eq!(circle.radius_x, ShapeRadius::Length(LengthUnit::Px(50.0)));
      assert_eq!(circle.position.0.x, LengthUnit::Percentage(50.0));
      assert_eq!(circle.position.0.y, LengthUnit::Percentage(50.0));
    } else {
      panic!("Expected circle shape");
    }
  }

  #[test]
  fn test_parse_circle_with_position() {
    let result = BasicShape::from_str("circle(50px at 25% 75%)").unwrap();
    if let BasicShape::Ellipse(circle) = result {
      assert_eq!(circle.radius_x, circle.radius_y);
      assert_eq!(circle.radius_x, ShapeRadius::Length(LengthUnit::Px(50.0)));
      assert_eq!(circle.position.0.x, LengthUnit::Percentage(25.0));
      assert_eq!(circle.position.0.y, LengthUnit::Percentage(75.0));
    } else {
      panic!("Expected circle shape");
    }
  }

  #[test]
  fn test_parse_circle_default_radius() {
    let result = BasicShape::from_str("circle(at 25% 75%)").unwrap();
    if let BasicShape::Ellipse(circle) = result {
      assert_eq!(circle.radius_x, circle.radius_y);
      assert_eq!(circle.radius_x, ShapeRadius::ClosestSide);
      assert_eq!(circle.position.0.x, LengthUnit::Percentage(25.0));
      assert_eq!(circle.position.0.y, LengthUnit::Percentage(75.0));
    } else {
      panic!("Expected circle shape");
    }
  }

  #[test]
  fn test_parse_ellipse_simple() {
    let result = BasicShape::from_str("ellipse(50px 30px)").unwrap();
    if let BasicShape::Ellipse(ellipse) = result {
      assert_eq!(ellipse.radius_x, ShapeRadius::Length(LengthUnit::Px(50.0)));
      assert_eq!(ellipse.radius_y, ShapeRadius::Length(LengthUnit::Px(30.0)));
      assert_eq!(ellipse.position.0.x, LengthUnit::Percentage(50.0));
      assert_eq!(ellipse.position.0.y, LengthUnit::Percentage(50.0));
    } else {
      panic!("Expected ellipse shape");
    }
  }

  #[test]
  fn test_parse_ellipse_with_position() {
    let result = BasicShape::from_str("ellipse(50px 30px at 25% 75%)").unwrap();
    if let BasicShape::Ellipse(ellipse) = result {
      assert_eq!(ellipse.radius_x, ShapeRadius::Length(LengthUnit::Px(50.0)));
      assert_eq!(ellipse.radius_y, ShapeRadius::Length(LengthUnit::Px(30.0)));
      assert_eq!(ellipse.position.0.x, LengthUnit::Percentage(25.0));
      assert_eq!(ellipse.position.0.y, LengthUnit::Percentage(75.0));
    } else {
      panic!("Expected ellipse shape");
    }
  }

  #[test]
  fn test_parse_polygon_triangle() {
    let result = BasicShape::from_str("polygon(50% 0%, 0% 100%, 100% 100%)").unwrap();
    if let BasicShape::Polygon(polygon) = result {
      assert_eq!(polygon.fill_rule, None);
      assert_eq!(polygon.coordinates.len(), 3);

      assert_eq!(polygon.coordinates[0].x, LengthUnit::Percentage(50.0));
      assert_eq!(polygon.coordinates[0].y, LengthUnit::Percentage(0.0));
      assert_eq!(polygon.coordinates[1].x, LengthUnit::Percentage(0.0));
      assert_eq!(polygon.coordinates[1].y, LengthUnit::Percentage(100.0));
      assert_eq!(polygon.coordinates[2].x, LengthUnit::Percentage(100.0));
      assert_eq!(polygon.coordinates[2].y, LengthUnit::Percentage(100.0));
    } else {
      panic!("Expected polygon shape");
    }
  }

  #[test]
  fn test_parse_polygon_with_fill_rule() {
    let result = BasicShape::from_str("polygon(evenodd, 50% 0%, 0% 100%, 100% 100%)").unwrap();
    if let BasicShape::Polygon(polygon) = result {
      assert_eq!(polygon.fill_rule, Some(FillRule::EvenOdd));
      assert_eq!(polygon.coordinates.len(), 3);
    } else {
      panic!("Expected polygon shape");
    }
  }

  #[test]
  fn test_parse_path() {
    let result = BasicShape::from_str("path('M 10 10 L 90 90')").unwrap();
    if let BasicShape::Path(path) = result {
      assert_eq!(path.fill_rule, None);
      assert_eq!(path.path, "M 10 10 L 90 90");
    } else {
      panic!("Expected path shape");
    }
  }

  #[test]
  fn test_parse_path_with_fill_rule() {
    let result = BasicShape::from_str("path(evenodd, 'M 10 10 L 90 90')").unwrap();
    if let BasicShape::Path(path) = result {
      assert_eq!(path.fill_rule, Some(FillRule::EvenOdd));
      assert_eq!(path.path, "M 10 10 L 90 90");
    } else {
      panic!("Expected path shape");
    }
  }

  #[test]
  fn test_parse_circle_percentage_radius() {
    let result = BasicShape::from_str("circle(50%)").unwrap();
    if let BasicShape::Ellipse(circle) = result {
      assert_eq!(
        circle.radius_x,
        ShapeRadius::Length(LengthUnit::Percentage(50.0))
      );
    } else {
      panic!("Expected circle shape");
    }
  }

  #[test]
  fn test_parse_circle_closest_side() {
    let result = BasicShape::from_str("circle(closest-side)").unwrap();
    if let BasicShape::Ellipse(circle) = result {
      assert_eq!(circle.radius_x, ShapeRadius::ClosestSide);
    } else {
      panic!("Expected circle shape");
    }
  }

  #[test]
  fn test_parse_circle_farthest_side() {
    let result = BasicShape::from_str("circle(farthest-side)").unwrap();
    if let BasicShape::Ellipse(circle) = result {
      assert_eq!(circle.radius_x, ShapeRadius::FarthestSide);
    } else {
      panic!("Expected circle shape");
    }
  }
}
