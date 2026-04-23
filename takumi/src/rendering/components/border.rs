use std::f32::consts::{PI, SQRT_2};

use taffy::{Point, Rect, Size};

use crate::{
  layout::style::{Affine, BlendMode, BorderStyle, Color, ImageScalingAlgorithm, Sides, SpacePair},
  rendering::{
    Canvas, Cap, Command, DashPattern, Fill, MaskSamplingOptions, PaintSource, PathBuilder,
    Placement, RenderContext, Stroke, Style, fast_div_255, render_mask,
  },
};

/// Represents the properties of a border, including corner radii and drawing metadata.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BorderProperties {
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
  const PATH_COMMANDS_AMOUNT: usize = 10;

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

  fn is_side_visible(style: BorderStyle, width: f32) -> bool {
    style.is_rendered() && width > 0.0
  }

  fn has_visible_sides(&self) -> bool {
    Self::is_side_visible(self.style.top, self.width.top)
      || Self::is_side_visible(self.style.right, self.width.right)
      || Self::is_side_visible(self.style.bottom, self.width.bottom)
      || Self::is_side_visible(self.style.left, self.width.left)
  }

  fn has_uniform_visible_color(&self) -> Option<Color> {
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

  fn visible_sides_match(&self, style: BorderStyle) -> bool {
    (!Self::is_side_visible(self.style.top, self.width.top) || self.style.top == style)
      && (!Self::is_side_visible(self.style.right, self.width.right) || self.style.right == style)
      && (!Self::is_side_visible(self.style.bottom, self.width.bottom)
        || self.style.bottom == style)
      && (!Self::is_side_visible(self.style.left, self.width.left) || self.style.left == style)
  }

  fn append_border_ring_commands(&self, paths: &mut Vec<Command>, border_box: Size<f32>) {
    self.append_border_ring_commands_at(paths, border_box, Point::ZERO);
  }

  fn append_border_ring_commands_at(
    &self,
    paths: &mut Vec<Command>,
    border_box: Size<f32>,
    offset: Point<f32>,
  ) {
    let mut border = *self;

    border.append_mask_commands(paths, border_box, offset);
    let inner_offset = Point {
      x: offset.x + border.width.left,
      y: offset.y + border.width.top,
    };
    let inner_size = border_box
      - Size {
        width: border.width.left + border.width.right,
        height: border.width.top + border.width.bottom,
      };
    border.inset_by_border_width();
    border.append_mask_commands(paths, inner_size, inner_offset);
  }

  fn append_side_polygon_commands_at(
    &self,
    side: BorderSide,
    path: &mut Vec<Command>,
    border_box: Size<f32>,
    offset: Point<f32>,
  ) {
    let inner_left = self.width.left.min(border_box.width);
    let inner_right = (border_box.width - self.width.right).max(0.0);
    let inner_top = self.width.top.min(border_box.height);
    let inner_bottom = (border_box.height - self.width.bottom).max(0.0);

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

  fn append_side_clip_polygon_commands_at(
    &self,
    side: BorderSide,
    path: &mut Vec<Command>,
    border_box: Size<f32>,
    offset: Point<f32>,
  ) {
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

  fn approximate_rounded_rect_perimeter(&self, border_box: Size<f32>) -> f32 {
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

  fn scaled_corner_radii(&self, border_box: Size<f32>) -> Sides<SpacePair<f32>> {
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

  pub(crate) fn draw(
    self,
    canvas: &mut Canvas,
    border_box: Size<f32>,
    transform: Affine,
    clip_image: Option<PaintSource<'_>>,
  ) {
    if let Some(clip_image) = &clip_image {
      assert_eq!(
        (clip_image.width(), clip_image.height()),
        (border_box.width as u32, border_box.height as u32),
      );
    }

    if !self.has_visible_sides() {
      return;
    }

    if let Some(color) = self.has_uniform_visible_color() {
      if self.visible_sides_match(BorderStyle::Solid) {
        let mut paths = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);
        self.append_border_ring_commands(&mut paths, border_box);
        let (mask, placement) = render_mask(
          &paths,
          Some(transform),
          Some(Fill::EvenOdd.into()),
          &mut canvas.buffer_pool,
        );

        paint_mask(
          canvas,
          &mask,
          placement,
          color,
          clip_image,
          transform,
          self.image_rendering,
        );
        canvas.buffer_pool.release(mask);
        return;
      }

      if self.visible_sides_match(BorderStyle::Double) {
        self.draw_uniform_double(canvas, border_box, transform, clip_image, color);
        return;
      }

      if self.visible_sides_match(BorderStyle::Dashed) {
        self.draw_uniform_pattern(
          canvas,
          border_box,
          transform,
          clip_image,
          color,
          BorderStyle::Dashed,
        );
        return;
      }

      if self.visible_sides_match(BorderStyle::Dotted) {
        self.draw_uniform_pattern(
          canvas,
          border_box,
          transform,
          clip_image,
          color,
          BorderStyle::Dotted,
        );
        return;
      }
    }

    let inverse = if clip_image.is_some() {
      transform.invert()
    } else {
      None
    };
    let mut paint = SidePaintContext {
      canvas,
      transform,
      clip_image,
      inverse,
    };

    for (side, style, width, color) in [
      (
        BorderSide::Top,
        self.style.top,
        self.width.top,
        self.color.top,
      ),
      (
        BorderSide::Right,
        self.style.right,
        self.width.right,
        self.color.right,
      ),
      (
        BorderSide::Bottom,
        self.style.bottom,
        self.width.bottom,
        self.color.bottom,
      ),
      (
        BorderSide::Left,
        self.style.left,
        self.width.left,
        self.color.left,
      ),
    ] {
      if !Self::is_side_visible(style, width) {
        continue;
      }

      match style {
        BorderStyle::Dashed | BorderStyle::Dotted => {
          self.draw_side_pattern_border(&mut paint, side, border_box, color, style);
        }
        BorderStyle::Double => {
          let stripe_width = self.width.map(|value| value / 3.0);
          self.draw_side_band(
            &mut paint,
            side,
            border_box,
            Rect::ZERO,
            stripe_width,
            color,
          );

          let inset = self.width.map(|value| value * (2.0 / 3.0));
          self.draw_side_band(&mut paint, side, border_box, inset, stripe_width, color);
        }
        BorderStyle::Inset | BorderStyle::Outset => {
          self.draw_side_band(
            &mut paint,
            side,
            border_box,
            Rect::ZERO,
            self.width,
            shade_3d_border_color(color, side, style),
          );
        }
        BorderStyle::Groove | BorderStyle::Ridge => {
          let outer_width = self.width.map(|value| value / 2.0);
          let inner_inset = outer_width;
          let inner_width = subtract_rect(self.width, outer_width);
          let outer_style = match style {
            BorderStyle::Groove => BorderStyle::Inset,
            BorderStyle::Ridge => BorderStyle::Outset,
            _ => style,
          };
          let inner_style = match style {
            BorderStyle::Groove => BorderStyle::Outset,
            BorderStyle::Ridge => BorderStyle::Inset,
            _ => style,
          };

          self.draw_side_band(
            &mut paint,
            side,
            border_box,
            Rect::ZERO,
            outer_width,
            shade_3d_border_color(color, side, outer_style),
          );
          self.draw_side_band(
            &mut paint,
            side,
            border_box,
            inner_inset,
            inner_width,
            shade_3d_border_color(color, side, inner_style),
          );
        }
        BorderStyle::None | BorderStyle::Hidden => {}
        BorderStyle::Solid => {
          self.draw_side_band(&mut paint, side, border_box, Rect::ZERO, self.width, color);
        }
      }
    }
  }

  fn draw_uniform_double(
    self,
    canvas: &mut Canvas,
    border_box: Size<f32>,
    transform: Affine,
    clip_image: Option<PaintSource<'_>>,
    color: Color,
  ) {
    let stripe_width = self.width.map(|value| value / 3.0);
    let mut outer = self;
    outer.width = stripe_width;

    let mut paths = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 4);
    outer.append_border_ring_commands(&mut paths, border_box);

    let inset = self.width.map(|value| value * (2.0 / 3.0));
    let mut inner = self;
    inner.width = stripe_width;
    inner.expand_by(inset.map(|value| -value));
    inner.append_border_ring_commands_at(
      &mut paths,
      inset_size(border_box, inset),
      rect_offset(inset),
    );

    let (mask, placement) = render_mask(
      &paths,
      Some(transform),
      Some(Fill::EvenOdd.into()),
      &mut canvas.buffer_pool,
    );
    paint_mask(
      canvas,
      &mask,
      placement,
      color,
      clip_image,
      transform,
      self.image_rendering,
    );
    canvas.buffer_pool.release(mask);
  }

  fn draw_uniform_pattern(
    self,
    canvas: &mut Canvas,
    border_box: Size<f32>,
    transform: Affine,
    clip_image: Option<PaintSource<'_>>,
    color: Color,
    style: BorderStyle,
  ) {
    let width = self.width.top;
    if width <= 0.0 {
      return;
    }

    let half_width = self.width.map(|v| v / 2.0);
    let mut center_rect = self;
    center_rect.expand_by(half_width.map(|v| -v));

    let center_size = inset_size(border_box, half_width);
    let center_offset = rect_offset(half_width);

    let mut paths = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);
    center_rect.append_mask_commands(&mut paths, center_size, center_offset);

    let perimeter = center_rect.approximate_rounded_rect_perimeter(center_size);

    let stroke = compute_side_stroke(width, style, perimeter, true);

    let (mask, placement) = render_mask(
      &paths,
      Some(transform),
      Some(Style::Stroke(stroke)),
      &mut canvas.buffer_pool,
    );

    paint_mask(
      canvas,
      &mask,
      placement,
      color,
      clip_image,
      transform,
      self.image_rendering,
    );
    canvas.buffer_pool.release(mask);
  }

  fn draw_side_band(
    self,
    paint: &mut SidePaintContext<'_, '_>,
    side: BorderSide,
    border_box: Size<f32>,
    inset: Rect<f32>,
    width: Rect<f32>,
    color: Color,
  ) {
    if border_box.width <= 0.0 || border_box.height <= 0.0 {
      return;
    }

    let mut border = self;
    border.width = width;

    let band_box = inset_size(border_box, inset);
    if band_box.width <= 0.0 || band_box.height <= 0.0 {
      return;
    }
    let offset = rect_offset(inset);
    border.expand_by(inset.map(|value| -value));

    if border.is_zero() {
      let mut paths = Vec::with_capacity(5);
      border.append_side_polygon_commands_at(side, &mut paths, band_box, offset);
      let (mask, placement) = render_mask(
        &paths,
        Some(paint.transform),
        Some(Fill::NonZero.into()),
        &mut paint.canvas.buffer_pool,
      );
      paint_mask_with_inverse(
        paint.canvas,
        &mask,
        placement,
        color,
        paint.clip_image,
        paint.inverse,
        self.image_rendering,
      );
      paint.canvas.buffer_pool.release(mask);
      return;
    }

    let mut ring_paths = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);
    border.append_border_ring_commands_at(&mut ring_paths, band_box, offset);
    let (ring_mask, ring_placement) = render_mask(
      &ring_paths,
      Some(paint.transform),
      Some(Fill::EvenOdd.into()),
      &mut paint.canvas.buffer_pool,
    );

    if !ring_mask.is_empty() {
      let mut clip_paths = Vec::with_capacity(5);
      border.append_side_clip_polygon_commands_at(side, &mut clip_paths, band_box, offset);
      let (clip_mask, clip_placement) = render_mask(
        &clip_paths,
        Some(paint.transform),
        Some(Fill::NonZero.into()),
        &mut paint.canvas.buffer_pool,
      );

      if let Some((mask, placement)) =
        intersect_masks(&ring_mask, ring_placement, &clip_mask, clip_placement)
      {
        paint_mask_with_inverse(
          paint.canvas,
          &mask,
          placement,
          color,
          paint.clip_image,
          paint.inverse,
          self.image_rendering,
        );
      }
      paint.canvas.buffer_pool.release(clip_mask);
    }
    paint.canvas.buffer_pool.release(ring_mask);
  }

  fn draw_side_pattern_border(
    self,
    paint: &mut SidePaintContext<'_, '_>,
    side: BorderSide,
    border_box: Size<f32>,
    color: Color,
    style: BorderStyle,
  ) {
    let (width, is_horizontal, fixed, start, end) = match side {
      BorderSide::Top => (
        self.width.top,
        true,
        self.width.top / 2.0,
        self.width.left / 2.0,
        border_box.width - self.width.right / 2.0,
      ),
      BorderSide::Right => (
        self.width.right,
        false,
        border_box.width - self.width.right / 2.0,
        self.width.top / 2.0,
        border_box.height - self.width.bottom / 2.0,
      ),
      BorderSide::Bottom => (
        self.width.bottom,
        true,
        border_box.height - self.width.bottom / 2.0,
        self.width.left / 2.0,
        border_box.width - self.width.right / 2.0,
      ),
      BorderSide::Left => (
        self.width.left,
        false,
        self.width.left / 2.0,
        self.width.top / 2.0,
        border_box.height - self.width.bottom / 2.0,
      ),
    };

    if width <= 0.0 || end <= start {
      return;
    }

    let mut path = Vec::with_capacity(2);
    if is_horizontal {
      path.move_to((start, fixed));
      path.line_to((end, fixed));
    } else {
      path.move_to((fixed, start));
      path.line_to((fixed, end));
    }

    let stroke = compute_side_stroke(width, style, end - start, false);
    let (pattern_mask, pattern_placement) = render_mask(
      &path,
      Some(paint.transform),
      Some(Style::Stroke(stroke)),
      &mut paint.canvas.buffer_pool,
    );

    if !pattern_mask.is_empty() {
      let mut clip_path = Vec::with_capacity(5);
      self.append_side_clip_polygon_commands_at(side, &mut clip_path, border_box, Point::ZERO);
      let (clip_mask, clip_placement) = render_mask(
        &clip_path,
        Some(paint.transform),
        Some(Fill::NonZero.into()),
        &mut paint.canvas.buffer_pool,
      );

      if let Some((mask, placement)) =
        intersect_masks(&pattern_mask, pattern_placement, &clip_mask, clip_placement)
      {
        paint_mask_with_inverse(
          paint.canvas,
          &mask,
          placement,
          color,
          paint.clip_image,
          paint.inverse,
          self.image_rendering,
        );
      }
      paint.canvas.buffer_pool.release(clip_mask);
    }
    paint.canvas.buffer_pool.release(pattern_mask);
  }
}

