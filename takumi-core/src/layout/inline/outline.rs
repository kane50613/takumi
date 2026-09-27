//! Outline rectangles of an inline formatting context, merged into islands.

use crate::geometry::{LAYOUT_UNIT_EPSILON, PathBuilder, PathCommand, Point};

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
  /// Rect height (the font's content area).
  pub(crate) height: f32,
}

impl InlineOutlineRect {
  /// The rect on a line scaled for text-fit.
  pub(super) fn scaled(self, state: LineScaleState, static_inline_prefix: f32) -> Self {
    if (state.scale - 1.0).abs() <= f32::EPSILON {
      return self;
    }
    let x_correction = text_fit_x_correction(
      state.scale,
      static_inline_prefix,
      state.alignment_correction,
    );
    Self {
      x: x_correction + state.layout_origin.x + (self.x - state.layout_origin.x) * state.scale,
      y: state.layout_origin.y + (self.y - state.layout_origin.y) * state.scale,
      width: self.width * state.scale,
      height: self.height * state.scale,
      ..self
    }
  }

  /// Whether the two rects meet, within a layout unit, once both grow by `reach`.
  pub(super) fn meets(self, other: Self, reach: f32) -> bool {
    let slack = 2.0 * reach + LAYOUT_UNIT_EPSILON;

    self.x <= other.x + other.width + slack
      && other.x <= self.x + self.width + slack
      && self.y <= other.y + other.height + slack
      && other.y <= self.y + self.height + slack
  }

  /// The rect grown by `amount` on every side, or `None` once it has no area.
  fn expanded(self, amount: f32) -> Option<Self> {
    let width = self.width + amount * 2.0;
    let height = self.height + amount * 2.0;
    if width <= 0.0 || height <= 0.0 {
      return None;
    }
    Some(Self {
      x: self.x - amount,
      y: self.y - amount,
      width,
      height,
      ..self
    })
  }
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

/// Rects of one span's outline that touch from line to line, stroked as one contour.
pub struct OutlineIsland {
  rects: Vec<InlineOutlineRect>,
}

impl OutlineIsland {
  /// Merges adjacent per-line outline rects, then groups the rects of consecutive lines that meet
  /// once grown by their span's `reach`, as Blink unites the grown rects into one region.
  pub fn of(outline_rects: Vec<InlineOutlineRect>, reach: impl Fn(u64) -> f32) -> Vec<Self> {
    let mut islands: Vec<Self> = Vec::new();

    for rect in merge_inline_rects(outline_rects) {
      let reach = reach(rect.span_id);
      let island = islands.iter_mut().find(|island| {
        island.rects.last().is_some_and(|previous| {
          previous.span_id == rect.span_id
            && rect.line_index == previous.line_index + 1
            && previous.meets(rect, reach)
        })
      });

      match island {
        Some(island) => island.rects.push(rect),
        None => islands.push(Self { rects: vec![rect] }),
      }
    }

    islands
  }

  /// The span whose outline this is.
  pub fn span_id(&self) -> Option<u64> {
    self.rects.first().map(|rect| rect.span_id)
  }

  /// The corners of the rectilinear contour around the island, grown by `expansion` past its
  /// rects, clockwise from the top-left.
  pub fn corners(&self, expansion: f32) -> Vec<Point<f32>> {
    let island = &self.rects;
    let mut corners = Vec::with_capacity(island.len() * 4);
    let point = |x, y| Point { x, y };
    let mut expanded_rects = island.iter().filter_map(|rect| rect.expanded(expansion));
    let Some(first_rect) = expanded_rects.next() else {
      return corners;
    };

    corners.push(point(first_rect.x, first_rect.y));
    corners.push(point(first_rect.x + first_rect.width, first_rect.y));

    let mut current_rect = first_rect;
    for next_rect in expanded_rects {
      corners.push(point(current_rect.x + current_rect.width, next_rect.y));
      corners.push(point(next_rect.x + next_rect.width, next_rect.y));
      current_rect = next_rect;
    }
    let last_rect = current_rect;

    corners.push(point(
      last_rect.x + last_rect.width,
      last_rect.y + last_rect.height,
    ));
    corners.push(point(last_rect.x, last_rect.y + last_rect.height));

    let mut expanded_rev = island
      .iter()
      .rev()
      .filter_map(|rect| rect.expanded(expansion));
    let Some(mut lower_rect) = expanded_rev.next() else {
      return corners;
    };

    for upper_rect in expanded_rev {
      corners.push(point(lower_rect.x, upper_rect.y + upper_rect.height));
      corners.push(point(upper_rect.x, upper_rect.y + upper_rect.height));
      lower_rect = upper_rect;
    }

    corners
  }

  /// The closed contour through [`OutlineIsland::corners`].
  pub fn contour(&self, expansion: f32) -> Vec<PathCommand> {
    let corners = self.corners(expansion);
    let mut path = Vec::with_capacity(corners.len() + 1);
    let Some((first, rest)) = corners.split_first() else {
      return path;
    };

    path.move_to((first.x, first.y));

    for corner in rest {
      path.line_to((corner.x, corner.y));
    }

    path.close();
    path
  }
}
