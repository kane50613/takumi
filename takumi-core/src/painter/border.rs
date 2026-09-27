//! A box's border, painted after Blink's
//! [`BoxBorderPainter`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/box_border_painter.cc).

use std::cmp::Ordering;

use smallvec::SmallVec;

use super::{FillShape, LayerBounds, PaintDevice, StrokeStyle, box_side::BoxSideRect};
use crate::{
  geometry::{PathCommand, Point, Size},
  layout::border::{BorderProperties, BorderSide, PaintedSide, SideBand},
  style::{Affine, BorderStyle, Color, FillRule},
};

// The curved dash overstroke, `StyledLine`, the square side order and `Miter` follow Blink, under
// the notice in LICENSE-CHROMIUM.

/// How far a curved dashed side overstrokes its centerline, so the ring clip
/// rather than the stroke decides where each dash ends.
const CURVED_DASH_OVERSTROKE: f32 = 2.2;

/// A border on a border box of `size`, ready to paint.
pub struct BoxBorderPainter<'b> {
  border: &'b BorderProperties,
  size: Size<f32>,
}

/// How a border paints as a whole, before any per-side work.
enum BorderPaint {
  /// Every side alike in a style that fills its bands as whole rings: `solid` fills one, `double`
  /// two.
  Rings(PaintedSide),
  /// Every side alike and dashed or dotted round a rounded box: one stroke along the centerline.
  Stroked(PaintedSide),
  /// Each side on its own.
  Sides,
}

/// How one end of a side meets its neighbour, after Blink's `MiterType`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Miter {
  /// The side runs through the corner and one of the two overdraws the other.
  None,
  /// An antialiased diagonal.
  Soft,
  /// An aliased diagonal, so sides of one colour seam without a visible line.
  Hard,
}

impl Miter {
  /// Blink's `ComputeMiter` for where `side` meets `adjacent`, with the `completed` sides painted.
  fn between(
    border: &BorderProperties,
    side: PaintedSide,
    adjacent: BorderSide,
    completed: SideSet,
  ) -> Self {
    let neighbour = border.sides()[adjacent as usize];

    if neighbour.width == 0.0 {
      return Self::None;
    }

    let fills_area = !matches!(
      neighbour.style,
      BorderStyle::Dotted | BorderStyle::Dashed | BorderStyle::Double
    );

    if !completed.includes(adjacent) && fills_area {
      return Self::None;
    }

    let shaded = matches!(
      side.style,
      BorderStyle::Inset | BorderStyle::Outset | BorderStyle::Groove | BorderStyle::Ridge
    );
    let corner = SideSet::default().with(side.side).with(adjacent);
    let unmatched_shades = shaded
      && (corner == SideSet::of_sides([BorderSide::Top, BorderSide::Right])
        || corner == SideSet::of_sides([BorderSide::Bottom, BorderSide::Left]));
    let colors_match = neighbour.is_visible()
      && neighbour.color.0[3] != 0
      && neighbour.color == side.color
      && !unmatched_shades;

    if !colors_match {
      return Self::Soft;
    }

    let dotted_or_dashed = |style| matches!(style, BorderStyle::Dotted | BorderStyle::Dashed);
    let styles_require_miter = side.style == BorderStyle::Double
      || matches!(
        neighbour.style,
        BorderStyle::Double | BorderStyle::Groove | BorderStyle::Ridge
      )
      || dotted_or_dashed(side.style) != dotted_or_dashed(neighbour.style)
      || side.style != neighbour.style;

    if styles_require_miter {
      Self::Hard
    } else {
      Self::None
    }
  }
}

/// A set of sides, after Blink's `BorderEdgeFlags`.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct SideSet(u8);

impl SideSet {
  const ALL: u8 = 0b1111;

  fn of(sides: &[PaintedSide]) -> Self {
    Self::of_sides(sides.iter().map(|side| side.side))
  }

  fn of_sides(sides: impl IntoIterator<Item = BorderSide>) -> Self {
    sides.into_iter().fold(Self::default(), Self::with)
  }

  fn with(self, side: BorderSide) -> Self {
    Self(self.0 | 1 << side as u8)
  }

  fn includes(self, side: BorderSide) -> bool {
    self.0 & (1 << side as u8) != 0
  }

  fn complement(self) -> Self {
    Self(!self.0 & Self::ALL)
  }

