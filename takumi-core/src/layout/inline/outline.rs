//! Outline rectangles of an inline formatting context, merged into islands.

use crate::geometry::{LAYOUT_UNIT_EPSILON, PathBuilder, PathCommand};
use std::collections::HashMap;

use super::text_fit::{LineScaleState, text_fit_x_correction};

/// A glyph run's text-outline rectangle on a line, in border-box space.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct InlineOutlineRect {
  /// Source inline span id (identifies the styled run the rect belongs to).
  pub span_id: u64,
  /// Line index the rect sits on.
  pub(crate) line_index: usize,
  /// Left edge in border-box space.
  pub(crate) x: f32,
  /// Top edge in border-box space.
  pub(crate) y: f32,
  /// Rect width (run advance).
  pub(crate) width: f32,
  /// Rect height (resolved line height).
  pub(crate) height: f32,
}

pub(super) fn scale_outline_rect(
  rect: InlineOutlineRect,
  state: LineScaleState,
  static_inline_prefix: f32,
) -> InlineOutlineRect {
  if (state.scale - 1.0).abs() <= f32::EPSILON {
    return rect;
  }
  let x_correction = text_fit_x_correction(
    state.scale,
    static_inline_prefix,
    state.alignment_correction,
  );
  InlineOutlineRect {
    x: x_correction + state.layout_origin.x + (rect.x - state.layout_origin.x) * state.scale,
    y: state.layout_origin.y + (rect.y - state.layout_origin.y) * state.scale,
    width: rect.width * state.scale,
    height: rect.height * state.scale,
    ..rect
  }
}

pub(super) fn x_ranges_touch(left: InlineOutlineRect, right: InlineOutlineRect) -> bool {
  left.x <= right.x + right.width + LAYOUT_UNIT_EPSILON
    && right.x <= left.x + left.width + LAYOUT_UNIT_EPSILON
}

fn expand_outline_rect(rect: InlineOutlineRect, amount: f32) -> Option<InlineOutlineRect> {
  let width = rect.width + amount * 2.0;
  let height = rect.height + amount * 2.0;
  if width <= 0.0 || height <= 0.0 {
    return None;
  }
  Some(InlineOutlineRect {
    x: rect.x - amount,
    y: rect.y - amount,
    width,
    height,
    ..rect
  })
}

/// Merges rects that touch on the same span and line into one rect per contiguous group, sorted by
/// span then line.
fn merge_inline_rects(mut rects: Vec<InlineOutlineRect>) -> Vec<InlineOutlineRect> {
  rects.sort_by(|left, right| {
    left
      .span_id
      .cmp(&right.span_id)
      .then(left.line_index.cmp(&right.line_index))
      .then(left.x.total_cmp(&right.x))
  });

  let mut merged_rects: Vec<InlineOutlineRect> = Vec::with_capacity(rects.len());
  for rect in rects {
    let Some(previous_rect) = merged_rects.last_mut() else {
      merged_rects.push(rect);
      continue;
    };

    let same_group =
      previous_rect.span_id == rect.span_id && previous_rect.line_index == rect.line_index;
    let touching = rect.x <= previous_rect.x + previous_rect.width + LAYOUT_UNIT_EPSILON;
    let same_band = (rect.y - previous_rect.y).abs() <= LAYOUT_UNIT_EPSILON
      && (rect.height - previous_rect.height).abs() <= LAYOUT_UNIT_EPSILON;

    if same_group && same_band && touching {
      let right_edge = (previous_rect.x + previous_rect.width).max(rect.x + rect.width);
      previous_rect.x = previous_rect.x.min(rect.x);
      previous_rect.y = previous_rect.y.min(rect.y);
      previous_rect.width = right_edge - previous_rect.x;
      previous_rect.height = previous_rect.height.max(rect.height);
    } else {
      merged_rects.push(rect);
    }
  }
  merged_rects
}

/// Merges adjacent per-line outline rects, then groups them into vertically-continuous islands;
/// each island becomes one stroked contour.
pub fn outline_islands(outline_rects: Vec<InlineOutlineRect>) -> Vec<Vec<InlineOutlineRect>> {
  let merged_rects = merge_inline_rects(outline_rects);

  let mut line_rect_counts = HashMap::new();
  for outline_rect in &merged_rects {
    *line_rect_counts
      .entry((outline_rect.span_id, outline_rect.line_index))
      .or_insert(0usize) += 1;
  }

  let mut islands: Vec<Vec<InlineOutlineRect>> = Vec::new();
  for outline_rect in merged_rects {
    let mut matched_island = None;

    for (index, island) in islands.iter().enumerate() {
      let Some(previous_rect) = island.last().copied() else {
        continue;
      };
      if previous_rect.span_id != outline_rect.span_id {
        continue;
      }
      if outline_rect.line_index != previous_rect.line_index + 1 {
        continue;
      }

      let previous_is_unique =
        line_rect_counts.get(&(previous_rect.span_id, previous_rect.line_index)) == Some(&1);
      let current_is_unique =
        line_rect_counts.get(&(outline_rect.span_id, outline_rect.line_index)) == Some(&1);
      if (previous_is_unique && current_is_unique) || x_ranges_touch(previous_rect, outline_rect) {
        matched_island = Some(index);
        break;
      }
    }

    if let Some(index) = matched_island {
      islands[index].push(outline_rect);
    } else {
      islands.push(vec![outline_rect]);
    }
  }

  islands
}

/// Builds the rectilinear contour around one island of outline rects, expanded by `expansion`
/// (outline-offset plus half the outline width).
pub fn outline_island_contour(island: &[InlineOutlineRect], expansion: f32) -> Vec<PathCommand> {
  let mut path = Vec::with_capacity(island.len() * 6);
  let mut expanded_rects = island
    .iter()
    .filter_map(|r| expand_outline_rect(*r, expansion));
  let Some(first_rect) = expanded_rects.next() else {
    return path;
  };

  path.move_to((first_rect.x, first_rect.y));
  path.line_to((first_rect.x + first_rect.width, first_rect.y));

  let mut current_rect = first_rect;
  for next_rect in expanded_rects {
    path.line_to((current_rect.x + current_rect.width, next_rect.y));
    path.line_to((next_rect.x + next_rect.width, next_rect.y));
    current_rect = next_rect;
  }
  let last_rect = current_rect;

  path.line_to((
    last_rect.x + last_rect.width,
    last_rect.y + last_rect.height,
  ));
  path.line_to((last_rect.x, last_rect.y + last_rect.height));

  let mut expanded_rev = island
    .iter()
    .rev()
    .filter_map(|r| expand_outline_rect(*r, expansion));
  let Some(mut lower_rect) = expanded_rev.next() else {
    return path;
  };

  for upper_rect in expanded_rev {
    path.line_to((lower_rect.x, upper_rect.y + upper_rect.height));
    path.line_to((upper_rect.x, upper_rect.y + upper_rect.height));
    lower_rect = upper_rect;
  }

  path.close();
  path
}
