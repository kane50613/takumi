//! Outlines of inline elements: each element's line fragments, grouped into islands.

use std::collections::HashMap;

use crate::{
  context::RenderContext,
  geometry::{PathBuilder, PathCommand, Point, Size},
  layout::corner_shape::KAPPA,
  layout_unit::UnitRect,
  sort_key::sort_by_key,
  style::{BorderStyle, Color, Sides, SpacePair},
};

/// How an inline element draws its outline.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InlineOutline {
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
    let width = style
      .misc3_data
      .outline_width
      .to_used_px(&context.sizing)
      .max(0.0);
    let color = style
      .misc3_data
      .outline_color
      .resolve(context.current_color);

    (width > 0.0 && style.misc_data.outline_style.is_rendered() && color.0[3] != 0).then(|| Self {
      width,
      offset: style
        .misc3_data
        .outline_offset
        .to_border_px(&context.sizing, 0.0),
      color,
      style: style.misc_data.outline_style,
    })
  }

  /// How far the outline's outer edge reaches past the element's border box.
  pub(crate) fn reach(self) -> f32 {
    self.offset + self.width
  }

  /// The width and offset it paints with, in whole pixels as Blink's `OutlineInfo` holds them.
  pub(crate) fn painted(self) -> (f32, f32) {
    (self.width.trunc(), self.offset.trunc())
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
  /// Whether the two rects meet once each grows as its outline paints, so one contour can trace
  /// them both.
  pub(super) fn meets(self, other: Self) -> bool {
    let (width, offset) = self.outline.painted();
    let [a, b] = [self, other].map(|rect| rect.grown(offset, width));

    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
  }

  /// The pixel-snapped rect grown by `offset`, no further in than half its size, then by
  /// `outset`, as Blink's `ComputeRightAnglePath` grows it: left, top, right, bottom.
  pub(super) fn grown(self, offset: f32, outset: f32) -> [f32; 4] {
    let [left, top, right, bottom] = self.pixel_snapped();
    let horizontal = offset.max(-((right - left) / 2.0).trunc()) + outset;
    let vertical = offset.max(-((bottom - top) / 2.0).trunc()) + outset;

    [
      left - horizontal,
      top - vertical,
      right + horizontal,
      bottom + vertical,
    ]
  }

  /// The rect as Blink's `ToPixelSnappedRect` snaps it: left, top, right, bottom.
  pub(crate) fn pixel_snapped(self) -> [f32; 4] {
    let rect = UnitRect::nearest(
      Point {
        x: self.x,
        y: self.y,
      },
      Size {
        width: self.width,
        height: self.height,
      },
    )
    .pixel_snapped()
    .to_rect();

    [rect.left, rect.top, rect.right, rect.bottom]
  }
}

/// Fragments of one element's outline that touch from line to line, stroked as one contour.
pub(crate) struct OutlineIsland {
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
        && previous.meets(rect)
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
  pub(crate) fn lone_rect(&self) -> Option<InlineOutlineRect> {
    self.lone.then(|| self.rects[0])
  }

  /// The element's outline and opacity.
  pub fn outline(&self) -> (InlineOutline, f32) {
    let rect = self.rects[0];

    (rect.outline, rect.opacity)
  }

  /// Blink's `ComputeRightAnglePath`: the pixel-snapped rects, each grown by `offset` (no further
  /// in than half its size) and `outset`, united.
  pub(crate) fn right_angle_path(&self, offset: f32, outset: f32) -> Option<RightAngleContour> {
    let grown: Vec<[f32; 4]> = self
      .rects
      .iter()
      .map(|rect| rect.grown(offset, outset))
      .filter(|[left, top, right, bottom]| left < right && top < bottom)
      .collect();

    RightAngleContour::of(union_outline(&grown))
  }

  /// The element's corner radii, resolved against the island's first fragment.
  pub fn radius(&self) -> Sides<SpacePair<f32>> {
    self.rects[0].radius
  }
}

