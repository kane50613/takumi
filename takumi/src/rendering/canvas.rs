//! Canvas operations and image blending for the takumi rendering system.
//!
//! This module provides performance-optimized canvas operations including
//! fast image blending and pixel manipulation operations.

use std::{borrow::Cow, mem::replace};

use image::{
  GenericImageView, ImageError, Rgba, RgbaImage,
  error::{ParameterError, ParameterErrorKind},
};
use smallvec::SmallVec;
use taffy::{Layout, Point, Size};
use zeno::{Command, Mask, Placement, Scratch};

use crate::{Result, layout::style::BlendMode};
use crate::{
  layout::style::{
    Affine, Color, GradientOverlayTile, ImageScalingAlgorithm, Overflow, ResolvedStyle,
    compute_overlay_bounds, overlay_gradient_tile_fast_normal_unconstrained,
  },
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
    border_radius_mask: Option<(Vec<u8>, u32)>,
  },
  ClipPath {
    mask: Vec<u8>,
    placement: Placement,
  },
  MaskImage {
    mask: Vec<u8>,
    from: Point<u32>,
    to: Point<u32>,
    inverse_transform: Affine,
  },
}

impl CanvasConstrain {
  pub(crate) fn from_node(
    context: &RenderContext,
    style: &ResolvedStyle,
    layout: Layout,
    transform: Affine,
    mask_memory: &mut MaskMemory,
    buffer_pool: &mut BufferPool,
  ) -> Result<CanvasConstrainResult> {
    // Clip path would just clip everything, and behaves like overflow: hidden.
    if let Some(clip_path) = &style.clip_path {
      let (mask, placement) = clip_path.render_mask(context, layout.size, mask_memory, buffer_pool);

      let end_x = placement.left + placement.width as i32;
      let end_y = placement.top + placement.height as i32;

      if end_x < 0 || end_y < 0 {
        buffer_pool.release(mask);
        return Ok(CanvasConstrainResult::SkipRendering);
      }

      return Ok(CanvasConstrainResult::Some(CanvasConstrain::ClipPath {
        mask,
        placement,
      }));
    }

    let Some(inverse_transform) = transform.invert() else {
      return Ok(CanvasConstrainResult::SkipRendering);
    };

    if let Some(mask) = create_mask(context, layout.size, mask_memory, buffer_pool)? {
      return Ok(CanvasConstrainResult::Some(CanvasConstrain::MaskImage {
        mask,
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

      let (mask_data, placement) = mask_memory.render(&paths, None, None, buffer_pool);

      if placement.width == 0 || placement.height == 0 {
        buffer_pool.release(mask_data);
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
        border_radius_mask: Some((mask_data, placement.width)),
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

/// Memory for mask rasterization scratch space and output buffer.
#[derive(Default)]
pub(crate) struct MaskMemory {
  scratch: Scratch,
}

impl MaskMemory {
  pub(crate) fn render(
    &mut self,
    paths: &[Command],
    transform: Option<Affine>,
    style: Option<zeno::Style>,
    buffer_pool: &mut BufferPool,
  ) -> (Vec<u8>, Placement) {
    let style = style.unwrap_or_default();
    let mut bounds = self.scratch.bounds(paths, style, transform.map(Into::into));

    bounds.min = bounds.min.floor();
    bounds.max = bounds.max.ceil();

    let expected_len = (bounds.width() as usize) * (bounds.height() as usize);
    let mut buffer = buffer_pool.acquire(expected_len);

    let placement = Mask::with_scratch(paths, &mut self.scratch)
      .transform(transform.map(Into::into))
      .style(style)
      .render_into(&mut buffer, None);

    assert_eq!(bounds.width() as u32, placement.width);
    assert_eq!(bounds.height() as u32, placement.height);

    (buffer, placement)
  }
}

const BUCKET_COUNT: usize = 32;

/// A pool of reusable RGBA image buffers to avoid repeated heap allocations.
pub(crate) struct BufferPool {
  pools: [Vec<Vec<u8>>; BUCKET_COUNT],
  current_size: usize,
  max_size: usize,
}

impl Default for BufferPool {
  fn default() -> Self {
    const EMPTY_VEC: Vec<Vec<u8>> = Vec::new();
    Self {
      pools: [EMPTY_VEC; BUCKET_COUNT],
      current_size: 0,
      // Default to 64MB limit to avoid excessive memory usage
      max_size: 64 * 1024 * 1024,
    }
  }
}

impl BufferPool {
  fn bucket_index(capacity: usize) -> usize {
    if capacity == 0 {
      return 0;
    }
    capacity.next_power_of_two().trailing_zeros() as usize
  }

  /// Acquires a zero-filled `Vec<u8>` of the given capacity from the pool.
  /// Call [`release`](Self::release) when done to return the buffer.
  pub(crate) fn acquire(&mut self, capacity: usize) -> Vec<u8> {
    let mut index = Self::bucket_index(capacity);
    if index >= BUCKET_COUNT {
      index = BUCKET_COUNT - 1;
    }

    // Find the smallest non-empty bucket that can satisfy this capacity
    for i in index..BUCKET_COUNT {
      if let Some(mut buf) = self.pools[i].pop() {
        self.current_size -= buf.capacity();

        buf.clear();
        buf.resize(capacity, 0);

        return buf;
      }
    }

    // Always allocate at least the power-of-2 size so we neatly fit buckets
    let alloc_cap = (1_usize.checked_shl(index as u32).unwrap_or(capacity)).max(capacity);
    let mut buf = Vec::with_capacity(alloc_cap);

    // For safety, we zero-initialize newly allocated OS memory
    // to avoid potential UB or data leaks from uninitialized OS pages.
    buf.resize(capacity, 0);
    buf
  }

  /// Acquires an uninitialized `Vec<u8>` of the given capacity from the pool.
  /// Call [`release`](Self::release) when done to return the buffer.
  #[allow(clippy::uninit_vec)]
  pub(crate) fn acquire_dirty(&mut self, capacity: usize) -> Vec<u8> {
    let mut index = Self::bucket_index(capacity);
    if index >= BUCKET_COUNT {
      index = BUCKET_COUNT - 1;
    }

    // Find the smallest non-empty bucket that can satisfy this capacity
    for i in index..BUCKET_COUNT {
      if let Some(mut buf) = self.pools[i].pop() {
        self.current_size -= buf.capacity();

        buf.clear();
        unsafe {
          buf.set_len(capacity);
        }

        return buf;
      }
    }

    // Always allocate at least the power-of-2 size so we neatly fit buckets
    let alloc_cap = (1_usize.checked_shl(index as u32).unwrap_or(capacity)).max(capacity);
    let mut buf = Vec::with_capacity(alloc_cap);

    unsafe {
      buf.set_len(capacity);
    }
    buf
  }

  /// Returns a previously acquired buffer to the pool for reuse.
  pub(crate) fn release(&mut self, buffer: Vec<u8>) {
    let cap = buffer.capacity();

    // If adding this buffer exceeds our size limit, just let it be dropped.
    if self.current_size + cap > self.max_size {
      // Actually if dropping it exceeds memory but it's large, we might want to pop smaller ones,
      // but simpler to just drop this one.
      return;
    }

    let mut index = Self::bucket_index(cap);
    if index >= BUCKET_COUNT {
      index = BUCKET_COUNT - 1;
    }

    self.current_size += cap;
    self.pools[index].push(buffer);
  }

  /// Acquires a zeroed `RgbaImage` of the given dimensions from the pool.
  ///
  /// If the pool contains a buffer with enough capacity to hold `width * height * 4` bytes,
  /// it is reused (zero-filled); otherwise a fresh allocation is made.
  /// Call [`release_image`](Self::release_image) when done to return the buffer.
  pub(crate) fn acquire_image(&mut self, width: u32, height: u32) -> Result<RgbaImage> {
    let needed = (width * height * 4) as usize;
    let raw = self.acquire(needed);

    RgbaImage::from_raw(width, height, raw).ok_or_else(|| {
      ImageError::Parameter(ParameterError::from_kind(
        ParameterErrorKind::DimensionMismatch,
      ))
      .into()
    })
  }

  /// Acquires an uninitialized `RgbaImage` of the given dimensions from the pool.
  pub(crate) fn acquire_image_dirty(&mut self, width: u32, height: u32) -> Result<RgbaImage> {
    let needed = (width * height * 4) as usize;
    let raw = self.acquire_dirty(needed);

    RgbaImage::from_raw(width, height, raw).ok_or_else(|| {
      ImageError::Parameter(ParameterError::from_kind(
        ParameterErrorKind::DimensionMismatch,
      ))
      .into()
    })
  }

  /// Returns a previously acquired image's backing buffer to the pool for reuse.
  ///
  /// If the pool is currently at its memory limit, the buffer is dropped instead.
  pub(crate) fn release_image(&mut self, image: RgbaImage) {
    self.release(image.into_raw());
  }
}

/// A canvas that can be used to draw images onto.
pub struct Canvas {
  pub(crate) image: RgbaImage,
  pub(crate) constrains: SmallVec<[CanvasConstrain; 1]>,
  // Since canvas is shared with mutable borrows everywhere already,
  // we can just include the memory here instead of making the function argument bloated.
  pub(crate) mask_memory: MaskMemory,
  pub(crate) buffer_pool: BufferPool,
}

impl Canvas {
  /// Creates a new canvas handle from a draw command sender.
  pub(crate) fn new(size: Size<u32>) -> Self {
    Self {
      image: RgbaImage::new(size.width, size.height),
      constrains: SmallVec::new(),
      mask_memory: MaskMemory::default(),
      buffer_pool: BufferPool::default(),
    }
  }

  pub(crate) fn replace_new_image(&mut self) -> Result<RgbaImage> {
    let size = self.size();

    let new_image = self.buffer_pool.acquire_image(size.width, size.height)?;

    Ok(replace(&mut self.image, new_image))
  }

  pub(crate) fn push_constrain(&mut self, overflow_constrain: CanvasConstrain) {
    self.constrains.push(overflow_constrain);
  }

  pub(crate) fn pop_constrain(&mut self) {
    if let Some(constrain) = self.constrains.pop() {
      match constrain {
        CanvasConstrain::Overflow {
          border_radius_mask: Some((mask, _)),
          ..
        } => {
          self.buffer_pool.release(mask);
        }
        CanvasConstrain::ClipPath { mask, .. } => {
          self.buffer_pool.release(mask);
        }
        CanvasConstrain::MaskImage { mask, .. } => {
          self.buffer_pool.release(mask);
        }
        _ => {}
      }
    }
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
      &mut self.buffer_pool,
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

  let mut current = *canvas.get_pixel(x, y);

  blend_pixel(&mut current, color, mode);

  canvas.put_pixel(x, y, current);
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

#[inline(always)]
pub(crate) fn interpolate_nearest<I: GenericImageView<Pixel = Rgba<u8>>>(
  image: &I,
  x: f32,
  y: f32,
) -> Option<Rgba<u8>> {
  let (w, h) = image.dimensions();
  if w == 0 || h == 0 {
    return None;
  }

  // We accept coordinates slightly outside the boundary due to float precision,
  // clamping to the nearest valid pixel index.
  let px = x.floor().max(0.0) as u32;
  let px = px.min(w.saturating_sub(1));
  let py = y.floor().max(0.0) as u32;
  let py = py.min(h.saturating_sub(1));

  Some(image.get_pixel(px, py))
}

#[inline(always)]
#[allow(clippy::needless_range_loop)]
pub(crate) fn interpolate_bilinear<I: GenericImageView<Pixel = Rgba<u8>>>(
  image: &I,
  x: f32,
  y: f32,
) -> Option<Rgba<u8>> {
  let (w, h) = image.dimensions();
  if w == 0 || h == 0 {
    return None;
  }

  // Map continuous coordinates [0, w] to pixel center coordinates [0, w-1]
  let x = (x - 0.5).clamp(0.0, w.saturating_sub(1) as f32);
  let y = (y - 0.5).clamp(0.0, h.saturating_sub(1) as f32);

  let uf = x.floor() as u32;
  let vf = y.floor() as u32;
  let uc = (uf + 1).min(w - 1);
  let vc = (vf + 1).min(h - 1);

  let p00 = image.get_pixel(uf, vf);
  let p01 = image.get_pixel(uf, vc);
  let p10 = image.get_pixel(uc, vf);
  let p11 = image.get_pixel(uc, vc);

  let u_ratio = ((x - uf as f32) * 256.0) as u32;
  let v_ratio = ((y - vf as f32) * 256.0) as u32;

  let u_opposite = 256 - u_ratio;
  let v_opposite = 256 - v_ratio;

  let w00 = u_opposite * v_opposite;
  let w01 = u_opposite * v_ratio;
  let w10 = u_ratio * v_opposite;
  let w11 = u_ratio * v_ratio;

  let mut out = [0u8; 4];
  for i in 0..4 {
    let val = (p00.0[i] as u32 * w00
      + p10.0[i] as u32 * w10
      + p01.0[i] as u32 * w01
      + p11.0[i] as u32 * w11)
      >> 16;
    out[i] = val as u8;
  }

  Some(image::Rgba(out))
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
  buffer_pool: &mut BufferPool,
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

  let (mask, placement) = mask_memory.render(&paths, Some(transform), None, buffer_pool);

  let inverse = transform.invert();
  let is_identity = transform.is_identity() && placement.left >= 0 && placement.top >= 0;

  if is_identity {
    let get_original_pixel = |x, y| {
      let alpha = mask[mask_index_from_coord(x, y, placement.width)];

      if alpha == 0 {
        return Color::transparent().into();
      }

      let mut pixel = image.get_pixel(x + placement.left as u32, y + placement.top as u32);

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
  } else if let Some(inverse) = inverse {
    let get_original_pixel = |x, y| {
      let alpha = mask[mask_index_from_coord(x, y, placement.width)];

      if alpha == 0 {
        return Color::transparent().into();
      }

      let Some(mut pixel) = sample_transformed_pixel(
        image,
        inverse,
        algorithm,
        (x as i32 + placement.left) as f32 + 0.5,
        (y as i32 + placement.top) as f32 + 0.5,
        Point::ZERO,
      ) else {
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

  buffer_pool.release(mask);
}

#[inline(always)]
pub(crate) fn mask_index_from_coord(x: u32, y: u32, width: u32) -> usize {
  (y * width + x) as usize
}

fn overlay_area_fast_normal_unconstrained(
  bottom: &mut RgbaImage,
  offset: Point<f32>,
  top_size: Size<u32>,
  f: impl Fn(u32, u32) -> Rgba<u8>,
) {
  let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds(bottom, offset, top_size.width, top_size.height)
  else {
    return;
  };

  for dest_y in dest_y_min..dest_y_max {
    let src_y = (dest_y - offset_y) as u32;

    for dest_x in dest_x_min..dest_x_max {
      let src_x = (dest_x - offset_x) as u32;
      let pixel = f(src_x, src_y);
      if pixel.0[3] == 0 {
        continue;
      }

      let current = bottom.get_pixel_mut(dest_x as u32, dest_y as u32);
      blend_pixel(current, pixel, BlendMode::Normal);
    }
  }
}

pub(crate) fn overlay_gradient_tile<T>(
  bottom: &mut RgbaImage,
  gradient: &T,
  offset: Point<f32>,
  mode: BlendMode,
  constrains: &[CanvasConstrain],
) where
  T: GenericImageView<Pixel = Rgba<u8>> + GradientOverlayTile,
{
  let top_size = Size {
    width: GenericImageView::width(gradient),
    height: GenericImageView::height(gradient),
  };

  if mode != BlendMode::Normal || !constrains.is_empty() {
    return overlay_area(bottom, offset, top_size, mode, constrains, |x, y| {
      gradient.get_pixel(x, y)
    });
  }

  overlay_gradient_tile_fast_normal_unconstrained(bottom, gradient, offset);
}

pub(crate) fn overlay_area(
  bottom: &mut RgbaImage,
  offset: Point<f32>,
  top_size: Size<u32>,
  mode: BlendMode,
  constrains: &[CanvasConstrain],
  f: impl Fn(u32, u32) -> Rgba<u8>,
) {
  if constrains.is_empty() && mode == BlendMode::Normal {
    return overlay_area_fast_normal_unconstrained(bottom, offset, top_size, f);
  }

  let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds(bottom, offset, top_size.width, top_size.height)
  else {
    return;
  };

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

#[cfg(test)]
mod tests {
  use image::RgbaImage;

  use crate::{
    GlobalContext,
    layout::style::{
      BlendMode, ConicGradient, ConicGradientTile, FromCss, LinearGradient, LinearGradientTile,
      RadialGradient, RadialGradientTile,
    },
    rendering::{BufferPool, RenderContext, blend_pixel},
  };

  use super::*;

  fn overlay_area_reference(
    bottom: &mut RgbaImage,
    offset: Point<f32>,
    top_size: Size<u32>,
    f: impl Fn(u32, u32) -> Rgba<u8>,
  ) {
    let offset_x = offset.x as i32;
    let offset_y = offset.y as i32;
    let dest_x_min = offset_x.max(0);
    let dest_x_max = (offset_x + top_size.width as i32).min(bottom.width() as i32);
    let dest_y_min = offset_y.max(0);
    let dest_y_max = (offset_y + top_size.height as i32).min(bottom.height() as i32);

    for dest_y in dest_y_min..dest_y_max {
      let src_y = (dest_y - offset_y) as u32;
      for dest_x in dest_x_min..dest_x_max {
        let src_x = (dest_x - offset_x) as u32;
        let pixel = f(src_x, src_y);
        if pixel.0[3] == 0 {
          continue;
        }
        let current = bottom.get_pixel_mut(dest_x as u32, dest_y as u32);
        blend_pixel(current, pixel, BlendMode::Normal);
      }
    }
  }

  #[test]
  fn test_overlay_area_fast_path_normal_matches_reference() {
    let mut fast = RgbaImage::from_pixel(8, 6, Rgba([10, 20, 30, 255]));
    let mut reference = fast.clone();

    let offset = Point { x: 2.0, y: 1.0 };
    let top_size = Size {
      width: 4,
      height: 3,
    };

    overlay_area(
      &mut fast,
      offset,
      top_size,
      BlendMode::Normal,
      &[],
      |x, y| {
        let alpha = ((x + y * 2) * 40).min(255) as u8;
        Rgba([200, 80, 30, alpha])
      },
    );

    overlay_area_reference(&mut reference, offset, top_size, |x, y| {
      let alpha = ((x + y * 2) * 40).min(255) as u8;
      Rgba([200, 80, 30, alpha])
    });

    assert_eq!(fast.as_raw(), reference.as_raw());
  }

  #[test]
  fn test_overlay_linear_gradient_matches_reference() {
    let Ok(gradient) = LinearGradient::from_str("linear-gradient(to right, red, blue)") else {
      unreachable!()
    };
    let global_context = GlobalContext::default();
    let render_context = RenderContext::new_test(&global_context, (32, 16).into());
    let mut buffer_pool = BufferPool::default();
    let tile = LinearGradientTile::new(&gradient, 32, 16, &render_context, &mut buffer_pool);

    let mut fast = RgbaImage::from_pixel(40, 24, Rgba([0, 0, 0, 0]));
    let mut reference = fast.clone();
    let offset = Point { x: 3.0, y: 4.0 };

    overlay_gradient_tile(&mut fast, &tile, offset, BlendMode::Normal, &[]);

    let top_size = Size {
      width: tile.width,
      height: tile.height,
    };
    overlay_area_reference(&mut reference, offset, top_size, |x, y| {
      tile.get_pixel(x, y)
    });

    assert_eq!(fast.as_raw(), reference.as_raw());
  }

  #[test]
  fn test_overlay_radial_gradient_matches_reference() {
    let Ok(gradient) = RadialGradient::from_str("radial-gradient(circle, red, blue)") else {
      unreachable!()
    };
    let global_context = GlobalContext::default();
    let render_context = RenderContext::new_test(&global_context, (32, 24).into());
    let mut buffer_pool = BufferPool::default();
    let tile = RadialGradientTile::new(&gradient, 32, 24, &render_context, &mut buffer_pool);

    let mut fast = RgbaImage::from_pixel(40, 30, Rgba([0, 0, 0, 0]));
    let mut reference = fast.clone();
    let offset = Point { x: 4.0, y: 3.0 };

    overlay_gradient_tile(&mut fast, &tile, offset, BlendMode::Normal, &[]);

    let top_size = Size {
      width: tile.width,
      height: tile.height,
    };
    overlay_area_reference(&mut reference, offset, top_size, |x, y| {
      tile.get_pixel(x, y)
    });

    assert_eq!(fast.as_raw(), reference.as_raw());
  }

  #[test]
  fn test_overlay_conic_gradient_matches_reference() {
    let Ok(gradient) = ConicGradient::from_str("conic-gradient(red, blue)") else {
      unreachable!()
    };

    let global_context = GlobalContext::default();
    let render_context = RenderContext::new_test(&global_context, (32, 24).into());
    let mut buffer_pool = BufferPool::default();
    let tile = ConicGradientTile::new(&gradient, 32, 24, &render_context, &mut buffer_pool);

    let mut fast = RgbaImage::from_pixel(40, 30, Rgba([0, 0, 0, 0]));
    let mut reference = fast.clone();
    let offset = Point { x: 4.0, y: 3.0 };

    overlay_gradient_tile(&mut fast, &tile, offset, BlendMode::Normal, &[]);

    let top_size = Size {
      width: tile.width,
      height: tile.height,
    };
    overlay_area_reference(&mut reference, offset, top_size, |x, y| {
      tile.get_pixel(x, y)
    });

    assert_eq!(fast.as_raw(), reference.as_raw());
  }
}
