//! Canvas operations and image blending for the takumi rendering system.
//!
//! This module provides performance-optimized canvas operations including
//! fast image blending and pixel manipulation operations.

use std::borrow::Cow;

use image::{
  GenericImageView, Rgba, RgbaImage,
  imageops::{interpolate_bilinear, interpolate_nearest},
};
use smallvec::SmallVec;
use taffy::{Layout, Point, Size};
use zeno::{Mask, PathData, Placement, Scratch};

use crate::layout::style::BlendMode;
use crate::{
  layout::style::{Affine, Color, ImageScalingAlgorithm, InheritedStyle, Overflow},
  rendering::{BorderProperties, RenderContext, blend_pixel, create_mask, fast_div_255},
};

#[derive(Clone)]
pub(crate) struct CowImage<'a> {
  inner: Cow<'a, RgbaImage>,
  crop_bounds: Option<(Point<u32>, Size<u32>)>,
}

impl GenericImageView for CowImage<'_> {
  type Pixel = Rgba<u8>;

  fn dimensions(&self) -> (u32, u32) {
    if let Some((_, size)) = self.crop_bounds {
      (size.width, size.height)
    } else {
      (self.inner.width(), self.inner.height())
    }
  }

  fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
    if let Some((start, _)) = self.crop_bounds {
      *self.inner.get_pixel(x + start.x, y + start.y)
    } else {
      *self.inner.get_pixel(x, y)
    }
  }
}

impl<'a> From<&'a RgbaImage> for CowImage<'a> {
  fn from(image: &'a RgbaImage) -> Self {
    CowImage {
      inner: Cow::Borrowed(image),
      crop_bounds: None,
    }
  }
}

impl<'a> From<RgbaImage> for CowImage<'a> {
  fn from(image: RgbaImage) -> Self {
    CowImage {
      inner: Cow::Owned(image),
      crop_bounds: None,
    }
  }
}

impl<'a> From<Cow<'a, RgbaImage>> for CowImage<'a> {
  fn from(image: Cow<'a, RgbaImage>) -> Self {
    CowImage {
      inner: image,
      crop_bounds: None,
    }
  }
}

impl<'a> CowImage<'a> {
  pub(crate) fn crop<I: Into<Cow<'a, RgbaImage>>>(
    image: I,
    mut crop_x: u32,
    mut crop_y: u32,
    mut crop_width: u32,
    mut crop_height: u32,
  ) -> Self {
    let image = image.into();

    crop_x = crop_x.clamp(0, image.width() - 1);
    crop_y = crop_y.clamp(0, image.height() - 1);
    crop_width = crop_width.clamp(0, image.width() - crop_x);
    crop_height = crop_height.clamp(0, image.height() - crop_y);

    CowImage {
      inner: image,
      crop_bounds: Some((
        Point {
          x: crop_x,
          y: crop_y,
        },
        Size {
          width: crop_width,
          height: crop_height,
        },
      )),
    }
  }
}

pub(crate) enum CanvasConstrainResult {
  Some(CanvasConstrain),
  None,
  SkipRendering,
}

impl CanvasConstrainResult {
  pub(crate) fn is_some(&self) -> bool {
    matches!(self, CanvasConstrainResult::Some(_))
  }
}

pub(crate) enum CanvasConstrain {
  Overflow {
    from: Point<u32>,
    to: Point<u32>,
    inverse_transform: Affine,
    border_radius_mask: Option<(Box<[u8]>, u32)>,
  },
  ClipPath {
    mask: Box<[u8]>,
    placement: Placement,
  },
  MaskImage {
    mask: Box<[u8]>,
    from: Point<u32>,
    to: Point<u32>,
    inverse_transform: Affine,
  },
}

