//! Outlines of inline elements: each element's line fragments, grouped into islands.

use std::collections::HashMap;

use crate::{
  context::RenderContext,
  geometry::{LAYOUT_UNIT_EPSILON, PathBuilder, PathCommand, Point},
  style::{BorderStyle, Color, Sides, SpacePair},
};

/// How an inline element draws its outline.
#[derive(Debug, Clone, Copy)]
pub struct InlineOutline {
  /// `outline-width`, in pixels.
  pub width: f32,
  /// `outline-offset`, in pixels.
  pub offset: f32,
  /// `outline-color`.
  pub color: Color,
  /// `outline-style`.
  pub style: BorderStyle,
}

impl InlineOutline {
  /// The outline an element in `context` paints, or `None` when it paints none.
  pub(crate) fn of(context: &RenderContext) -> Option<Self> {
    let style = &context.style;
    let width = style.outline_width.to_used_px(&context.sizing).max(0.0);
    let color = style.outline_color.resolve(context.current_color);

    (width > 0.0 && style.outline_style.is_rendered() && color.0[3] != 0).then(|| Self {
      width,
      offset: style.outline_offset.to_border_px(&context.sizing, 0.0),
      color,
      style: style.outline_style,
    })
  }

  /// How far the outline's outer edge reaches past the element's border box.
  pub(crate) fn reach(self) -> f32 {
    self.offset + self.width
  }
}

/// One line fragment of an outlined inline element, in border-box space.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct InlineOutlineRect {
  /// The element the fragment belongs to, unique within its inline layout.
  pub(crate) owner: usize,
  /// Line index the fragment sits on.
  pub(crate) line_index: usize,
  /// Left edge in border-box space.
  pub(crate) x: f32,
  /// Top edge in border-box space.
  pub(crate) y: f32,
  /// The fragment's border-box width.
  pub(crate) width: f32,
  /// The fragment's border-box height.
  pub(crate) height: f32,
  /// The fragment's corner radii.
  pub(crate) radius: Sides<SpacePair<f32>>,
  /// The element's outline.
  pub(crate) outline: InlineOutline,
  /// The element's `opacity`.
  pub(crate) opacity: f32,
}

impl InlineOutlineRect {
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

/// Fragments of one element's outline that touch from line to line, stroked as one contour.
pub struct OutlineIsland {
  rects: Vec<InlineOutlineRect>,
  /// Whether the island's one rect is its element's whole outline.
  lone: bool,
}

impl OutlineIsland {
  /// Groups each element's fragments, sorted by element then line, into islands of consecutive
  /// lines that meet once grown by the outline's reach, as Blink unites the grown rects into one
  /// region.
  pub fn of(rects: &[InlineOutlineRect]) -> Vec<Self> {
    let mut rect_counts: HashMap<usize, usize> = HashMap::new();
    let mut islands: Vec<Self> = Vec::new();

    for rect in rects {
      *rect_counts.entry(rect.owner).or_default() += 1;
    }

    for &rect in rects {
      if let Some(island) = islands.last_mut()
        && let Some(&previous) = island.rects.last()
        && previous.owner == rect.owner
        && previous.line_index + 1 == rect.line_index
        && previous.meets(rect, rect.outline.reach())
      {
        island.rects.push(rect);
        continue;
      }

      islands.push(Self {
        rects: vec![rect],
        lone: rect_counts[&rect.owner] == 1,
      });
    }

    islands
  }

  /// The rect when it is its element's whole outline, which Blink paints as a box border.
  pub fn lone_rect(&self) -> Option<InlineOutlineRect> {
    self.lone.then(|| self.rects[0])
  }

  /// The element's outline and opacity.
  pub fn outline(&self) -> (InlineOutline, f32) {
    let rect = self.rects[0];

    (rect.outline, rect.opacity)
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