  /// Whether the set holds two sides that meet at a corner.
  fn has_adjacent_pair(self) -> bool {
    (self.includes(BorderSide::Top) || self.includes(BorderSide::Bottom))
      && (self.includes(BorderSide::Left) || self.includes(BorderSide::Right))
  }
}

/// The sides that fill one band in one colour, merged so adjacent sides share no seam.
struct SideFill {
  band: SideBand,
  area: Vec<PathCommand>,
}

impl<'b> BoxBorderPainter<'b> {
  /// Prepares `border` for a border box of `size`.
  pub fn new(border: &'b BorderProperties, size: Size<f32>) -> Self {
    Self { border, size }
  }

  /// Paints the border with the border box's top-left at `origin`.
  pub fn paint<D: PaintDevice>(&self, origin: Point<f32>, device: &mut D) {
    if !self.border.has_visible_sides() {
      return;
    }

    let at = Affine::translation(origin.x, origin.y);

    match self.kind() {
      BorderPaint::Sides => self.paint_sides(at, device),
      // A transparent ring is a fill nobody sees, and painting it would only
      // lengthen the output.
      BorderPaint::Rings(side) | BorderPaint::Stroked(side) if side.color.0[3] == 0 => {}
      BorderPaint::Rings(side) => {
        for band in self.border.side_bands(side) {
          device.fill_shape(&self.band_ring(self.border, &band), band.color, at);
        }
      }
      BorderPaint::Stroked(side) => {
        let (centerline, perimeter) = self.centerline_loop(self.border);
        let dash = side.style.dash_pattern(side.width, perimeter, true);

        device.stroke_shape(
          &centerline,
          &StrokeStyle::border(side.color, side.width, dash),
          at,
        );
      }
    }
  }

  /// Whether the border paints in one pass, as Blink's `PaintBorderFastPath` does for a solid or
  /// double border of one colour on all four sides. A rounded dashed or dotted border of one
  /// width strokes its whole centerline, which is what its per-side strokes add up to.
  fn kind(&self) -> BorderPaint {
    let border = self.border;
    let [first, ..] = border.sides();

    if border.has_uniform_visible_color().is_none() {
      return BorderPaint::Sides;
    }

    let all_alike = border
      .sides()
      .iter()
      .all(|side| side.is_visible() && side.style == first.style);

    match first.style {
      BorderStyle::Solid | BorderStyle::Double if all_alike => BorderPaint::Rings(first),
      BorderStyle::Dashed | BorderStyle::Dotted
        if !border.is_zero() && border.is_uniform_all_sides_style(first.style) =>
      {
        BorderPaint::Stroked(first)
      }
      _ => BorderPaint::Sides,
    }
  }

  /// Paints each side on its own. On a rounded or collapsed border, sides meet along the diagonal
  /// from the outer to the inner corner, and sides that fill a band in the same colour merge into
  /// one fill. A rounded border fills each band's ring clipped to its sides' regions.
  fn paint_sides<D: PaintDevice>(&self, at: Affine, device: &mut D) {
    let mut border = *self.border;

    border.width = border.visible_side_widths();

    let rounded = !border.is_zero();

    if !rounded && !border.collapsed {
      return self.paint_square_sides(&border, at, device);
    }

    let mut fills: SmallVec<[SideFill; 4]> = SmallVec::new();

    for side in border.painted_sides() {
      if matches!(side.style, BorderStyle::Dashed | BorderStyle::Dotted) {
        self.paint_side_pattern(&border, side, at, device);
        continue;
      }

      for band in border.side_bands(side) {
        let area = if rounded {
          self.side_region(&border, side.side)
        } else {
          self.band_region(&border, side.side, &band)
        };

        match fills.iter_mut().find(|fill| fill.band == band) {
          Some(fill) => fill.area.extend(area),
          None => fills.push(SideFill { band, area }),
        }
      }
    }

    for fill in fills {
      let area = FillShape::Path {
        commands: fill.area,
        rule: FillRule::NonZero,
      };

      if rounded {
        device.push_clip(&area, at);
        device.fill_shape(&self.band_ring(&border, &fill.band), fill.band.color, at);
        device.pop_clip();
      } else {
        device.fill_shape(&area, fill.band.color, at);
      }
    }
  }

