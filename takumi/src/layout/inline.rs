use std::borrow::Cow;

use taffy::{AvailableSpace, Size};

use crate::{
  layout::{node::Node, style::Color},
  rendering::{MaxHeight, RenderContext},
};

pub(crate) enum InlineItem<'a, N: Node<N>> {
  Node(&'a N),
  Text(Cow<'a, str>),
}

pub enum InlineContentKind {
  Text(String),
  Box,
}

pub type InlineLayout = parley::Layout<InlineBrush>;

#[derive(Clone, PartialEq, Copy, Debug)]
pub struct InlineBrush {
  pub color: Color,
  pub decoration_color: Color,
  pub stroke_color: Color,
}

impl Default for InlineBrush {
  fn default() -> Self {
    Self {
      color: Color::black(),
      decoration_color: Color::black(),
      stroke_color: Color::black(),
    }
  }
}

pub(crate) fn create_inline_constraint(
  context: &RenderContext,
  available_space: Size<AvailableSpace>,
  known_dimensions: Size<Option<f32>>,
) -> (f32, Option<MaxHeight>) {
  let width_constraint = known_dimensions
    .width
    .or(match available_space.width {
      AvailableSpace::MinContent => Some(0.0),
      AvailableSpace::MaxContent => None,
      AvailableSpace::Definite(width) => Some(width),
    })
    .unwrap_or(f32::MAX);

  // applies a maximum height to reduce unnecessary calculation.
  let max_height = match (
    context.sizing.viewport.height,
    context.style.text_wrap_mode_and_line_clamp().1,
  ) {
    (Some(height), Some(line_clamp)) => {
      Some(MaxHeight::HeightAndLines(height as f32, line_clamp.count))
    }
    (Some(height), None) => Some(MaxHeight::Absolute(height as f32)),
    (None, Some(line_clamp)) => Some(MaxHeight::Lines(line_clamp.count)),
    (None, None) => None,
  };

  (width_constraint, max_height)
}

pub(crate) fn break_lines(
  layout: &mut InlineLayout,
  max_width: f32,
  max_height: Option<MaxHeight>,
) {
  let Some(max_height) = max_height else {
    return layout.break_all_lines(Some(max_width));
  };

  match max_height {
    MaxHeight::Lines(lines) => {
      let mut breaker = layout.break_lines();

      for _ in 0..lines {
        if breaker.break_next(max_width).is_none() {
          // no more lines to break
          break;
        };
      }

      breaker.finish();
    }
    MaxHeight::Absolute(max_height) => {
      let mut total_height = 0.0;
      let mut breaker = layout.break_lines();

      while total_height < max_height {
        let Some((_, height)) = breaker.break_next(max_width) else {
          // no more lines to break
          break;
        };

        total_height += height;
      }

      // if its over the max height after last break, revert the break
      if total_height > max_height {
        breaker.revert();
      }

      breaker.finish();
    }
    MaxHeight::HeightAndLines(max_height, max_lines) => {
      let mut total_height = 0.0;
      let mut line_count = 0;
      let mut breaker = layout.break_lines();

      while total_height < max_height {
        if line_count >= max_lines {
          break;
        }

        let Some((_, height)) = breaker.break_next(max_width) else {
          // no more lines to break
          break;
        };

        line_count += 1;
        total_height += height;
      }

      if total_height > max_height {
        breaker.revert();
      }

      breaker.finish();
    }
  }
}
