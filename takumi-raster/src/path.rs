use takumi_core::{geometry::PathCommand, path::Stroke};

use crate::style::FillRule;

/// How a mask covers its path: filled by a rule, or stroked.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum Style {
  #[default]
  FillNonZero,
  FillEvenOdd,
  Stroke(Stroke),
}

impl From<FillRule> for Style {
  fn from(rule: FillRule) -> Self {
    match rule {
      FillRule::NonZero => Self::FillNonZero,
      FillRule::EvenOdd => Self::FillEvenOdd,
    }
  }
}

impl From<Stroke> for Style {
  fn from(stroke: Stroke) -> Self {
    Self::Stroke(stroke)
  }
}

impl Style {
  pub(crate) fn fill_rule(&self) -> FillRule {
    match self {
      Style::FillEvenOdd => FillRule::EvenOdd,
      Style::FillNonZero | Style::Stroke(_) => FillRule::NonZero,
    }
  }

  pub(crate) fn stroke(&self) -> Option<&Stroke> {
    match self {
      Style::Stroke(stroke) => Some(stroke),
      _ => None,
    }
  }
}

pub(crate) type Command = PathCommand;
