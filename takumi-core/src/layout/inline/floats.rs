//! Float placement along the line stack.

use parley::{InlineBox, PositionedInlineBox};

use super::{InlineBrush, items::ProcessedInlineSpan};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FloatSide {
  Left,
  Right,
}

#[derive(Clone, Copy, Debug)]
struct ActiveFloat {
  side: FloatSide,
  x: f32,
  y: f32,
  width: f32,
  height: f32,
}

impl ActiveFloat {
  fn bottom(self) -> f32 {
    self.y + self.height
  }

  fn overlaps_range(self, top: f32, bottom: f32) -> bool {
    self.y < bottom && top < self.bottom()
  }
}

pub(super) struct FloatLayoutState {
  max_width: f32,
  line_height_hint: f32,
  active_floats: Vec<ActiveFloat>,
}

impl FloatLayoutState {
  pub(super) fn new(max_width: f32, line_height_hint: f32) -> Self {
    Self {
      max_width,
      line_height_hint,
      active_floats: Vec::new(),
    }
  }

  pub(super) fn side_for_inline_box(
    &self,
    spans: &[ProcessedInlineSpan<'_>],
    inline_box_id: u64,
  ) -> Option<FloatSide> {
    let ProcessedInlineSpan::Box(item) = spans.get(inline_box_id as usize)? else {
      return None;
    };

    match item
      .render_node
      .context
      .style
      .float
      .resolve(item.render_node.context.style.direction)
    {
      taffy::Float::Left => Some(FloatSide::Left),
      taffy::Float::Right => Some(FloatSide::Right),
      taffy::Float::None => None,
    }
  }

  pub(super) fn clear_for_inline_box(
    &self,
    spans: &[ProcessedInlineSpan<'_>],
    inline_box_id: u64,
  ) -> taffy::Clear {
    let Some(ProcessedInlineSpan::Box(item)) = spans.get(inline_box_id as usize) else {
      return taffy::Clear::None;
    };

    item
      .render_node
      .context
      .style
      .clear
      .resolve(item.render_node.context.style.direction)
  }

  fn next_float_bottom(&self, top: f32, height: f32) -> Option<f32> {
    let bottom = top + height.max(0.0);
    self
      .active_floats
      .iter()
      .filter_map(|float| float.overlaps_range(top, bottom).then_some(float.bottom()))
      .min_by(f32::total_cmp)
  }

  fn bounds_for_range(&self, top: f32, height: f32) -> (f32, f32) {
    let bottom = top + height.max(0.0);
    let mut left = 0.0_f32;
    let mut right = self.max_width;

    for active_float in &self.active_floats {
      if !active_float.overlaps_range(top, bottom) {
        continue;
      }

      match active_float.side {
        FloatSide::Left => left = left.max(active_float.x + active_float.width),
        FloatSide::Right => right = right.min(active_float.x),
      }
    }

    (
      left.min(self.max_width),
      right.max(left).min(self.max_width),
    )
  }

  fn line_bounds(&self, line_y: f32) -> (f32, f32) {
    self.bounds_for_range(line_y, self.line_height_hint)
  }

  fn clearance_y(&self, start_y: f32, clear: taffy::Clear) -> f32 {
    self
      .active_floats
      .iter()
      .filter(|float| float.bottom() > start_y)
      .filter(|float| {
        matches!(
          (clear, float.side),
          (taffy::Clear::Left, FloatSide::Left)
            | (taffy::Clear::Right, FloatSide::Right)
            | (taffy::Clear::Both, _)
        )
      })
      .map(|float| float.bottom())
      .fold(start_y.max(0.0), f32::max)
  }

  fn find_float_y(&self, start_y: f32, width: f32, height: f32) -> f32 {
    let mut line_y = start_y.max(0.0);

    loop {
      let (left, right) = self.bounds_for_range(line_y, height);
      if width <= right - left || (left == 0.0 && right == self.max_width) {
        return line_y;
      }

      let Some(next_y) = self.next_float_bottom(line_y, height) else {
        return line_y;
      };
      line_y = next_y;
    }
  }

  pub(super) fn find_line_y_for_advance(&self, start_y: f32, current_advance: f32) -> f32 {
    let mut line_y = start_y.max(0.0);

    loop {
      let (left, right) = self.line_bounds(line_y);
      if current_advance <= right - left || (left == 0.0 && right == self.max_width) {
        return line_y;
      }

      let Some(next_y) = self.next_float_bottom(line_y, self.line_height_hint) else {
        return line_y;
      };
      line_y = next_y;
    }
  }

  pub(super) fn push_float(
    &mut self,
    side: FloatSide,
    clear: taffy::Clear,
    start_y: f32,
    inline_box: &InlineBox,
  ) -> PositionedInlineBox {
    let cleared_y = self.clearance_y(start_y, clear);
    let float_y = self.find_float_y(cleared_y, inline_box.width, inline_box.height);
    let (left, right) = self.bounds_for_range(float_y, inline_box.height);
    let float_x = match side {
      FloatSide::Left => left,
      FloatSide::Right => (right - inline_box.width).max(left),
    };

    self.active_floats.push(ActiveFloat {
      side,
      x: float_x,
      y: float_y,
      width: inline_box.width,
      height: inline_box.height,
    });

    PositionedInlineBox {
      x: float_x,
      y: float_y,
      width: inline_box.width,
      height: inline_box.height,
      id: inline_box.id,
      kind: inline_box.kind,
    }
  }

  pub(super) fn update_breaker_line(
    &self,
    breaker: &mut parley::BreakLines<'_, InlineBrush>,
    line_y: f32,
  ) {
    let (line_x, line_right) = self.line_bounds(line_y);
    let state = breaker.state_mut();
    state.set_layout_max_advance(self.max_width);
    state.set_line_x(line_x);
    state.set_line_y(f64::from(line_y));
    state.set_line_max_advance((line_right - line_x).max(0.0));
  }
}