impl CanvasConstrain {
  pub(crate) fn from_node(
    context: &RenderContext,
    style: &InheritedStyle,
    layout: Layout,
    transform: Affine,
    mask_memory: &mut MaskMemory,
  ) -> crate::Result<CanvasConstrainResult> {
    // Clip path would just clip everything, and behaves like overflow: hidden.
    if let Some(clip_path) = &style.clip_path {
      let (mask, placement) = clip_path.render_mask(context, layout.size, mask_memory);

      let end_x = placement.left + placement.width as i32;
      let end_y = placement.top + placement.height as i32;

      if end_x < 0 || end_y < 0 {
        return Ok(CanvasConstrainResult::SkipRendering);
      }

      return Ok(CanvasConstrainResult::Some(CanvasConstrain::ClipPath {
        mask: mask.into(),
        placement,
      }));
    }

    let Some(inverse_transform) = transform.invert() else {
      return Ok(CanvasConstrainResult::SkipRendering);
    };

    if let Some(mask) = create_mask(context, layout.size, mask_memory)? {
      return Ok(CanvasConstrainResult::Some(CanvasConstrain::MaskImage {
        mask: mask.into_boxed_slice(),
        from: Point { x: 0, y: 0 },
        to: Point {
          x: layout.size.width as u32,
          y: layout.size.height as u32,
        },
        inverse_transform,
      }));
    }

    let overflow = style.resolve_overflows();

    let clip_x = overflow.x != Overflow::Visible;
    let clip_y = overflow.y != Overflow::Visible;

    if !overflow.should_clip_content() {
      return Ok(CanvasConstrainResult::None);
    }

    if (clip_x && layout.content_box_width() < f32::EPSILON)
      || (clip_y && layout.content_box_height() < f32::EPSILON)
    {
      return Ok(CanvasConstrainResult::SkipRendering);
    }

    // When border-radius is non-zero, create a mask-based overflow constraint
    // so that children (including abs-pos) are clipped to the padding-box
    // rounded corners (inset from the border edge by border widths).
    let border_props = BorderProperties::from_context(context, layout.size, layout.border);
    if !border_props.is_zero() {
      // Compute padding-box: border-box inset by border widths on each side.
      let padding_box = Size {
        width: (layout.size.width - layout.border.left - layout.border.right).max(0.0),
        height: (layout.size.height - layout.border.top - layout.border.bottom).max(0.0),
      };

      // Shrink corner radii inward by border widths to get padding-box radii.
      let mut inner_props = border_props;
      inner_props.inset_by_border_width();

      let mut paths = Vec::with_capacity(10);
      // Offset origin so the mask starts at the padding edge (inside the border).
      let padding_origin = Point {
        x: layout.border.left,
        y: layout.border.top,
      };
      inner_props.append_mask_commands(&mut paths, padding_box, padding_origin);

      let (mask_data, placement) = mask_memory.render(&paths, None, None);

      if placement.width == 0 || placement.height == 0 {
        return Ok(CanvasConstrainResult::SkipRendering);
      }

      let from = Point {
        x: placement.left.max(0) as u32,
        y: placement.top.max(0) as u32,
      };

      return Ok(CanvasConstrainResult::Some(CanvasConstrain::Overflow {
        from,
        to: Point {
          x: from.x + placement.width,
          y: from.y + placement.height,
        },
        inverse_transform,
        border_radius_mask: Some((Box::from(mask_data), placement.width)),
      }));
    }

    let from = Point {
      x: if clip_x {
        (layout.padding.left + layout.border.left) as u32
      } else {
        0
      },
      y: if clip_y {
        (layout.padding.top + layout.border.top) as u32
      } else {
        0
      },
    };
    let to = Point {
      x: if clip_x {
        from.x + layout.content_box_width() as u32
      } else {
        u32::MAX
      },
      y: if clip_y {
        from.y + layout.content_box_height() as u32
      } else {
        u32::MAX
      },
    };

    Ok(CanvasConstrainResult::Some(CanvasConstrain::Overflow {
      from,
      to,
      inverse_transform,
      border_radius_mask: None,
    }))
  }

