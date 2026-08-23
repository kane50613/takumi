//! `clip-path` basic shapes resolved to path commands, shared by the backends.

use crate::{
  context::RenderContext,
  geometry::{PathBuilder, PathCommand, Point, Rect, Size},
  layout::border::BorderProperties,
  style::{
    Axis, BasicShape, BorderStyle, Color, ImageScalingAlgorithm, ShapeRadius, Sides, SizingContext,
    SpacePair,
  },
};

/// Control-point ratio that turns four cubics into a circle.
const KAPPA: f32 = 0.552_284_8;

/// Resolves a shape against a border box, in the box's own coordinates.
///
/// The commands are unclosed for `path()`, which carries its own closes, and
/// closed for the shapes that describe a region.
/// Returns `None` when the shape cannot be resolved at all, which today means
/// a `path()` in a build without the `svg` feature and its path parser. That is
/// different from a shape that resolves to no area: callers must not turn it
/// into an empty clip, which would hide the element.
pub fn clip_shape_commands(
  shape: &BasicShape,
  context: &RenderContext,
  size: Size<f32>,
) -> Option<Vec<PathCommand>> {
  let mut commands = Vec::new();

  match shape {
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
        color: Rect {
          top: Color::transparent(),
          right: Color::transparent(),
          bottom: Color::transparent(),
          left: Color::transparent(),
        },
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
        style: Rect {
          top: BorderStyle::Solid,
          right: BorderStyle::Solid,
          bottom: BorderStyle::Solid,
          left: BorderStyle::Solid,
        },
        shape: Sides::default(),
        collapsed: false,
      };

      border.append_mask_commands(
        &mut commands,
        Size {
          width: size.width - inset.horizontal(),
          height: size.height - inset.vertical(),
        },
        Point {
          x: inset.left,
          y: inset.top,
        },
      );
    }
    BasicShape::Ellipse(shape) => {
      let center = (
        shape.position.0.x.to_px(&context.sizing, size.width),
        shape.position.0.y.to_px(&context.sizing, size.height),
      );

      push_ellipse(
        &mut commands,
        center,
        resolve_radius(shape.radius_x, center.0, &context.sizing, size.width),
        resolve_radius(shape.radius_y, center.1, &context.sizing, size.height),
      );
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

/// Appends an axis-aligned ellipse outline as four cubics.
fn push_ellipse(commands: &mut Vec<PathCommand>, center: (f32, f32), radius_x: f32, radius_y: f32) {
  if radius_x <= 0.0 || radius_y <= 0.0 {
    return;
  }
  let (cx, cy) = center;
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
fn resolve_radius(radius: ShapeRadius, center: f32, sizing: &SizingContext, full: f32) -> f32 {
  let (near, far) = (center, full - center);

  match radius {
    ShapeRadius::ClosestSide => near.min(far),
    ShapeRadius::FarthestSide => near.max(far),
    ShapeRadius::Length(length) => length.to_px(sizing, full),
  }
}

fn scale_commands(commands: Vec<PathCommand>, scale: f32) -> Vec<PathCommand> {
  let point = |point: Point<f32>| Point::new(point.x * scale, point.y * scale);

  commands
    .into_iter()
    .map(|command| match command {
      PathCommand::MoveTo(a) => PathCommand::MoveTo(point(a)),
      PathCommand::LineTo(a) => PathCommand::LineTo(point(a)),
      PathCommand::QuadTo(a, b) => PathCommand::QuadTo(point(a), point(b)),
      PathCommand::CubicTo(a, b, c) => PathCommand::CubicTo(point(a), point(b), point(c)),
      PathCommand::Close => PathCommand::Close,
    })
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