  /// Paints a square border side by side, as Blink's `PaintOpacityGroup` does: sides sorted by
  /// alpha, style and position, each alpha in its own nested layer.
  fn paint_square_sides<D: PaintDevice>(
    &self,
    border: &BorderProperties,
    at: Affine,
    device: &mut D,
  ) {
    let mut sides: SmallVec<[PaintedSide; 4]> = border.painted_sides().collect();

    if sides.is_empty() {
      return;
    }

    if let Some(color) = border.has_uniform_visible_color()
      && color.0[3] != u8::MAX
      && sides.iter().all(|side| side.style == BorderStyle::Solid)
    {
      let rects = sides
        .iter()
        .map(|side| self.side_rect(border, side.side).corners());

      device.fill_shape(&FillShape::polygons(rects), color, at);
      return;
    }

    sides.sort_by_key(|side| {
      let style = match side.style {
        BorderStyle::None | BorderStyle::Hidden => 0,
        BorderStyle::Dotted | BorderStyle::Dashed | BorderStyle::Double => 1,
        BorderStyle::Inset | BorderStyle::Outset | BorderStyle::Groove | BorderStyle::Ridge => 2,
        BorderStyle::Solid => 3,
      };
      let position = match side.side {
        BorderSide::Top => 0,
        BorderSide::Bottom => 1,
        BorderSide::Right => 2,
        BorderSide::Left => 3,
      };

      (side.color.0[3], style, position)
    });

    let groups: SmallVec<[&[PaintedSide]; 4]> = sides
      .chunk_by(|a, b| a.color.0[3] == b.color.0[3])
      .collect();
    let visible = SideSet::of(&sides);

    self.paint_opacity_groups(border, &groups, visible, 1.0, at, device);
  }

  /// Paints the most opaque of `groups` over the rest inside ancestor layers `opacity` opaque, and
  /// returns the sides done, counting the `visible` sides' complement as done.
  fn paint_opacity_groups<D: PaintDevice>(
    &self,
    border: &BorderProperties,
    groups: &[&[PaintedSide]],
    visible: SideSet,
    opacity: f32,
    at: Affine,
    device: &mut D,
  ) -> SideSet {
    let Some((&group, rest)) = groups.split_last() else {
      return visible.complement();
    };
    let alpha = f32::from(group[0].color.0[3]) / f32::from(u8::MAX);
    let layered = alpha != 1.0 && (SideSet::of(group).has_adjacent_pair() || !rest.is_empty());
    let (paint_alpha, opacity) = if layered {
      device.begin_layer(
        alpha / opacity,
        Some(LayerBounds {
          size: self.size,
          transform: at,
        }),
      );
      (1.0, alpha)
    } else {
      (alpha / opacity, opacity)
    };
    let mut completed = self.paint_opacity_groups(border, rest, visible, opacity, at, device);

    for &side in group {
      let mut color = side.color;

      color.0[3] = (paint_alpha * f32::from(u8::MAX)).round() as u8;
      self.paint_square_side(border, side, color, completed, at, device);
      completed = completed.with(side.side);
    }

    if layered {
      device.end_layer();
    }

    completed
  }

  /// Paints one side of a square border in `color`, as Blink's `PaintOneBorderSide` does for a
  /// straight side, with the `completed` sides already painted.
  fn paint_square_side<D: PaintDevice>(
    &self,
    border: &BorderProperties,
    side: PaintedSide,
    color: Color,
    completed: SideSet,
    at: Affine,
    device: &mut D,
  ) {
    let adjacent = side.side.adjacent();
    let miters = adjacent.map(|adjacent| Miter::between(border, side, adjacent, completed));
    let clipped = miters.contains(&Miter::Hard)
      || (miters != [Miter::None; 2]
        && matches!(side.style, BorderStyle::Dashed | BorderStyle::Dotted));
    let clips = if clipped {
      self.push_miter_clips(border, side.side, miters, at, device)
    } else {
      0
    };
    let widths = [0, 1].map(|index| {
      if clipped || miters[index] == Miter::None {
        0
      } else {
        adjacent[index].of(border.width).round() as i32
      }
    });

    self
      .side_rect(border, side.side)
      .paint(color, side.style, widths, at, device);

    for _ in 0..clips {
      device.pop_clip();
    }
  }