  pub(crate) fn get_alpha(&self, x: u32, y: u32) -> u8 {
    match *self {
      CanvasConstrain::Overflow {
        from,
        to,
        inverse_transform,
        ref border_radius_mask,
      } => {
        let original_point = inverse_transform.transform_point(Point {
          x: x as f32,
          y: y as f32,
        });

        if original_point.x < 0.0 || original_point.y < 0.0 {
          return 0;
        }

        let original_point = original_point.map(|point| point as u32);

        let is_contained = original_point.x >= from.x
          && original_point.x < to.x
          && original_point.y >= from.y
          && original_point.y < to.y;

        if !is_contained {
          return 0;
        }

        // Apply border-radius mask if present
        if let Some((mask, mask_w)) = border_radius_mask {
          let mx = original_point.x - from.x;
          let my = original_point.y - from.y;
          return mask[mask_index_from_coord(mx, my, *mask_w)];
        }

        u8::MAX
      }
      CanvasConstrain::MaskImage {
        ref mask,
        from,
        to,
        inverse_transform,
      } => {
        let original_point = inverse_transform.transform_point(Point {
          x: x as f32,
          y: y as f32,
        });

        if original_point.x < 0.0 || original_point.y < 0.0 {
          return 0;
        }

        let original_point = original_point.map(|point| point as u32);

        let is_contained = original_point.x >= from.x
          && original_point.x < to.x
          && original_point.y >= from.y
          && original_point.y < to.y;

        if !is_contained {
          return 0;
        }

        mask[mask_index_from_coord(original_point.x, original_point.y, to.x - from.x)]
      }
      CanvasConstrain::ClipPath {
        ref mask,
        placement,
      } => {
        let mask_x = x as i32 - placement.left;
        let mask_y = y as i32 - placement.top;

        if mask_x < 0
          || mask_y < 0
          || mask_x >= placement.width as i32
          || mask_y >= placement.height as i32
        {
          return 0;
        }

        mask[mask_index_from_coord(mask_x as u32, mask_y as u32, placement.width)]
      }
    }
  }
}

/// Memory for reusing dynamic memory allocations for masking operations.
#[derive(Default)]
pub(crate) struct MaskMemory {
  scratch: Scratch,
  buffer: Vec<u8>,
}

impl MaskMemory {
  pub(crate) fn render<D: PathData>(
    &mut self,
    paths: D,
    transform: Option<Affine>,
    style: Option<zeno::Style>,
  ) -> (&[u8], Placement) {
    let style = style.unwrap_or_default();
    let mut bounds = self
      .scratch
      .bounds(&paths, style, transform.map(Into::into));

    bounds.min = bounds.min.floor();
    bounds.max = bounds.max.ceil();

    // Make sure the buffer is the correct size AND its initialized with zeros.
    self.buffer.clear();
    self
      .buffer
      .resize((bounds.width() as usize) * (bounds.height() as usize), 0);

    let placement = Mask::with_scratch(paths, &mut self.scratch)
      .transform(transform.map(Into::into))
      .style(style)
      .render_into(&mut self.buffer, None);

    assert_eq!(bounds.width() as u32, placement.width);
    assert_eq!(bounds.height() as u32, placement.height);

    (self.buffer.as_slice(), placement)
  }
}

/// A canvas that can be used to draw images onto.
pub struct Canvas {
  pub(crate) image: RgbaImage,
  pub(crate) constrains: SmallVec<[CanvasConstrain; 1]>,
  // Since canvas is shared with mutable borrows everywhere already,
  // we can just include the memory here instead of making the function argument bloated.
  pub(crate) mask_memory: MaskMemory,
}

impl Canvas {
  /// Creates a new canvas handle from a draw command sender.
  pub(crate) fn new(size: Size<u32>) -> Self {
    Self {
      image: RgbaImage::new(size.width, size.height),
      constrains: SmallVec::new(),
      mask_memory: MaskMemory::default(),
    }
  }

  pub(crate) fn replace_new_image(&mut self) -> RgbaImage {
    let size = self.size();

    std::mem::replace(&mut self.image, RgbaImage::new(size.width, size.height))
  }

  pub(crate) fn push_constrain(&mut self, overflow_constrain: CanvasConstrain) {
    self.constrains.push(overflow_constrain);
  }

  pub(crate) fn pop_constrain(&mut self) {
    self.constrains.pop();
  }