/// The clockwise outline of rects that, band by band down the page, cover one run each, as an
/// `SkRegion` traces their union.
fn union_outline(rects: &[[f32; 4]]) -> Vec<Point<f32>> {
  let mut edges: Vec<f32> = rects.iter().flat_map(|rect| [rect[1], rect[3]]).collect();

  sort_by_key(&mut edges, |&edge| edge);
  edges.dedup();

  let bands: Vec<(f32, f32, f32, f32)> = edges
    .windows(2)
    .filter_map(|band| {
      let (top, bottom) = (band[0], band[1]);
      let covering = rects
        .iter()
        .filter(|rect| rect[1] < bottom && rect[3] > top);
      let left = covering.clone().map(|rect| rect[0]).reduce(f32::min)?;
      let right = covering.map(|rect| rect[2]).reduce(f32::max)?;

      Some((left, top, right, bottom))
    })
    .collect();
  let Some(&(left, top, ..)) = bands.first() else {
    return Vec::new();
  };
  let point = |x, y| Point { x, y };
  let mut corners = vec![point(left, top)];

  for &(_, top, right, bottom) in &bands {
    corners.extend([point(right, top), point(right, bottom)]);
  }
  for &(left, top, _, bottom) in bands.iter().rev() {
    corners.extend([point(left, bottom), point(left, top)]);
  }
  corners
}

/// A clockwise contour turning a right angle at every corner, as Blink's `IterateRightAnglePath`
/// reads it. Follows Blink's `outline_painter.cc` under the notice in LICENSE-CHROMIUM.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RightAngleContour {
  corners: Vec<Point<f32>>,
}

impl RightAngleContour {
  /// The contour through `corners`, or `None` once it encloses nothing.
  pub fn of(corners: Vec<Point<f32>>) -> Option<Self> {
    let corners = right_angle_corners(corners);

    (corners.len() >= 4).then_some(Self { corners })
  }

  fn at(&self, index: usize) -> Point<f32> {
    self.corners[index % self.corners.len()]
  }

  fn before(&self, index: usize) -> Point<f32> {
    self.at(index + self.corners.len() - 1)
  }

