use smallvec::SmallVec;

use crate::{
  context::RenderContext,
  geometry::{LAYOUT_UNIT_EPSILON, PathBuilder, PathCommand as Command, Point, Rect, Size},
  layout::{
    contoured_rect::{Corner, opposite_corners_factor},
    corner_shape::{CornerContour, KAPPA, corner_contour},
    decoration::{ClipBox, ContourOrigin},
  },
  style::{BorderStyle, Color, ImageScalingAlgorithm, Sides, SpacePair, Superellipse},
};

/// Border side identifier used by per-side geometry and rasterization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderSide {
  /// Top side.
  Top,
  /// Right side.
  Right,
  /// Bottom side.
  Bottom,
  /// Left side.
  Left,
}

impl BorderSide {
  /// The two sides that meet this one at its corners.
  pub fn adjacent(self) -> [Self; 2] {
    match self {
      Self::Top | Self::Bottom => [Self::Left, Self::Right],
      Self::Right | Self::Left => [Self::Top, Self::Bottom],
    }
  }

  /// Whether a 3D `style` shades this side dark: `inset` darkens the top and left, and `outset`
  /// the bottom and right, as Blink's `DarkenBoxSide` decides.
  pub(crate) fn darkened_by(self, style: BorderStyle) -> bool {
    matches!(self, Self::Top | Self::Left) == (style == BorderStyle::Inset)
  }

  /// This side's entry in `sides`.
  pub fn of<T: Copy>(self, sides: Rect<T>) -> T {
    match self {
      Self::Top => sides.top,
      Self::Right => sides.right,
      Self::Bottom => sides.bottom,
      Self::Left => sides.left,
    }
  }
}

/// One strip of a border side: where it sits, how thick it is, and what colour fills it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SideBand {
  /// How far in from the border box the strip starts, per side.
  pub inset: Rect<f32>,
  /// The strip's thickness, per side.
  pub width: Rect<f32>,
  /// The fill colour, already shaded for the 3D styles.
  pub color: Color,
}

/// One side of a border that paints, with the values needed to draw it.
#[derive(Debug, Clone, Copy)]
pub struct PaintedSide {
  /// Which side this is.
  pub side: BorderSide,
  /// The side's width in pixels.
  pub width: f32,
  /// The side's resolved colour.
  pub color: Color,
  /// The side's line style.
  pub style: BorderStyle,
}

/// Represents the properties of a border, including corner radii and drawing metadata.
#[derive(Debug, Clone, Copy, Default)]
pub struct BorderProperties {
  /// The width of the border on each side (top, right, bottom, left)
  pub width: Rect<f32>,
  /// The color of each border side.
  pub color: Rect<Color>,
  /// Corner radii: top, right, bottom, left (in pixels)
  pub radius: Sides<SpacePair<f32>>,
  /// Corner shapes: top-left, top-right, bottom-right, bottom-left.
  pub shape: Sides<Superellipse>,
  /// The style of each border side.
  pub style: Rect<BorderStyle>,
  /// The image rendering algorithm to use when sampling the image.
  pub image_rendering: ImageScalingAlgorithm,
  /// Whether the sides square off at the corners instead of mitring, the way
  /// Blink hands a collapsed table's intersection to one edge.
  pub collapsed: bool,
}

impl BorderProperties {
  /// The amount of path commands to append for this border.
  /// This is used to pre-allocate the vector size for the mask commands.
  pub const PATH_COMMANDS_AMOUNT: usize = 14;

  /// Resolves the border radius from the context and layout.
  pub(crate) fn resolve_radius_part(
    context: &RenderContext,
    border_box: Size<f32>,
  ) -> Sides<SpacePair<f32>> {
    let style = &context.style;

    Sides(
      [
        style.surround_data.border_top_left_radius,
        style.surround_data.border_top_right_radius,
        style.surround_data.border_bottom_right_radius,
        style.surround_data.border_bottom_left_radius,
      ]
      .map(|radius| radius.to_px(&context.sizing, border_box.width, border_box.height)),
    )
  }

