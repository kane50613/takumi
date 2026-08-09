use std::f32::consts::{PI, SQRT_2};

use smallvec::SmallVec;

use crate::{
  context::RenderContext,
  geometry::{PathBuilder, PathCommand as Command, Point, Rect, Size},
  layout::corner_shape::{CornerContour, contour_arc_length, corner_contour},
  style::{BorderStyle, Color, ImageScalingAlgorithm, Sides, SpacePair, Superellipse},
};

/// Border side identifier used by per-side geometry and rasterization.
#[derive(Debug, Clone, Copy)]
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

/// One strip of a border side: where it sits, how thick it is, and what colour
/// fills it. `solid` and the 3D bevels paint one; `double` and `groove`/`ridge`
/// paint two.
#[derive(Debug, Clone, Copy)]
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
}

impl BorderProperties {
  /// The amount of path commands to append for this border.
  /// This is used to pre-allocate the vector size for the mask commands.
  pub const PATH_COMMANDS_AMOUNT: usize = 14;

  /// Resolves the border radius from the context and layout.
  pub fn resolve_radius_part(
    context: &RenderContext,
    border_box: Size<f32>,
  ) -> Sides<SpacePair<f32>> {
    let top_left = context.style.border_top_left_radius.to_px(
      &context.sizing,
      border_box.width,
      border_box.height,
    );
    let top_right = context.style.border_top_right_radius.to_px(
      &context.sizing,
      border_box.width,
      border_box.height,
    );
    let bottom_right = context.style.border_bottom_right_radius.to_px(
      &context.sizing,
      border_box.width,
      border_box.height,
    );
    let bottom_left = context.style.border_bottom_left_radius.to_px(
      &context.sizing,
      border_box.width,
      border_box.height,
    );

    Sides([top_left, top_right, bottom_right, bottom_left])
  }

  /// Resolves the corner shapes from the context.
  pub(crate) fn resolve_shape_part(context: &RenderContext) -> Sides<Superellipse> {
    Sides([
      context.style.corner_top_left_shape,
      context.style.corner_top_right_shape,
      context.style.corner_bottom_right_shape,
      context.style.corner_bottom_left_shape,
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
          .border_top_color
          .resolve(context.current_color),
        right: context
          .style
          .border_right_color
          .resolve(context.current_color),
        bottom: context
          .style
          .border_bottom_color
          .resolve(context.current_color),
        left: context
          .style
          .border_left_color
          .resolve(context.current_color),
      },
      radius: Self::resolve_radius_part(context, border_box),
      shape: Self::resolve_shape_part(context),
      style: Rect {
        top: context.style.border_top_style,
        right: context.style.border_right_style,
        bottom: context.style.border_bottom_style,
        left: context.style.border_left_style,
      },
      image_rendering: context.style.image_rendering,
    }
  }

  /// True if a side with this style and width is rendered.
  pub fn is_side_visible(style: BorderStyle, width: f32) -> bool {
    style.is_rendered() && width > 0.0
  }

  /// The sides that put ink on the page, clockwise from the top. A side is left
  /// out when it has no width, when its style draws nothing, or when its colour
  /// is fully transparent. Every backend that walks the sides one by one walks
  /// this list, so an all-transparent border yields nothing and the caller can
  /// skip the clip and the marked-content region it would otherwise open.
  pub fn painted_sides(&self) -> impl Iterator<Item = PaintedSide> {
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
    .into_iter()
    .filter_map(|(side, width, color, style)| {
      (Self::is_side_visible(style, width) && color.0[3] != 0).then_some(PaintedSide {
        side,
        width,
        color,
        style,
      })
    })
  }

  /// True if any side is rendered with nonzero width.
  pub fn has_visible_sides(&self) -> bool {
    Self::is_side_visible(self.style.top, self.width.top)
      || Self::is_side_visible(self.style.right, self.width.right)
      || Self::is_side_visible(self.style.bottom, self.width.bottom)
      || Self::is_side_visible(self.style.left, self.width.left)
  }

