//! `clip-path` basic shapes resolved to path commands, shared by the backends.

use std::f32::consts::SQRT_2;

use crate::{
  context::RenderContext,
  geometry::{PathBuilder, PathCommand, Point, Rect, Size},
  layout::border::BorderProperties,
  painter::FillShape,
  style::{
    Axis, BasicShape, BorderStyle, CircleShape, Color, EllipseShape, FillRule,
    ImageScalingAlgorithm, ShapeRadius, Sides, SizingContext, SpacePair,
  },
};

/// Control-point ratio that turns four cubics into a circle.
const KAPPA: f32 = 0.552_284_8;

impl BasicShape {
  /// Resolves the shape against a border box, in the box's own coordinates.
  ///
  /// The commands are unclosed for `path()`, which carries its own closes, and
  /// closed for the shapes that describe a region.
  /// Returns `None` when the shape cannot be resolved at all, which today means
  /// a `path()` in a build without the `svg` feature and its path parser. That is
  /// different from a shape that resolves to no area: callers must not turn it
  /// into an empty clip, which would hide the element.
  pub(crate) fn path_commands(
    &self,
    context: &RenderContext,
    size: Size<f32>,
  ) -> Option<Vec<PathCommand>> {
    let mut commands = Vec::new();

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
          width: Rect::ZERO,
          color: Sides::from(Color::transparent()).into(),
          // A corner's horizontal radius resolves against the box width and its
          // vertical one against the height, like `border-radius`.
          radius: shape
            .border_radius
            .map(|radius| {
              Sides(radius.0.map(|corner| SpacePair {
                x: corner.to_px(&context.sizing, size.width),
                y: corner.to_px(&context.sizing, size.height),
              }))
            })
            .unwrap_or_default(),
          image_rendering: ImageScalingAlgorithm::Auto,
          style: Sides::from(BorderStyle::Solid).into(),
          shape: Sides::default(),
          collapsed: false,
        };

        border.append_mask_commands(
          &mut commands,
          Size {
            width: size.width - inset.horizontal(),
            height: size.height - inset.vertical(),
          },
          inset.top_left(),
        );
      }
      BasicShape::Circle(shape) => {
        let (center, radius) = shape.resolve(&context.sizing, size);

        push_ellipse(&mut commands, center, SpacePair::from_single(radius));
      }
      BasicShape::Ellipse(shape) => {
        let (center, radius) = shape.resolve(&context.sizing, size);

        push_ellipse(&mut commands, center, radius);
      }
      BasicShape::Polygon(shape) => {
        let Some((first, rest)) = shape.coordinates.split_first() else {
          return Some(commands);
        };

        commands.move_to((
          first.x.to_px(&context.sizing, size.width),
          first.y.to_px(&context.sizing, size.height),
        ));
        for coordinate in rest {
          commands.line_to((
            coordinate.x.to_px(&context.sizing, size.width),
            coordinate.y.to_px(&context.sizing, size.height),
          ));
        }
        commands.close();
      }
      BasicShape::Path(shape) => {
        // path() coordinates are CSS px; scale them like the to_px shapes.
        let scale = context.sizing.to_device(1.0);

        commands.extend(scale_commands(parse_path(shape.path.as_ref())?, scale));
      }
    }
    Some(commands)
  }
}

impl BasicShape {
  /// The shape resolved against a border box as a fill, its rule from `clip_rule` unless the shape
  /// sets its own, or `None` when it cannot resolve at all.
  pub fn fill_shape(
    &self,
    context: &RenderContext,
    size: Size<f32>,
    clip_rule: FillRule,
  ) -> Option<FillShape> {
    match self {
      BasicShape::Circle(shape) => {
        let (center, radius) = shape.resolve(&context.sizing, size);

        return Some(FillShape::Ellipse {
          center,
          radius: SpacePair::from_single(radius),
        });
      }
      BasicShape::Ellipse(shape) => {
        let (center, radius) = shape.resolve(&context.sizing, size);

        return Some(FillShape::Ellipse { center, radius });
      }
      _ => {}
    }

    Some(FillShape::Path {
      commands: self.path_commands(context, size)?,
      rule: self.fill_rule().unwrap_or(clip_rule),
    })
  }
}

impl CircleShape {
  /// The circle's centre and radius in a reference box of `size`, after
  /// [CSS Shapes](https://drafts.csswg.org/css-shapes-1/#funcdef-basic-shape-circle): a side
  /// keyword measures to the nearest or farthest of all four sides, and a percentage resolves
  /// against the box's diagonal over √2.
  pub(crate) fn resolve(&self, sizing: &SizingContext, size: Size<f32>) -> (Point<f32>, f32) {
    let center = self.position.to_point(sizing, size);
    let [left, right] = side_distances(center.x, size.width);
    let [top, bottom] = side_distances(center.y, size.height);
    let radius = match self.radius {
      ShapeRadius::ClosestSide => left.min(right).min(top).min(bottom),
      ShapeRadius::FarthestSide => left.max(right).max(top).max(bottom),
      ShapeRadius::Length(length) => length.to_px(sizing, size.width.hypot(size.height) / SQRT_2),
    };

    (center, radius)
  }
}