  /// Resolves the corner shapes from the context.
  pub(crate) fn resolve_shape_part(context: &RenderContext) -> Sides<Superellipse> {
    Sides([
      context.style.surround_data.corner_top_left_shape,
      context.style.surround_data.corner_top_right_shape,
      context.style.surround_data.corner_bottom_right_shape,
      context.style.surround_data.corner_bottom_left_shape,
    ])
  }

  /// Resolves the border radius from the context and layout.
  pub fn from_context(
    context: &RenderContext,
    border_box: Size<f32>,
    border_width: Rect<f32>,
  ) -> Self {
    Self {
      width: border_width,
      color: Rect {
        top: context
          .style
          .surround_data
          .border_top_color
          .resolve(context.current_color),
        right: context
          .style
          .surround_data
          .border_right_color
          .resolve(context.current_color),
        bottom: context
          .style
          .surround_data
          .border_bottom_color
          .resolve(context.current_color),
        left: context
          .style
          .surround_data
          .border_left_color
          .resolve(context.current_color),
      },
      radius: Self::resolve_radius_part(context, border_box),
      shape: Self::resolve_shape_part(context),
      style: Rect {
        top: context.style.box_data.border_top_style,
        right: context.style.box_data.border_right_style,
        bottom: context.style.box_data.border_bottom_style,
        left: context.style.box_data.border_left_style,
      },
      image_rendering: context.style.misc_inherited_data.image_rendering,
      collapsed: context.collapsed_borders,
    }
  }

  /// Every side, painted or not, clockwise from the top.
  pub(crate) fn sides(&self) -> [PaintedSide; 4] {
    [
      (
        BorderSide::Top,
        self.width.top,
        self.color.top,
        self.style.top,
      ),
      (
        BorderSide::Right,
        self.width.right,
        self.color.right,
        self.style.right,
      ),
      (
        BorderSide::Bottom,
        self.width.bottom,
        self.color.bottom,
        self.style.bottom,
      ),
      (
        BorderSide::Left,
        self.width.left,
        self.color.left,
        self.style.left,
      ),
    ]
    .map(|(side, width, color, style)| PaintedSide {
      side,
      width,
      color,
      style: style.effective(width),
    })
  }

  /// The sides that put ink on the page, clockwise from the top.
  pub fn painted_sides(&self) -> impl Iterator<Item = PaintedSide> {
    self
      .sides()
      .into_iter()
      .filter(|side| side.is_visible() && side.color.0[3] != 0)
  }

  /// True if any side is rendered with nonzero width.
  pub fn has_visible_sides(&self) -> bool {
    self.sides().iter().any(PaintedSide::is_visible)
  }

  /// Per-side widths with invisible sides zeroed.
  pub fn visible_side_widths(&self) -> Rect<f32> {
    let [top, right, bottom, left] = self
      .sides()
      .map(|side| if side.is_visible() { side.width } else { 0.0 });

    Rect {
      top,
      right,
      bottom,
      left,
    }
  }

  /// The shared color if all visible sides match, else `None`.
  pub fn has_uniform_visible_color(&self) -> Option<Color> {
    let mut colors = self
      .sides()
      .into_iter()
      .filter(PaintedSide::is_visible)
      .map(|side| side.color);
    let color = colors.next()?;

    colors.all(|other| other == color).then_some(color)
  }

  /// True if every side has equal nonzero width and the given style.
  pub fn is_uniform_all_sides_style(&self, style: BorderStyle) -> bool {
    let has_uniform_width = self.width.top > 0.0
      && (self.width.top - self.width.right).abs() <= LAYOUT_UNIT_EPSILON
      && (self.width.top - self.width.bottom).abs() <= LAYOUT_UNIT_EPSILON
      && (self.width.top - self.width.left).abs() <= LAYOUT_UNIT_EPSILON;

    has_uniform_width
      && self.style.top == style
      && self.style.right == style
      && self.style.bottom == style
      && self.style.left == style
  }

  /// Appends the outer and inner ring contours for the border at the origin.
  pub fn append_border_ring_commands(&self, paths: &mut Vec<Command>, border_box: Size<f32>) {
    self.append_border_ring_commands_at(paths, border_box, Point::ZERO);
  }