  /// Per-side widths with invisible sides zeroed.
  pub fn visible_side_widths(&self) -> Rect<f32> {
    Rect {
      top: if Self::is_side_visible(self.style.top, self.width.top) {
        self.width.top
      } else {
        0.0
      },
      right: if Self::is_side_visible(self.style.right, self.width.right) {
        self.width.right
      } else {
        0.0
      },
      bottom: if Self::is_side_visible(self.style.bottom, self.width.bottom) {
        self.width.bottom
      } else {
        0.0
      },
      left: if Self::is_side_visible(self.style.left, self.width.left) {
        self.width.left
      } else {
        0.0
      },
    }
  }

  /// The shared color if all visible sides match, else `None`.
  pub fn has_uniform_visible_color(&self) -> Option<Color> {
    let mut color = None;

    if Self::is_side_visible(self.style.top, self.width.top) {
      color = Some(self.color.top);
    }
    if Self::is_side_visible(self.style.right, self.width.right) {
      if let Some(existing) = color {
        if existing != self.color.right {
          return None;
        }
      } else {
        color = Some(self.color.right);
      }
    }
    if Self::is_side_visible(self.style.bottom, self.width.bottom) {
      if let Some(existing) = color {
        if existing != self.color.bottom {
          return None;
        }
      } else {
        color = Some(self.color.bottom);
      }
    }
    if Self::is_side_visible(self.style.left, self.width.left) {
      if let Some(existing) = color {
        if existing != self.color.left {
          return None;
        }
      } else {
        color = Some(self.color.left);
      }
    }

    color
  }

  /// True if all visible sides use the given style.
  pub fn visible_sides_match(&self, style: BorderStyle) -> bool {
    (!Self::is_side_visible(self.style.top, self.width.top) || self.style.top == style)
      && (!Self::is_side_visible(self.style.right, self.width.right) || self.style.right == style)
      && (!Self::is_side_visible(self.style.bottom, self.width.bottom)
        || self.style.bottom == style)
      && (!Self::is_side_visible(self.style.left, self.width.left) || self.style.left == style)
  }

