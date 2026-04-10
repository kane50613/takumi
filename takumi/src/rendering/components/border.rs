use std::f32::consts::SQRT_2;

use taffy::{Point, Rect, Size};

use crate::{
  layout::style::{Affine, BlendMode, BorderStyle, Color, ImageScalingAlgorithm, Sides, SpacePair},
  rendering::{
    Canvas, Command, Fill, MaskSamplingOptions, PaintSource, PathBuilder, RenderContext,
    render_mask,
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
    style != BorderStyle::None && width > 0.0
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

  fn append_border_ring_commands(&self, paths: &mut Vec<Command>, border_box: Size<f32>) {
    let mut border = *self;

    border.append_mask_commands(paths, border_box, Point::ZERO);
    border.inset_by_border_width();
    border.append_mask_commands(
      paths,
      border_box
        - Size {
          width: border.width.left + border.width.right,
          height: border.width.top + border.width.bottom,
        },
      Point {
        x: border.width.left,
        y: border.width.top,
      },
    );
  }

  fn append_side_polygon_commands(
    &self,
    side: BorderSide,
    path: &mut Vec<Command>,
    border_box: Size<f32>,
  ) {
    let inner_left = self.width.left.min(border_box.width);
    let inner_right = (border_box.width - self.width.right).max(0.0);
    let inner_top = self.width.top.min(border_box.height);
    let inner_bottom = (border_box.height - self.width.bottom).max(0.0);

    match side {
      BorderSide::Top => {
        path.move_to((0.0, 0.0));
        path.line_to((border_box.width, 0.0));
        path.line_to((inner_right, inner_top));
        path.line_to((inner_left, inner_top));
      }
      BorderSide::Right => {
        path.move_to((border_box.width, 0.0));
        path.line_to((border_box.width, border_box.height));
        path.line_to((inner_right, inner_bottom));
        path.line_to((inner_right, inner_top));
      }
      BorderSide::Bottom => {
        path.move_to((border_box.width, border_box.height));
        path.line_to((0.0, border_box.height));
        path.line_to((inner_left, inner_bottom));
        path.line_to((inner_right, inner_bottom));
      }
      BorderSide::Left => {
        path.move_to((0.0, border_box.height));
        path.line_to((0.0, 0.0));
        path.line_to((inner_left, inner_top));
        path.line_to((inner_left, inner_bottom));
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
      let mut paths = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);
      self.append_border_ring_commands(&mut paths, border_box);
      let (mask, placement) = render_mask(
        &paths,
        Some(transform),
        Some(Fill::EvenOdd.into()),
        &mut canvas.buffer_pool,
      );

      if clip_image.is_none() {
        canvas.draw_mask(&mask, placement, color, BlendMode::Normal);
        canvas.buffer_pool.release(mask);
        return;
      }

      let Some(inverse) = transform.invert() else {
        canvas.buffer_pool.release(mask);
        return;
      };

      if let Some(clip_image) = clip_image {
        canvas.composite_mask_source_over_color(
          &mask,
          placement,
          clip_image,
          color,
          MaskSamplingOptions {
            canvas_to_source: inverse,
            sample_bias: Point::ZERO,
            algorithm: self.image_rendering,
          },
          BlendMode::Normal,
        );
      }

      canvas.buffer_pool.release(mask);
      return;
    }

    let inverse = if clip_image.is_some() {
      transform.invert()
    } else {
      None
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

      let mut paths = Vec::with_capacity(5);
      self.append_side_polygon_commands(side, &mut paths, border_box);
      let (mask, placement) = render_mask(
        &paths,
        Some(transform),
        Some(Fill::NonZero.into()),
        &mut canvas.buffer_pool,
      );

      if let Some(clip_image) = clip_image {
        let Some(inverse) = inverse else {
          canvas.buffer_pool.release(mask);
          continue;
        };
        canvas.composite_mask_source_over_color(
          &mask,
          placement,
          clip_image,
          color,
          MaskSamplingOptions {
            canvas_to_source: inverse,
            sample_bias: Point::ZERO,
            algorithm: self.image_rendering,
          },
          BlendMode::Normal,
        );
      } else {
        canvas.draw_mask(&mask, placement, color, BlendMode::Normal);
      }

      canvas.buffer_pool.release(mask);
    }
  }
}

#[derive(Clone, Copy)]
enum BorderSide {
  Top,
  Right,
  Bottom,
  Left,
}