  /// Appends the outer and inner ring contours for the border at the given offset.
  pub fn append_border_ring_commands_at(
    &self,
    paths: &mut Vec<Command>,
    border_box: Size<f32>,
    offset: Point<f32>,
  ) {
    let mut border = *self;

    border.append_mask_commands(paths, border_box, offset);
    let inner_size = Size {
      width: (border_box.width - border.width.left - border.width.right).max(0.0),
      height: (border_box.height - border.width.top - border.width.bottom).max(0.0),
    };
    let max_inner_x = (offset.x + border_box.width - inner_size.width).max(offset.x);
    let max_inner_y = (offset.y + border_box.height - inner_size.height).max(offset.y);
    let inner_offset = Point {
      x: (offset.x + border.width.left).clamp(offset.x, max_inner_x),
      y: (offset.y + border.width.top).clamp(offset.y, max_inner_y),
    };
    border.inset_by_border_width();

    let inner = ClipBox {
      border,
      size: inner_size,
      offset: inner_offset - offset,
      origin: Some(ContourOrigin {
        border: *self,
        size: border_box,
        offset: Point::ZERO,
      }),
    };

    if !inner.follows_origin() {
      return border.append_mask_commands(paths, inner_size, inner_offset);
    }

    let start = paths.len();

    inner.append_contour(paths);
    for command in &mut paths[start..] {
      *command = command.map_points(|point| point + offset);
    }
  }

  /// Appends a trapezoid polygon covering one border side at the given offset.
  pub fn append_side_polygon_commands_at(
    &self,
    side: BorderSide,
    path: &mut Vec<Command>,
    border_box: Size<f32>,
    offset: Point<f32>,
  ) {
    if border_box.width <= 0.0 || border_box.height <= 0.0 {
      return;
    }

    if self.collapsed {
      self.append_squared_side_polygon_commands_at(side, path, border_box, offset);
      return;
    }

    let Rect {
      left: inner_left,
      right: inner_right,
      top: inner_top,
      bottom: inner_bottom,
    } = self.inner_edges(border_box);

    match side {
      BorderSide::Top => {
        path.move_to((offset.x, offset.y));
        path.line_to((offset.x + border_box.width, offset.y));
        path.line_to((offset.x + inner_right, offset.y + inner_top));
        path.line_to((offset.x + inner_left, offset.y + inner_top));
      }
      BorderSide::Right => {
        path.move_to((offset.x + border_box.width, offset.y));
        path.line_to((offset.x + border_box.width, offset.y + border_box.height));
        path.line_to((offset.x + inner_right, offset.y + inner_bottom));
        path.line_to((offset.x + inner_right, offset.y + inner_top));
      }
      BorderSide::Bottom => {
        path.move_to((offset.x + border_box.width, offset.y + border_box.height));
        path.line_to((offset.x, offset.y + border_box.height));
        path.line_to((offset.x + inner_left, offset.y + inner_bottom));
        path.line_to((offset.x + inner_right, offset.y + inner_bottom));
      }
      BorderSide::Left => {
        path.move_to((offset.x, offset.y + border_box.height));
        path.line_to((offset.x, offset.y));
        path.line_to((offset.x + inner_left, offset.y + inner_top));
        path.line_to((offset.x + inner_left, offset.y + inner_bottom));
      }
    }

    path.close();
  }

