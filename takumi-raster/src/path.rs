use takumi_core::{
  geometry::PathCommand,
  painter::StrokeStyle,
  path::{Path, PathBuilder, PathSegment},
};
use tiny_skia_path::{
  LineCap as TinyLineCap, LineJoin as TinyLineJoin, Stroke as TinyStroke,
  StrokeDash as TinyStrokeDash,
};

use crate::style::{Affine, FillRule, LineJoin};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum Fill {
  #[default]
  NonZero,
  EvenOdd,
}

impl From<FillRule> for Fill {
  fn from(value: FillRule) -> Self {
    match value {
      FillRule::EvenOdd => Fill::EvenOdd,
      _ => Fill::NonZero,
    }
  }
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

impl From<&StrokeStyle> for Stroke {
  fn from(style: &StrokeStyle) -> Self {
    Self {
      cap: if style.round_cap {
        Cap::Round
      } else {
        Cap::Butt
      },
      dash: style.dash.map(|intervals| DashPattern {
        intervals,
        offset: 0.0,
      }),
      ..Self::new(style.width)
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
  pub(crate) fn fill_rule(self) -> Fill {
    match self {
      Style::FillEvenOdd => Fill::EvenOdd,
      Style::FillNonZero | Style::Stroke(_) => Fill::NonZero,
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

/// `path` dashed and then stroked as `stroke` says, at the precision `resolution_scale` asks for.
// TODO: port Skia's `SkStroke` and `SkDashPath` in place of tiny-skia-path's.
pub(crate) fn stroke_path(path: &Path, stroke: &TinyStroke, resolution_scale: f32) -> Option<Path> {
  let mut builder = tiny_skia_path::PathBuilder::new();

  for segment in path.segments() {
    match segment {
      PathSegment::MoveTo(to) => builder.move_to(to.x, to.y),
      PathSegment::LineTo(to) => builder.line_to(to.x, to.y),
      PathSegment::QuadTo(control, to) => builder.quad_to(control.x, control.y, to.x, to.y),
      PathSegment::CubicTo(first, second, to) => {
        builder.cubic_to(first.x, first.y, second.x, second.y, to.x, to.y);
      }
      PathSegment::Close => builder.close(),
    }
  }

  let mut outline = builder.finish()?;

  if let Some(dash) = &stroke.dash {
    outline = outline.dash(dash, resolution_scale)?;
  }

  let outline = outline.stroke(stroke, resolution_scale)?;
  let mut builder = PathBuilder::new();

  for segment in outline.segments() {
    match segment {
      tiny_skia_path::PathSegment::MoveTo(to) => builder.move_to(to.x, to.y),
      tiny_skia_path::PathSegment::LineTo(to) => builder.line_to(to.x, to.y),
      tiny_skia_path::PathSegment::QuadTo(control, to) => {
        builder.quad_to(control.x, control.y, to.x, to.y);
      }
      tiny_skia_path::PathSegment::CubicTo(first, second, to) => {
        builder.cubic_to(first.x, first.y, second.x, second.y, to.x, to.y);
      }
      tiny_skia_path::PathSegment::Close => builder.close(),
    }
  }

  builder.finish()
}

/// `SkStrokeRec::GetResScale`: how finely a stroke under `transform` is approximated.
pub(crate) fn resolution_scale(transform: Affine) -> f32 {
  tiny_skia_path::PathStroker::compute_resolution_scale(&tiny_skia_path::Transform::from_row(
    transform.a,
    transform.b,
    transform.c,
    transform.d,
    transform.x,
    transform.y,
  ))
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