  pub(crate) fn into_inner(self) -> RgbaImage {
    self.image
  }

  pub(crate) fn size(&self) -> Size<u32> {
    Size {
      width: self.image.width(),
      height: self.image.height(),
    }
  }

  /// Overlays an image onto the canvas with optional border radius.
  pub(crate) fn overlay_image<I: GenericImageView<Pixel = Rgba<u8>>>(
    &mut self,
    image: &I,
    border: BorderProperties,
    transform: Affine,
    algorithm: ImageScalingAlgorithm,
    mode: BlendMode,
  ) {
    overlay_image(
      &mut self.image,
      image,
      border,
      transform,
      algorithm,
      mode,
      &self.constrains,
      &mut self.mask_memory,
    );
  }
}

/// Draws a single pixel on the canvas with alpha blending.
///
/// If the color is fully transparent (alpha = 0), no operation is performed.
/// Otherwise, the pixel is blended with the existing canvas pixel using alpha blending.
///
/// All active constraints in the stack are checked — each constraint's alpha
/// is combined multiplicatively so that nested clip-path + mask-image etc.
/// all contribute to the final pixel alpha.
#[inline(always)]
fn draw_pixel(
  canvas: &mut RgbaImage,
  x: u32,
  y: u32,
  mut color: Rgba<u8>,
  mode: BlendMode,
  constrains: &[CanvasConstrain],
) {
  if color.0[3] == 0 {
    return;
  }

  for constrain in constrains {
    let alpha = constrain.get_alpha(x, y);
    if alpha == 0 {
      return;
    }
    if alpha < 255 {
      apply_mask_alpha_to_pixel(&mut color, alpha);
      if color.0[3] == 0 {
        return;
      }
    }
  }

  // image-rs blend will skip the operation if the source color is fully transparent
  let pixel = canvas.get_pixel_mut(x, y);

  blend_pixel(pixel, color, mode);
}

/// Removes 2 LSBs from the alpha value.
pub(crate) fn quantize_alpha(alpha: u8) -> u8 {
  alpha & 0b1111_1100
}

#[inline(always)]
pub(crate) fn apply_mask_alpha_to_pixel(pixel: &mut Rgba<u8>, alpha: u8) {
  match alpha {
    0 => {
      pixel.0[3] = 0;
    }
    255 => {}
    alpha => {
      pixel.0[3] = fast_div_255(pixel.0[3] as u32 * alpha as u32);
    }
  }
}

pub(crate) fn draw_mask<C: Into<Rgba<u8>>>(
  canvas: &mut RgbaImage,
  mask: &[u8],
  placement: Placement,
  color: C,
  mode: BlendMode,
  constrains: &[CanvasConstrain],
) {
  if mask.is_empty() {
    return;
  }

  assert_eq!(
    mask.len(),
    placement.width as usize * placement.height as usize,
  );

  let offset = Point {
    x: placement.left as f32,
    y: placement.top as f32,
  };
  let top_size = Size {
    width: placement.width,
    height: placement.height,
  };

  let color = color.into();

  overlay_area(canvas, offset, top_size, mode, constrains, |x, y| {
    let alpha = mask[mask_index_from_coord(x, y, placement.width)];

    let mut pixel = color;

    apply_mask_alpha_to_pixel(&mut pixel, alpha);

    pixel
  });
}