  /// Appends the rectangle one side covers once the wider of the two sides at each corner takes the
  /// whole intersection.
  fn append_squared_side_polygon_commands_at(
    &self,
    side: BorderSide,
    path: &mut Vec<Command>,
    border_box: Size<f32>,
    offset: Point<f32>,
  ) {
    let Rect {
      left: inner_left,
      right: inner_right,
      top: inner_top,
      bottom: inner_bottom,
    } = self.inner_edges(border_box);
    let (start, end) = match side {
      BorderSide::Top => (
        (
          if self.width.top >= self.width.left {
            0.0
          } else {
            inner_left
          },
          0.0,
        ),
        (
          if self.width.top >= self.width.right {
            border_box.width
          } else {
            inner_right
          },
          inner_top,
        ),
      ),
      BorderSide::Bottom => (
        (
          if self.width.bottom >= self.width.left {
            0.0
          } else {
            inner_left
          },
          inner_bottom,
        ),
        (
          if self.width.bottom >= self.width.right {
            border_box.width
          } else {
            inner_right
          },
          border_box.height,
        ),
      ),
      BorderSide::Left => (
        (
          0.0,
          if self.width.left > self.width.top {
            0.0
          } else {
            inner_top
          },
        ),
        (
          inner_left,
          if self.width.left > self.width.bottom {
            border_box.height
          } else {
            inner_bottom
          },
        ),
      ),
      BorderSide::Right => (
        (
          inner_right,
          if self.width.right > self.width.top {
            0.0
          } else {
            inner_top
          },
        ),
        (
          border_box.width,
          if self.width.right > self.width.bottom {
            border_box.height
          } else {
            inner_bottom
          },
        ),
      ),
    };

    if end.0 <= start.0 || end.1 <= start.1 {
      return;
    }

    path.move_to((offset.x + start.0, offset.y + start.1));
    path.line_to((offset.x + end.0, offset.y + start.1));
    path.line_to((offset.x + end.0, offset.y + end.1));
    path.line_to((offset.x + start.0, offset.y + end.1));
    path.close();
  }

  /// The padding-box edges inside `border_box`, clamped so opposite edges never cross.
  fn inner_edges(&self, border_box: Size<f32>) -> Rect<f32> {
    let left = self.width.left.min(border_box.width);
    let top = self.width.top.min(border_box.height);

    Rect {
      left,
      right: (border_box.width - self.width.right).max(left),
      top,
      bottom: (border_box.height - self.width.bottom).max(top),
    }
  }

  /// Returns true if all corner radii are zero.
  #[inline]
  pub fn is_zero(&self) -> bool {
    const ZERO: Sides<SpacePair<f32>> = Sides([SpacePair::from_single(0.0); 4]);

    self.radius == ZERO
  }

  /// Expand or shrink corner radii by the specified amounts.
  pub fn expand_by(&mut self, amount: Rect<f32>) {
    if amount == Rect::ZERO {
      return;
    }

    // A square corner stays square, as Blink's `FloatRoundedRect::Radii::Outset` keeps it.
    let grow = |radius: &mut f32, by: f32| {
      if *radius > 0.0 {
        *radius = (*radius + by).max(0.0);
      }
    };

    grow(&mut self.radius.0[0].x, amount.left);
    grow(&mut self.radius.0[0].y, amount.top);

    grow(&mut self.radius.0[1].x, amount.right);
    grow(&mut self.radius.0[1].y, amount.top);

    grow(&mut self.radius.0[2].x, amount.right);
    grow(&mut self.radius.0[2].y, amount.bottom);

    grow(&mut self.radius.0[3].x, amount.left);
    grow(&mut self.radius.0[3].y, amount.bottom);
  }

  /// Grows the corner radii of a `border_box` whose edges move `outset` outward, after
  /// css-backgrounds-3's [outset-adjusted border radius](https://drafts.csswg.org/css-backgrounds-3/#outset-adjusted-border-radius),
  /// so a small corner stays proportionally sharp and a square one stays square.
  pub(crate) fn outset_radii(&mut self, border_box: Size<f32>, outset: f32) {
    let used = self.scaled_corner_radii(border_box);

    for (corner, used) in self.radius.0.iter_mut().zip(used.0) {
      if used.x <= 0.0 && used.y <= 0.0 {
        *corner = SpacePair::from_single(0.0);
        continue;
      }

      let coverage = 2.0 * (used.x / border_box.width).min(used.y / border_box.height);
      let adjusted = |radius: f32| {
        if radius > outset || coverage > 1.0 {
          return radius + outset;
        }

        radius + outset * (1.0 - (1.0 - radius / outset).powi(3) * (1.0 - coverage.powi(3)))
      };

      corner.x = adjusted(used.x);
      corner.y = adjusted(used.y);
    }
  }

