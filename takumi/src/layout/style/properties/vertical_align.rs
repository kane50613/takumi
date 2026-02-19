use parley::LineMetrics;

use crate::layout::style::{tw::TailwindPropertyParser, *};

/// Defines the vertical alignment of an inline-level box.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum VerticalAlign {
  /// Aligns the baseline of the box with the baseline of the parent box.
  #[default]
  Baseline,
  /// Aligns the top of the box with the top of the line box.
  Top,
  /// Aligns the middle of the box with the baseline of the parent box plus half the x-height of the parent.
  Middle,
  /// Aligns the bottom of the box with the bottom of the line box.
  Bottom,
  /// Aligns the top of the box with the top of the parent's font.
  TextTop,
  /// Aligns the bottom of the box with the bottom of the parent's font.
  TextBottom,
  /// Aligns the baseline of the box with the subscript-baseline of the parent box.
  Sub,
  /// Aligns the baseline of the box with the superscript-baseline of the parent box.
  Super,
}

declare_enum_from_css_impl!(
  VerticalAlign,
  "baseline" => VerticalAlign::Baseline,
  "top" => VerticalAlign::Top,
  "middle" => VerticalAlign::Middle,
  "bottom" => VerticalAlign::Bottom,
  "text-top" => VerticalAlign::TextTop,
  "text-bottom" => VerticalAlign::TextBottom,
  "sub" => VerticalAlign::Sub,
  "super" => VerticalAlign::Super
);

impl VerticalAlign {
  pub(crate) fn apply(
    self,
    y: &mut f32,
    metrics: &LineMetrics,
    box_height: f32,
    parent_x_height: Option<f32>,
  ) {
    match self {
      VerticalAlign::Baseline => *y = metrics.baseline - box_height,
      VerticalAlign::Top => {
        // Aligns with top of line box
        *y = metrics.min_coord;
      }
      VerticalAlign::Middle => {
        let x_height = parent_x_height.unwrap_or(metrics.ascent * 0.5);
        *y = metrics.baseline - (x_height * 0.5) - (box_height / 2.0);
      }
      VerticalAlign::Bottom => {
        // Aligns with bottom of line box
        *y = metrics.max_coord - box_height;
      }
      VerticalAlign::TextTop => *y = metrics.baseline - metrics.ascent,
      VerticalAlign::TextBottom => *y = metrics.baseline + metrics.descent - box_height,
      VerticalAlign::Sub => *y = metrics.baseline + (metrics.descent * 0.2), // Places top below baseline
      VerticalAlign::Super => *y = metrics.baseline - metrics.ascent + (metrics.ascent * 0.4), // Places top high up
    }
  }
}

impl TailwindPropertyParser for VerticalAlign {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_str(token).ok()
  }
}