/// Samples a pixel from an image given a transform and canvas coordinates.
///
/// This function handles the inverse transform and the scaling algorithm.
/// It also optimizes for translate-only transforms by skipping bilinear interpolation.
#[inline(always)]
pub(crate) fn sample_transformed_pixel<I: GenericImageView<Pixel = Rgba<u8>>>(
  image: &I,
  inverse_transform: Affine,
  algorithm: ImageScalingAlgorithm,
  canvas_x: f32,
  canvas_y: f32,
  offset: Point<f32>,
) -> Option<Rgba<u8>> {
  let sampled_point = inverse_transform.transform_point(Point {
    x: canvas_x,
    y: canvas_y,
  }) + offset;

  if inverse_transform.only_translation() || matches!(algorithm, ImageScalingAlgorithm::Pixelated) {
    interpolate_nearest(image, sampled_point.x, sampled_point.y)
  } else {
    interpolate_bilinear(image, sampled_point.x, sampled_point.y)
  }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn overlay_image<I: GenericImageView<Pixel = Rgba<u8>>>(
  canvas: &mut RgbaImage,
  image: &I,
  border: BorderProperties,
  transform: Affine,
  algorithm: ImageScalingAlgorithm,
  mode: BlendMode,
  constrains: &[CanvasConstrain],
  mask_memory: &mut MaskMemory,
) {
  let (width, height) = image.dimensions();
  let size = Size { width, height };

  // Fast path: if no sub-pixel interpolation is needed, we can just draw the image directly
  if transform.only_translation() && border.is_zero() {
    let translation = transform.decompose_translation();

    return overlay_area(canvas, translation, size, mode, constrains, |x, y| {
      image.get_pixel(x, y)
    });
  }

  let mut paths = Vec::new();

  border.append_mask_commands(&mut paths, size.map(|size| size as f32), Point::ZERO);

  let (mask, placement) = mask_memory.render(&paths, Some(transform), None);

  let inverse = transform.invert();

  let get_original_pixel = |x, y| {
    let alpha = mask[mask_index_from_coord(x, y, placement.width)];

    if alpha == 0 {
      return Color::transparent().into();
    }

    // Fast path: If only border radius is applied, we can just map the pixel directly
    if transform.is_identity() && placement.left >= 0 && placement.top >= 0 {
      let mut pixel = image.get_pixel(x + placement.left as u32, y + placement.top as u32);

      apply_mask_alpha_to_pixel(&mut pixel, alpha);

      return pixel;
    }

    let Some(mut pixel) = inverse.and_then(|inverse| {
      sample_transformed_pixel(
        image,
        inverse,
        algorithm,
        (x as i32 + placement.left) as f32,
        (y as i32 + placement.top) as f32,
        Point::ZERO,
      )
    }) else {
      return Color::transparent().into();
    };

    apply_mask_alpha_to_pixel(&mut pixel, alpha);

    pixel
  };

  overlay_area(
    canvas,
    Point {
      x: placement.left as f32,
      y: placement.top as f32,
    },
    Size {
      width: placement.width,
      height: placement.height,
    },
    mode,
    constrains,
    get_original_pixel,
  );
}

#[inline(always)]
pub(crate) fn mask_index_from_coord(x: u32, y: u32, width: u32) -> usize {
  (y * width + x) as usize
}

pub(crate) fn overlay_area(
  bottom: &mut RgbaImage,
  offset: Point<f32>,
  top_size: Size<u32>,
  mode: BlendMode,
  constrains: &[CanvasConstrain],
  f: impl Fn(u32, u32) -> Rgba<u8>,
) {
  if top_size.width == 0 || top_size.height == 0 {
    return;
  }

  let offset_x = offset.x as i32;
  let offset_y = offset.y as i32;
  let bottom_width = bottom.width() as i32;
  let bottom_height = bottom.height() as i32;

  // Calculate the valid range in the destination image
  let dest_y_min = offset_y.max(0);
  let dest_y_max = (offset_y + top_size.height as i32).min(bottom_height);

  if dest_y_min >= dest_y_max {
    return; // No overlap
  }

  let dest_x_min = offset_x.max(0);
  let dest_x_max = (offset_x + top_size.width as i32).min(bottom_width);

  if dest_x_min >= dest_x_max {
    return; // No horizontal overlap on this row
  }

  // For each destination y, calculate corresponding source y
  for dest_y in dest_y_min..dest_y_max {
    let src_y = (dest_y - offset_y) as u32;

    for dest_x in dest_x_min..dest_x_max {
      let src_x = (dest_x - offset_x) as u32;
      let pixel = f(src_x, src_y);

      draw_pixel(
        bottom,
        dest_x as u32,
        dest_y as u32,
        pixel,
        mode,
        constrains,
      );
    }
  }
}