  /// True if every side has equal nonzero width and the given style.
  pub fn is_uniform_all_sides_style(&self, style: BorderStyle) -> bool {
    let has_uniform_width = self.width.top > 0.0
      && (self.width.top - self.width.right).abs() <= f32::EPSILON
      && (self.width.top - self.width.bottom).abs() <= f32::EPSILON
      && (self.width.top - self.width.left).abs() <= f32::EPSILON;

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
    border.append_mask_commands(paths, inner_size, inner_offset);
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

    let inner_left = self.width.left.min(border_box.width);
    let inner_right = (border_box.width - self.width.right).max(inner_left);
    let inner_top = self.width.top.min(border_box.height);
    let inner_bottom = (border_box.height - self.width.bottom).max(inner_top);

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

  /// Appends a clip polygon for one side that follows the rounded inner contour.
  pub fn append_side_clip_polygon_commands_at(
    &self,
    side: BorderSide,
    path: &mut Vec<Command>,
    border_box: Size<f32>,
    offset: Point<f32>,
  ) {
    if border_box.width <= 0.0 || border_box.height <= 0.0 {
      return;
    }

    if self.is_zero() {
      self.append_side_polygon_commands_at(side, path, border_box, offset);
      return;
    }

    let outer_left = offset.x;
    let outer_top = offset.y;
    let outer_right = offset.x + border_box.width;
    let outer_bottom = offset.y + border_box.height;

    let inner_left = outer_left + self.width.left.min(border_box.width);
    let inner_top = outer_top + self.width.top.min(border_box.height);
    let inner_right = (outer_right - self.width.right).max(inner_left);
    let inner_bottom = (outer_bottom - self.width.bottom).max(inner_top);

    let inner_size = inset_size(border_box, self.width);
    let mut inner_border = *self;
    inner_border.inset_by_border_width();
    let inner_radii = inner_border.scaled_corner_radii(inner_size);
    let [top_left, top_right, bottom_right, bottom_left] = inner_radii.0;

    let outer_tl = Point {
      x: outer_left,
      y: outer_top,
    };
    let outer_tr = Point {
      x: outer_right,
      y: outer_top,
    };
    let outer_br = Point {
      x: outer_right,
      y: outer_bottom,
    };
    let outer_bl = Point {
      x: outer_left,
      y: outer_bottom,
    };

    let mut inner_tl = Point {
      x: inner_left,
      y: inner_top,
    };
    let mut inner_tr = Point {
      x: inner_right,
      y: inner_top,
    };
    let mut inner_br = Point {
      x: inner_right,
      y: inner_bottom,
    };
    let mut inner_bl = Point {
      x: inner_left,
      y: inner_bottom,
    };

    match side {
      BorderSide::Top => {
        inner_tl = side_clip_inner_corner(self.shape.0[0], outer_tl, inner_tl, top_left, 1.0, 1.0);
        inner_tr =
          side_clip_inner_corner(self.shape.0[1], outer_tr, inner_tr, top_right, -1.0, 1.0);
        path.move_to((outer_tl.x, outer_tl.y));
        path.line_to((inner_tl.x, inner_tl.y));
        path.line_to((inner_tr.x, inner_tr.y));
        path.line_to((outer_tr.x, outer_tr.y));
      }
      BorderSide::Right => {
        inner_tr =
          side_clip_inner_corner(self.shape.0[1], outer_tr, inner_tr, top_right, -1.0, 1.0);
        inner_br = side_clip_inner_corner(
          self.shape.0[2],
          outer_br,
          inner_br,
          bottom_right,
          -1.0,
          -1.0,
        );
        path.move_to((outer_tr.x, outer_tr.y));
        path.line_to((inner_tr.x, inner_tr.y));
        path.line_to((inner_br.x, inner_br.y));
        path.line_to((outer_br.x, outer_br.y));
      }
      BorderSide::Bottom => {
        inner_bl =
          side_clip_inner_corner(self.shape.0[3], outer_bl, inner_bl, bottom_left, 1.0, -1.0);
        inner_br = side_clip_inner_corner(
          self.shape.0[2],
          outer_br,
          inner_br,
          bottom_right,
          -1.0,
          -1.0,
        );
        path.move_to((outer_br.x, outer_br.y));
        path.line_to((inner_br.x, inner_br.y));
        path.line_to((inner_bl.x, inner_bl.y));
        path.line_to((outer_bl.x, outer_bl.y));
      }
      BorderSide::Left => {
        inner_tl = side_clip_inner_corner(self.shape.0[0], outer_tl, inner_tl, top_left, 1.0, 1.0);
        inner_bl =
          side_clip_inner_corner(self.shape.0[3], outer_bl, inner_bl, bottom_left, 1.0, -1.0);
        path.move_to((outer_bl.x, outer_bl.y));
        path.line_to((inner_bl.x, inner_bl.y));
        path.line_to((inner_tl.x, inner_tl.y));
        path.line_to((outer_tl.x, outer_tl.y));
      }
    }

    path.close();
  }

  /// Returns true if all corner radii are zero.
  #[inline]
  pub fn is_zero(&self) -> bool {
    const ZERO: Sides<SpacePair<f32>> = Sides([SpacePair::from_single(0.0); 4]);

    self.radius == ZERO
  }

  /// Expand or shrink corner radii by the specified amounts.
  ///
  /// Each corner's x-radius is adjusted by the corresponding horizontal side (left or right),
  /// and each corner's y-radius is adjusted by the corresponding vertical side (top or bottom).
  /// Negative values in `amount` will shrink the radii, and the result is clamped to 0.0.
  pub fn expand_by(&mut self, amount: Rect<f32>) {
    if amount == Rect::ZERO {
      return;
    }

    // top-left
    self.radius.0[0].x = (self.radius.0[0].x + amount.left).max(0.0);
    self.radius.0[0].y = (self.radius.0[0].y + amount.top).max(0.0);

    // top-right
    self.radius.0[1].x = (self.radius.0[1].x + amount.right).max(0.0);
    self.radius.0[1].y = (self.radius.0[1].y + amount.top).max(0.0);

    // bottom-right
    self.radius.0[2].x = (self.radius.0[2].x + amount.right).max(0.0);
    self.radius.0[2].y = (self.radius.0[2].y + amount.bottom).max(0.0);

    // bottom-left
    self.radius.0[3].x = (self.radius.0[3].x + amount.left).max(0.0);
    self.radius.0[3].y = (self.radius.0[3].y + amount.bottom).max(0.0);
  }

  /// Shrink radii by the border width to get inner radius path.
  /// Each side's border width is applied independently to the corresponding radius components.
  pub fn inset_by_border_width(&mut self) {
    self.expand_by(self.width.map(|size| -size))
  }

  /// Outset `box-shadow` shape: a copy with corner radii expanded by `spread` on every side,
  /// paired with the spread-expanded box size. Shared by the raster and svg backends.
  pub fn outset_shadow_box(&self, size: Size<f32>, spread: f32) -> (Self, Size<f32>) {
    let mut expanded = *self;
    expanded.expand_by(Rect {
      top: spread,
      right: spread,
      bottom: spread,
      left: spread,
    });

    let spread_size = Size {
      width: (size.width + 2.0 * spread).max(0.0),
      height: (size.height + 2.0 * spread).max(0.0),
    };

    (expanded, spread_size)
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

  /// Shrinks diagonally-opposite corner pairs involving a concave shape until
  /// their corner boxes no longer overlap, so concave contours cannot
  /// self-intersect. Coarser than Chromium's hull-based solve, which scales by
  /// the curve hull instead of the full corner box.
  fn constrain_concave_pairs(&self, radii: &mut Sides<SpacePair<f32>>, border_box: Size<f32>) {
    for (a, b) in [(0, 2), (1, 3)] {
      if !self.shape.0[a].is_concave() && !self.shape.0[b].is_concave() {
        continue;
      }

      let sum_x = radii.0[a].x + radii.0[b].x;
      let sum_y = radii.0[a].y + radii.0[b].y;

      if sum_x > border_box.width && sum_y > border_box.height {
        let factor = (border_box.width / sum_x).max(border_box.height / sum_y);

        for corner in [a, b] {
          radii.0[corner].x *= factor;
          radii.0[corner].y *= factor;
        }
      }
    }
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

    // The magic number for the cubic bezier curve
    const KAPPA: f32 = 4.0 / 3.0 * (SQRT_2 - 1.0);

    let radii = self.scaled_corner_radii(border_box);
    let [top_left, top_right, bottom_right, bottom_left] = radii.0;

    // --- Top Edge ---
    // Start after Top-Left corner
    path.move_to((offset.x + top_left.x, offset.y));

    // Line to start of Top-Right corner
    path.line_to((offset.x + border_box.width - top_right.x, offset.y));

    // --- Top-Right Corner ---
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

    // --- Right Edge ---
    path.line_to((
      offset.x + border_box.width,
      offset.y + border_box.height - bottom_right.y,
    ));

    // --- Bottom-Right Corner ---
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

    // --- Bottom Edge ---
    path.line_to((offset.x + bottom_left.x, offset.y + border_box.height));

    // --- Bottom-Left Corner ---
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

    // --- Left Edge ---
    path.line_to((offset.x, offset.y + top_left.y));

    // --- Top-Left Corner ---
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

  /// Perimeter of the border-box outline, including rounded corner arcs.
  pub fn approximate_rounded_rect_perimeter(&self, border_box: Size<f32>) -> f32 {
    if border_box.width <= 0.0 || border_box.height <= 0.0 {
      return 0.0;
    }

    let radii = self.scaled_corner_radii(border_box);
    let [top_left, top_right, bottom_right, bottom_left] = radii.0;

    let top = (border_box.width - top_left.x - top_right.x).max(0.0);
    let right = (border_box.height - top_right.y - bottom_right.y).max(0.0);
    let bottom = (border_box.width - bottom_left.x - bottom_right.x).max(0.0);
    let left = (border_box.height - top_left.y - bottom_left.y).max(0.0);

    top
      + right
      + bottom
      + left
      + corner_arc_length(self.shape.0[0], top_left.x, top_left.y)
      + corner_arc_length(self.shape.0[1], top_right.x, top_right.y)
      + corner_arc_length(self.shape.0[2], bottom_right.x, bottom_right.y)
      + corner_arc_length(self.shape.0[3], bottom_left.x, bottom_left.y)
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

    self.constrain_concave_pairs(&mut scaled, border_box);

    scaled
  }
}

/// Top-left corner of a rect as a point.
pub fn rect_offset(rect: Rect<f32>) -> Point<f32> {
  Point {
    x: rect.left,
    y: rect.top,
  }
}

/// The corner point where a side's clip polygon meets the inner contour.
///
/// Convex corners miter along the line from the outer corner through the inner
/// rect corner to the inner curve's chord. Concave contours reach past that
/// chord to the corner's center point, so the polygon clips there instead —
/// otherwise the notch/scoop flank would fall outside every side's clip.
/// `direction_x`/`direction_y` are `±1.0` pointing from the inner rect corner
/// toward the box center.
fn side_clip_inner_corner(
  shape: Superellipse,
  outer: Point<f32>,
  inner: Point<f32>,
  radius: SpacePair<f32>,
  direction_x: f32,
  direction_y: f32,
) -> Point<f32> {
  if radius.x <= 0.0 || radius.y <= 0.0 {
    return inner;
  }

  let chord_x = Point {
    x: inner.x + direction_x * radius.x,
    y: inner.y,
  };
  let chord_y = Point {
    x: inner.x,
    y: inner.y + direction_y * radius.y,
  };

  if shape.is_concave() {
    return Point {
      x: chord_x.x,
      y: chord_y.y,
    };
  }

  line_intersection(outer, inner, chord_x, chord_y).unwrap_or(inner)
}

pub(crate) fn line_intersection(
  a0: Point<f32>,
  a1: Point<f32>,
  b0: Point<f32>,
  b1: Point<f32>,
) -> Option<Point<f32>> {
  let denom = (a0.x - a1.x) * (b0.y - b1.y) - (a0.y - a1.y) * (b0.x - b1.x);
  if denom.abs() < 1e-6 {
    return None;
  }

  let a_cross = a0.x * a1.y - a0.y * a1.x;
  let b_cross = b0.x * b1.y - b0.y * b1.x;

  Some(Point {
    x: (a_cross * (b0.x - b1.x) - (a0.x - a1.x) * b_cross) / denom,
    y: (a_cross * (b0.y - b1.y) - (a0.y - a1.y) * b_cross) / denom,
  })
}

/// Appends a non-`round` corner contour, mapping normalized contour points
/// through `anchor + p[0] * u + p[1] * v` into pixel space.
fn append_shaped_corner(
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

/// Length of one corner's outline arc, honoring its `corner-shape`.
fn corner_arc_length(shape: Superellipse, radius_x: f32, radius_y: f32) -> f32 {
  if radius_x <= 0.0 || radius_y <= 0.0 {
    return 0.0;
  }

  if shape.is_round() {
    return approximate_quarter_ellipse_arc_length(radius_x, radius_y);
  }

  contour_arc_length(&corner_contour(shape), radius_x, radius_y)
}

pub(crate) fn approximate_quarter_ellipse_arc_length(radius_x: f32, radius_y: f32) -> f32 {
  if radius_x <= 0.0 || radius_y <= 0.0 {
    return 0.0;
  }

  // Ramanujan II approximation for ellipse circumference.
  let sum = radius_x + radius_y;
  let diff = radius_x - radius_y;
  let h = (diff * diff) / (sum * sum);
  let circumference = PI * sum * (1.0 + (3.0 * h) / (10.0 + (4.0 - 3.0 * h).sqrt()));
  circumference / 4.0
}

/// Shrinks a size by the given inset on each edge, clamped to zero.
pub fn inset_size(size: Size<f32>, inset: Rect<f32>) -> Size<f32> {
  Size {
    width: (size.width - inset.left - inset.right).max(0.0),
    height: (size.height - inset.top - inset.bottom).max(0.0),
  }
}

/// Per-side `lhs - rhs`, clamped to zero.
pub(crate) fn subtract_rect(lhs: Rect<f32>, rhs: Rect<f32>) -> Rect<f32> {
  Rect {
    top: (lhs.top - rhs.top).max(0.0),
    right: (lhs.right - rhs.right).max(0.0),
    bottom: (lhs.bottom - rhs.bottom).max(0.0),
    left: (lhs.left - rhs.left).max(0.0),
  }
}

const DASHED_THICK_WIDTH_THRESHOLD: f32 = 3.0;
const DASHED_LENGTH_RATIO_THICK: f32 = 2.0;
const DASHED_LENGTH_RATIO_THIN: f32 = 3.0;
const DASHED_GAP_RATIO_THICK: f32 = 1.0;
const DASHED_GAP_RATIO_THIN: f32 = 2.0;
const DOTTED_ENDPOINT_EPSILON: f32 = 1.0e-2;

/// The dash pattern for a stroked `dashed`/`dotted` border or outline side,
/// shared by both backends: `([dash, gap], round_cap)`, or `None` for a solid
/// stroke (non-dash style, or a segment too short to dash). `length` is the side
/// length (or the ring perimeter when `closed`). Dash/gap adjust by width and
/// length so the pattern fits the side evenly.
/// How a border paints as a whole, before any per-side work.
///
/// A uniform dashed, dotted, or double border cannot be filled side by side:
/// the pattern has to run around the whole ring, so it strokes a centerline
/// instead. Every backend makes this call the same way.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum BorderPaint {
  /// Stroke the centerline with the dash pattern for `style`.
  Stroked {
    /// The uniform colour.
    color: Color,
    /// The uniform width.
    width: f32,
    /// `Dashed` or `Dotted`.
    style: BorderStyle,
  },
  /// Two concentric rings.
  Double {
    /// The uniform colour.
    color: Color,
    /// The uniform width.
    width: f32,
  },
  /// One even-odd fill of the whole ring.
  Ring {
    /// The uniform colour.
    color: Color,
  },
  /// Each side on its own.
  Sides,
}

/// Decides how `border` paints. See [`BorderPaint`].
pub(crate) fn border_paint(border: &BorderProperties) -> BorderPaint {
  let Some(color) = border.has_uniform_visible_color() else {
    return BorderPaint::Sides;
  };
  let width = border.width.top;

  for style in [BorderStyle::Dashed, BorderStyle::Dotted] {
    if border.is_uniform_all_sides_style(style) {
      return BorderPaint::Stroked {
        color,
        width,
        style,
      };
    }
  }
  if border.is_uniform_all_sides_style(BorderStyle::Double) {
    return BorderPaint::Double { color, width };
  }
  // Only a solid side fills as part of one ring: a dashed or dotted side breaks
  // the ring into segments, and the 3D bevels shade each side differently.
  if !border.visible_sides_match(BorderStyle::Solid) {
    return BorderPaint::Sides;
  }

  BorderPaint::Ring { color }
}

/// The dash pattern for a stroked `dashed`/`dotted` border or outline side,
/// shared by every backend: `([dash, gap], round_cap)`, or `None` for a solid
/// stroke (non-dash style, or a segment too short to dash). `length` is the side
/// length (or the ring perimeter when `closed`). Dash/gap adjust by width and
/// length so the pattern fits the side evenly.
pub fn border_dash_pattern(
  width: f32,
  style: BorderStyle,
  length: f32,
  closed: bool,
) -> Option<([f32; 2], bool)> {
  if !matches!(style, BorderStyle::Dashed | BorderStyle::Dotted) || width <= 0.0 || length <= 0.0 {
    return None;
  }

  if style == BorderStyle::Dashed {
    let (dash, gap) = compute_dashed_intervals(width, length, closed)?;
    return Some(([dash, gap], false));
  }

  let per_dot_length = width * 2.0;
  let gap = if length < per_dot_length {
    per_dot_length
  } else {
    select_best_dash_gap(length, width, width, closed) + width - DOTTED_ENDPOINT_EPSILON
  };
  Some(([0.0, gap], true))
}

fn compute_dashed_intervals(width: f32, length: f32, closed: bool) -> Option<(f32, f32)> {
  let thick = width >= DASHED_THICK_WIDTH_THRESHOLD;
  let dash = width
    * if thick {
      DASHED_LENGTH_RATIO_THICK
    } else {
      DASHED_LENGTH_RATIO_THIN
    };
  let gap = width
    * if thick {
      DASHED_GAP_RATIO_THICK
    } else {
      DASHED_GAP_RATIO_THIN
    };

  if length <= dash * 2.0 {
    return None;
  }

  let mut applied_dash = dash;
  let mut applied_gap = gap;
  let mut two_dashes_with_gap = 2.0 * dash + gap;
  if closed {
    two_dashes_with_gap += gap;
  }
  if length <= two_dashes_with_gap {
    let multiplier = length / two_dashes_with_gap;
    applied_dash *= multiplier;
    applied_gap *= multiplier;
  } else {
    applied_gap = select_best_dash_gap(length, dash, gap, closed);
  }
  Some((applied_dash, applied_gap))
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

/// Lightens or darkens a side color for `inset`/`outset` 3D border shading.
pub(crate) fn shade_3d_border_color(color: Color, side: BorderSide, style: BorderStyle) -> Color {
  let lighten = match style {
    BorderStyle::Outset => matches!(side, BorderSide::Top | BorderSide::Left),
    BorderStyle::Inset => matches!(side, BorderSide::Right | BorderSide::Bottom),
    _ => false,
  };

  mix_color(
    color,
    if lighten {
      Color::white()
    } else {
      Color::black()
    },
    0.35,
  )
}

/// The strips a side fills, outermost first.
///
/// A dashed or dotted side has none: it strokes a centerline instead. The 3D
/// bevels shade their strips, which is the only thing that separates them from
/// a plain `solid` side.
pub fn side_bands(border: &BorderProperties, side: PaintedSide) -> SmallVec<[SideBand; 2]> {
  let mut bands = SmallVec::new();
  let color = side.color;
  let shaded = |style| shade_3d_border_color(color, side.side, style);

  match side.style {
    BorderStyle::Dashed | BorderStyle::Dotted => {}
    BorderStyle::Double => {
      let width = border.width.map(|value| value / 3.0);

      bands.push(SideBand {
        inset: Rect::ZERO,
        width,
        color,
      });
      bands.push(SideBand {
        inset: border.width.map(|value| value * (2.0 / 3.0)),
        width,
        color,
      });
    }
    BorderStyle::Inset | BorderStyle::Outset => bands.push(SideBand {
      inset: Rect::ZERO,
      width: border.width,
      color: shaded(side.style),
    }),
    BorderStyle::Groove | BorderStyle::Ridge => {
      let outer_width = border.width.map(|value| value / 2.0);
      let (outer, inner) = match side.style {
        BorderStyle::Groove => (BorderStyle::Inset, BorderStyle::Outset),
        _ => (BorderStyle::Outset, BorderStyle::Inset),
      };

      bands.push(SideBand {
        inset: Rect::ZERO,
        width: outer_width,
        color: shaded(outer),
      });
      bands.push(SideBand {
        inset: outer_width,
        width: subtract_rect(border.width, outer_width),
        color: shaded(inner),
      });
    }
    _ => bands.push(SideBand {
      inset: Rect::ZERO,
      width: border.width,
      color,
    }),
  }

  bands
}

pub(crate) fn mix_color(color: Color, target: Color, amount: f32) -> Color {
  let amount = amount.clamp(0.0, 1.0);
  let inverse = 1.0 - amount;

  Color([
    (color.0[0] as f32 * inverse + target.0[0] as f32 * amount).round() as u8,
    (color.0[1] as f32 * inverse + target.0[1] as f32 * amount).round() as u8,
    (color.0[2] as f32 * inverse + target.0[2] as f32 * amount).round() as u8,
    color.0[3],
  ])
}