  /// Each edge, from a corner to the next.
  pub fn lines(&self) -> impl Iterator<Item = (Point<f32>, Point<f32>)> + '_ {
    (0..self.corners.len()).map(|index| (self.at(index), self.at(index + 1)))
  }

  /// Blink's `ShrinkRightAnglePath`.
  pub(crate) fn shrunk(&self, inset: f32) -> Self {
    let corners = (0..self.corners.len())
      .map(|index| {
        let (previous, corner, next) = (self.before(index), self.at(index), self.at(index + 1));
        let (x, y) = if previous.x == corner.x {
          match (previous.y < corner.y, corner.x < next.x) {
            (true, true) => (-inset, inset),
            (true, false) => (-inset, -inset),
            (false, true) => (inset, inset),
            (false, false) => (inset, -inset),
          }
        } else {
          match (previous.x < corner.x, corner.y < next.y) {
            (true, true) => (-inset, inset),
            (true, false) => (inset, inset),
            (false, true) => (-inset, -inset),
            (false, false) => (inset, -inset),
          }
        };

        Point {
          x: corner.x + x,
          y: corner.y + y,
        }
      })
      .collect();

    Self { corners }
  }

  /// The contour as a path.
  pub fn path(&self) -> Vec<PathCommand> {
    let mut path = Vec::with_capacity(self.corners.len() + 2);

    path.move_to((self.corners[0].x, self.corners[0].y));
    for corner in &self.corners[1..] {
      path.line_to((corner.x, corner.y));
    }
    path.close();
    path
  }

  /// The top-left and bottom-right corners of the rect around the contour.
  pub fn bounds(&self) -> (Point<f32>, Point<f32>) {
    self
      .corners
      .iter()
      .fold((self.corners[0], self.corners[0]), |(low, high), corner| {
        (
          Point {
            x: low.x.min(corner.x),
            y: low.y.min(corner.y),
          },
          Point {
            x: high.x.max(corner.x),
            y: high.y.max(corner.y),
          },
        )
      })
  }

  /// Each edge shortened by its corners' radii, Blink's `AdjustLineBetweenCorners`.
  fn rounded_lines(
    &self,
    convex: Sides<SpacePair<f32>>,
    concave: Sides<SpacePair<f32>>,
  ) -> Vec<(Point<f32>, Point<f32>)> {
    let radius = |index: usize| {
      corner_radius(
        convex,
        concave,
        self.before(index),
        self.at(index),
        self.at(index + 1),
      )
    };

    (0..self.corners.len())
      .map(|index| {
        let (start, end) = (self.at(index), self.at(index + 1));
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
      .collect()
  }

  /// Blink's `AddCornerRadiiToPath`, a quarter ellipse at each corner.
  pub fn rounded(
    &self,
    convex: Sides<SpacePair<f32>>,
    concave: Sides<SpacePair<f32>>,
  ) -> Vec<PathCommand> {
    let lines = self.rounded_lines(convex, concave);
    let count = lines.len();
    let mut path = Vec::with_capacity(count * 2 + 2);
    let mut current = lines[count - 1].1;

    path.move_to((current.x, current.y));
    for (index, &(start, end)) in lines.iter().enumerate() {
      path.push(arc(current, self.at(index), start));
      path.line_to((end.x, end.y));
      current = end;
    }
    path.close();
    path
  }

  /// Blink's `RoundedEdgePathIterator`: each edge's stroke through its whole corner arcs, the
  /// ends run on by `extension` so they fill the corner's mitre.
  pub(crate) fn rounded_edges(
    &self,
    convex: Sides<SpacePair<f32>>,
    concave: Sides<SpacePair<f32>>,
    extension: f32,
  ) -> Vec<Vec<PathCommand>> {
    let lines = self.rounded_lines(convex, concave);
    let count = lines.len();

    (0..count)
      .map(|index| {
        let (line_start, line_end) = lines[index];
        let arc_start = lines[(index + count - 1) % count].1;
        let arc_end = lines[(index + 1) % count].0;
        let (first_corner, second_corner) = (self.at(index), self.at(index + 1));
        let mut path = Vec::with_capacity(6);

        if arc_start == line_start {
          let start = extended(line_start, second_corner, extension);

          path.move_to((start.x, start.y));
        } else {
          let start = extended(arc_start, first_corner, extension);

          path.move_to((start.x, start.y));
          path.line_to((arc_start.x, arc_start.y));
          path.push(arc(arc_start, first_corner, line_start));
        }
        if line_end == arc_end {
          let end = extended(line_end, first_corner, extension);

          path.line_to((end.x, end.y));
        } else {
          let end = extended(arc_end, second_corner, extension);

          path.line_to((line_end.x, line_end.y));
          path.push(arc(line_end, second_corner, arc_end));
          path.line_to((end.x, end.y));
        }
        path
      })
      .collect()
  }
}

/// The quarter ellipse from `from` to `to` about the right-angle `corner`.
fn arc(from: Point<f32>, corner: Point<f32>, to: Point<f32>) -> PathCommand {
  PathCommand::CubicTo(
    Point {
      x: from.x + (corner.x - from.x) * KAPPA,
      y: from.y + (corner.y - from.y) * KAPPA,
    },
    Point {
      x: to.x + (corner.x - to.x) * KAPPA,
      y: to.y + (corner.y - to.y) * KAPPA,
    },
    to,
  )
}

/// `point` moved `by` away from `other`, along the axis they share, Blink's `ExtendLineAtEndpoint`.
fn extended(point: Point<f32>, other: Point<f32>, by: f32) -> Point<f32> {
  if point.x == other.x {
    Point {
      x: point.x,
      y: point.y + if point.y < other.y { -by } else { by },
    }
  } else {
    Point {
      x: point.x + if point.x < other.x { -by } else { by },
      y: point.y,
    }
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