fn compute_side_stroke(width: f32, style: BorderStyle, length: f32, closed: bool) -> Stroke {
  let mut stroke = Stroke::new(width);
  let (dash, gap, is_dotted) = match style {
    BorderStyle::Dashed => (
      if width < 3.0 {
        width * 3.0
      } else {
        width * 2.0
      },
      if width < 3.0 { width * 2.0 } else { width },
      false,
    ),
    BorderStyle::Dotted => (width, width * 2.0, true),
    _ => (0.0, 0.0, false),
  };

  if is_dotted && width > 3.0 {
    stroke.cap = Cap::Round;
    let adjusted_gap = select_best_dash_gap(length, 0.0, width * 2.0, closed);
    stroke.dash = Some(DashPattern {
      intervals: [0.0, adjusted_gap],
      offset: 0.0,
    });
  } else {
    let adjusted_gap = select_best_dash_gap(length, dash, gap, closed);
    stroke.dash = Some(DashPattern {
      intervals: [dash, adjusted_gap],
      offset: 0.0,
    });
  }
  stroke
}

fn select_best_dash_gap(length: f32, dash: f32, gap: f32, closed: bool) -> f32 {
  let available = if closed { length } else { length + gap };
  let n = (available / (dash + gap))
    .floor()
    .max(if closed { 1.0 } else { 2.0 });

  let g1 = if closed {
    length / n - dash
  } else {
    (length - n * dash) / (n - 1.0)
  };
  let g2 = if closed {
    length / (n + 1.0) - dash
  } else {
    (length - (n + 1.0) * dash) / n
  };

  if (g1 - gap).abs() <= (g2 - gap).abs() {
    g1.max(0.0)
  } else {
    g2.max(0.0)
  }
}