impl EllipseShape {
  /// The ellipse's centre and radii in a reference box of `size`, each radius measured along
  /// its own axis.
  pub(crate) fn resolve(
    &self,
    sizing: &SizingContext,
    size: Size<f32>,
  ) -> (Point<f32>, SpacePair<f32>) {
    let center = self.position.to_point(sizing, size);

    (
      center,
      SpacePair {
        x: resolve_radius(self.radius_x, center.x, sizing, size.width),
        y: resolve_radius(self.radius_y, center.y, sizing, size.height),
      },
    )
  }
}

/// Appends an axis-aligned ellipse outline as four cubics.
pub(crate) fn push_ellipse(
  commands: &mut Vec<PathCommand>,
  center: Point<f32>,
  radius: SpacePair<f32>,
) {
  let SpacePair {
    x: radius_x,
    y: radius_y,
  } = radius;

  if radius_x <= 0.0 || radius_y <= 0.0 {
    return;
  }
  let (cx, cy) = (center.x, center.y);
  let (ox, oy) = (radius_x * KAPPA, radius_y * KAPPA);

  commands.move_to((cx + radius_x, cy));
  commands.curve_to(
    (cx + radius_x, cy + oy),
    (cx + ox, cy + radius_y),
    (cx, cy + radius_y),
  );
  commands.curve_to(
    (cx - ox, cy + radius_y),
    (cx - radius_x, cy + oy),
    (cx - radius_x, cy),
  );
  commands.curve_to(
    (cx - radius_x, cy - oy),
    (cx - ox, cy - radius_y),
    (cx, cy - radius_y),
  );
  commands.curve_to(
    (cx + ox, cy - radius_y),
    (cx + radius_x, cy - oy),
    (cx + radius_x, cy),
  );
  commands.close();
}

/// The keyword radii measure to the sides on the shape's own axis, so both
/// distances come from the same edge pair: the center's coordinate and what is
/// left of the box beyond it.
/// One radius of an ellipse centred at `center` on an axis `full` long.
fn resolve_radius(radius: ShapeRadius, center: f32, sizing: &SizingContext, full: f32) -> f32 {
  let [near, far] = side_distances(center, full);

  match radius {
    ShapeRadius::ClosestSide => near.min(far),
    ShapeRadius::FarthestSide => near.max(far),
    ShapeRadius::Length(length) => length.to_px(sizing, full),
  }
}

/// How far `center` sits from each end of an axis `full` long.
fn side_distances(center: f32, full: f32) -> [f32; 2] {
  [center.abs(), (full - center).abs()]
}

fn scale_commands(commands: Vec<PathCommand>, scale: f32) -> Vec<PathCommand> {
  let point = |point: Point<f32>| Point::new(point.x * scale, point.y * scale);

  commands
    .into_iter()
    .map(|command| command.map_points(point))
    .collect()
}

#[cfg(feature = "svg")]
fn parse_path(input: &str) -> Option<Vec<PathCommand>> {
  use svgtypes::{SimplePathSegment, SimplifyingPathParser};

  let mut commands = Vec::new();

  for segment in SimplifyingPathParser::from(input) {
    let Ok(segment) = segment else {
      return Some(Vec::new());
    };

    match segment {
      SimplePathSegment::MoveTo { x, y } => commands.move_to((x as f32, y as f32)),
      SimplePathSegment::LineTo { x, y } => commands.line_to((x as f32, y as f32)),
      SimplePathSegment::CurveTo {
        x1,
        y1,
        x2,
        y2,
        x,
        y,
      } => commands.curve_to(
        (x1 as f32, y1 as f32),
        (x2 as f32, y2 as f32),
        (x as f32, y as f32),
      ),
      SimplePathSegment::Quadratic { x1, y1, x, y } => commands.push(PathCommand::QuadTo(
        Point::new(x1 as f32, y1 as f32),
        Point::new(x as f32, y as f32),
      )),
      SimplePathSegment::ClosePath => commands.close(),
    }
  }
  Some(commands)
}

/// Without the path parser a `path()` shape cannot be resolved at all.
#[cfg(not(feature = "svg"))]
fn parse_path(_input: &str) -> Option<Vec<PathCommand>> {
  None
}

#[cfg(test)]
mod tests {
  use super::resolve_radius;
  use crate::{
    style::{Length, ShapeRadius, SizingContext},
    viewport::Viewport,
  };

  #[test]
  fn keyword_radii_measure_along_their_own_axis() {
    let sizing = SizingContext::builder()
      .viewport(Viewport::new((110, 110)))
      .build();

    // A 110px axis with the center at 10px: the near side is 10 away, the far
    // side 100.
    assert_eq!(
      resolve_radius(ShapeRadius::ClosestSide, 10.0, &sizing, 110.0),
      10.0
    );
    assert_eq!(
      resolve_radius(ShapeRadius::FarthestSide, 10.0, &sizing, 110.0),
      100.0
    );
    assert_eq!(
      resolve_radius(ShapeRadius::Length(Length::Px(25.0)), 10.0, &sizing, 110.0),
      25.0
    );
  }
}
