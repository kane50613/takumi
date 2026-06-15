use std::f32::consts::{PI, SQRT_2};

use taffy::{Point, Rect, Size};
use tiny_skia::{PathSegment as Command, Point as TinyPoint};

use crate::{
  context::RenderContext,
  layout::style::{BorderStyle, Color, ImageScalingAlgorithm, Sides, SpacePair},
};

/// Border side identifier used by per-side geometry and rasterization.
#[derive(Clone, Copy)]
pub enum BorderSide {
  Top,
  Right,
  Bottom,
  Left,
}

pub(crate) trait BorderPath {
  fn move_to(&mut self, point: (f32, f32));
  fn line_to(&mut self, point: (f32, f32));
  fn curve_to(&mut self, p1: (f32, f32), p2: (f32, f32), p3: (f32, f32));
  fn close(&mut self);
}

impl BorderPath for Vec<Command> {
  fn move_to(&mut self, point: (f32, f32)) {
    self.push(Command::MoveTo(TinyPoint::from_xy(point.0, point.1)));
  }

  fn line_to(&mut self, point: (f32, f32)) {
    self.push(Command::LineTo(TinyPoint::from_xy(point.0, point.1)));
  }

  fn curve_to(&mut self, p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) {
    self.push(Command::CubicTo(
      TinyPoint::from_xy(p1.0, p1.1),
      TinyPoint::from_xy(p2.0, p2.1),
      TinyPoint::from_xy(p3.0, p3.1),
    ));
  }

  fn close(&mut self) {
    self.push(Command::Close);
  }
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
  /// The style of each border side.
  pub style: Rect<BorderStyle>,
  /// The image rendering algorithm to use when sampling the image.
  pub image_rendering: ImageScalingAlgorithm,
}

impl BorderProperties {
  /// The amount of path commands to append for this border.
  /// This is used to pre-allocate the vector size for the mask commands.
  pub const PATH_COMMANDS_AMOUNT: usize = 10;