  /// Shrink radii by the border width to get inner radius path.
  pub(crate) fn inset_by_border_width(&mut self) {
    self.expand_by(self.width.map(|size| -size))
  }

  /// CSS overlapping-curves scale factor: shrinks corner radii so adjacent radii on a side never
  /// exceed the border-box edge.
  fn overlapping_curves_scale(radii: &Sides<SpacePair<f32>>, border_box: Size<f32>) -> f32 {
    let axis_scale = |a: f32, b: f32, extent: f32| {
      let sum = a + b;
      if sum > extent { extent / sum } else { 1.0 }
    };

    1.0f32
      .min(axis_scale(radii.0[0].x, radii.0[1].x, border_box.width))
      .min(axis_scale(radii.0[3].x, radii.0[2].x, border_box.width))
      .min(axis_scale(radii.0[0].y, radii.0[3].y, border_box.height))
      .min(axis_scale(radii.0[1].y, radii.0[2].y, border_box.height))
  }

  /// Append rounded-rect path commands for this border's corner radii.
  pub fn append_mask_commands(
    &self,
    path: &mut Vec<Command>,
    border_box: Size<f32>,
    offset: Point<f32>,
  ) {
    if border_box.width <= 0.0 || border_box.height <= 0.0 {
      return;
    }

    path.reserve_exact(BorderProperties::PATH_COMMANDS_AMOUNT);

    let radii = self.scaled_corner_radii(border_box);
    let [top_left, top_right, bottom_right, bottom_left] = radii.0;

    path.move_to((offset.x + top_left.x, offset.y));

    path.line_to((offset.x + border_box.width - top_right.x, offset.y));

    if top_right.x > 0.0 && top_right.y > 0.0 {
      let SpacePair { x: rx, y: ry } = top_right;

      if self.shape.0[1].is_round() {
        path.curve_to(
          (offset.x + border_box.width - rx * (1.0 - KAPPA), offset.y),
          (offset.x + border_box.width, offset.y + ry * (1.0 - KAPPA)),
          (offset.x + border_box.width, offset.y + ry),
        );
      } else {
        append_shaped_corner(
          path,
          self.shape.0[1],
          Point {
            x: offset.x + border_box.width - rx,
            y: offset.y + ry,
          },
          Point { x: rx, y: 0.0 },
          Point { x: 0.0, y: -ry },
        );
      }
    } else {
      path.line_to((offset.x + border_box.width, offset.y));
    }

    path.line_to((
      offset.x + border_box.width,
      offset.y + border_box.height - bottom_right.y,
    ));

    if bottom_right.x > 0.0 && bottom_right.y > 0.0 {
      let SpacePair { x: rx, y: ry } = bottom_right;

      if self.shape.0[2].is_round() {
        path.curve_to(
          (
            offset.x + border_box.width,
            offset.y + border_box.height - ry * (1.0 - KAPPA),
          ),
          (
            offset.x + border_box.width - rx * (1.0 - KAPPA),
            offset.y + border_box.height,
          ),
          (
            offset.x + border_box.width - rx,
            offset.y + border_box.height,
          ),
        );
      } else {
        append_shaped_corner(
          path,
          self.shape.0[2],
          Point {
            x: offset.x + border_box.width - rx,
            y: offset.y + border_box.height - ry,
          },
          Point { x: 0.0, y: ry },
          Point { x: rx, y: 0.0 },
        );
      }
    } else {
      path.line_to((offset.x + border_box.width, offset.y + border_box.height));
    }

    path.line_to((offset.x + bottom_left.x, offset.y + border_box.height));

    if bottom_left.x > 0.0 && bottom_left.y > 0.0 {
      let SpacePair { x: rx, y: ry } = bottom_left;

      if self.shape.0[3].is_round() {
        path.curve_to(
          (offset.x + rx * (1.0 - KAPPA), offset.y + border_box.height),
          (offset.x, offset.y + border_box.height - ry * (1.0 - KAPPA)),
          (offset.x, offset.y + border_box.height - ry),
        );
      } else {
        append_shaped_corner(
          path,
          self.shape.0[3],
          Point {
            x: offset.x + rx,
            y: offset.y + border_box.height - ry,
          },
          Point { x: -rx, y: 0.0 },
          Point { x: 0.0, y: ry },
        );
      }
    } else {
      path.line_to((offset.x, offset.y + border_box.height));
    }

    path.line_to((offset.x, offset.y + top_left.y));

    if top_left.x > 0.0 && top_left.y > 0.0 {
      let SpacePair { x: rx, y: ry } = top_left;

      if self.shape.0[0].is_round() {
        path.curve_to(
          (offset.x, offset.y + ry * (1.0 - KAPPA)),
          (offset.x + rx * (1.0 - KAPPA), offset.y),
          (offset.x + rx, offset.y),
        );
      } else {
        append_shaped_corner(
          path,
          self.shape.0[0],
          Point {
            x: offset.x + rx,
            y: offset.y + ry,
          },
          Point { x: 0.0, y: -ry },
          Point { x: -rx, y: 0.0 },
        );
      }
    } else {
      path.line_to((offset.x, offset.y));
    }

    path.close();
  }

