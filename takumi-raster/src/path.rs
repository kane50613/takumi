use svgtypes::{SimplePathSegment, SimplifyingPathParser};
use takumi_core::geometry::{PathCommand, Point};
use tiny_skia::{
  FillRule as TinyFillRule, LineCap as TinyLineCap, LineJoin as TinyLineJoin, Path as TinyPath,
  PathBuilder as TinyPathBuilder, PathSegment as TinyPathSegment, Rect as TinyRect,
  Stroke as TinyStroke, StrokeDash as TinyStrokeDash,
};

use crate::style::LineJoin;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum Fill {
  #[default]
  NonZero,
  EvenOdd,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum Join {
  #[default]
  Miter,
  Round,
  Bevel,
}

impl From<LineJoin> for Join {
  fn from(value: LineJoin) -> Self {
    match value {
      LineJoin::Miter => Join::Miter,
      LineJoin::Round => Join::Round,
      LineJoin::Bevel => Join::Bevel,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum Cap {
  #[default]
  Butt,
  Round,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DashPattern {
  pub(crate) intervals: [f32; 2],
  pub(crate) offset: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Stroke {
  pub(crate) width: f32,
  pub(crate) join: Join,
  pub(crate) cap: Cap,
  pub(crate) dash: Option<DashPattern>,
}

impl Stroke {
  pub(crate) fn new(width: f32) -> Self {
    Self {
      width,
      join: Join::Miter,
      cap: Cap::Butt,
      dash: None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum Style {
  #[default]
  FillNonZero,
  FillEvenOdd,
  Stroke(Stroke),
}

impl From<Fill> for Style {
  fn from(fill: Fill) -> Self {
    match fill {
      Fill::NonZero => Self::FillNonZero,
      Fill::EvenOdd => Self::FillEvenOdd,
    }
  }
}

impl From<Stroke> for Style {
  fn from(stroke: Stroke) -> Self {
    Self::Stroke(stroke)
  }
}

impl From<Fill> for TinyFillRule {
  fn from(fill: Fill) -> Self {
    match fill {
      Fill::NonZero => TinyFillRule::Winding,
      Fill::EvenOdd => TinyFillRule::EvenOdd,
    }
  }
}

impl From<Join> for TinyLineJoin {
  fn from(join: Join) -> Self {
    match join {
      Join::Miter => TinyLineJoin::Miter,
      Join::Round => TinyLineJoin::Round,
      Join::Bevel => TinyLineJoin::Bevel,
    }
  }
}

impl From<Cap> for TinyLineCap {
  fn from(cap: Cap) -> Self {
    match cap {
      Cap::Butt => TinyLineCap::Butt,
      Cap::Round => TinyLineCap::Round,
    }
  }
}

impl From<Stroke> for TinyStroke {
  fn from(stroke: Stroke) -> Self {
    Self {
      width: stroke.width,
      line_cap: stroke.cap.into(),
      line_join: stroke.join.into(),
      dash: stroke
        .dash
        .and_then(|pattern| TinyStrokeDash::new(pattern.intervals.into(), pattern.offset)),
      ..TinyStroke::default()
    }
  }
}

impl Style {
  pub(crate) fn fill_rule(self) -> TinyFillRule {
    match self {
      Style::FillEvenOdd => Fill::EvenOdd.into(),
      Style::FillNonZero | Style::Stroke(_) => Fill::NonZero.into(),
    }
  }

  pub(crate) fn stroke(self) -> Option<TinyStroke> {
    match self {
      Style::Stroke(stroke) => Some(stroke.into()),
      _ => None,
    }
  }
}

pub(crate) type Command = PathCommand;

fn from_tiny(segment: TinyPathSegment) -> Command {
  match segment {
    TinyPathSegment::MoveTo(p) => Command::MoveTo(Point::new(p.x, p.y)),
    TinyPathSegment::LineTo(p) => Command::LineTo(Point::new(p.x, p.y)),
    TinyPathSegment::QuadTo(p1, p2) => {
      Command::QuadTo(Point::new(p1.x, p1.y), Point::new(p2.x, p2.y))
    }
    TinyPathSegment::CubicTo(p1, p2, p3) => Command::CubicTo(
      Point::new(p1.x, p1.y),
      Point::new(p2.x, p2.y),
      Point::new(p3.x, p3.y),
    ),
    TinyPathSegment::Close => Command::Close,
  }
}

pub(crate) use takumi_core::geometry::PathBuilder;

/// Appends an axis-aligned ellipse outline to `commands`.
pub(crate) fn push_ellipse(
  commands: &mut Vec<Command>,
  center: (f32, f32),
  radius_x: f32,
  radius_y: f32,
) {
  let Some(rect) = TinyRect::from_ltrb(
    center.0 - radius_x,
    center.1 - radius_y,
    center.0 + radius_x,
    center.1 + radius_y,
  ) else {
    return;
  };

  let mut builder = TinyPathBuilder::new();
  builder.push_oval(rect);
  if let Some(path) = builder.finish() {
    commands.extend(path.segments().map(from_tiny));
  }
}

/// Scale a command list's coordinates uniformly (CSS px → device space).
pub(crate) fn scale_commands(commands: Vec<Command>, scale: f32) -> Vec<Command> {
  let point = |pt: Point<f32>| Point::new(pt.x * scale, pt.y * scale);

  commands
    .into_iter()
    .map(|command| match command {
      Command::MoveTo(a) => Command::MoveTo(point(a)),
      Command::LineTo(a) => Command::LineTo(point(a)),
      Command::QuadTo(a, b) => Command::QuadTo(point(a), point(b)),
      Command::CubicTo(a, b, c) => Command::CubicTo(point(a), point(b), point(c)),
      Command::Close => Command::Close,
    })
    .collect()
}

pub(crate) fn build_path(commands: &[Command]) -> Option<TinyPath> {
  let mut builder = TinyPathBuilder::new();

  for command in commands {
    match command {
      Command::MoveTo(point) => builder.move_to(point.x, point.y),
      Command::LineTo(point) => builder.line_to(point.x, point.y),
      Command::QuadTo(p1, p2) => builder.quad_to(p1.x, p1.y, p2.x, p2.y),
      Command::CubicTo(p1, p2, p3) => {
        builder.cubic_to(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y);
      }
      Command::Close => builder.close(),
    }
  }

  builder.finish()
}

pub(crate) fn parse_svg_path_segments(input: &str) -> Option<Vec<Command>> {
  let mut commands = Vec::new();

  for segment in SimplifyingPathParser::from(input) {
    match segment.ok()? {
      SimplePathSegment::MoveTo { x, y } => {
        commands.push(Command::MoveTo(Point::new(x as f32, y as f32)));
      }
      SimplePathSegment::LineTo { x, y } => {
        commands.push(Command::LineTo(Point::new(x as f32, y as f32)));
      }
      SimplePathSegment::CurveTo {
        x1,
        y1,
        x2,
        y2,
        x,
        y,
      } => {
        commands.push(Command::CubicTo(
          Point::new(x1 as f32, y1 as f32),
          Point::new(x2 as f32, y2 as f32),
          Point::new(x as f32, y as f32),
        ));
      }
      SimplePathSegment::Quadratic { x1, y1, x, y } => {
        commands.push(Command::QuadTo(
          Point::new(x1 as f32, y1 as f32),
          Point::new(x as f32, y as f32),
        ));
      }
      SimplePathSegment::ClosePath => {
        commands.push(Command::Close);
      }
    }
  }

  Some(commands)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn stroke_dash_is_forwarded_to_tiny_skia() {
    let tiny: TinyStroke = Stroke {
      width: 4.0,
      join: Join::Miter,
      cap: Cap::Round,
      dash: Some(DashPattern {
        intervals: [12.0, 8.0],
        offset: 1.5,
      }),
    }
    .into();

    assert_eq!(tiny.width, 4.0);
    assert_eq!(tiny.line_cap, TinyLineCap::Round);
    assert!(tiny.dash.is_some());
  }
}
