use takumi_core::{
  geometry::PathCommand,
  painter::StrokeStyle,
  path::{Path, Stroke as PathStroke, StrokeDash},
};

use crate::style::{FillRule, LineCap, LineJoin};

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DashPattern {
  pub(crate) intervals: [f32; 2],
  pub(crate) offset: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Stroke {
  pub(crate) width: f32,
  pub(crate) join: LineJoin,
  pub(crate) cap: LineCap,
  pub(crate) dash: Option<DashPattern>,
}

impl Stroke {
  /// The outline this stroke draws and the dashes it cuts first.
  pub(crate) fn outline(self) -> (PathStroke, Option<StrokeDash>) {
    (
      PathStroke {
        width: self.width,
        line_join: self.join,
        line_cap: self.cap,
        ..PathStroke::default()
      },
      self
        .dash
        .and_then(|pattern| StrokeDash::new(pattern.intervals.into(), pattern.offset)),
    )
  }

  pub(crate) fn new(width: f32) -> Self {
    Self {
      width,
      join: LineJoin::Miter,
      cap: LineCap::Butt,
      dash: None,
    }
  }
}

impl From<&StrokeStyle> for Stroke {
  fn from(style: &StrokeStyle) -> Self {
    Self {
      cap: if style.round_cap {
        LineCap::Round
      } else {
        LineCap::Butt
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

impl Style {
  pub(crate) fn fill_rule(self) -> Fill {
    match self {
      Style::FillEvenOdd => Fill::EvenOdd,
      Style::FillNonZero | Style::Stroke(_) => Fill::NonZero,
    }
  }

  pub(crate) fn stroke(self) -> Option<Stroke> {
    match self {
      Style::Stroke(stroke) => Some(stroke),
      _ => None,
    }
  }
}

pub(crate) type Command = PathCommand;

/// `path` cut into `dash` and then stroked as `stroke` says, at the precision `resolution_scale`
/// asks for.
pub(crate) fn stroke_path(
  path: &Path,
  stroke: &PathStroke,
  dash: Option<&StrokeDash>,
  resolution_scale: f32,
) -> Option<Path> {
  match dash {
    Some(dash) => path
      .dash(dash, resolution_scale)?
      .stroke(stroke, resolution_scale),
    None => path.stroke(stroke, resolution_scale),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_stroke_outline_carries_its_cap_and_dash() {
    let (outline, dash) = Stroke {
      width: 4.0,
      join: LineJoin::Miter,
      cap: LineCap::Round,
      dash: Some(DashPattern {
        intervals: [12.0, 8.0],
        offset: 1.5,
      }),
    }
    .outline();

    assert_eq!(outline.width, 4.0);
    assert_eq!(outline.line_cap, LineCap::Round);
    assert!(dash.is_some());
  }
}