  /// Clips to the part of `side` between its `miters`, as Blink's `ClipBorderSidePolygon` does
  /// for a square border, and returns how many clips it pushed.
  fn push_miter_clips<D: PaintDevice>(
    &self,
    border: &BorderProperties,
    side: BorderSide,
    [first, second]: [Miter; 2],
    at: Affine,
    device: &mut D,
  ) -> usize {
    const EXTENSION: f32 = 0.1;

    let Size { width, height } = self.size;
    let point = |x, y| Point { x, y };
    let (left, top) = (border.width.left, border.width.top);
    let right = (width - border.width.right).max(left);
    let bottom = (height - border.width.bottom).max(top);
    let outer = [
      point(0.0, 0.0),
      point(width, 0.0),
      point(width, height),
      point(0.0, height),
    ];
    let inner = [
      point(left, top),
      point(right, top),
      point(right, bottom),
      point(left, bottom),
    ];
    let (quad, extension, [first, second]) = match side {
      BorderSide::Top => (
        [outer[0], inner[0], inner[1], outer[1]],
        point(-EXTENSION, 0.0),
        [first, second],
      ),
      BorderSide::Right => (
        [outer[1], inner[1], inner[2], outer[2]],
        point(0.0, -EXTENSION),
        [first, second],
      ),
      BorderSide::Bottom => (
        [outer[2], inner[2], inner[3], outer[3]],
        point(EXTENSION, 0.0),
        [second, first],
      ),
      BorderSide::Left => (
        [outer[3], inner[3], inner[0], outer[0]],
        point(0.0, EXTENSION),
        [second, first],
      ),
    };
    let [bound_start, bound_end] = match side {
      BorderSide::Top | BorderSide::Bottom => {
        [point(quad[0].x, quad[1].y), point(quad[3].x, quad[2].y)]
      }
      BorderSide::Left | BorderSide::Right => {
        [point(quad[1].x, quad[0].y), point(quad[2].x, quad[3].y)]
      }
    };
    let mut clips: SmallVec<[([Point<f32>; 4], Miter); 2]> = SmallVec::new();

    if first == second {
      clips.push((quad, first));
    } else {
      if first != Miter::None {
        clips.push((
          [quad[0] + extension, quad[1] + extension, bound_end, quad[3]],
          first,
        ));
      }
      if second != Miter::None {
        clips.push((
          [
            quad[0],
            bound_start,
            quad[2] - extension,
            quad[3] - extension,
          ],
          second,
        ));
      }
    }

    for &(polygon, miter) in &clips {
      let shape = FillShape::polygons([polygon]);

      if miter == Miter::Hard {
        device.push_aliased_clip(&shape, at);
      } else {
        device.push_clip(&shape, at);
      }
    }

    clips.len()
  }

  /// The rectangle `side` fills on a square border, across the whole border box.
  fn side_rect(&self, border: &BorderProperties, side: BorderSide) -> BoxSideRect {
    let Size { width, height } = self.size;
    let thickness = side.of(border.width);
    let (x1, y1, x2, y2) = match side {
      BorderSide::Top => (0.0, 0.0, width, thickness),
      BorderSide::Bottom => (0.0, height - thickness, width, height),
      BorderSide::Left => (0.0, 0.0, thickness, height),
      BorderSide::Right => (width - thickness, 0.0, width, height),
    };

    BoxSideRect {
      side,
      x1,
      y1,
      x2,
      y2,
    }
  }

  /// Strokes a dashed or dotted side along its centerline, clipped to its corner mitres when a
  /// neighbour has width, and to the ring when the border is rounded.
  fn paint_side_pattern<D: PaintDevice>(
    &self,
    border: &BorderProperties,
    side: PaintedSide,
    at: Affine,
    device: &mut D,
  ) {
    let rounded = !border.is_zero();
    let curved = rounded && border.inner_edge_arcs(side.side, self.size);
    let widest_neighbour = side
      .side
      .adjacent()
      .iter()
      .map(|adjacent| adjacent.of(border.width))
      .fold(0.0, f32::max);
    let ring = rounded.then(|| FillShape::border_ring(border, self.size));
    let mitre = (curved || widest_neighbour > 0.0).then(|| FillShape::Path {
      commands: self.side_region(border, side.side),
      rule: FillRule::NonZero,
    });
    let clips = [&ring, &mitre].into_iter().flatten();

    for clip in clips.clone() {
      device.push_clip(clip, at);
    }

    if curved {
      let (centerline, perimeter) = self.centerline_loop(border);
      let dash = side.style.dash_pattern(side.width, perimeter, true);
      let width = if side.style == BorderStyle::Dashed {
        side.width.max(widest_neighbour) * CURVED_DASH_OVERSTROKE
      } else {
        side.width
      };

      device.stroke_shape(
        &centerline,
        &StrokeStyle::border(side.color, width, dash),
        at,
      );
    } else {
      let [start, end] = self.side_line(side);

      StyledLine::box_side(start, end, side.width, side.style, side.color).paint(at, device);
    }

    for _ in clips {
      device.pop_clip();
    }
  }