#[derive(Clone, Copy)]
enum BorderSide {
  Top,
  Right,
  Bottom,
  Left,
}

struct SidePaintContext<'canvas, 'source> {
  canvas: &'canvas mut Canvas,
  transform: Affine,
  clip_image: Option<PaintSource<'source>>,
  inverse: Option<Affine>,
}

fn paint_mask(
  canvas: &mut Canvas,
  mask: &[u8],
  placement: Placement,
  color: Color,
  clip_image: Option<PaintSource<'_>>,
  transform: Affine,
  image_rendering: ImageScalingAlgorithm,
) {
  paint_mask_with_inverse(
    canvas,
    mask,
    placement,
    color,
    clip_image,
    transform.invert(),
    image_rendering,
  );
}

fn paint_mask_with_inverse(
  canvas: &mut Canvas,
  mask: &[u8],
  placement: Placement,
  color: Color,
  clip_image: Option<PaintSource<'_>>,
  inverse: Option<Affine>,
  image_rendering: ImageScalingAlgorithm,
) {
  if let Some(clip_image) = clip_image {
    let Some(inverse) = inverse else {
      return;
    };
    canvas.composite_mask_source_over_color(
      mask,
      placement,
      clip_image,
      color,
      MaskSamplingOptions {
        canvas_to_source: inverse,
        sample_bias: Point::ZERO,
        algorithm: image_rendering,
      },
      BlendMode::Normal,
    );
  } else {
    canvas.draw_mask(mask, placement, color, BlendMode::Normal);
  }
}

