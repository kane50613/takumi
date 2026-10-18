//! Blink's [`relative_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/relative_utils.cc).

use crate::{
  geometry::Point,
  layout::{constraint_space::ConstraintSpace, logical_rect::LogicalOffset, tree::RenderNode},
  layout_unit::LayoutUnit,
  style::{Direction, Length},
};

impl ConstraintSpace {
  /// Blink's `ComputeRelativeOffsetForInline`, line-left and down.
  pub(crate) fn compute_relative_offset_for_inline(&self, node: &RenderNode) -> LogicalOffset {
    let offset = compute_relative_offset_for_inline(
      node,
      self.direction,
      self
        .percentage_resolution_inline_size
        .map(LayoutUnit::to_float),
      self
        .percentage_resolution_block_size
        .map(LayoutUnit::to_float),
    );

    LogicalOffset::new(
      LayoutUnit::from_float(offset.x),
      LayoutUnit::from_float(offset.y),
    )
  }
}

/// Blink's `ComputeRelativeOffsetForInline`: how far `position: relative` moves the inline-level
/// `node` in a container of `direction`, its start inset winning when both horizontal ones are
/// set. A percentage inset against an indefinite size counts as `auto`.
pub(crate) fn compute_relative_offset_for_inline(
  node: &RenderNode,
  direction: Direction,
  inline_size: Option<f32>,
  block_size: Option<f32>,
) -> Point<f32> {
  let style = &node.context.style;

  if !style.base_data.position.is_relative() {
    return Point::ZERO;
  }

  let inset = |length: Length, basis: Option<f32>| match (length, basis) {
    (Length::Auto, _) => None,
    (length, None) if length.has_percentage() => None,
    (length, basis) => Some(length.to_px(&node.context.sizing, basis.unwrap_or(0.0))),
  };
  let surround = &style.surround_data;
  let (left, right) = (
    inset(surround.left, inline_size),
    inset(surround.right, inline_size),
  );
  let x = match (left, right, direction) {
    (Some(left), _, Direction::Ltr) | (Some(left), None, Direction::Rtl) => left,
    (_, Some(right), _) => -right,
    (None, None, _) => 0.0,
  };
  let y = inset(surround.top, block_size)
    .or_else(|| inset(surround.bottom, block_size).map(|bottom| -bottom))
    .unwrap_or(0.0);

  Point { x, y }
}