  /// The ring one band of the border fills.
  fn band_ring(&self, border: &BorderProperties, band: &SideBand) -> FillShape {
    let (strip, size, offset) = self.band_strip(border, band);
    let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

    strip.append_border_ring_commands_at(&mut commands, size, offset);

    FillShape::Path {
      commands,
      rule: FillRule::EvenOdd,
    }
  }

  /// The part of one band of `side` between its corner mitres.
  fn band_region(
    &self,
    border: &BorderProperties,
    side: BorderSide,
    band: &SideBand,
  ) -> Vec<PathCommand> {
    let (strip, size, offset) = self.band_strip(border, band);
    let mut commands = Vec::with_capacity(5);

    strip.append_side_clip_polygon_commands_at(side, &mut commands, size, offset);

    commands
  }

  /// `border` narrowed to one band, with the band's outer size and offset in the border box.
  fn band_strip(
    &self,
    border: &BorderProperties,
    band: &SideBand,
  ) -> (BorderProperties, Size<f32>, Point<f32>) {
    let mut strip = *border;

    strip.width = band.width;
    strip.expand_by(band.inset.map(|value| -value));

    (strip, self.size.inset(band.inset), band.inset.top_left())
  }

  /// The region `side` owns, from its outer corners to the padding edge.
  fn side_region(&self, border: &BorderProperties, side: BorderSide) -> Vec<PathCommand> {
    let mut commands = Vec::with_capacity(5);

    border.append_side_clip_polygon_commands_at(side, &mut commands, self.size, Point::ZERO);

    commands
  }

  /// The closed path through the middle of every side, and its length.
  fn centerline_loop(&self, border: &BorderProperties) -> (FillShape, f32) {
    let half = border.width.map(|width| width / 2.0);
    let mut center = *border;
    let size = self.size.inset(half);
    let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);

    center.expand_by(half.map(|value| -value));
    center.append_mask_commands(&mut commands, size, half.top_left());

    let perimeter = center.approximate_rounded_rect_perimeter(size);

    (
      FillShape::Path {
        commands,
        rule: FillRule::NonZero,
      },
      perimeter,
    )
  }

  /// The ends of the line through the middle of `side`, across the whole border box.
  fn side_line(&self, side: PaintedSide) -> [Point<f32>; 2] {
    let Size { width, height } = self.size;
    let half = side.width / 2.0;
    let point = |x, y| Point { x, y };

    match side.side {
      BorderSide::Top => [point(0.0, half), point(width, half)],
      BorderSide::Bottom => [point(0.0, height - half), point(width, height - half)],
      BorderSide::Left => [point(half, 0.0), point(half, height)],
      BorderSide::Right => [point(width - half, 0.0), point(width - half, height)],
    }
  }
}

/// A dashed or dotted line along one axis, after Blink's `DrawLineWithStyle`: its dashes spread
/// over the whole line, and round dots stop half a dot short of each end so the end dots stay
/// inside.
pub(super) struct StyledLine {
  line: FillShape,
  stroke: StrokeStyle,
  /// End dots filled on their own, each a top-left and a size.
  dots: SmallVec<[(Point<f32>, Size<f32>); 2]>,
}

impl StyledLine {
  /// The line from `start` to `end` in `style`, `width` thick.
  pub(super) fn new(
    start: Point<f32>,
    end: Point<f32>,
    width: f32,
    style: BorderStyle,
    color: Color,
  ) -> Self {
    let length = (end.x - start.x).abs() + (end.y - start.y).abs();
    let dash = style.dash_pattern(width, length, false);
    let [start, end] = if dash.is_some_and(|dash| dash.round_cap) {
      let half = width / 2.0;
      let step = |from: f32, to: f32| match to.total_cmp(&from) {
        Ordering::Greater => half,
        Ordering::Less => -half,
        Ordering::Equal => 0.0,
      };
      let (dx, dy) = (step(start.x, end.x), step(start.y, end.y));

      [
        Point {
          x: start.x + dx,
          y: start.y + dy,
        },
        Point {
          x: end.x - dx,
          y: end.y - dy,
        },
      ]
    } else {
      [start, end]
    };

    Self {
      line: FillShape::Path {
        commands: vec![PathCommand::MoveTo(start), PathCommand::LineTo(end)],
        rule: FillRule::NonZero,
      },
      stroke: StrokeStyle::border(color, width, dash),
      dots: SmallVec::new(),
    }
  }

