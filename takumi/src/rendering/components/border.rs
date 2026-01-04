use std::f32::consts::SQRT_2;

use image::{GenericImageView, Rgba};
use taffy::{Point, Rect, Size};
use zeno::{Command, Fill, PathBuilder};

use crate::{
  layout::style::{Affine, Color, ColorInput, Sides, SpacePair},
  rendering::{
    Canvas, RenderContext, apply_mask_alpha_to_pixel, blend_pixel, mask_index_from_coord,
    overlay_area,
  },
};

/// Represents the properties of a border, including corner radii and drawing metadata.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BorderProperties {
  /// The width of the border on each side (top, right, bottom, left)
  pub width: Rect<f32>,
  /// The color of the border
  pub color: Color,
  /// Corner radii: top, right, bottom, left (in pixels)
  pub radius: Sides<SpacePair<f32>>,
}

impl BorderProperties {
  /// The amount of path commands to append for this border.
  /// This is used to pre-allocate the vector size for the mask commands.
  const PATH_COMMANDS_AMOUNT: usize = 10;

  /// Create an empty BorderProperties with zeroed radii and default values.
  pub const fn zero() -> Self {
    Self {
      width: Rect::ZERO,
      color: Color([0, 0, 0, 255]),
      radius: Sides([SpacePair::from_single(0.0); 4]),
    }
  }

  /// Resolves the border radius from the context and layout.
  pub fn from_context(
    context: &RenderContext,
    border_box: Size<f32>,
    border_width: Rect<f32>,
  ) -> Self {
    let resolved = context.style.resolved_border_radius();

    let top_left = resolved.top.to_px(&context.sizing, border_box);
    let top_right = resolved.right.to_px(&context.sizing, border_box);
    let bottom_right = resolved.bottom.to_px(&context.sizing, border_box);
    let bottom_left = resolved.left.to_px(&context.sizing, border_box);

    Self {
      width: border_width,
      color: context
        .style
        .border_color
        .or(context.style.border.color)
        .unwrap_or(ColorInput::CurrentColor)
        .resolve(context.current_color, context.opacity),
      radius: Sides([top_left, top_right, bottom_right, bottom_left]),
    }
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

  pub(crate) fn draw<I: GenericImageView<Pixel = Rgba<u8>>>(
    mut self,
    canvas: &mut Canvas,
    border_box: Size<f32>,
    transform: Affine,
    fill_image: Option<&I>,
  ) {
    if let Some(fill_image) = &fill_image {
      assert_eq!(
        fill_image.dimensions(),
        (border_box.width as u32, border_box.height as u32)
      );
    }

    if self.width.left == 0.0
      && self.width.right == 0.0
      && self.width.top == 0.0
      && self.width.bottom == 0.0
    {
      return;
    }

    let mut paths = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

    self.append_mask_commands(&mut paths, border_box, Point::ZERO);

    self.inset_by_border_width();
    self.append_mask_commands(
      &mut paths,
      border_box
        - Size {
          width: self.width.left + self.width.right,
          height: self.width.top + self.width.bottom,
        },
      Point {
        x: self.width.left,
        y: self.width.top,
      },
    );

    let (mask, placement) =
      canvas
        .mask_memory
        .render(&paths, Some(transform), Some(Fill::EvenOdd.into()));

    overlay_area(
      &mut canvas.image,
      Point {
        x: placement.left as f32,
        y: placement.top as f32,
      },
      Size {
        width: placement.width,
        height: placement.height,
      },
      canvas.constrains.last(),
      |x, y| {
        let alpha = mask[mask_index_from_coord(x, y, placement.width)];

        let mut pixel = fill_image
          .as_ref()
          .map(|image| {
            let mut pixel = image.get_pixel(x, y);

            blend_pixel(&mut pixel, self.color.into());

            pixel
          })
          .unwrap_or(self.color.into());

        apply_mask_alpha_to_pixel(&mut pixel, alpha);

        pixel
      },
    );
  }
}