fn intersect_masks(
  lhs: &[u8],
  lhs_placement: Placement,
  rhs: &[u8],
  rhs_placement: Placement,
) -> Option<(Vec<u8>, Placement)> {
  let left = lhs_placement.left.max(rhs_placement.left);
  let top = lhs_placement.top.max(rhs_placement.top);
  let right = lhs_placement.right().min(rhs_placement.right());
  let bottom = lhs_placement.bottom().min(rhs_placement.bottom());
  let placement = Placement::from_bounds(left, top, right, bottom)?;

  let mut mask = vec![0; (placement.width * placement.height) as usize];

  for y in 0..placement.height {
    for x in 0..placement.width {
      let canvas_x = placement.left + x as i32;
      let canvas_y = placement.top + y as i32;
      let lhs_x = (canvas_x - lhs_placement.left) as u32;
      let lhs_y = (canvas_y - lhs_placement.top) as u32;
      let rhs_x = (canvas_x - rhs_placement.left) as u32;
      let rhs_y = (canvas_y - rhs_placement.top) as u32;
      let lhs_alpha = lhs[(lhs_y * lhs_placement.width + lhs_x) as usize] as u32;
      let rhs_alpha = rhs[(rhs_y * rhs_placement.width + rhs_x) as usize] as u32;
      mask[(y * placement.width + x) as usize] = fast_div_255(lhs_alpha * rhs_alpha);
    }
  }

  Some((mask, placement))
}