  pub(crate) fn scaled_corner_radii(&self, border_box: Size<f32>) -> Sides<SpacePair<f32>> {
    let mut scaled = self.radius;

    // `square` corners render with no curvature regardless of `border-radius`.
    for (corner, shape) in scaled.0.iter_mut().zip(self.shape.0) {
      if shape.is_degenerate() {
        *corner = SpacePair::from_single(0.0);
      }
    }

    let scale = Self::overlapping_curves_scale(&scaled, border_box);

    for corner in &mut scaled.0 {
      corner.x = (corner.x * scale).max(0.0);
      corner.y = (corner.y * scale).max(0.0);
    }

    let corners = Corner::of_box(
      Point::ZERO,
      border_box,
      &scaled,
      Corner::curvatures(&scaled, &self.shape),
    );

    if corners.iter().any(|corner| corner.curvature < 1.0) {
      let factor = opposite_corners_factor(&corners);

      for corner in &mut scaled.0 {
        corner.x *= factor;
        corner.y *= factor;
      }
    }

    scaled
  }
}

/// Appends a non-`round` corner contour, mapping normalized contour points through `anchor + p[0] *
/// u + p[1] * v` into pixel space.
pub(crate) fn append_shaped_corner(
  path: &mut Vec<Command>,
  shape: Superellipse,
  anchor: Point<f32>,
  u: Point<f32>,
  v: Point<f32>,
) {
  match corner_contour(shape) {
    CornerContour::Bevel => path.line_to(corner_point(anchor, u, v, [1.0, 0.0])),
    CornerContour::Notch => {
      path.line_to(corner_point(anchor, u, v, [0.0, 0.0]));
      path.line_to(corner_point(anchor, u, v, [1.0, 0.0]));
    }
    CornerContour::Cubic(cubic) => append_corner_cubic(path, anchor, u, v, cubic),
    CornerContour::Cubics(first, second) => {
      append_corner_cubic(path, anchor, u, v, first);
      append_corner_cubic(path, anchor, u, v, second);
    }
  }
}

fn append_corner_cubic(
  path: &mut Vec<Command>,
  anchor: Point<f32>,
  u: Point<f32>,
  v: Point<f32>,
  [control1, control2, end]: [[f32; 2]; 3],
) {
  path.curve_to(
    corner_point(anchor, u, v, control1),
    corner_point(anchor, u, v, control2),
    corner_point(anchor, u, v, end),
  );
}

fn corner_point(anchor: Point<f32>, u: Point<f32>, v: Point<f32>, point: [f32; 2]) -> (f32, f32) {
  (
    anchor.x + point[0] * u.x + point[1] * v.x,
    anchor.y + point[0] * u.y + point[1] * v.y,
  )
}

// The dash spacing below derives from Chromium's styled_stroke_data.cc, under
// the BSD notice in LICENSE-CHROMIUM.
const DASHED_THICK_WIDTH_THRESHOLD: f32 = 3.0;
const DASHED_LENGTH_RATIO_THICK: f32 = 2.0;
const DASHED_LENGTH_RATIO_THIN: f32 = 3.0;
const DASHED_GAP_RATIO_THICK: f32 = 1.0;
const DASHED_GAP_RATIO_THIN: f32 = 2.0;
const DOTTED_ENDPOINT_EPSILON: f32 = 1.0e-2;

