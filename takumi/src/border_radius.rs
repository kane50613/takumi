use image::RgbaImage;
use taffy::{Layout, Point, Rect};

use crate::{node::style::LengthUnit, render::RenderContext};

/// Represents the four corners of an image for border radius processing.
#[derive(Debug, Clone, Copy)]
pub struct BorderRadius {
  /// The radius of the top-left corner
  pub top_left: f32,
  /// The radius of the top-right corner
  pub top_right: f32,
  /// The radius of the bottom-right corner
  pub bottom_right: f32,
  /// The radius of the bottom-left corner
  pub bottom_left: f32,
}

impl BorderRadius {
  /// Creates a new `BorderRadius` from a `Layout` and a `Rect` of `LengthUnit`s.
  pub fn from_layout(context: &RenderContext, layout: &Layout, radius: Rect<LengthUnit>) -> Self {
    // CSS border-radius percentages: use smaller of width/height for circular corners
    let reference_size = layout.size.width.min(layout.size.height);

    Self {
      top_left: resolve_border_radius_from_percentage_css(context, radius.top, reference_size),
      top_right: resolve_border_radius_from_percentage_css(context, radius.right, reference_size),
      bottom_right: resolve_border_radius_from_percentage_css(
        context,
        radius.bottom,
        reference_size,
      ),
      bottom_left: resolve_border_radius_from_percentage_css(context, radius.left, reference_size),
    }
  }
}

fn resolve_border_radius_from_percentage_css(
  context: &RenderContext,
  radius: LengthUnit,
  reference_size: f32,
) -> f32 {
  match radius {
    LengthUnit::Px(value) => value,
    LengthUnit::Percentage(value) => value * reference_size / 100.0,
    LengthUnit::Auto => 0.0,
    LengthUnit::Rem(value) => value * context.parent_font_size,
    LengthUnit::Em(value) => value * context.parent_font_size,
    LengthUnit::Vh(value) => value * context.viewport.height as f32 / 100.0,
    LengthUnit::Vw(value) => value * context.viewport.width as f32 / 100.0,
  }
}

/// Applies antialiased border radius to an image.
///
/// This function processes the corners of the image to create smooth, antialiased rounded corners.
/// The border radius is automatically clamped to half the minimum dimension of the image.
pub fn apply_border_radius_antialiased(img: &mut RgbaImage, radius: BorderRadius) {
  let (width, height) = img.dimensions();
  let max_radius = width.min(height) as f32 / 2.0;

  let clamped_radius = BorderRadius {
    top_left: radius.top_left.min(max_radius),
    top_right: radius.top_right.min(max_radius),
    bottom_right: radius.bottom_right.min(max_radius),
    bottom_left: radius.bottom_left.min(max_radius),
  };

  let transition_width = 1.0;

  // Process each corner with its individual radius
  let corners = [
    (Corner::TopLeft, clamped_radius.top_left),
    (Corner::TopRight, clamped_radius.top_right),
    (Corner::BottomLeft, clamped_radius.bottom_left),
    (Corner::BottomRight, clamped_radius.bottom_right),
  ];

  for (corner, corner_radius) in corners {
    if corner_radius > 0.0 {
      let outer_radius = corner_radius + transition_width;
      let outer_radius_sq = outer_radius * outer_radius;
      let radius_sq = corner_radius * corner_radius;

      let band_size = (outer_radius.ceil() as u32)
        .max(corner_radius as u32 + (transition_width * 2.0).ceil() as u32);

      let (start, end) = match corner {
        Corner::TopLeft => (
          Point { x: 0, y: 0 },
          Point {
            x: band_size,
            y: band_size,
          },
        ),
        Corner::TopRight => (
          Point {
            x: width.saturating_sub(band_size),
            y: 0,
          },
          Point {
            x: width,
            y: band_size,
          },
        ),
        Corner::BottomLeft => (
          Point {
            x: 0,
            y: height.saturating_sub(band_size),
          },
          Point {
            x: band_size,
            y: height,
          },
        ),
        Corner::BottomRight => (
          Point {
            x: width.saturating_sub(band_size),
            y: height.saturating_sub(band_size),
          },
          Point {
            x: width,
            y: height,
          },
        ),
      };

      process_corner_aa(
        img,
        start,
        end,
        corner_radius,
        radius_sq,
        outer_radius_sq,
        corner,
      );
    }
  }
}

/// Represents the four corners of an image for border radius processing.
#[derive(Copy, Clone)]
enum Corner {
  /// Top-left corner
  TopLeft,
  /// Top-right corner
  TopRight,
  /// Bottom-left corner
  BottomLeft,
  /// Bottom-right corner
  BottomRight,
}

/// Processes a single corner of the image for antialiased border radius.
///
/// This function handles the antialiasing calculations for a specific corner,
/// creating a smooth transition between the rounded corner and the rest of the image.
#[inline]
fn process_corner_aa(
  img: &mut RgbaImage,
  start: Point<u32>,
  end: Point<u32>,
  radius: f32,
  radius_sq: f32,
  outer_radius_sq: f32,
  corner: Corner,
) {
  let (corner_x, corner_y) = match corner {
    Corner::TopLeft => (radius, radius),
    Corner::TopRight => (start.x as f32, radius),
    Corner::BottomLeft => (radius, start.y as f32),
    Corner::BottomRight => (start.x as f32, start.y as f32),
  };

  for y in start.y..end.y {
    let fy = y as f32;
    let dy = (fy - corner_y).abs();
    let dy_sq = dy * dy;

    // Early exit optimization - if entire row is outside outer radius
    if dy_sq > outer_radius_sq {
      set_row_alpha(img, start.x, end.x, y, 0);
      continue;
    }

    // Early exit - if entire row is inside radius
    if dy_sq < radius_sq
      && (start.x as f32 - corner_x).abs() < radius
      && (end.x as f32 - corner_x).abs() < radius
    {
      continue; // Keep original alpha
    }

    for x in start.x..end.x {
      let fx = x as f32;
      let dx = (fx - corner_x).abs();
      let dist_sq = dx * dx + dy_sq;

      let alpha = if dist_sq <= radius_sq {
        255 // Inside radius - keep original
      } else if dist_sq >= outer_radius_sq {
        0 // Outside antialiasing band - transparent
      } else {
        // Antialiasing zone - smooth transition
        let dist = dist_sq.sqrt();
        let factor = (outer_radius_sq.sqrt() - dist) / (outer_radius_sq.sqrt() - radius_sq.sqrt());
        (factor * 255.0).clamp(0.0, 255.0) as u8
      };

      if alpha < 255 {
        let idx = ((y * img.width() + x) * 4 + 3) as usize;
        let pixels = img.as_mut();
        if alpha == 0 {
          pixels[idx] = 0;
        } else {
          // Blend with existing alpha
          let existing = pixels[idx] as u32;
          pixels[idx] = ((existing * alpha as u32) / 255) as u8;
        }
      }
    }
  }
}

/// Sets the alpha value for an entire row of pixels in the image.
#[inline]
fn set_row_alpha(img: &mut RgbaImage, start_x: u32, end_x: u32, y: u32, alpha: u8) {
  let width = img.width();
  let pixels = img.as_mut();
  for x in start_x..end_x {
    let idx = ((y * width + x) * 4 + 3) as usize;
    pixels[idx] = alpha;
  }
}