  /// A left-to-right or top-to-bottom box side, as Blink's `DrawLineWithStyle` draws it.
  pub(super) fn box_side(
    start: Point<f32>,
    end: Point<f32>,
    width: f32,
    style: BorderStyle,
    color: Color,
  ) -> Self {
    let dot = width.round();

    if style != BorderStyle::Dotted || dot > 3.0 || dot < 1.0 {
      return Self::new(start, end, width, style, color);
    }

    let vertical = start.x == end.x;
    let length = ((end.x - start.x) + (end.y - start.y)).round() as i32;
    let ends = EndDots::of(dot as i32, length);
    let mut line = Self::new(start, end, width, style, color);
    let along = |point: Point<f32>, by: f32| {
      if vertical {
        Point {
          x: point.x,
          y: point.y + by,
        }
      } else {
        Point {
          x: point.x + by,
          y: point.y,
        }
      }
    };
    let dot_rect = |from: Point<f32>, length: f32| {
      let half = dot / 2.0;

      if vertical {
        (
          Point {
            x: from.x - half,
            y: from.y,
          },
          Size {
            width: dot,
            height: length,
          },
        )
      } else {
        (
          Point {
            x: from.x,
            y: from.y - half,
          },
          Size {
            width: length,
            height: dot,
          },
        )
      }
    };
    let (mut start, mut end) = (start, end);

    if let Some(growth) = ends.start {
      line.dots.push(dot_rect(start, dot + growth as f32));
      start = along(start, 2.0 * dot + ends.start_offset as f32);
    }
    if let Some(growth) = ends.end {
      let size = dot + growth as f32;

      line.dots.push(dot_rect(along(end, -size), size));
      end = along(end, -(size + 1.0));
    }
    line.line = FillShape::Path {
      commands: vec![PathCommand::MoveTo(start), PathCommand::LineTo(end)],
      rule: FillRule::NonZero,
    };
    line
  }

  /// Fills the end dots and strokes the line under `at`.
  pub(super) fn paint<D: PaintDevice>(&self, at: Affine, device: &mut D) {
    for &(origin, size) in &self.dots {
      device.fill_shape(
        &FillShape::Rect(size),
        self.stroke.color,
        at * Affine::translation(origin.x, origin.y),
      );
    }
    device.stroke_shape(&self.line, &self.stroke, at);
  }
}

/// Blink's `EnforceDotsAtEndpoints` for a `dot`-wide dotted line `length` long.
struct EndDots {
  /// The start dot's growth, when there is one.
  start: Option<i32>,
  /// How far the first gap after the start dot shrinks or grows.
  start_offset: i32,
  /// The end dot's growth, when there is one.
  end: Option<i32>,
}

impl EndDots {
  fn of(dot: i32, length: i32) -> Self {
    let (mod_4, mod_6) = (length.rem_euclid(4), length.rem_euclid(6));
    let mut ends = Self {
      start: None,
      start_offset: 0,
      end: None,
    };

    if (dot == 1 && length % 2 == 0) || (dot == 3 && mod_6 == 0) {
      ends.start = Some(1);
      ends.start_offset = 1;
    }
    if (dot == 2 && (mod_4 == 0 || mod_4 == 1)) || (dot == 3 && (mod_6 == 1 || mod_6 == 2)) {
      ends.start = Some(ends.start.unwrap_or(0));
      ends.start_offset = -1;
    }
    if (dot == 2 && mod_4 == 0) || (dot == 3 && mod_6 == 1) {
      ends.end = Some(0);
    }
    if (dot == 2 && mod_4 == 3) || (dot == 3 && (mod_6 == 4 || mod_6 == 5)) {
      ends.start = Some(ends.start.unwrap_or(0));
      ends.start_offset = 1;
    }
    if dot == 3 && mod_6 == 5 {
      ends.end = Some(0);
    } else if dot == 3 && mod_6 == 0 {
      ends.end = Some(1);
    }
    ends
  }
}
