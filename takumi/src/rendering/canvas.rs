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

use crate::{
  layout::style::{Affine, Color, ImageScalingAlgorithm, InheritedStyle, Overflow},
  rendering::{BorderProperties, RenderContext, create_mask, fast_div_255},
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
    }))
  }

  pub(crate) fn get_alpha(&self, x: u32, y: u32) -> u8 {
    match *self {
      CanvasConstrain::Overflow {
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
  pub(crate) fn placement<D: PathData>(
    &mut self,
    paths: D,
    transform: Option<Affine>,
    style: Option<zeno::Style>,
  ) -> Placement {
    let style = style.unwrap_or_default();
    let mut bounds = self
      .scratch
      .bounds(&paths, style, transform.map(Into::into));

    bounds.min = bounds.min.floor();
    bounds.max = bounds.max.ceil();

    Placement {
      left: bounds.min.x as i32,
      top: bounds.min.y as i32,
      width: (bounds.max.x - bounds.min.x) as u32,
      height: (bounds.max.y - bounds.min.y) as u32,
    }
  }

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
    opacity: u8,
  ) {
    overlay_image(
      &mut self.image,
      image,
      border,
      transform,
      algorithm,
      opacity,
      self.constrains.last(),
      &mut self.mask_memory,
    );
  }

  /// Fills a rectangular area with the specified color and optional border radius.
  pub(crate) fn fill_color(
    &mut self,
    size: Size<f32>,
    color: Color,
    border: BorderProperties,
    transform: Affine,
  ) {
    if color.0[3] == 0 {
      return;
    }

    // Fast path: if drawing on the entire canvas, we can just replace the entire canvas with the color
    if transform.is_identity()
      && border.is_zero()
      && self.constrains.last().is_none()
      && color.0[3] == 255
      && size.width as u32 == self.image.width()
      && size.height as u32 == self.image.height()
    {
      let image_mut = self.image.as_mut();

      for chunk in image_mut.chunks_exact_mut(4) {
        chunk.copy_from_slice(&color.0);
      }

      return;
    }

    let can_direct_draw = transform.only_translation() && border.is_zero();

    // Fast path: if no sub-pixel interpolation is needed, we can just draw the color directly
    if can_direct_draw {
      let translation = transform.decompose_translation();

      let color: Rgba<u8> = color.into();
      return overlay_area(
        &mut self.image,
        translation,
        size.map(|size| size as u32),
        self.constrains.last(),
        |_, _| color,
      );
    }

    let mut paths = Vec::new();

    border.append_mask_commands(&mut paths, size, Point::ZERO);

    let (mask, placement) = self.mask_memory.render(&paths, Some(transform), None);

    draw_mask(
      &mut self.image,
      mask,
      placement,
      color,
      self.constrains.last(),
    );
  }
}

/// Draws a single pixel on the canvas with alpha blending.
///
/// If the color is fully transparent (alpha = 0), no operation is performed.
/// Otherwise, the pixel is blended with the existing canvas pixel using alpha blending.
#[inline(always)]
fn draw_pixel(
  canvas: &mut RgbaImage,
  x: u32,
  y: u32,
  mut color: Rgba<u8>,
  constrain: Option<&CanvasConstrain>,
) {
  if color.0[3] == 0 {
    return;
  }

  if let Some(constrain_alpha) = constrain.map(|c| c.get_alpha(x, y)) {
    if constrain_alpha == 0 {
      return;
    }

    apply_mask_alpha_to_pixel(&mut color, constrain_alpha);
  }

  // image-rs blend will skip the operation if the source color is fully transparent
  let pixel = canvas.get_pixel_mut(x, y);

  blend_pixel(pixel, color);
}

/// Removes 2 LSBs from the alpha value.
pub(crate) fn quantize_alpha(alpha: u8) -> u8 {
  alpha & 0b1111_1100
}

#[inline(always)]
pub(crate) fn blend_pixel(bottom: &mut Rgba<u8>, top: Rgba<u8>) {
  match (bottom.0[3], top.0[3]) {
    // Source is fully transparent, no operation needed
    (_, 0) => {}
    // Destination is fully transparent, or source is fully opaque: direct assignment
    (0, _) | (_, 255) => *bottom = top,
    // Destination is fully opaque: use integer math to preserve alpha = 255
    // This avoids floating point precision errors from image::Rgba::blend
    (255, src_a) => {
      let src_a = src_a as u16;
      let inv_a = 255 - src_a;

      bottom.0[0] = fast_div_255(top.0[0] as u16 * src_a + bottom.0[0] as u16 * inv_a);
      bottom.0[1] = fast_div_255(top.0[1] as u16 * src_a + bottom.0[1] as u16 * inv_a);
      bottom.0[2] = fast_div_255(top.0[2] as u16 * src_a + bottom.0[2] as u16 * inv_a);
    }
    // Both have partial transparency: use integer-only blend
    (dst_a, src_a) => {
      // Alpha compositing: out_a = src_a + dst_a * (1 - src_a)
      // Using integer math: out_a = src_a + dst_a - (dst_a * src_a) / 255
      let src_a_u16 = src_a as u16;
      let dst_a_u16 = dst_a as u16;
      let out_a = src_a_u16 + dst_a_u16 - fast_div_255(dst_a_u16 * src_a_u16) as u16;

      if out_a == 0 {
        return;
      }

      // Premultiply RGB channels: premul = color * alpha
      let src_r_pm = top.0[0] as u32 * src_a as u32;
      let src_g_pm = top.0[1] as u32 * src_a as u32;
      let src_b_pm = top.0[2] as u32 * src_a as u32;

      let dst_r_pm = bottom.0[0] as u32 * dst_a as u32;
      let dst_g_pm = bottom.0[1] as u32 * dst_a as u32;
      let dst_b_pm = bottom.0[2] as u32 * dst_a as u32;

      // Alpha compositing on premultiplied: out_pm = src_pm + dst_pm * (255 - src_a) / 255
      let inv_src_a = 255 - src_a as u32;
      let out_r_pm = src_r_pm + ((dst_r_pm * inv_src_a + 127) / 255);
      let out_g_pm = src_g_pm + ((dst_g_pm * inv_src_a + 127) / 255);
      let out_b_pm = src_b_pm + ((dst_b_pm * inv_src_a + 127) / 255);

      // Unpremultiply: out = out_pm / out_a
      let out_a_u32 = out_a as u32;

      bottom.0[0] = ((out_r_pm + out_a_u32 / 2) / out_a_u32).min(255) as u8;
      bottom.0[1] = ((out_g_pm + out_a_u32 / 2) / out_a_u32).min(255) as u8;
      bottom.0[2] = ((out_b_pm + out_a_u32 / 2) / out_a_u32).min(255) as u8;
      bottom.0[3] = out_a.min(255) as u8;
    }
  }
}

#[inline(always)]
pub(crate) fn apply_mask_alpha_to_pixel(pixel: &mut Rgba<u8>, alpha: u8) {
  match alpha {
    0 => {
      pixel.0[3] = 0;
    }
    255 => {}
    alpha => {
      pixel.0[3] = fast_div_255(pixel.0[3] as u16 * alpha as u16);
    }
  }
}

pub(crate) fn draw_mask<C: Into<Rgba<u8>>>(
  canvas: &mut RgbaImage,
  mask: &[u8],
  placement: Placement,
  color: C,
  constrain: Option<&CanvasConstrain>,
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

  overlay_area(canvas, offset, top_size, constrain, |x, y| {
    let alpha = mask[mask_index_from_coord(x, y, placement.width)];

    let mut pixel = color;

    apply_mask_alpha_to_pixel(&mut pixel, alpha);

    pixel
  });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn overlay_image<I: GenericImageView<Pixel = Rgba<u8>>>(
  canvas: &mut RgbaImage,
  image: &I,
  border: BorderProperties,
  transform: Affine,
  algorithm: ImageScalingAlgorithm,
  opacity: u8,
  constrain: Option<&CanvasConstrain>,
  mask_memory: &mut MaskMemory,
) {
  let (width, height) = image.dimensions();
  let size = Size { width, height };

  // Fast path: if no sub-pixel interpolation is needed, we can just draw the image directly
  if transform.only_translation() && border.is_zero() {
    let translation = transform.decompose_translation();

    return overlay_area(canvas, translation, size, constrain, |x, y| {
      let mut pixel = image.get_pixel(x, y);

      apply_mask_alpha_to_pixel(&mut pixel, opacity);

      pixel
    });
  }

  let Some(inverse) = transform.invert() else {
    return;
  };

  let mut paths = Vec::new();

  border.append_mask_commands(&mut paths, size.map(|size| size as f32), Point::ZERO);

  let (mask, placement) = mask_memory.render(&paths, Some(transform), None);

  let is_identity = transform.is_identity();

  let get_original_pixel = |x, y| {
    let alpha = mask[mask_index_from_coord(x, y, placement.width)];

    if alpha == 0 {
      return Color::transparent().into();
    }

    // Fast path: If only border radius is applied, we can just map the pixel directly
    if is_identity && placement.left >= 0 && placement.top >= 0 {
      let mut pixel = image.get_pixel(x + placement.left as u32, y + placement.top as u32);

      apply_mask_alpha_to_pixel(&mut pixel, alpha);
      apply_mask_alpha_to_pixel(&mut pixel, opacity);

      return pixel;
    }

    let point = inverse.transform_point(Point {
      x: (x as f32 + placement.left as f32).round(),
      y: (y as f32 + placement.top as f32).round(),
    });

    let sampled_pixel = match algorithm {
      ImageScalingAlgorithm::Pixelated => interpolate_nearest(image, point.x, point.y),
      _ => interpolate_bilinear(image, point.x, point.y),
    };

    let Some(mut pixel) = sampled_pixel else {
      return Color::transparent().into();
    };

    apply_mask_alpha_to_pixel(&mut pixel, alpha);
    apply_mask_alpha_to_pixel(&mut pixel, opacity);

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
    constrain,
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
  constrain: Option<&CanvasConstrain>,
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

      draw_pixel(bottom, dest_x as u32, dest_y as u32, pixel, constrain);
    }
  }
}