  /// Resolves the border radius from the context and layout.
  pub fn resolve_radius_part(
    context: &RenderContext,
    border_box: Size<f32>,
  ) -> Sides<SpacePair<f32>> {
    let top_left = context
      .style
      .border_top_left_radius
      .to_px(&context.sizing, border_box);
    let top_right = context
      .style
      .border_top_right_radius
      .to_px(&context.sizing, border_box);
    let bottom_right = context
      .style
      .border_bottom_right_radius
      .to_px(&context.sizing, border_box);
    let bottom_left = context
      .style
      .border_bottom_left_radius
      .to_px(&context.sizing, border_box);

    Sides([top_left, top_right, bottom_right, bottom_left])
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
      style: Rect {
        top: context.style.border_top_style,
        right: context.style.border_right_style,
        bottom: context.style.border_bottom_style,
        left: context.style.border_left_style,
      },
      image_rendering: context.style.image_rendering,
    }
  }

  pub fn is_side_visible(style: BorderStyle, width: f32) -> bool {
    style.is_rendered() && width > 0.0
  }

  pub fn has_visible_sides(&self) -> bool {
    Self::is_side_visible(self.style.top, self.width.top)
      || Self::is_side_visible(self.style.right, self.width.right)
      || Self::is_side_visible(self.style.bottom, self.width.bottom)
      || Self::is_side_visible(self.style.left, self.width.left)
  }

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

  pub fn visible_sides_match(&self, style: BorderStyle) -> bool {
    (!Self::is_side_visible(self.style.top, self.width.top) || self.style.top == style)
      && (!Self::is_side_visible(self.style.right, self.width.right) || self.style.right == style)
      && (!Self::is_side_visible(self.style.bottom, self.width.bottom)
        || self.style.bottom == style)
      && (!Self::is_side_visible(self.style.left, self.width.left) || self.style.left == style)
  }

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

  pub fn append_border_ring_commands(&self, paths: &mut Vec<Command>, border_box: Size<f32>) {
    self.append_border_ring_commands_at(paths, border_box, Point::ZERO);
  }

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
        if top_left.x > 0.0 && top_left.y > 0.0 {
          inner_tl = line_intersection(
            outer_tl,
            inner_tl,
            Point {
              x: inner_tl.x + top_left.x,
              y: inner_tl.y,
            },
            Point {
              x: inner_tl.x,
              y: inner_tl.y + top_left.y,
            },
          )
          .unwrap_or(inner_tl);
        }
        if top_right.x > 0.0 && top_right.y > 0.0 {
          inner_tr = line_intersection(
            outer_tr,
            inner_tr,
            Point {
              x: inner_tr.x - top_right.x,
              y: inner_tr.y,
            },
            Point {
              x: inner_tr.x,
              y: inner_tr.y + top_right.y,
            },
          )
          .unwrap_or(inner_tr);
        }
        path.move_to((outer_tl.x, outer_tl.y));
        path.line_to((inner_tl.x, inner_tl.y));
        path.line_to((inner_tr.x, inner_tr.y));
        path.line_to((outer_tr.x, outer_tr.y));
      }
      BorderSide::Right => {
        if top_right.x > 0.0 && top_right.y > 0.0 {
          inner_tr = line_intersection(
            outer_tr,
            inner_tr,
            Point {
              x: inner_tr.x - top_right.x,
              y: inner_tr.y,
            },
            Point {
              x: inner_tr.x,
              y: inner_tr.y + top_right.y,
            },
          )
          .unwrap_or(inner_tr);
        }
        if bottom_right.x > 0.0 && bottom_right.y > 0.0 {
          inner_br = line_intersection(
            outer_br,
            inner_br,
            Point {
              x: inner_br.x - bottom_right.x,
              y: inner_br.y,
            },
            Point {
              x: inner_br.x,
              y: inner_br.y - bottom_right.y,
            },
          )
          .unwrap_or(inner_br);
        }
        path.move_to((outer_tr.x, outer_tr.y));
        path.line_to((inner_tr.x, inner_tr.y));
        path.line_to((inner_br.x, inner_br.y));
        path.line_to((outer_br.x, outer_br.y));
      }
      BorderSide::Bottom => {
        if bottom_left.x > 0.0 && bottom_left.y > 0.0 {
          inner_bl = line_intersection(
            outer_bl,
            inner_bl,
            Point {
              x: inner_bl.x + bottom_left.x,
              y: inner_bl.y,
            },
            Point {
              x: inner_bl.x,
              y: inner_bl.y - bottom_left.y,
            },
          )
          .unwrap_or(inner_bl);
        }
        if bottom_right.x > 0.0 && bottom_right.y > 0.0 {
          inner_br = line_intersection(
            outer_br,
            inner_br,
            Point {
              x: inner_br.x - bottom_right.x,
              y: inner_br.y,
            },
            Point {
              x: inner_br.x,
              y: inner_br.y - bottom_right.y,
            },
          )
          .unwrap_or(inner_br);
        }
        path.move_to((outer_br.x, outer_br.y));
        path.line_to((inner_br.x, inner_br.y));
        path.line_to((inner_bl.x, inner_bl.y));
        path.line_to((outer_bl.x, outer_bl.y));
      }
      BorderSide::Left => {
        if top_left.x > 0.0 && top_left.y > 0.0 {
          inner_tl = line_intersection(
            outer_tl,
            inner_tl,
            Point {
              x: inner_tl.x + top_left.x,
              y: inner_tl.y,
            },
            Point {
              x: inner_tl.x,
              y: inner_tl.y + top_left.y,
            },
          )
          .unwrap_or(inner_tl);
        }
        if bottom_left.x > 0.0 && bottom_left.y > 0.0 {
          inner_bl = line_intersection(
            outer_bl,
            inner_bl,
            Point {
              x: inner_bl.x + bottom_left.x,
              y: inner_bl.y,
            },
            Point {
              x: inner_bl.x,
              y: inner_bl.y - bottom_left.y,
            },
          )
          .unwrap_or(inner_bl);
        }
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

    // Calculate scale factor inline (CSS Overlapping Curves)
    let scale = 1.0f32
      .min(
        if self.radius.0[0].x + self.radius.0[1].x > border_box.width {
          border_box.width / (self.radius.0[0].x + self.radius.0[1].x)
        } else {
          1.0
        },
      )
      .min(
        if self.radius.0[3].x + self.radius.0[2].x > border_box.width {
          border_box.width / (self.radius.0[3].x + self.radius.0[2].x)
        } else {
          1.0
        },
      )
      .min(
        if self.radius.0[0].y + self.radius.0[3].y > border_box.height {
          border_box.height / (self.radius.0[0].y + self.radius.0[3].y)
        } else {
          1.0
        },
      )
      .min(
        if self.radius.0[1].y + self.radius.0[2].y > border_box.height {
          border_box.height / (self.radius.0[1].y + self.radius.0[2].y)
        } else {
          1.0
        },
      );

    // --- Top Edge ---
    // Start after Top-Left corner
    path.move_to((offset.x + (self.radius.0[0].x * scale).max(0.0), offset.y));

    // Line to start of Top-Right corner
    path.line_to((
      offset.x + border_box.width - (self.radius.0[1].x * scale).max(0.0),
      offset.y,
    ));

    // --- Top-Right Corner ---
    if self.radius.0[1].x > 0.0 && self.radius.0[1].y > 0.0 {
      let rx = self.radius.0[1].x * scale;
      let ry = self.radius.0[1].y * scale;
      path.curve_to(
        (offset.x + border_box.width - rx * (1.0 - KAPPA), offset.y),
        (offset.x + border_box.width, offset.y + ry * (1.0 - KAPPA)),
        (offset.x + border_box.width, offset.y + ry),
      );
    } else {
      path.line_to((offset.x + border_box.width, offset.y));
    }

    // --- Right Edge ---
    path.line_to((
      offset.x + border_box.width,
      offset.y + border_box.height - (self.radius.0[2].y * scale).max(0.0),
    ));

    // --- Bottom-Right Corner ---
    if self.radius.0[2].x > 0.0 && self.radius.0[2].y > 0.0 {
      let rx = self.radius.0[2].x * scale;
      let ry = self.radius.0[2].y * scale;
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
      path.line_to((offset.x + border_box.width, offset.y + border_box.height));
    }

    // --- Bottom Edge ---
    path.line_to((
      offset.x + (self.radius.0[3].x * scale).max(0.0),
      offset.y + border_box.height,
    ));

    // --- Bottom-Left Corner ---
    if self.radius.0[3].x > 0.0 && self.radius.0[3].y > 0.0 {
      let rx = self.radius.0[3].x * scale;
      let ry = self.radius.0[3].y * scale;
      path.curve_to(
        (offset.x + rx * (1.0 - KAPPA), offset.y + border_box.height),
        (offset.x, offset.y + border_box.height - ry * (1.0 - KAPPA)),
        (offset.x, offset.y + border_box.height - ry),
      );
    } else {
      path.line_to((offset.x, offset.y + border_box.height));
    }

    // --- Left Edge ---
    path.line_to((offset.x, offset.y + (self.radius.0[0].y * scale).max(0.0)));

    // --- Top-Left Corner ---
    if self.radius.0[0].x > 0.0 && self.radius.0[0].y > 0.0 {
      let rx = self.radius.0[0].x * scale;
      let ry = self.radius.0[0].y * scale;
      path.curve_to(
        (offset.x, offset.y + ry * (1.0 - KAPPA)),
        (offset.x + rx * (1.0 - KAPPA), offset.y),
        (offset.x + rx, offset.y),
      );
    } else {
      path.line_to((offset.x, offset.y));
    }

    path.close();
  }

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
      + approximate_quarter_ellipse_arc_length(top_left.x, top_left.y)
      + approximate_quarter_ellipse_arc_length(top_right.x, top_right.y)
      + approximate_quarter_ellipse_arc_length(bottom_right.x, bottom_right.y)
      + approximate_quarter_ellipse_arc_length(bottom_left.x, bottom_left.y)
  }

  pub fn scaled_corner_radii(&self, border_box: Size<f32>) -> Sides<SpacePair<f32>> {
    // Match `append_mask_commands` overlapping-curves scaling so dash adjustment aligns with the
    // actual rendered contour.
    let scale = 1.0f32
      .min(
        if self.radius.0[0].x + self.radius.0[1].x > border_box.width {
          border_box.width / (self.radius.0[0].x + self.radius.0[1].x)
        } else {
          1.0
        },
      )
      .min(
        if self.radius.0[3].x + self.radius.0[2].x > border_box.width {
          border_box.width / (self.radius.0[3].x + self.radius.0[2].x)
        } else {
          1.0
        },
      )
      .min(
        if self.radius.0[0].y + self.radius.0[3].y > border_box.height {
          border_box.height / (self.radius.0[0].y + self.radius.0[3].y)
        } else {
          1.0
        },
      )
      .min(
        if self.radius.0[1].y + self.radius.0[2].y > border_box.height {
          border_box.height / (self.radius.0[1].y + self.radius.0[2].y)
        } else {
          1.0
        },
      );

    let mut scaled = self.radius;
    for corner in &mut scaled.0 {
      corner.x = (corner.x * scale).max(0.0);
      corner.y = (corner.y * scale).max(0.0);
    }
    scaled
  }
}

pub fn rect_offset(rect: Rect<f32>) -> Point<f32> {
  Point {
    x: rect.left,
    y: rect.top,
  }
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

pub fn inset_size(size: Size<f32>, inset: Rect<f32>) -> Size<f32> {
  Size {
    width: (size.width - inset.left - inset.right).max(0.0),
    height: (size.height - inset.top - inset.bottom).max(0.0),
  }
}

pub fn subtract_rect(lhs: Rect<f32>, rhs: Rect<f32>) -> Rect<f32> {
  Rect {
    top: (lhs.top - rhs.top).max(0.0),
    right: (lhs.right - rhs.right).max(0.0),
    bottom: (lhs.bottom - rhs.bottom).max(0.0),
    left: (lhs.left - rhs.left).max(0.0),
  }
}

pub fn shade_3d_border_color(color: Color, side: BorderSide, style: BorderStyle) -> Color {
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
