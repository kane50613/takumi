//! Outlines of inline elements: each element's line fragments, grouped into islands.

use std::collections::HashMap;

use crate::{
  context::RenderContext,
  geometry::{LAYOUT_UNIT_EPSILON, PathBuilder, PathCommand, Point},
  layout::corner_shape::KAPPA,
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
  /// The element's corner radii resolved against this fragment, wrapped edges included.
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

  /// The element's corner radii, resolved against the island's first fragment.
  pub fn radius(&self) -> Sides<SpacePair<f32>> {
    self.rects[0].radius
  }

  /// The contour through [`OutlineIsland::corners`] grown by `expansion`, its convex corners
  /// rounded by `convex` and its concave ones by `concave`, after Blink's `AddCornerRadiiToPath`
  /// in `outline_painter.cc`. Follows Blink under the notice in LICENSE-CHROMIUM.
  pub fn rounded_contour(
    &self,
    expansion: f32,
    convex: Sides<SpacePair<f32>>,
    concave: Sides<SpacePair<f32>>,
  ) -> Vec<PathCommand> {
    let corners = right_angle_corners(self.corners(expansion));
    let count = corners.len();

    if count < 4 {
      return self.contour(expansion);
    }

    let at = |index: usize| corners[(index + count) % count];
    let radius = |index: usize| {
      corner_radius(
        convex,
        concave,
        at(index + count - 1),
        at(index),
        at(index + 1),
      )
    };
    // Each line runs from corner `index` to the next, shortened by both corners' radii.
    let lines: Vec<(Point<f32>, Point<f32>)> = (0..count)
      .map(|index| {
        let (start, end) = (at(index), at(index + 1));
        let (first, second) = (radius(index), radius(index + 1));
        let vertical = start.x == end.x;
        let length = if vertical {
          (end.y - start.y).abs()
        } else {
          (end.x - start.x).abs()
        };
        let (mut near, mut far) = if vertical {
          (first.y, second.y)
        } else {
          (first.x, second.x)
        };

        if near + far > length {
          let scale = length / (near + far);

          near = (near * scale).floor();
          far = (far * scale).floor();
        }

        let step = |from: Point<f32>, toward: Point<f32>, by: f32| Point {
          x: from.x + (toward.x - from.x).signum() * if vertical { 0.0 } else { by },
          y: from.y + (toward.y - from.y).signum() * if vertical { by } else { 0.0 },
        };

        (step(start, end, near), step(end, start, far))
      })
      .collect();
    let mut path = Vec::with_capacity(count * 2 + 2);
    let mut current = lines[count - 1].1;

    path.move_to((current.x, current.y));

    for (index, &(start, end)) in lines.iter().enumerate() {
      let corner = at(index);

      path.push(PathCommand::CubicTo(
        Point {
          x: current.x + (corner.x - current.x) * KAPPA,
          y: current.y + (corner.y - current.y) * KAPPA,
        },
        Point {
          x: start.x + (corner.x - start.x) * KAPPA,
          y: start.y + (corner.y - start.y) * KAPPA,
        },
        start,
      ));
      path.line_to((end.x, end.y));
      current = end;
    }

    path.close();
    path
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

/// `corners` without repeated points or points in the middle of a straight run, so each one turns
/// a right angle.
fn right_angle_corners(corners: Vec<Point<f32>>) -> Vec<Point<f32>> {
  let straight = |a: Point<f32>, b: Point<f32>, c: Point<f32>| {
    b == a || (a.x == b.x && b.x == c.x) || (a.y == b.y && b.y == c.y)
  };
  let mut kept: Vec<Point<f32>> = Vec::with_capacity(corners.len());

  for point in corners {
    while let [.., before, last] = kept[..]
      && straight(before, last, point)
    {
      kept.pop();
    }
    if kept.last() != Some(&point) {
      kept.push(point);
    }
  }

  // The closing seam joins the last points to the first, which the pass above never compared.
  while kept.len() > 4 {
    let count = kept.len();

    if straight(kept[count - 2], kept[count - 1], kept[0]) {
      kept.pop();
    } else if straight(kept[count - 1], kept[0], kept[1]) {
      kept.remove(0);
    } else {
      break;
    }
  }

  kept
}

/// The radius the right-angle corner at `point` takes, coming from `previous` and going to `next`
/// clockwise: a convex corner takes `convex`'s, a concave one `concave`'s, after Blink's
/// `GetRadiiCorner`. Radii run top-left, top-right, bottom-right, bottom-left.
fn corner_radius(
  convex: Sides<SpacePair<f32>>,
  concave: Sides<SpacePair<f32>>,
  previous: Point<f32>,
  point: Point<f32>,
  next: Point<f32>,
) -> SpacePair<f32> {
  let [convex_tl, convex_tr, convex_br, convex_bl] = convex.0;
  let [concave_tl, concave_tr, concave_br, concave_bl] = concave.0;

  if previous.x == point.x {
    if point.y < previous.y {
      if next.x > point.x {
        convex_tl
      } else {
        concave_tr
      }
    } else if next.x > point.x {
      concave_bl
    } else {
      convex_br
    }
  } else if previous.x < point.x {
    if next.y > point.y {
      convex_tr
    } else {
      concave_br
    }
  } else if next.y > point.y {
    concave_tl
  } else {
    convex_bl
  }
}