impl BorderStyle {
  /// Blink's `BorderEdge::EffectiveStyle`: a double side under 3px and a groove or ridge side up
  /// to 1px paint solid.
  pub(crate) fn effective(self, width: f32) -> Self {
    match self {
      BorderStyle::Double if width < 3.0 => BorderStyle::Solid,
      BorderStyle::Groove | BorderStyle::Ridge if width <= 1.0 => BorderStyle::Solid,
      style => style,
    }
  }

  /// Returns a dash interval and round-cap flag for this style. A dotted line up to 3px wide
  /// draws square dots, as Blink does, and a box side fills its end dots on its own.
  pub fn dash_pattern(self, width: f32, length: f32, closed: bool) -> Option<BorderDash> {
    if width <= 0.0 || length <= 0.0 {
      return None;
    }

    match self {
      BorderStyle::Dashed => {
        let thick = width >= DASHED_THICK_WIDTH_THRESHOLD;
        let (dash, gap) = if thick {
          (DASHED_LENGTH_RATIO_THICK, DASHED_GAP_RATIO_THICK)
        } else {
          (DASHED_LENGTH_RATIO_THIN, DASHED_GAP_RATIO_THIN)
        };

        square_dash(width * dash, width * gap, length, closed, true)
      }
      BorderStyle::Dotted if width <= DASHED_THICK_WIDTH_THRESHOLD => {
        square_dash(width, width, length, closed, false)
      }
      BorderStyle::Dotted => {
        let per_dot_length = width * 2.0;
        let gap = if length < per_dot_length {
          per_dot_length
        } else {
          select_best_dash_gap(length, width, width, closed) + width - DOTTED_ENDPOINT_EPSILON
        };

        Some(BorderDash {
          intervals: [0.0, gap],
          round_cap: true,
        })
      }
      _ => None,
    }
  }
}

/// The stroke dash of a `dashed`/`dotted` border or outline side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderDash {
  /// Dash and gap lengths.
  pub intervals: [f32; 2],
  /// Whether dots are drawn with round caps.
  pub round_cap: bool,
}

/// Butt-capped dashes of `dash` with gaps near `gap` fitted to `length`, or `None` when two
/// dashes do not fit and the line draws solid. `spread` stretches the gaps so the line ends on a
/// dash.
fn square_dash(dash: f32, gap: f32, length: f32, closed: bool, spread: bool) -> Option<BorderDash> {
  if length <= dash * 2.0 {
    return None;
  }

  let two_dashes_with_gap = 2.0 * dash + gap + if closed { gap } else { 0.0 };
  let intervals = if length <= two_dashes_with_gap {
    let multiplier = length / two_dashes_with_gap;

    [dash * multiplier, gap * multiplier]
  } else if spread {
    [dash, select_best_dash_gap(length, dash, gap, closed)]
  } else {
    [dash, gap]
  };

  Some(BorderDash {
    intervals,
    round_cap: false,
  })
}

fn select_best_dash_gap(length: f32, dash: f32, gap: f32, closed: bool) -> f32 {
  let available = if closed { length } else { length + gap };
  let min_dashes = (available / (dash + gap)).floor();
  let max_dashes = min_dashes + 1.0;
  let min_gaps = if closed { min_dashes } else { min_dashes - 1.0 };
  let max_gaps = if closed { max_dashes } else { max_dashes - 1.0 };

  if min_gaps <= 0.0 || max_gaps <= 0.0 {
    return gap.max(0.0);
  }

  let min_gap = (length - min_dashes * dash) / min_gaps;
  let max_gap = (length - max_dashes * dash) / max_gaps;
  if max_gap <= 0.0 || (min_gap - gap).abs() < (max_gap - gap).abs() {
    min_gap.max(0.0)
  } else {
    max_gap.max(0.0)
  }
}

impl PaintedSide {
  pub(crate) fn is_visible(&self) -> bool {
    self.style.is_rendered() && self.width > 0.0
  }

