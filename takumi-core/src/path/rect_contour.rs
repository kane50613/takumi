//! `SkPathPriv::IsRectContour` (`src/core/SkPathPriv.cpp`), as `SkPath::isRect` calls it: the rect a
//! path of lines traces.

use super::{Path, PathVerb};
use crate::geometry::{Point, Rect};

/// The rect a path traces, whether it closes, and which way it turns.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RectContour {
  /// The rect, sorted.
  pub rect: Rect<f32>,
  /// Whether the path ends with a close.
  pub closed: bool,
  /// Whether it turns clockwise in a y-down space.
  pub clockwise: bool,
}

impl Path {
  /// `SkPath::isRect`: the rect the path traces through collinear and repeated points and an
  /// implicit close, none unless it is one rect of lines.
  pub fn rect_contour(&self) -> Option<RectContour> {
    let (verbs, points) = (self.verbs(), self.points());

    if points.len() < 4
      || verbs.len() < 4
      || !verbs.contains(&PathVerb::Line)
      || verbs
        .iter()
        .any(|verb| !matches!(verb, PathVerb::Move | PathVerb::Line | PathVerb::Close))
    {
      return None;
    }

    self.trivial_rect().or_else(|| self.traced_rect())
  }

  /// `trivial_rect`: the rect of a move, three lines, and a close.
  fn trivial_rect(&self) -> Option<RectContour> {
    let (verbs, points) = (self.verbs(), self.points());

    if verbs
      != [
        PathVerb::Move,
        PathVerb::Line,
        PathVerb::Line,
        PathVerb::Line,
        PathVerb::Close,
      ]
      || points.len() != 4
    {
      return None;
    }

    let sides = [
      points[1] - points[0],
      points[2] - points[1],
      points[3] - points[2],
      points[0] - points[3],
    ];
    let axis_aligned = |v: Point<f32>| (v.x == 0.0) ^ (v.y == 0.0);
    let orthogonal =
      |a: Point<f32>, b: Point<f32>| ((a.x == 0.0) ^ (b.x == 0.0)) & ((a.y == 0.0) ^ (b.y == 0.0));

    if !(axis_aligned(sides[0])
      & orthogonal(sides[0], sides[1])
      & orthogonal(sides[1], sides[2])
      & orthogonal(sides[2], sides[3]))
    {
      return None;
    }

    Some(RectContour {
      rect: Rect::from_points(&[points[0], points[2]])?,
      closed: true,
      clockwise: sides[0].x * sides[1].y - sides[0].y * sides[1].x > 0.0,
    })
  }

  /// The general walk of `IsRectContour`, through every side of the contour.
  fn traced_rect(&self) -> Option<RectContour> {
    // `rect_make_dir`: bit 0 for a horizontal side, bit 1 for one pointing right or down.
    let direction = |delta: Point<f32>| {
      i8::from(delta.x != 0.0) | (i8::from(delta.x > 0.0 || delta.y > 0.0) << 1)
    };
    let diagonal = |delta: Point<f32>| delta.x != 0.0 && delta.y != 0.0;
    let mut points = self.points().iter().copied();
    let mut corners = 0;
    let mut line_start = Point::ZERO;
    let mut first = None;
    let mut last = None;
    let mut first_corner = Point::ZERO;
    let mut third_corner = Point::ZERO;
    let mut directions = [-1_i8; 5];
    let mut closed_or_moved = false;
    let mut auto_close = false;

    for &verb in self.verbs() {
      if verb == PathVerb::Move {
        let point = points.next()?;

        if corners == 0 {
          first = Some(point);
        } else if diagonal(first? - last?) {
          return None;
        }

        line_start = point;
        closed_or_moved = true;
        continue;
      }

      let line_end = if verb == PathVerb::Close {
        auto_close = true;
        first?
      } else {
        let point = points.next()?;

        last = Some(point);
        point
      };
      let delta = line_end - line_start;

      if diagonal(delta) || !delta.is_finite() {
        return None;
      }

      if line_start == line_end {
        continue;
      }

      let next = direction(delta);

      if corners == 0 {
        directions[0] = next;
        corners = 1;
        closed_or_moved = false;
        line_start = line_end;
        continue;
      }

      if closed_or_moved {
        return None;
      }

      if auto_close && next == directions[0] {
        continue;
      }

      closed_or_moved = auto_close;

      if directions[corners - 1] == next {
        if corners == 3 && verb == PathVerb::Line {
          third_corner = line_end;
        }

        line_start = line_end;
        continue;
      }

      directions[corners] = next;
      corners += 1;

      // Opposite sides point opposite ways, which flips only bit 1.
      match corners {
        2 => first_corner = line_start,
        3 if directions[0] ^ directions[2] == 2 => third_corner = line_end,
        4 if directions[1] ^ directions[3] == 2 => {}
        _ => return None,
      }

      line_start = line_end;
    }

    if !(3..=4).contains(&corners) || diagonal(first? - last?) {
      return None;
    }

    Some(RectContour {
      rect: Rect::from_points(&[first_corner, third_corner])?,
      closed: auto_close,
      clockwise: directions[0] == ((directions[1] + 1) & 3),
    })
  }
}
