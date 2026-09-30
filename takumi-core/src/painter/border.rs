//! A box's border, painted after Blink's
//! [`BoxBorderPainter`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/box_border_painter.cc).

use std::cmp::Ordering;

use smallvec::SmallVec;

use super::{
  FillShape, LayerBounds, PaintDevice, StrokeStyle, box_side::BoxSideRect, outline::path_length,
};
use crate::{
  geometry::{PathCommand, Point, Rect, Size},
  layout::border::{BorderProperties, BorderSide, PaintedSide, SideBand},
  style::{Affine, BorderStyle, Color, FillRule, Sides, SpacePair},
};

// `BorderShape`, `Miter`, `StyledLine` and the side order follow Blink, under the notice in
// LICENSE-CHROMIUM.

/// How far a curved dashed side overstrokes its centerline, so the clips rather than the stroke
/// decide where each dash ends: Blink's `kThicknessMultiplier`.
const CURVED_DASH_OVERSTROKE: f32 = 2.2;

/// A border on a border box of `size`, ready to paint.
pub struct BoxBorderPainter<'b> {
  border: &'b BorderProperties,
  size: Size<f32>,
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

    if !colors_match_at_corner(border, side, adjacent) {
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

/// Blink's `ColorsMatchAtCorner`: whether `side` and `adjacent` paint one colour where they meet.
fn colors_match_at_corner(
  border: &BorderProperties,
  side: PaintedSide,
  adjacent: BorderSide,
) -> bool {
  let neighbour = border.sides()[adjacent as usize];
  let shaded = matches!(
    side.style,
    BorderStyle::Inset | BorderStyle::Outset | BorderStyle::Groove | BorderStyle::Ridge
  );
  let corner = SideSet::default().with(side.side).with(adjacent);
  let unmatched_shades = shaded
    && (corner == SideSet::of_sides([BorderSide::Top, BorderSide::Right])
      || corner == SideSet::of_sides([BorderSide::Bottom, BorderSide::Left]));

  neighbour.is_visible()
    && neighbour.color.0[3] != 0
    && neighbour.color == side.color
    && !unmatched_shades
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
    if !self.border.has_visible_sides() || self.size.width <= 0.0 || self.size.height <= 0.0 {
      return;
    }

    let at = Affine::translation(origin.x, origin.y);
    let mut border = *self.border;

    border.width = border.visible_side_widths();

    if border.collapsed {
      self.paint_collapsed(&border, at, device);
    } else {
      BorderShape::new(border, self.size).paint(at, device);
    }
  }

  /// Paints a collapsed table border square, whatever its radii, with each side meeting its
  /// neighbours along the wider one's edge and sides that fill a band in the same colour merged
  /// into one fill.
  fn paint_collapsed<D: PaintDevice>(&self, border: &BorderProperties, at: Affine, device: &mut D) {
    let mut fills: SmallVec<[SideFill; 4]> = SmallVec::new();

    for side in border.painted_sides() {
      if matches!(side.style, BorderStyle::Dashed | BorderStyle::Dotted) {
        self.paint_collapsed_pattern(border, side, at, device);
        continue;
      }

      for band in border.side_bands(side) {
        let area = self.band_region(border, side.side, &band);

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

      device.fill_shape(&area, fill.band.color, at);
    }
  }

  /// Strokes a dashed or dotted collapsed side along its centerline, clipped to its own region
  /// when a neighbour has width.
  fn paint_collapsed_pattern<D: PaintDevice>(
    &self,
    border: &BorderProperties,
    side: PaintedSide,
    at: Affine,
    device: &mut D,
  ) {
    let has_neighbour = side
      .side
      .adjacent()
      .iter()
      .any(|adjacent| adjacent.of(border.width) > 0.0);

    if has_neighbour {
      let mut commands = Vec::with_capacity(5);

      border.append_side_polygon_commands_at(side.side, &mut commands, self.size, Point::ZERO);
      device.push_clip(
        &FillShape::Path {
          commands,
          rule: FillRule::NonZero,
        },
        at,
      );
    }

    let [start, end] = self.side_line(side);

    StyledLine::box_side(start, end, side.width, side.style, side.color).paint(at, device);

    if has_neighbour {
      device.pop_clip();
    }
  }

  /// The part of one band of `side` a collapsed border fills.
  fn band_region(
    &self,
    border: &BorderProperties,
    side: BorderSide,
    band: &SideBand,
  ) -> Vec<PathCommand> {
    let mut strip = *border;
    let mut commands = Vec::with_capacity(5);

    strip.width = band.width;
    strip.append_side_polygon_commands_at(
      side,
      &mut commands,
      self.size.inset(band.inset),
      band.inset.top_left(),
    );

    commands
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

/// A border with its sides' used widths, and the outer and inner edges Blink's
/// `BoxBorderPainter` keeps as `outer_` and `inner_`.
///
/// Approximate: a corner shape other than `round`, `squircle` or `square` clips its sides with
/// the polygon Blink uses for round corners, where Blink's `ClipBorderSidePolygonCloseToEdges`
/// follows the corner curve.
struct BorderShape {
  border: BorderProperties,
  size: Size<f32>,
  /// The outer corner radii, scaled to fit the box.
  outer_radii: Sides<SpacePair<f32>>,
  /// The padding box's top-left.
  inner_origin: Point<f32>,
  inner_size: Size<f32>,
  /// The outer radii shrunk by the border widths, not scaled again.
  inner_radii: Sides<SpacePair<f32>>,
}

impl BorderShape {
  fn new(border: BorderProperties, size: Size<f32>) -> Self {
    let outer_radii = border.scaled_corner_radii(size);

    Self {
      border,
      size,
      outer_radii,
      inner_origin: border.width.top_left(),
      inner_size: size.inset(border.width),
      inner_radii: shrink_radii(outer_radii, border.width),
    }
  }

  /// Paints every side, as Blink's `BoxBorderPainter::Paint` does.
  fn paint<D: PaintDevice>(&self, at: Affine, device: &mut D) {
    let mut sides: SmallVec<[PaintedSide; 4]> = self.border.painted_sides().collect();

    if sides.is_empty() || self.paint_fast_path(&sides, at, device) {
      return;
    }

    let rounded = self.is_rounded();
    let clip_out_inner = rounded
      && self.inner_renderable()
      && self.inner_size.width > 0.0
      && self.inner_size.height > 0.0;

    if rounded {
      device.push_clip(&self.outer_rrect(), at);
    }
    if clip_out_inner {
      device.push_clip_out(&self.inner_rrect(), at);
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

    self.paint_opacity_groups(&groups, SideSet::of(&sides), 1.0, at, device);

    if clip_out_inner {
      device.pop_clip();
    }
    if rounded {
      device.pop_clip();
    }
  }

  /// Blink's `PaintBorderFastPath`: a solid or double border of one colour on all four sides
  /// fills its rings, and a translucent square solid border of one colour fills its sides at once.
  fn paint_fast_path<D: PaintDevice>(
    &self,
    sides: &[PaintedSide],
    at: Affine,
    device: &mut D,
  ) -> bool {
    let Some(color) = self.border.has_uniform_visible_color() else {
      return false;
    };
    let style = sides[0].style;

    if !sides.iter().all(|side| side.style == style)
      || !self.inner_renderable()
      || !self.inner_round()
    {
      return false;
    }

    let all_sides = sides.len() == 4;

    match style {
      BorderStyle::Solid if all_sides => {
        device.fill_shape(&ring(self.outer_rrect(), self.inner_rrect()), color, at);
      }
      BorderStyle::Double if all_sides => {
        let stripe = |fraction: f32| self.border.width.map(|width| (width * fraction).round());

        device.fill_shape(
          &ring(self.outer_rrect(), self.inset_rrect(stripe(1.0 / 3.0))),
          color,
          at,
        );
        device.fill_shape(
          &ring(self.inset_rrect(stripe(2.0 / 3.0)), self.inner_rrect()),
          color,
          at,
        );
      }
      BorderStyle::Solid if !self.is_rounded() && color.0[3] != u8::MAX => {
        let rects = sides.iter().map(|side| self.side_rect(side.side).corners());

        device.fill_shape(&FillShape::polygons(rects), color, at);
      }
      _ => return false,
    }

    true
  }

  /// Paints the most opaque of `groups` over the rest inside ancestor layers `opacity` opaque, and
  /// returns the sides done, counting the `visible` sides' complement as done.
  fn paint_opacity_groups<D: PaintDevice>(
    &self,
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
    let mut completed = self.paint_opacity_groups(rest, visible, opacity, at, device);

    for &side in group {
      let mut color = side.color;

      color.0[3] = (paint_alpha * f32::from(u8::MAX)).round() as u8;
      self.paint_side(side, color, completed, at, device);
      completed = completed.with(side.side);
    }

    if layered {
      device.end_layer();
    }

    completed
  }

  /// Paints one side in `color`, as Blink's `PaintOneBorderSide` does, with the `completed` sides
  /// already painted.
  fn paint_side<D: PaintDevice>(
    &self,
    side: PaintedSide,
    color: Color,
    completed: SideSet,
    at: Affine,
    device: &mut D,
  ) {
    let border = &self.border;
    let adjacent = side.side.adjacent();
    let curved = self.is_rounded()
      && (matches!(
        side.style,
        BorderStyle::Groove | BorderStyle::Ridge | BorderStyle::Double
      ) || !self.inner_round()
        || self.inner_arcs(side.side));

    if curved {
      let miters = adjacent.map(|adjacent| {
        if colors_match_at_corner(border, side, adjacent) {
          Miter::Hard
        } else {
          Miter::Soft
        }
      });
      let mut clips = self.push_miter_clips(side.side, miters, at, device);

      if !self.inner_renderable()
        && let Some(inner) = self.adjusted_inner(side.side)
      {
        device.push_clip_out(&inner, at);
        clips += 1;
      }

      let stroke = adjacent
        .iter()
        .map(|adjacent| adjacent.of(border.width))
        .fold(side.width, f32::max);

      self.paint_curved_side(side, color, stroke, at, device);

      for _ in 0..clips {
        device.pop_clip();
      }
      return;
    }

    let miters = adjacent.map(|adjacent| Miter::between(border, side, adjacent, completed));
    let clipped = miters.contains(&Miter::Hard)
      || (miters != [Miter::None; 2]
        && matches!(side.style, BorderStyle::Dashed | BorderStyle::Dotted));
    let clips = if clipped {
      self.push_miter_clips(side.side, miters, at, device)
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
      .side_rect(side.side)
      .paint(color, side.style, widths, at, device);

    for _ in 0..clips {
      device.pop_clip();
    }
  }

  /// Blink's `DrawCurvedBoxSide`: fills the whole box, or strokes the whole centerline, for the
  /// clips to cut down to one side. A dashed side strokes `stroke` wide past its own width.
  fn paint_curved_side<D: PaintDevice>(
    &self,
    side: PaintedSide,
    color: Color,
    stroke: f32,
    at: Affine,
    device: &mut D,
  ) {
    let whole = FillShape::Rect(self.size);
    let center = self.border.width.map(|width| (width * 0.5).trunc());

    match side.style {
      BorderStyle::Dotted | BorderStyle::Dashed => {
        let commands = self.inset_rrect(center).to_commands();
        let length = path_length(&commands).trunc();
        let dashed = side.style == BorderStyle::Dashed || side.width <= 3.0;
        let width = if dashed {
          stroke * CURVED_DASH_OVERSTROKE
        } else {
          side.width
        };
        let dash = side.style.dash_pattern(side.width, length, true);

        device.stroke_shape(
          &FillShape::Path {
            commands,
            rule: FillRule::NonZero,
          },
          &StrokeStyle::border(color, width, dash),
          at,
        );
      }
      BorderStyle::Double => {
        let stripe = |fraction: f32| self.border.width.map(|width| (width * fraction).round());

        device.push_clip(&self.inset_rrect(stripe(2.0 / 3.0)), at);
        device.fill_shape(&whole, color, at);
        device.pop_clip();
        device.push_clip_out(&self.inset_rrect(stripe(1.0 / 3.0)), at);
        device.fill_shape(&whole, color, at);
        device.pop_clip();
      }
      BorderStyle::Groove | BorderStyle::Ridge => {
        let outer = if side.style == BorderStyle::Groove {
          BorderStyle::Inset
        } else {
          BorderStyle::Outset
        };
        let darken = side.side.darkened_by(outer);

        device.fill_shape(&whole, color.inset_outset(darken), at);
        device.push_clip(&self.inset_rrect(center), at);
        device.fill_shape(&whole, color.inset_outset(!darken), at);
        device.pop_clip();
      }
      BorderStyle::Inset | BorderStyle::Outset => {
        device.fill_shape(
          &whole,
          color.inset_outset(side.side.darkened_by(side.style)),
          at,
        );
      }
      BorderStyle::Solid => device.fill_shape(&whole, color, at),
      BorderStyle::None | BorderStyle::Hidden => {}
    }
  }

  /// Clips to the part of `side` between its `miters`, as Blink's `ClipBorderSidePolygon` does,
  /// and returns how many clips it pushed.
  fn push_miter_clips<D: PaintDevice>(
    &self,
    side: BorderSide,
    miters: [Miter; 2],
    at: Affine,
    device: &mut D,
  ) -> usize {
    const EXTENSION: f32 = 0.1;

    let point = |x, y| Point { x, y };
    let meet = |a, b, c, d, fallback| intersection(a, b, c, d).unwrap_or(fallback);
    let Size { width, height } = self.size;
    let outer = [
      point(0.0, 0.0),
      point(width, 0.0),
      point(width, height),
      point(0.0, height),
    ];
    let inner = self.inner_corners();
    let [top_left, top_right, bottom_right, bottom_left] = self
      .inner_radii
      .0
      .map(|radius| (radius.x != 0.0 || radius.y != 0.0).then_some(radius));
    let mut pentagon = None;
    let [first, second] = match side {
      BorderSide::Top | BorderSide::Right => miters,
      BorderSide::Bottom | BorderSide::Left => [miters[1], miters[0]],
    };
    let (mut quad, mut bound, extension);

    match side {
      BorderSide::Top => {
        quad = [outer[0], inner[0], inner[1], outer[1]];
        bound = [point(quad[0].x, quad[1].y), point(quad[3].x, quad[2].y)];
        extension = point(-EXTENSION, 0.0);

        if let Some(radius) = top_left {
          let q = quad;

          quad[1] = meet(
            q[0],
            q[1],
            point(q[1].x + radius.x, q[1].y),
            point(q[1].x, q[1].y + radius.y),
            q[1],
          );
          bound[0].y = quad[1].y;
          bound[1].y = quad[1].y;
          if quad[1].y > inner[2].y {
            quad[1] = meet(quad[0], quad[1], inner[3], inner[2], quad[1]);
          }
          if quad[1].x > inner[2].x {
            quad[1] = meet(quad[0], quad[1], inner[1], inner[2], quad[1]);
          }
          if quad[2].y < quad[1].y && quad[2].x > quad[1].x {
            pentagon = Some([
              quad[0],
              quad[1],
              point(quad[2].x, quad[1].y),
              quad[2],
              quad[3],
            ]);
          }
        }
        if let Some(radius) = top_right {
          let q = quad;

          quad[2] = meet(
            q[3],
            q[2],
            point(q[2].x - radius.x, q[2].y),
            point(q[2].x, q[2].y + radius.y),
            q[2],
          );
          if bound[0].y < quad[2].y {
            bound[0].y = quad[2].y;
            bound[1].y = quad[2].y;
          }
          if quad[2].y > inner[3].y {
            quad[2] = meet(quad[3], quad[2], inner[3], inner[2], quad[2]);
          }
          if quad[2].x < inner[3].x {
            quad[2] = meet(quad[3], quad[2], inner[0], inner[3], quad[2]);
          }
          if quad[2].y > quad[1].y && quad[2].x > quad[1].x {
            pentagon = Some([
              quad[0],
              quad[1],
              point(quad[1].x, quad[2].y),
              quad[2],
              quad[3],
            ]);
          }
        }
      }
      BorderSide::Left => {
        quad = [outer[3], inner[3], inner[0], outer[0]];
        bound = [point(quad[1].x, quad[0].y), point(quad[2].x, quad[3].y)];
        extension = point(0.0, EXTENSION);

        if let Some(radius) = top_left {
          let q = quad;

          quad[2] = meet(
            q[3],
            q[2],
            point(q[2].x + radius.x, q[2].y),
            point(q[2].x, q[2].y + radius.y),
            q[2],
          );
          bound[0].x = quad[2].x;
          bound[1].x = quad[2].x;
          if quad[2].y > inner[2].y {
            quad[2] = meet(quad[3], quad[2], inner[3], inner[2], quad[2]);
          }
          if quad[2].x > inner[2].x {
            quad[2] = meet(quad[3], quad[2], inner[1], inner[2], quad[2]);
          }
          if quad[2].y < quad[1].y && quad[2].x > quad[1].x {
            pentagon = Some([
              quad[0],
              quad[1],
              point(quad[2].x, quad[1].y),
              quad[2],
              quad[3],
            ]);
          }
        }
        if let Some(radius) = bottom_left {
          let q = quad;

          quad[1] = meet(
            q[0],
            q[1],
            point(q[1].x + radius.x, q[1].y),
            point(q[1].x, q[1].y - radius.y),
            q[1],
          );
          if bound[0].x < quad[1].x {
            bound[0].x = quad[1].x;
            bound[1].x = quad[1].x;
          }
          if quad[1].y < inner[1].y {
            quad[1] = meet(quad[0], quad[1], inner[0], inner[1], quad[1]);
          }
          if quad[1].x > inner[1].x {
            quad[1] = meet(quad[0], quad[1], inner[1], inner[2], quad[1]);
          }
          if quad[2].y < quad[1].y && quad[2].x < quad[1].x {
            pentagon = Some([
              quad[0],
              quad[1],
              point(quad[1].x, quad[2].y),
              quad[2],
              quad[3],
            ]);
          }
        }
      }
      BorderSide::Bottom => {
        quad = [outer[2], inner[2], inner[3], outer[3]];
        bound = [point(quad[0].x, quad[1].y), point(quad[3].x, quad[2].y)];
        extension = point(EXTENSION, 0.0);

        if let Some(radius) = bottom_left {
          let q = quad;

          quad[2] = meet(
            q[3],
            q[2],
            point(q[2].x + radius.x, q[2].y),
            point(q[2].x, q[2].y - radius.y),
            q[2],
          );
          bound[0].y = quad[2].y;
          bound[1].y = quad[2].y;
          if quad[2].y < inner[1].y {
            quad[2] = meet(quad[3], quad[2], inner[0], inner[1], quad[2]);
          }
          if quad[2].x > inner[1].x {
            quad[2] = meet(quad[3], quad[2], inner[1], inner[2], quad[2]);
          }
          if quad[2].y < quad[1].y && quad[2].x < quad[1].x {
            pentagon = Some([
              quad[0],
              quad[1],
              point(quad[1].x, quad[2].y),
              quad[2],
              quad[3],
            ]);
          }
        }
        if let Some(radius) = bottom_right {
          let q = quad;

          quad[1] = meet(
            q[0],
            q[1],
            point(q[1].x - radius.x, q[1].y),
            point(q[1].x, q[1].y - radius.y),
            q[1],
          );
          if bound[0].y > quad[1].y {
            bound[0].y = quad[1].y;
            bound[1].y = quad[1].y;
          }
          if quad[1].y < inner[0].y {
            quad[1] = meet(quad[0], quad[1], inner[0], inner[1], quad[1]);
          }
          if quad[1].x < inner[0].x {
            quad[1] = meet(quad[0], quad[1], inner[0], inner[3], quad[1]);
          }
          if quad[2].x < quad[1].x && quad[2].y > quad[1].y {
            pentagon = Some([
              quad[0],
              quad[1],
              point(quad[2].x, quad[1].y),
              quad[2],
              quad[3],
            ]);
          }
        }
      }
      BorderSide::Right => {
        quad = [outer[1], inner[1], inner[2], outer[2]];
        bound = [point(quad[1].x, quad[0].y), point(quad[2].x, quad[3].y)];
        extension = point(0.0, -EXTENSION);

        if let Some(radius) = top_right {
          let q = quad;

          quad[1] = meet(
            q[0],
            q[1],
            point(q[1].x - radius.x, q[1].y),
            point(q[1].x, q[1].y + radius.y),
            q[1],
          );
          bound[0].x = quad[1].x;
          bound[1].x = quad[1].x;
          if quad[1].y > inner[3].y {
            quad[1] = meet(quad[0], quad[1], inner[3], inner[2], quad[1]);
          }
          if quad[1].x < inner[3].x {
            quad[1] = meet(quad[0], quad[1], inner[0], inner[3], quad[1]);
          }
          if quad[2].y > quad[1].y && quad[2].x > quad[1].x {
            pentagon = Some([
              quad[0],
              quad[1],
              point(quad[1].x, quad[2].y),
              quad[2],
              quad[3],
            ]);
          }
        }
        if let Some(radius) = bottom_right {
          let q = quad;

          quad[2] = meet(
            q[3],
            q[2],
            point(q[2].x - radius.x, q[2].y),
            point(q[2].x, q[2].y - radius.y),
            q[2],
          );
          if bound[0].x > quad[2].x {
            bound[0].x = quad[2].x;
            bound[1].x = quad[2].x;
          }
          if quad[2].y < inner[0].y {
            quad[2] = meet(quad[3], quad[2], inner[0], inner[1], quad[2]);
          }
          if quad[2].x < inner[0].x {
            quad[2] = meet(quad[3], quad[2], inner[0], inner[3], quad[2]);
          }
          if quad[2].x < quad[1].x && quad[2].y > quad[1].y {
            pentagon = Some([
              quad[0],
              quad[1],
              point(quad[2].x, quad[1].y),
              quad[2],
              quad[3],
            ]);
          }
        }
      }
    }

    let push = |shape: FillShape, miter: Miter, device: &mut D| {
      if miter == Miter::Hard {
        device.push_aliased_clip(&shape, at);
      } else {
        device.push_clip(&shape, at);
      }
    };

    if first == second {
      let shape = match pentagon {
        Some(pentagon) if !self.inner_renderable() => FillShape::polygons([pentagon]),
        _ => FillShape::polygons([quad]),
      };

      push(shape, first, device);
      return 1;
    }

    let mut clips = 0;

    if first != Miter::None {
      let cut = meet(quad[0], quad[1], bound[0], bound[1], Point::ZERO);

      push(
        FillShape::polygons([[quad[0] + extension, cut + extension, bound[1], quad[3]]]),
        first,
        device,
      );
      clips += 1;
    }
    if second != Miter::None {
      let cut = meet(quad[2], quad[3], bound[0], bound[1], Point::ZERO);

      push(
        FillShape::polygons([[quad[0], bound[0], cut - extension, quad[3] - extension]]),
        second,
        device,
      );
      clips += 1;
    }

    clips
  }

  /// Blink's `CalculateAdjustedInnerBorder`: the padding box grown so its radii along `side` fit,
  /// with the other corners square, or `None` when it is empty.
  fn adjusted_inner(&self, side: BorderSide) -> Option<FillShape> {
    let mut radii = self.inner_radii.0;
    let Point { mut x, mut y } = self.inner_origin;
    let Size {
      mut width,
      mut height,
    } = self.inner_size;
    let zero = SpacePair::from_single(0.0);

    if self.hyperellipse() {
      match side {
        BorderSide::Top | BorderSide::Bottom => {
          let [near, far] = if side == BorderSide::Top {
            [0, 1]
          } else {
            [3, 2]
          };
          let overshoot = radii[near].x + radii[far].x - width;

          if overshoot > 0.1 {
            width += overshoot;
            if radii[near].x == 0.0 {
              x -= overshoot;
            }
          }

          let tallest = radii[near].y.max(radii[far].y);

          for corner in [0, 1, 2, 3] {
            if corner != near && corner != far {
              radii[corner] = zero;
            }
          }
          if tallest > height {
            if side == BorderSide::Bottom {
              y += height - tallest;
            }
            height = tallest;
          }
        }
        BorderSide::Left | BorderSide::Right => {
          let [near, far] = if side == BorderSide::Left {
            [0, 3]
          } else {
            [1, 2]
          };
          let overshoot = radii[near].y + radii[far].y - height;

          if overshoot > 0.1 {
            height += overshoot;
            if radii[near].y == 0.0 {
              y -= overshoot;
            }
          }

          let widest = radii[near].x.max(radii[far].x);

          for corner in [0, 1, 2, 3] {
            if corner != near && corner != far {
              radii[corner] = zero;
            }
          }
          if widest > width {
            if side == BorderSide::Right {
              x += width - widest;
            }
            width = widest;
          }
        }
      }
    }

    (width > 0.0 && height > 0.0)
      .then(|| self.rrect(Sides(radii), Point { x, y }, Size { width, height }))
  }

  /// The rectangle `side` fills on a straight side, across the whole border box.
  fn side_rect(&self, side: BorderSide) -> BoxSideRect {
    let Size { width, height } = self.size;
    let thickness = side.of(self.border.width);
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

  /// The padding box's corners, clockwise from the top-left.
  fn inner_corners(&self) -> [Point<f32>; 4] {
    let Point { x, y } = self.inner_origin;
    let Size { width, height } = self.inner_size;

    [
      Point { x, y },
      Point { x: x + width, y },
      Point {
        x: x + width,
        y: y + height,
      },
      Point { x, y: y + height },
    ]
  }

  /// The border box as a rounded rectangle.
  fn outer_rrect(&self) -> FillShape {
    self.rrect(self.outer_radii, Point::ZERO, self.size)
  }

  /// The padding box as a rounded rectangle.
  fn inner_rrect(&self) -> FillShape {
    self.rrect(self.inner_radii, self.inner_origin, self.inner_size)
  }

  /// The border box inset by `insets`, its radii shrunk to match, as Blink's
  /// `PixelSnappedContouredBorderWithOutsets` builds it.
  fn inset_rrect(&self, insets: Rect<f32>) -> FillShape {
    self.rrect(
      shrink_radii(self.outer_radii, insets),
      insets.top_left(),
      self.size.inset(insets),
    )
  }

  /// A rectangle of `size` at `origin` with `radii`, drawn with the border's corner shapes. A
  /// corner with a radius at or below zero is square, as Skia's `SkRRect` draws it.
  fn rrect(&self, radii: Sides<SpacePair<f32>>, origin: Point<f32>, size: Size<f32>) -> FillShape {
    let mut border = self.border;

    border.radius = Sides(radii.0.map(|radius| {
      if radius.x <= 0.0 || radius.y <= 0.0 {
        SpacePair::from_single(0.0)
      } else {
        radius
      }
    }));

    FillShape::RoundedRect {
      border,
      size,
      offset: origin,
    }
  }

  /// Whether any outer corner is rounded.
  fn is_rounded(&self) -> bool {
    self
      .outer_radii
      .0
      .iter()
      .any(|radius| radius.x != 0.0 || radius.y != 0.0)
  }

  /// Whether the inner radii fit the padding box, as Blink's `FloatRoundedRect::IsRenderable`.
  fn inner_renderable(&self) -> bool {
    const TOLERANCE: f32 = 1.0001;

    let [top_left, top_right, bottom_right, bottom_left] = self.inner_radii.0;
    let Size { width, height } = self.inner_size;

    top_left.x + top_right.x <= width * TOLERANCE
      && bottom_left.x + bottom_right.x <= width * TOLERANCE
      && top_left.y + bottom_left.y <= height * TOLERANCE
      && top_right.y + bottom_right.y <= height * TOLERANCE
  }

  /// Whether every rounded corner is `round`, as Blink's `HasRoundCurvature` asks of the inner
  /// edge.
  fn inner_round(&self) -> bool {
    let rounded_inner = self
      .inner_radii
      .0
      .iter()
      .any(|radius| radius.x != 0.0 || radius.y != 0.0);

    !rounded_inner || self.curvatures().all(|exponent| exponent == 1.0)
  }

  /// Whether every corner curves at least as far out as `round`, as Blink's `IsHyperellipse`.
  fn hyperellipse(&self) -> bool {
    self.curvatures().all(|exponent| exponent >= 1.0)
  }

  /// Each corner's `corner-shape` parameter, `round` where the corner has no radius.
  fn curvatures(&self) -> impl Iterator<Item = f32> {
    self
      .outer_radii
      .0
      .into_iter()
      .zip(self.border.shape.0)
      .map(|(radius, shape)| {
        if radius.x == 0.0 || radius.y == 0.0 {
          1.0
        } else {
          shape.0
        }
      })
  }

  /// Whether the padding edge curves at either end of `side`.
  fn inner_arcs(&self, side: BorderSide) -> bool {
    let [top_left, top_right, bottom_right, bottom_left] = self.inner_radii.0;
    let [first, second] = match side {
      BorderSide::Top => [top_left, top_right],
      BorderSide::Right => [top_right, bottom_right],
      BorderSide::Bottom => [bottom_left, bottom_right],
      BorderSide::Left => [top_left, bottom_left],
    };

    [first, second]
      .iter()
      .any(|radius| radius.x != 0.0 || radius.y != 0.0)
  }
}

/// `radii` shrunk by `insets` where they curve, as Blink's `FloatRoundedRect::Radii::Outset`
/// shrinks them, each part stopping at zero.
fn shrink_radii(radii: Sides<SpacePair<f32>>, insets: Rect<f32>) -> Sides<SpacePair<f32>> {
  let shrink = |radius: f32, inset: f32| {
    if radius > 0.0 {
      (radius - inset).max(0.0)
    } else {
      radius
    }
  };
  let [top_left, top_right, bottom_right, bottom_left] = radii.0;
  let corner = |radius: SpacePair<f32>, x: f32, y: f32| SpacePair {
    x: shrink(radius.x, x),
    y: shrink(radius.y, y),
  };

  Sides([
    corner(top_left, insets.left, insets.top),
    corner(top_right, insets.right, insets.top),
    corner(bottom_right, insets.right, insets.bottom),
    corner(bottom_left, insets.left, insets.bottom),
  ])
}

/// The area between `outer` and `inner`.
fn ring(outer: FillShape, inner: FillShape) -> FillShape {
  let mut commands = outer.to_commands();

  commands.extend(inner.to_commands());

  FillShape::Path {
    commands,
    rule: FillRule::EvenOdd,
  }
}

/// Where the lines through `a`–`b` and `c`–`d` cross, as gfx's `LineF::IntersectionWith` finds it.
fn intersection(a: Point<f32>, b: Point<f32>, c: Point<f32>, d: Point<f32>) -> Option<Point<f32>> {
  let (ab, cd) = (b - a, d - c);
  let cross = |u: Point<f32>, v: Point<f32>| u.x * v.y - u.y * v.x;
  let denominator = cross(ab, cd);

  if denominator == 0.0 {
    return None;
  }

  let param = cross(c - a, cd) / denominator;

  Some(Point {
    x: a.x + ab.x * param,
    y: a.y + ab.y * param,
  })
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