  /// The side's colour shaded for the `inset` or `outset` half of a 3D style.
  pub(crate) fn shaded(&self, style: BorderStyle) -> Color {
    self.color.inset_outset(self.side.darkened_by(style))
  }
}

impl BorderProperties {
  /// The strips a side fills, outermost first.
  pub fn side_bands(&self, side: PaintedSide) -> SmallVec<[SideBand; 2]> {
    let mut bands = SmallVec::new();
    let color = side.color;

    match side.style {
      BorderStyle::Dashed | BorderStyle::Dotted => {}
      BorderStyle::Double => {
        let width = self.width.map(|value| value / 3.0);

        bands.push(SideBand {
          inset: Rect::ZERO,
          width,
          color,
        });
        bands.push(SideBand {
          inset: self.width.map(|value| value * (2.0 / 3.0)),
          width,
          color,
        });
      }
      BorderStyle::Inset | BorderStyle::Outset => bands.push(SideBand {
        inset: Rect::ZERO,
        width: self.width,
        color: side.shaded(side.style),
      }),
      BorderStyle::Groove | BorderStyle::Ridge => {
        let outer_width = self.width.map(|value| value / 2.0);
        let (outer, inner) = match side.style {
          BorderStyle::Groove => (BorderStyle::Inset, BorderStyle::Outset),
          _ => (BorderStyle::Outset, BorderStyle::Inset),
        };

        bands.push(SideBand {
          inset: Rect::ZERO,
          width: outer_width,
          color: side.shaded(outer),
        });
        bands.push(SideBand {
          inset: outer_width,
          width: self.width.saturating_sub(outer_width),
          color: side.shaded(inner),
        });
      }
      _ => bands.push(SideBand {
        inset: Rect::ZERO,
        width: self.width,
        color,
      }),
    }

    bands
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    geometry::{PathCommand, Point, Rect, Size},
    layout::border::{BorderProperties, BorderSide},
    style::{Sides, SpacePair},
  };

  fn collapsed_border() -> BorderProperties {
    BorderProperties {
      width: Rect {
        top: 4.0,
        right: 1.0,
        bottom: 1.0,
        left: 1.0,
      },
      collapsed: true,
      ..BorderProperties::default()
    }
  }

  fn xs(side: BorderSide) -> Vec<Point<f32>> {
    let mut path = Vec::new();

    collapsed_border().append_side_polygon_commands_at(
      side,
      &mut path,
      Size {
        width: 100.0,
        height: 50.0,
      },
      Point::ZERO,
    );

    path
      .into_iter()
      .filter_map(|command| match command {
        PathCommand::MoveTo(point) | PathCommand::LineTo(point) => Some(point),
        _ => None,
      })
      .collect()
  }

  #[test]
  fn the_wider_side_takes_the_whole_corner() {
    let top = xs(BorderSide::Top);

    assert!(top.iter().all(|point| point.x == 0.0 || point.x == 100.0));
    assert!(top.iter().all(|point| point.y == 0.0 || point.y == 4.0));
  }

  #[test]
  fn the_narrower_side_starts_after_it() {
    let left = xs(BorderSide::Left);

    assert!(left.iter().all(|point| point.x == 0.0 || point.x == 1.0));
    assert!(left.iter().all(|point| point.y == 4.0 || point.y == 49.0));
  }

  #[test]
  fn an_outset_keeps_square_corners_square() {
    let mut border = BorderProperties::default();

    border.outset_radii(BOX, 10.0);

    assert!(border.is_zero());
  }

  #[test]
  fn an_outset_grows_a_small_corner_less_than_the_outset() {
    let mut border = BorderProperties {
      radius: Sides([SpacePair::from_single(2.0); 4]),
      ..BorderProperties::default()
    };

    border.outset_radii(BOX, 10.0);

    // 2 + 10 * (1 - (1 - 0.2)^3 * (1 - 0.04^3))
    assert!((border.radius.0[0].x - 6.88).abs() < 1e-3);
  }

  const BOX: Size<f32> = Size {
    width: 100.0,
    height: 100.0,
  };
}