fn rect_offset(rect: Rect<f32>) -> Point<f32> {
  Point {
    x: rect.left,
    y: rect.top,
  }
}

fn line_intersection(
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

fn approximate_quarter_ellipse_arc_length(radius_x: f32, radius_y: f32) -> f32 {
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

fn inset_size(size: Size<f32>, inset: Rect<f32>) -> Size<f32> {
  Size {
    width: (size.width - inset.left - inset.right).max(0.0),
    height: (size.height - inset.top - inset.bottom).max(0.0),
  }
}

fn subtract_rect(lhs: Rect<f32>, rhs: Rect<f32>) -> Rect<f32> {
  Rect {
    top: (lhs.top - rhs.top).max(0.0),
    right: (lhs.right - rhs.right).max(0.0),
    bottom: (lhs.bottom - rhs.bottom).max(0.0),
    left: (lhs.left - rhs.left).max(0.0),
  }
}

fn shade_3d_border_color(color: Color, side: BorderSide, style: BorderStyle) -> Color {
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

fn mix_color(color: Color, target: Color, amount: f32) -> Color {
  let amount = amount.clamp(0.0, 1.0);
  let inverse = 1.0 - amount;

  Color([
    (color.0[0] as f32 * inverse + target.0[0] as f32 * amount).round() as u8,
    (color.0[1] as f32 * inverse + target.0[1] as f32 * amount).round() as u8,
    (color.0[2] as f32 * inverse + target.0[2] as f32 * amount).round() as u8,
    color.0[3],
  ])
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_border(style: BorderStyle, width: f32) -> BorderProperties {
    BorderProperties {
      width: Rect {
        top: width,
        right: width,
        bottom: width,
        left: width,
      },
      color: Rect {
        top: Color([255, 0, 0, 255]),
        right: Color([255, 0, 0, 255]),
        bottom: Color([255, 0, 0, 255]),
        left: Color([255, 0, 0, 255]),
      },
      radius: Sides([SpacePair::from_single(0.0); 4]),
      style: Rect {
        top: style,
        right: style,
        bottom: style,
        left: style,
      },
      image_rendering: ImageScalingAlgorithm::Auto,
    }
  }

  #[test]
  fn solid_border_draws_continuous_edge() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });

    test_border(BorderStyle::Solid, 4.0).draw(
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));
    assert!((8..40).all(|x| image.get_pixel(x, 2).0[3] > 0));
  }

  #[test]
  fn hidden_border_does_not_draw() {
    let mut canvas = Canvas::new(Size {
      width: 24,
      height: 24,
    });

    test_border(BorderStyle::Hidden, 4.0).draw(
      &mut canvas,
      Size {
        width: 24.0,
        height: 24.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));
    assert!(image.pixels().all(|pixel| pixel.0[3] == 0));
  }

  #[test]
  fn dashed_border_draws_pattern() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });

    test_border(BorderStyle::Dashed, 4.0).draw(
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    // Check top border line (y=2)
    // It should have some transparent pixels (gaps) and some opaque pixels (dashes)
    let row: Vec<u8> = (0..48).map(|x| image.get_pixel(x, 2).0[3]).collect();
    let has_opaque = row.iter().any(|&a| a > 0);
    let has_transparent = row.iter().skip(8).take(32).any(|&a| a == 0);

    assert!(has_opaque, "Dashed border should have opaque pixels");
    assert!(
      has_transparent,
      "Dashed border should have transparent gaps"
    );
  }

  #[test]
  fn dotted_border_draws_pattern() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });

    test_border(BorderStyle::Dotted, 4.0).draw(
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    let row: Vec<u8> = (0..48).map(|x| image.get_pixel(x, 2).0[3]).collect();
    let has_opaque = row.iter().any(|&a| a > 0);
    let has_transparent = row.iter().skip(8).take(32).any(|&a| a == 0);

    assert!(has_opaque, "Dotted border should have opaque pixels");
    assert!(
      has_transparent,
      "Dotted border should have transparent gaps"
    );
  }
}
