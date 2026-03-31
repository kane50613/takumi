//! Canvas operations and image blending for the takumi rendering system.
//!
//! This module provides performance-optimized canvas operations including
//! fast image blending and pixel manipulation operations.

use std::mem::{replace, size_of};

use image::{
  ImageError, Rgba, RgbaImage,
  error::{ParameterError, ParameterErrorKind},
};
use taffy::{Layout, Point, Size};
use tiny_skia::{
  FillRule as TinyFillRule, FilterQuality as TinyFilterQuality, Mask as TinyMask,
  Paint as TinyPaint, Path as TinyPath, PathBuilder as TinyPathBuilder, Pattern as TinyPattern,
  Pixmap, PixmapMut, PixmapPaint, PixmapRef, PremultipliedColorU8, Rect as TinyRect,
  SpreadMode as TinySpreadMode, Transform as TinyTransform,
};

use super::stacking_context::blend_pixmap_software;
use crate::{Result, layout::style::BlendMode};
use crate::{
  layout::style::{
    Affine, Color, ComputedStyle, GradientOverlayTile, ImageScalingAlgorithm, Overflow,
    overlay_gradient_tile_fast_normal_unconstrained,
  },
  rendering::{
    BackgroundTile, BorderProperties, ColorTile, Command, Placement, RenderContext, Style,
    blend_pixel, build_path, create_mask, fast_div_255,
  },
};

#[derive(Clone, Copy)]
pub(crate) enum PaintSource<'a> {
  RgbaImage(&'a RgbaImage),
  Pixmap(&'a Pixmap),
  BackgroundTile(&'a BackgroundTile),
  ColorTile(&'a ColorTile),
}

impl<'a> PaintSource<'a> {
  pub(crate) fn width(self) -> u32 {
    match self {
      Self::RgbaImage(image) => image.width(),
      Self::Pixmap(pixmap) => pixmap.width(),
      Self::BackgroundTile(tile) => tile.width(),
      Self::ColorTile(tile) => tile.width(),
    }
  }

  pub(crate) fn height(self) -> u32 {
    match self {
      Self::RgbaImage(image) => image.height(),
      Self::Pixmap(pixmap) => pixmap.height(),
      Self::BackgroundTile(tile) => tile.height(),
      Self::ColorTile(tile) => tile.height(),
    }
  }

  pub(crate) fn get_pixel(self, x: u32, y: u32) -> PremultipliedColorU8 {
    match self {
      Self::RgbaImage(image) => {
        let width = image.width();
        let height = image.height();
        if x >= width || y >= height {
          return PremultipliedColorU8::TRANSPARENT;
        }
        let index = ((y * width + x) * 4) as usize;
        let raw = image.as_raw();
        let pixel = [raw[index], raw[index + 1], raw[index + 2], raw[index + 3]];
        let alpha = pixel[3] as u32;
        PremultipliedColorU8::from_rgba(
          fast_div_255(pixel[0] as u32 * alpha),
          fast_div_255(pixel[1] as u32 * alpha),
          fast_div_255(pixel[2] as u32 * alpha),
          pixel[3],
        )
        .unwrap_or_else(|| unreachable!())
      }
      Self::Pixmap(pixmap) => {
        let width = pixmap.width();
        let height = pixmap.height();
        if x >= width || y >= height {
          return PremultipliedColorU8::TRANSPARENT;
        }
        let index = (y * width + x) as usize;
        pixmap.pixels()[index]
      }
      Self::BackgroundTile(tile) => tile.get_pixel(x, y),
      Self::ColorTile(tile) => tile.get_pixel(x, y),
    }
  }

  fn as_pixmap_ref(self) -> Option<PixmapRef<'a>> {
    match self {
      Self::Pixmap(source) => Some(source.as_ref()),
      Self::BackgroundTile(BackgroundTile::Pixmap(source)) => Some(source.as_ref()),
      _ => None,
    }
  }

  fn write_premultiplied(self, dst: &mut [u8]) {
    if let Some(source) = self.as_pixmap_ref() {
      dst.copy_from_slice(bytemuck::cast_slice(source.pixels()));
      return;
    }

    match self {
      Self::RgbaImage(source) => {
        write_premultiplied_rgba(dst, source.as_raw());
      }
      _ => {
        let width = self.width();
        let height = self.height();
        for y in 0..height {
          for x in 0..width {
            let pixel = self.get_pixel(x, y);
            let offset = ((y * width + x) * 4) as usize;
            dst[offset] = pixel.red();
            dst[offset + 1] = pixel.green();
            dst[offset + 2] = pixel.blue();
            dst[offset + 3] = pixel.alpha();
          }
        }
      }
    }
  }

  fn with_pixmap_ref<R>(
    self,
    buffer_pool: &mut BufferPool,
    f: impl FnOnce(PixmapRef<'_>) -> R,
  ) -> Option<R> {
    if let Some(source) = self.as_pixmap_ref() {
      return Some(f(source));
    }

    let width = self.width();
    let height = self.height();
    let source_len = width as usize * height as usize * 4;
    let mut premul_source = buffer_pool.acquire_dirty(source_len);
    self.write_premultiplied(&mut premul_source);
    let result = PixmapRef::from_bytes(&premul_source, width, height).map(f);
    buffer_pool.release(premul_source);
    result
  }

  fn supports_rounded_fill_fast_path(self) -> bool {
    matches!(self, Self::RgbaImage(_) | Self::Pixmap(_))
  }
}

impl<'a> From<&'a RgbaImage> for PaintSource<'a> {
  fn from(value: &'a RgbaImage) -> Self {
    Self::RgbaImage(value)
  }
}

impl<'a> From<&'a Pixmap> for PaintSource<'a> {
  fn from(value: &'a Pixmap) -> Self {
    Self::Pixmap(value)
  }
}

impl<'a> From<&'a BackgroundTile> for PaintSource<'a> {
  fn from(value: &'a BackgroundTile) -> Self {
    Self::BackgroundTile(value)
  }
}

impl<'a> From<&'a ColorTile> for PaintSource<'a> {
  fn from(value: &'a ColorTile) -> Self {
    Self::ColorTile(value)
  }
}

#[inline(always)]
pub(crate) fn premultiplied_to_rgba(pixel: PremultipliedColorU8) -> Rgba<u8> {
  let color = pixel.demultiply();
  Rgba([color.red(), color.green(), color.blue(), color.alpha()])
}

pub(crate) enum NodeMaskAction {
  Shell(TinyMask),
  Content(TinyMask),
  None,
  SkipRendering,
}

#[derive(Clone, Copy)]
pub(crate) struct CanvasViewport {
  pub(crate) origin: Point<u32>,
  pub(crate) size: Size<u32>,
}

impl CanvasViewport {
  pub(crate) fn right(self) -> i32 {
    self.origin.x as i32 + self.size.width as i32
  }

  pub(crate) fn bottom(self) -> i32 {
    self.origin.y as i32 + self.size.height as i32
  }
}

impl NodeMaskAction {
  pub(crate) fn is_some(&self) -> bool {
    matches!(self, Self::Shell(_) | Self::Content(_))
  }
}

pub(crate) fn prepare_node_mask(
  context: &RenderContext,
  style: &ComputedStyle,
  layout: Layout,
  transform: Affine,
  viewport: CanvasViewport,
  buffer_pool: &mut BufferPool,
) -> Result<NodeMaskAction> {
  // Clip path would just clip everything, and behaves like overflow: hidden.
  if let Some(clip_path) = &style.clip_path {
    let (mask, placement) = clip_path.render_mask(context, layout.size, buffer_pool);

    let end_x = placement.left + placement.width as i32;
    let end_y = placement.top + placement.height as i32;

    if end_x < 0 || end_y < 0 {
      buffer_pool.release(mask);
      return Ok(NodeMaskAction::SkipRendering);
    }

    let Some(mut full_mask) = TinyMask::new(viewport.size.width, viewport.size.height) else {
      buffer_pool.release(mask);
      return Ok(NodeMaskAction::SkipRendering);
    };
    full_mask.data_mut().fill(0);
    copy_mask_into_canvas(&mut full_mask, viewport.origin, &mask, placement);
    buffer_pool.release(mask);
    return Ok(NodeMaskAction::Shell(full_mask));
  }

  let Some(inverse_transform) = transform.invert() else {
    return Ok(NodeMaskAction::SkipRendering);
  };

  if let Some(mask) = create_mask(context, layout.size, buffer_pool)? {
    let Some(placement) = transformed_rect_placement(layout.size, transform) else {
      buffer_pool.release(mask);
      return Ok(NodeMaskAction::SkipRendering);
    };
    let Some(full_mask) = rasterize_constraint_mask(viewport, placement, |x, y| {
      sample_mask_image_alpha(
        &mask,
        Point { x: 0, y: 0 },
        Point {
          x: layout.size.width as u32,
          y: layout.size.height as u32,
        },
        inverse_transform,
        x,
        y,
      )
    }) else {
      buffer_pool.release(mask);
      return Ok(NodeMaskAction::SkipRendering);
    };
    buffer_pool.release(mask);
    return Ok(NodeMaskAction::Shell(full_mask));
  }

  let overflow = style.resolve_overflows();

  let clip_x = overflow.x != Overflow::Visible;
  let clip_y = overflow.y != Overflow::Visible;

  if !overflow.should_clip_content() {
    return Ok(NodeMaskAction::None);
  }

  if (clip_x && layout.content_box_width() < f32::EPSILON)
    || (clip_y && layout.content_box_height() < f32::EPSILON)
  {
    return Ok(NodeMaskAction::SkipRendering);
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

    let (mask_data, placement) = render_mask(&paths, None, None, buffer_pool);

    if placement.width == 0 || placement.height == 0 {
      buffer_pool.release(mask_data);
      return Ok(NodeMaskAction::SkipRendering);
    }

    let from = Point {
      x: placement.left.max(0) as u32,
      y: placement.top.max(0) as u32,
    };

    let to = Point {
      x: from.x + placement.width,
      y: from.y + placement.height,
    };
    let Some(full_mask) = rasterize_constraint_mask(viewport, placement, |x, y| {
      sample_overflow_alpha(
        from,
        to,
        inverse_transform,
        Some((&mask_data, placement.width)),
        x,
        y,
      )
    }) else {
      buffer_pool.release(mask_data);
      return Ok(NodeMaskAction::SkipRendering);
    };
    buffer_pool.release(mask_data);
    return Ok(NodeMaskAction::Content(full_mask));
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

  if to.x != u32::MAX
    && to.y != u32::MAX
    && let Some(rect) = TinyRect::from_ltrb(from.x as f32, from.y as f32, to.x as f32, to.y as f32)
    && let Some(forward_transform) = inverse_transform.invert()
    && let Some(mut mask) = TinyMask::new(viewport.size.width, viewport.size.height)
  {
    mask.data_mut().fill(u8::MAX);
    let path = TinyPathBuilder::from_rect(rect);
    let localized_transform =
      Affine::translation(-(viewport.origin.x as f32), -(viewport.origin.y as f32))
        * forward_transform;
    mask.intersect_path(
      &path,
      TinyFillRule::Winding,
      true,
      TinyTransform::from(localized_transform),
    );
    return Ok(NodeMaskAction::Content(mask));
  }

  let Some(placement) = overflow_mask_placement(layout.size, transform, viewport, clip_x, clip_y)
  else {
    return Ok(NodeMaskAction::SkipRendering);
  };
  let Some(mask) = rasterize_constraint_mask(viewport, placement, |x, y| {
    sample_overflow_alpha(from, to, inverse_transform, None, x, y)
  }) else {
    return Ok(NodeMaskAction::SkipRendering);
  };
  Ok(NodeMaskAction::Content(mask))
}

fn overflow_mask_placement(
  size: Size<f32>,
  transform: Affine,
  viewport: CanvasViewport,
  clip_x: bool,
  clip_y: bool,
) -> Option<Placement> {
  let mut placement = transformed_rect_placement(size, transform)?;

  if clip_x == clip_y {
    return Some(placement);
  }

  if !transform.only_translation() {
    return Some(Placement {
      left: viewport.origin.x as i32,
      top: viewport.origin.y as i32,
      width: viewport.size.width,
      height: viewport.size.height,
    });
  }

  if !clip_x {
    placement.left = viewport.origin.x as i32;
    placement.width = viewport.size.width;
  }

  if !clip_y {
    placement.top = viewport.origin.y as i32;
    placement.height = viewport.size.height;
  }

  Some(placement)
}

fn copy_mask_into_canvas(
  canvas_mask: &mut TinyMask,
  canvas_origin: Point<u32>,
  mask: &[u8],
  placement: Placement,
) {
  let canvas_left = canvas_origin.x as i32;
  let canvas_top = canvas_origin.y as i32;
  let canvas_right = canvas_left + canvas_mask.width() as i32;
  let canvas_bottom = canvas_top + canvas_mask.height() as i32;
  let src_width = placement.width as i32;
  let src_height = placement.height as i32;
  let start_x = placement.left.max(canvas_left);
  let start_y = placement.top.max(canvas_top);
  let end_x = (placement.left + src_width).min(canvas_right);
  let end_y = (placement.top + src_height).min(canvas_bottom);

  if start_x >= end_x || start_y >= end_y {
    return;
  }

  let stride = canvas_mask.width() as usize;
  let data = canvas_mask.data_mut();
  for global_y in start_y..end_y {
    let src_y = (global_y - placement.top) as usize;
    let canvas_row = (global_y - canvas_top) as usize * stride;
    let src_row = src_y * placement.width as usize;
    for global_x in start_x..end_x {
      let src_x = (global_x - placement.left) as usize;
      data[canvas_row + (global_x - canvas_left) as usize] = mask[src_row + src_x];
    }
  }
}

fn rasterize_constraint_mask(
  viewport: CanvasViewport,
  placement: Placement,
  alpha_at: impl Fn(u32, u32) -> u8,
) -> Option<TinyMask> {
  let mut mask = TinyMask::new(viewport.size.width, viewport.size.height)?;
  mask.data_mut().fill(0);

  let start_x = placement.left.max(viewport.origin.x as i32);
  let start_y = placement.top.max(viewport.origin.y as i32);
  let end_x = (placement.left + placement.width as i32).min(viewport.right());
  let end_y = (placement.top + placement.height as i32).min(viewport.bottom());
  if start_x >= end_x || start_y >= end_y {
    return Some(mask);
  }

  let data = mask.data_mut();
  let stride = viewport.size.width as usize;
  for global_y in start_y..end_y {
    let row = (global_y - viewport.origin.y as i32) as usize * stride;
    for global_x in start_x..end_x {
      data[row + (global_x - viewport.origin.x as i32) as usize] =
        alpha_at(global_x as u32, global_y as u32);
    }
  }
  Some(mask)
}

fn transformed_rect_placement(size: Size<f32>, transform: Affine) -> Option<Placement> {
  let corners = [
    transform.transform_point(Point::ZERO),
    transform.transform_point(Point {
      x: size.width,
      y: 0.0,
    }),
    transform.transform_point(Point {
      x: 0.0,
      y: size.height,
    }),
    transform.transform_point(Point {
      x: size.width,
      y: size.height,
    }),
  ];

  let mut left = f32::INFINITY;
  let mut top = f32::INFINITY;
  let mut right = f32::NEG_INFINITY;
  let mut bottom = f32::NEG_INFINITY;
  for point in corners {
    left = left.min(point.x);
    top = top.min(point.y);
    right = right.max(point.x);
    bottom = bottom.max(point.y);
  }

  let left = left.floor() as i32;
  let top = top.floor() as i32;
  let right = right.ceil() as i32;
  let bottom = bottom.ceil() as i32;
  if right <= left || bottom <= top {
    return None;
  }

  Some(Placement {
    left,
    top,
    width: (right - left) as u32,
    height: (bottom - top) as u32,
  })
}

fn sample_mask_image_alpha(
  mask: &[u8],
  from: Point<u32>,
  to: Point<u32>,
  inverse_transform: Affine,
  x: u32,
  y: u32,
) -> u8 {
  let Some(original_point) = transformed_mask_point(inverse_transform, from, to, x, y) else {
    return 0;
  };
  mask[mask_index_from_coord(original_point.x, original_point.y, to.x - from.x)]
}

fn sample_overflow_alpha(
  from: Point<u32>,
  to: Point<u32>,
  inverse_transform: Affine,
  border_radius_mask: Option<(&[u8], u32)>,
  x: u32,
  y: u32,
) -> u8 {
  let Some(original_point) = transformed_mask_point(inverse_transform, from, to, x, y) else {
    return 0;
  };

  if let Some((mask, mask_width)) = border_radius_mask {
    let mask_x = original_point.x - from.x;
    let mask_y = original_point.y - from.y;
    return mask[mask_index_from_coord(mask_x, mask_y, mask_width)];
  }

  u8::MAX
}

fn transformed_mask_point(
  inverse_transform: Affine,
  from: Point<u32>,
  to: Point<u32>,
  x: u32,
  y: u32,
) -> Option<Point<u32>> {
  let original_point = inverse_transform.transform_point(Point {
    x: x as f32,
    y: y as f32,
  });
  if original_point.x < 0.0 || original_point.y < 0.0 {
    return None;
  }

  let original_point = original_point.map(|point| point as u32);
  let is_contained = original_point.x >= from.x
    && original_point.x < to.x
    && original_point.y >= from.y
    && original_point.y < to.y;
  if !is_contained {
    return None;
  }
  Some(original_point)
}

pub(crate) fn render_mask(
  paths: &[Command],
  transform: Option<Affine>,
  style: Option<Style>,
  buffer_pool: &mut BufferPool,
) -> (Vec<u8>, Placement) {
  let style = style.unwrap_or_default();
  let Some(mut path) = build_path(paths) else {
    return (Vec::new(), Placement::default());
  };

  if let Some(stroke) = style.stroke() {
    let Some(stroked_path) = path.stroke(&stroke, 1.0) else {
      return (Vec::new(), Placement::default());
    };
    path = stroked_path;
  }

  if let Some(transform) = transform {
    let Some(transformed) = path.transform(transform.into()) else {
      return (Vec::new(), Placement::default());
    };
    path = transformed;
  }

  let Some(bounds) = path.compute_tight_bounds() else {
    return (Vec::new(), Placement::default());
  };
  let left = bounds.left().floor() as i32;
  let top = bounds.top().floor() as i32;
  let right = bounds.right().ceil() as i32;
  let bottom = bounds.bottom().ceil() as i32;

  if right <= left || bottom <= top {
    return (Vec::new(), Placement::default());
  }

  let width = (right - left) as u32;
  let height = (bottom - top) as u32;
  let Some(mut mask) = TinyMask::new(width, height) else {
    return (Vec::new(), Placement::default());
  };
  let Some(local_path) =
    path.transform(TinyTransform::from_translate(-(left as f32), -(top as f32)))
  else {
    return (Vec::new(), Placement::default());
  };
  mask.fill_path(
    &local_path,
    style.fill_rule(),
    true,
    TinyTransform::identity(),
  );

  let mut buffer = buffer_pool.acquire(mask.data().len());
  buffer.copy_from_slice(mask.data());
  (
    buffer,
    Placement {
      left,
      top,
      width,
      height,
    },
  )
}

const BUCKET_COUNT: usize = 32;

/// A pool of reusable RGBA image buffers to avoid repeated heap allocations.
pub(crate) struct BufferPool {
  pools: [Vec<Vec<u8>>; BUCKET_COUNT],
  u32_pools: [Vec<Vec<u32>>; BUCKET_COUNT],
  current_size: usize,
  max_size: usize,
}

impl Default for BufferPool {
  fn default() -> Self {
    const EMPTY_VEC: Vec<Vec<u8>> = Vec::new();
    const EMPTY_U32_VEC: Vec<Vec<u32>> = Vec::new();
    Self {
      pools: [EMPTY_VEC; BUCKET_COUNT],
      u32_pools: [EMPTY_U32_VEC; BUCKET_COUNT],
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
    if buffer.is_empty() || buffer.capacity() == 0 {
      return;
    }

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

  /// Acquires a zero-filled `Vec<u32>` of the given capacity from the pool.
  /// Call [`release_u32`](Self::release_u32) when done to return the buffer.
  pub(crate) fn acquire_u32(&mut self, capacity: usize) -> Vec<u32> {
    let mut index = Self::bucket_index(capacity);
    if index >= BUCKET_COUNT {
      index = BUCKET_COUNT - 1;
    }

    for i in index..BUCKET_COUNT {
      if let Some(mut buf) = self.u32_pools[i].pop() {
        self.current_size -= buf.capacity() * size_of::<u32>();
        buf.clear();
        buf.resize(capacity, 0);
        return buf;
      }
    }

    let alloc_cap = (1_usize.checked_shl(index as u32).unwrap_or(capacity)).max(capacity);
    let mut buf = Vec::with_capacity(alloc_cap);
    buf.resize(capacity, 0);
    buf
  }

  /// Returns a previously acquired `Vec<u32>` to the pool for reuse.
  pub(crate) fn release_u32(&mut self, buffer: Vec<u32>) {
    if buffer.is_empty() || buffer.capacity() == 0 {
      return;
    }

    let cap_bytes = buffer.capacity() * size_of::<u32>();
    if self.current_size + cap_bytes > self.max_size {
      return;
    }

    let mut index = Self::bucket_index(buffer.capacity());
    if index >= BUCKET_COUNT {
      index = BUCKET_COUNT - 1;
    }

    self.current_size += cap_bytes;
    self.u32_pools[index].push(buffer);
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

  /// Returns a previously acquired image's backing buffer to the pool for reuse.
  ///
  /// If the pool is currently at its memory limit, the buffer is dropped instead.
  pub(crate) fn release_image(&mut self, image: RgbaImage) {
    self.release(image.into_raw());
  }
}

/// A canvas that can be used to draw images onto.
pub(crate) struct Canvas {
  image: Pixmap,
  origin: Point<u32>,
  offscreen_pool: Vec<Pixmap>,
  constraint_mask_stack: Vec<Option<TinyMask>>,
  pub(crate) buffer_pool: BufferPool,
}

pub(crate) struct CanvasSubcanvas {
  image: Pixmap,
  origin: Option<Point<u32>>,
  constraint_mask_stack: Option<Vec<Option<TinyMask>>>,
  offset: Point<i32>,
}

impl Canvas {
  /// Creates a new canvas handle from a draw command sender.
  pub(crate) fn new(size: Size<u32>) -> Self {
    let Some(image) = Pixmap::new(size.width, size.height) else {
      unreachable!()
    };
    Self {
      image,
      origin: Point { x: 0, y: 0 },
      offscreen_pool: Vec::new(),
      constraint_mask_stack: Vec::new(),
      buffer_pool: BufferPool::default(),
    }
  }

  fn acquire_offscreen(&mut self, size: Size<u32>) -> Result<Pixmap> {
    if let Some(index) = self
      .offscreen_pool
      .iter()
      .position(|image| image.width() == size.width && image.height() == size.height)
    {
      return Ok(self.offscreen_pool.swap_remove(index));
    }

    Pixmap::new(size.width, size.height).ok_or_else(|| {
      ImageError::Parameter(ParameterError::from_kind(
        ParameterErrorKind::DimensionMismatch,
      ))
      .into()
    })
  }

  pub(crate) fn begin_subcanvas(&mut self, bounds: Placement) -> Result<CanvasSubcanvas> {
    let size = Size {
      width: bounds.width,
      height: bounds.height,
    };
    let mut image = self.acquire_offscreen(size)?;
    image.data_mut().fill(0);

    let viewport = self.viewport();
    if bounds.left == viewport.origin.x as i32
      && bounds.top == viewport.origin.y as i32
      && bounds.width == viewport.size.width
      && bounds.height == viewport.size.height
    {
      return Ok(CanvasSubcanvas {
        image: replace(&mut self.image, image),
        origin: None,
        constraint_mask_stack: None,
        offset: Point { x: 0, y: 0 },
      });
    }

    let parent_origin = self.origin;
    let offset = Point {
      x: bounds.left - parent_origin.x as i32,
      y: bounds.top - parent_origin.y as i32,
    };
    let origin = Point {
      x: bounds.left as u32,
      y: bounds.top as u32,
    };
    let constraint_mask_stack = self
      .constraint_mask_stack
      .last()
      .and_then(Option::as_ref)
      .and_then(|mask| crop_mask(mask, offset, size))
      .map_or_else(Vec::new, |mask| vec![Some(mask)]);

    Ok(CanvasSubcanvas {
      image: replace(&mut self.image, image),
      origin: Some(replace(&mut self.origin, origin)),
      constraint_mask_stack: Some(replace(
        &mut self.constraint_mask_stack,
        constraint_mask_stack,
      )),
      offset,
    })
  }

  pub(crate) fn composite_subcanvas(&mut self, subcanvas: CanvasSubcanvas, mode: BlendMode) {
    let isolated_image = replace(&mut self.image, subcanvas.image);
    if let Some(origin) = subcanvas.origin {
      self.origin = origin;
    }
    if let Some(constraint_mask_stack) = subcanvas.constraint_mask_stack {
      self.constraint_mask_stack = constraint_mask_stack;
    }

    if let Some(blend_mode) = to_tiny_blend_mode(mode) {
      let paint = PixmapPaint {
        opacity: 1.0,
        blend_mode,
        quality: TinyFilterQuality::Nearest,
      };
      self.image.draw_pixmap(
        subcanvas.offset.x,
        subcanvas.offset.y,
        isolated_image.as_ref(),
        &paint,
        TinyTransform::identity(),
        None,
      );
    } else {
      blend_pixmap_software(&mut self.image, &isolated_image, mode, subcanvas.offset);
    }

    self.recycle_offscreen_image(isolated_image);
  }

  pub(crate) fn push_mask(&mut self, mask: TinyMask) {
    self
      .constraint_mask_stack
      .push(self.build_constraint_mask(&mask));
  }

  pub(crate) fn pop_mask(&mut self) {
    self.constraint_mask_stack.pop();
  }

  pub(crate) fn into_inner(self) -> RgbaImage {
    RgbaImage::from_raw(self.image.width(), self.image.height(), self.image.take())
      .unwrap_or_else(|| unreachable!())
  }

  pub(crate) fn recycle_offscreen_image(&mut self, mut image: Pixmap) {
    const MAX_OFFSCREEN_POOL: usize = 8;
    if self.offscreen_pool.len() >= MAX_OFFSCREEN_POOL {
      return;
    }
    image.data_mut().fill(0);
    self.offscreen_pool.push(image);
  }

  pub(crate) fn with_pixmap_and_pool<R>(
    &mut self,
    f: impl FnOnce(&mut Pixmap, &mut BufferPool) -> R,
  ) -> R {
    f(&mut self.image, &mut self.buffer_pool)
  }

  pub(crate) fn with_pixmap_ref_and_pool<R>(
    &mut self,
    f: impl FnOnce(&Pixmap, &mut BufferPool) -> R,
  ) -> R {
    f(&self.image, &mut self.buffer_pool)
  }

  pub(crate) fn overlay_area(
    &mut self,
    offset: Point<f32>,
    top_size: Size<u32>,
    mode: BlendMode,
    f: impl Fn(u32, u32) -> Rgba<u8>,
  ) {
    let offset = self.localize_offset(offset);
    self.with_overlay_state(|pixmap, combined_mask, _| {
      overlay_area(pixmap, offset, top_size, mode, combined_mask, f);
    });
  }

  pub(crate) fn draw_mask<C: Into<Rgba<u8>>>(
    &mut self,
    mask: &[u8],
    placement: Placement,
    color: C,
    mode: BlendMode,
  ) {
    let placement = self.localize_placement(placement);
    self.with_overlay_state(|pixmap, combined_mask, _| {
      draw_mask(pixmap, mask, placement, color.into(), mode, combined_mask);
    });
  }

  #[allow(clippy::too_many_arguments)]
  pub(crate) fn overlay_sampled_image(
    &mut self,
    source: &RgbaImage,
    width: u32,
    height: u32,
    border: BorderProperties,
    transform: Affine,
    logical_to_source: Affine,
    algorithm: ImageScalingAlgorithm,
    mode: BlendMode,
  ) {
    let transform = self.localize_transform(transform);
    self.with_overlay_state(|pixmap, combined_mask, buffer_pool| {
      overlay_sampled_image(
        pixmap,
        source,
        width,
        height,
        border,
        transform,
        logical_to_source,
        algorithm,
        mode,
        combined_mask,
        buffer_pool,
      );
    });
  }

  pub(crate) fn size(&self) -> Size<u32> {
    Size {
      width: self.image.width(),
      height: self.image.height(),
    }
  }

  pub(crate) fn viewport(&self) -> CanvasViewport {
    CanvasViewport {
      origin: self.origin,
      size: self.size(),
    }
  }

  /// Overlays an image onto the canvas with optional border radius.
  pub(crate) fn overlay_image<'a, I: Into<PaintSource<'a>>>(
    &mut self,
    image: I,
    border: BorderProperties,
    transform: Affine,
    algorithm: ImageScalingAlgorithm,
    mode: BlendMode,
  ) {
    let transform = self.localize_transform(transform);
    self.with_overlay_state(|pixmap, combined_mask, buffer_pool| {
      overlay_image(
        pixmap,
        image,
        border,
        transform,
        algorithm,
        mode,
        combined_mask,
        buffer_pool,
      );
    });
  }

  fn with_overlay_state<R>(
    &mut self,
    f: impl FnOnce(&mut PixmapMut<'_>, Option<&TinyMask>, &mut BufferPool) -> R,
  ) -> R {
    let combined_mask = self.constraint_mask_stack.last().and_then(Option::as_ref);
    let mut pixmap = self.image.as_mut();
    f(&mut pixmap, combined_mask, &mut self.buffer_pool)
  }

  fn build_constraint_mask(&self, mask: &TinyMask) -> Option<TinyMask> {
    let mut combined = TinyMask::new(mask.width(), mask.height())?;
    if let Some(previous) = self.constraint_mask_stack.last().and_then(Option::as_ref) {
      for (dst, (left, right)) in combined
        .data_mut()
        .iter_mut()
        .zip(previous.data().iter().zip(mask.data().iter()))
      {
        *dst = fast_div_255(*left as u32 * *right as u32);
      }
    } else {
      combined.data_mut().copy_from_slice(mask.data());
    }
    Some(combined)
  }

  fn localize_offset(&self, offset: Point<f32>) -> Point<f32> {
    Point {
      x: offset.x - self.origin.x as f32,
      y: offset.y - self.origin.y as f32,
    }
  }

  fn localize_transform(&self, transform: Affine) -> Affine {
    Affine::translation(-(self.origin.x as f32), -(self.origin.y as f32)) * transform
  }

  fn localize_placement(&self, placement: Placement) -> Placement {
    Placement {
      left: placement.left - self.origin.x as i32,
      top: placement.top - self.origin.y as i32,
      ..placement
    }
  }
}

fn crop_mask(mask: &TinyMask, offset: Point<i32>, size: Size<u32>) -> Option<TinyMask> {
  let mut cropped = TinyMask::new(size.width, size.height)?;
  cropped.data_mut().fill(0);

  let src_width = mask.width() as i32;
  let src_height = mask.height() as i32;
  let start_x = offset.x.max(0);
  let start_y = offset.y.max(0);
  let end_x = (offset.x + size.width as i32).min(src_width);
  let end_y = (offset.y + size.height as i32).min(src_height);
  if start_x >= end_x || start_y >= end_y {
    return Some(cropped);
  }

  let src = mask.data();
  let dst = cropped.data_mut();
  let dst_width = size.width as usize;
  let src_width = src_width as usize;
  for src_y in start_y..end_y {
    let dst_y = (src_y - offset.y) as usize;
    let src_row = src_y as usize * src_width;
    let dst_row = dst_y * dst_width;
    for src_x in start_x..end_x {
      let dst_x = (src_x - offset.x) as usize;
      dst[dst_row + dst_x] = src[src_row + src_x as usize];
    }
  }

  Some(cropped)
}

fn to_tiny_blend_mode(mode: BlendMode) -> Option<tiny_skia::BlendMode> {
  use tiny_skia::BlendMode as T;

  Some(match mode {
    BlendMode::Normal => T::SourceOver,
    BlendMode::Multiply => T::Multiply,
    BlendMode::Screen => T::Screen,
    BlendMode::Overlay => T::Overlay,
    BlendMode::Darken => T::Darken,
    BlendMode::Lighten => T::Lighten,
    BlendMode::ColorDodge => T::ColorDodge,
    BlendMode::ColorBurn => T::ColorBurn,
    BlendMode::HardLight => T::HardLight,
    BlendMode::SoftLight => T::SoftLight,
    BlendMode::Difference => T::Difference,
    BlendMode::Exclusion => T::Exclusion,
    BlendMode::Hue => T::Hue,
    BlendMode::Saturation => T::Saturation,
    BlendMode::Color => T::Color,
    BlendMode::Luminosity => T::Luminosity,
    BlendMode::PlusLighter => T::Plus,
    BlendMode::PlusDarker => return None,
  })
}

fn to_tiny_filter_quality(algorithm: ImageScalingAlgorithm) -> TinyFilterQuality {
  match algorithm {
    ImageScalingAlgorithm::Pixelated => TinyFilterQuality::Nearest,
    ImageScalingAlgorithm::Auto | ImageScalingAlgorithm::Smooth => TinyFilterQuality::Bilinear,
  }
}

fn write_premultiplied_rgba(dst: &mut [u8], src: &[u8]) {
  for (dst_px, src_px) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
    let alpha = src_px[3] as u32;
    dst_px[0] = fast_div_255(src_px[0] as u32 * alpha);
    dst_px[1] = fast_div_255(src_px[1] as u32 * alpha);
    dst_px[2] = fast_div_255(src_px[2] as u32 * alpha);
    dst_px[3] = src_px[3];
  }
}

#[inline(always)]
fn compute_overlay_bounds_for_canvas(
  canvas_width: u32,
  canvas_height: u32,
  offset: Point<f32>,
  width: u32,
  height: u32,
) -> Option<(i32, i32, i32, i32, i32, i32)> {
  if width == 0 || height == 0 {
    return None;
  }

  let offset_x = offset.x.trunc() as i32;
  let offset_y = offset.y.trunc() as i32;
  let bottom_width = canvas_width as i32;
  let bottom_height = canvas_height as i32;
  let dest_y_min = offset_y.max(0);
  let dest_y_max = (offset_y + height as i32).min(bottom_height);
  if dest_y_min >= dest_y_max {
    return None;
  }

  let dest_x_min = offset_x.max(0);
  let dest_x_max = (offset_x + width as i32).min(bottom_width);
  if dest_x_min >= dest_x_max {
    return None;
  }

  Some((
    offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max,
  ))
}

pub(crate) fn overlay_area(
  pixmap: &mut PixmapMut<'_>,
  offset: Point<f32>,
  top_size: Size<u32>,
  mode: BlendMode,
  combined_mask: Option<&TinyMask>,
  f: impl Fn(u32, u32) -> Rgba<u8>,
) {
  let canvas_width = pixmap.width();
  let canvas_height = pixmap.height();
  let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds_for_canvas(
      canvas_width,
      canvas_height,
      offset,
      top_size.width,
      top_size.height,
    )
  else {
    return;
  };

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let mask_data = combined_mask.map(TinyMask::data);
  for dest_y in dest_y_min..dest_y_max {
    let src_y = (dest_y - offset_y) as u32;

    for dest_x in dest_x_min..dest_x_max {
      let src_x = (dest_x - offset_x) as u32;
      let mut color = f(src_x, src_y);
      if color.0[3] == 0 {
        continue;
      }

      let dest_x = dest_x as u32;
      let dest_y = dest_y as u32;
      if let Some(mask_data) = mask_data {
        let alpha = mask_data[mask_index_from_coord(dest_x, dest_y, canvas_width)];
        if alpha == 0 {
          continue;
        }
        if alpha < 255 {
          apply_mask_alpha_to_pixel(&mut color, alpha);
          if color.0[3] == 0 {
            continue;
          }
        }
      }

      let index = (dest_y * canvas_width + dest_x) as usize;
      let [r, g, b, a] = pixels[index];
      let current_premul = PremultipliedColorU8::from_rgba(r.min(a), g.min(a), b.min(a), a)
        .unwrap_or(PremultipliedColorU8::TRANSPARENT);
      let mut current = premultiplied_to_rgba(current_premul);
      blend_pixel(&mut current, color, mode);

      let alpha = current.0[3] as u32;
      pixels[index] = [
        fast_div_255(current.0[0] as u32 * alpha),
        fast_div_255(current.0[1] as u32 * alpha),
        fast_div_255(current.0[2] as u32 * alpha),
        current.0[3],
      ];
    }
  }
}

pub(crate) fn draw_mask(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  placement: Placement,
  color: Rgba<u8>,
  mode: BlendMode,
  combined_mask: Option<&TinyMask>,
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

  overlay_area(pixmap, offset, top_size, mode, combined_mask, |x, y| {
    let alpha = mask[mask_index_from_coord(x, y, placement.width)];
    let mut pixel = color;
    apply_mask_alpha_to_pixel(&mut pixel, alpha);
    pixel
  });
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

/// Samples a pixel from an image given a transform and canvas coordinates.
///
/// This function handles the inverse transform and the scaling algorithm.
/// It also optimizes for translate-only transforms by skipping bilinear interpolation.
#[inline(always)]
pub(crate) fn sample_transformed_pixel(
  image: PaintSource<'_>,
  inverse_transform: Affine,
  algorithm: ImageScalingAlgorithm,
  canvas_x: f32,
  canvas_y: f32,
  offset: Point<f32>,
) -> Option<PremultipliedColorU8> {
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
pub(crate) fn interpolate_nearest(
  image: PaintSource<'_>,
  x: f32,
  y: f32,
) -> Option<PremultipliedColorU8> {
  let w = image.width();
  let h = image.height();
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
pub(crate) fn interpolate_bilinear(
  image: PaintSource<'_>,
  x: f32,
  y: f32,
) -> Option<PremultipliedColorU8> {
  let w = image.width();
  let h = image.height();
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
  for (i, channel) in out.iter_mut().enumerate() {
    let p00_i = match i {
      0 => p00.red(),
      1 => p00.green(),
      2 => p00.blue(),
      _ => p00.alpha(),
    };
    let p01_i = match i {
      0 => p01.red(),
      1 => p01.green(),
      2 => p01.blue(),
      _ => p01.alpha(),
    };
    let p10_i = match i {
      0 => p10.red(),
      1 => p10.green(),
      2 => p10.blue(),
      _ => p10.alpha(),
    };
    let p11_i = match i {
      0 => p11.red(),
      1 => p11.green(),
      2 => p11.blue(),
      _ => p11.alpha(),
    };
    let val =
      (p00_i as u32 * w00 + p10_i as u32 * w10 + p01_i as u32 * w01 + p11_i as u32 * w11) >> 16;
    *channel = val as u8;
  }

  PremultipliedColorU8::from_rgba(out[0], out[1], out[2], out[3])
}

fn try_draw_image_with_tiny_skia(
  pixmap: &mut PixmapMut<'_>,
  image: PaintSource<'_>,
  transform: Affine,
  algorithm: ImageScalingAlgorithm,
  mode: BlendMode,
  combined_mask: Option<&TinyMask>,
  buffer_pool: &mut BufferPool,
) -> bool {
  let Some(blend_mode) = to_tiny_blend_mode(mode) else {
    return false;
  };

  let paint = PixmapPaint {
    opacity: 1.0,
    blend_mode,
    quality: to_tiny_filter_quality(algorithm),
  };

  image
    .with_pixmap_ref(buffer_pool, |source_pixmap| {
      pixmap.draw_pixmap(0, 0, source_pixmap, &paint, transform.into(), combined_mask);
      true
    })
    .unwrap_or(false)
}

fn try_fill_color_with_tiny_skia(
  pixmap: &mut PixmapMut<'_>,
  color: &ColorTile,
  size: Size<u32>,
  border: BorderProperties,
  transform: Affine,
  mode: BlendMode,
  combined_mask: Option<&TinyMask>,
) -> bool {
  let Some(blend_mode) = to_tiny_blend_mode(mode) else {
    return false;
  };
  let Some(path) = build_border_path(border, size) else {
    return false;
  };

  let mut paint = TinyPaint::default();
  let [red, green, blue, alpha] = color.color().0;
  paint.set_color_rgba8(red, green, blue, alpha);
  paint.blend_mode = blend_mode;
  paint.anti_alias = true;
  pixmap.fill_path(
    &path,
    &paint,
    TinyFillRule::Winding,
    transform.into(),
    combined_mask,
  );
  true
}

fn build_border_path(border: BorderProperties, size: Size<u32>) -> Option<TinyPath> {
  let mut commands = Vec::new();
  border.append_mask_commands(&mut commands, size.map(|v| v as f32), Point::ZERO);
  build_path(&commands)
}

#[allow(clippy::too_many_arguments)]
fn try_fill_image_path_with_tiny_skia(
  pixmap: &mut PixmapMut<'_>,
  image: PaintSource<'_>,
  size: Size<u32>,
  border: BorderProperties,
  transform: Affine,
  source_to_canvas: Affine,
  algorithm: ImageScalingAlgorithm,
  mode: BlendMode,
  combined_mask: Option<&TinyMask>,
  buffer_pool: &mut BufferPool,
) -> bool {
  let Some(blend_mode) = to_tiny_blend_mode(mode) else {
    return false;
  };
  let Some(path) = build_border_path(border, size) else {
    return false;
  };

  image
    .with_pixmap_ref(buffer_pool, |source_pixmap| {
      let mut paint = TinyPaint::default();
      paint.shader = TinyPattern::new(
        source_pixmap,
        TinySpreadMode::Pad,
        to_tiny_filter_quality(algorithm),
        1.0,
        source_to_canvas.into(),
      );
      paint.blend_mode = blend_mode;
      paint.anti_alias = true;
      pixmap.fill_path(
        &path,
        &paint,
        TinyFillRule::Winding,
        transform.into(),
        combined_mask,
      );
      true
    })
    .unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn overlay_image<'a, I: Into<PaintSource<'a>>>(
  pixmap: &mut PixmapMut<'_>,
  image: I,
  border: BorderProperties,
  transform: Affine,
  algorithm: ImageScalingAlgorithm,
  mode: BlendMode,
  combined_mask: Option<&TinyMask>,
  buffer_pool: &mut BufferPool,
) {
  let image = image.into();
  let width = image.width();
  let height = image.height();
  let size = Size { width, height };

  if let PaintSource::ColorTile(color) = image {
    if try_fill_color_with_tiny_skia(pixmap, color, size, border, transform, mode, combined_mask) {
      return;
    }
  }

  if border.is_zero()
    && try_draw_image_with_tiny_skia(
      pixmap,
      image,
      transform,
      algorithm,
      mode,
      combined_mask,
      buffer_pool,
    )
  {
    return;
  }

  if !border.is_zero()
    && image.supports_rounded_fill_fast_path()
    && try_fill_image_path_with_tiny_skia(
      pixmap,
      image,
      size,
      border,
      transform,
      transform,
      algorithm,
      mode,
      combined_mask,
      buffer_pool,
    )
  {
    return;
  }

  if transform.only_translation() && border.is_zero() {
    let translation = transform.decompose_translation();
    overlay_area(pixmap, translation, size, mode, combined_mask, |x, y| {
      premultiplied_to_rgba(image.get_pixel(x, y))
    });
    return;
  }

  let mut paths = Vec::new();
  border.append_mask_commands(&mut paths, size.map(|v| v as f32), Point::ZERO);

  let (mask, placement) = render_mask(&paths, Some(transform), None, buffer_pool);
  let inverse = transform.invert();
  let is_identity = transform.is_identity() && placement.left >= 0 && placement.top >= 0;

  if is_identity {
    overlay_area(
      pixmap,
      Point {
        x: placement.left as f32,
        y: placement.top as f32,
      },
      Size {
        width: placement.width,
        height: placement.height,
      },
      mode,
      combined_mask,
      |x, y| {
        let alpha = mask[mask_index_from_coord(x, y, placement.width)];
        if alpha == 0 {
          return Color::transparent().into();
        }
        let mut pixel = premultiplied_to_rgba(
          image.get_pixel(x + placement.left as u32, y + placement.top as u32),
        );
        apply_mask_alpha_to_pixel(&mut pixel, alpha);
        pixel
      },
    );
  } else if let Some(inverse) = inverse {
    overlay_area(
      pixmap,
      Point {
        x: placement.left as f32,
        y: placement.top as f32,
      },
      Size {
        width: placement.width,
        height: placement.height,
      },
      mode,
      combined_mask,
      |x, y| {
        let alpha = mask[mask_index_from_coord(x, y, placement.width)];
        if alpha == 0 {
          return Color::transparent().into();
        }
        let Some(sampled_pixel) = sample_transformed_pixel(
          image,
          inverse,
          algorithm,
          (x as i32 + placement.left) as f32 + 0.5,
          (y as i32 + placement.top) as f32 + 0.5,
          Point::ZERO,
        ) else {
          return Color::transparent().into();
        };
        let mut pixel = premultiplied_to_rgba(sampled_pixel);
        apply_mask_alpha_to_pixel(&mut pixel, alpha);
        pixel
      },
    );
  }

  buffer_pool.release(mask);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn overlay_sampled_image(
  pixmap: &mut PixmapMut<'_>,
  source: &RgbaImage,
  width: u32,
  height: u32,
  border: BorderProperties,
  transform: Affine,
  logical_to_source: Affine,
  algorithm: ImageScalingAlgorithm,
  mode: BlendMode,
  combined_mask: Option<&TinyMask>,
  buffer_pool: &mut BufferPool,
) {
  let image = PaintSource::from(source);
  if border.is_zero()
    && logical_to_source.is_identity()
    && width == source.width()
    && height == source.height()
    && try_draw_image_with_tiny_skia(
      pixmap,
      image,
      transform,
      algorithm,
      mode,
      combined_mask,
      buffer_pool,
    )
  {
    return;
  }

  let size = Size { width, height };

  if transform.only_translation() && border.is_zero() {
    let translation = transform.decompose_translation();
    overlay_area(pixmap, translation, size, mode, combined_mask, |x, y| {
      sample_transformed_pixel(
        image,
        logical_to_source,
        algorithm,
        x as f32 + 0.5,
        y as f32 + 0.5,
        Point::ZERO,
      )
      .map(premultiplied_to_rgba)
      .unwrap_or_else(|| Color::transparent().into())
    });
    return;
  }

  let mut paths = Vec::new();
  border.append_mask_commands(&mut paths, size.map(|v| v as f32), Point::ZERO);
  let (mask, placement) = render_mask(&paths, Some(transform), None, buffer_pool);

  let inverse = transform.invert();
  let is_identity = transform.is_identity() && placement.left >= 0 && placement.top >= 0;

  if is_identity {
    overlay_area(
      pixmap,
      Point {
        x: placement.left as f32,
        y: placement.top as f32,
      },
      Size {
        width: placement.width,
        height: placement.height,
      },
      mode,
      combined_mask,
      |x, y| {
        let alpha = mask[mask_index_from_coord(x, y, placement.width)];
        if alpha == 0 {
          return Color::transparent().into();
        }
        let Some(sampled_pixel) = sample_transformed_pixel(
          image,
          logical_to_source,
          algorithm,
          (x + placement.left as u32) as f32 + 0.5,
          (y + placement.top as u32) as f32 + 0.5,
          Point::ZERO,
        ) else {
          return Color::transparent().into();
        };
        let mut pixel = premultiplied_to_rgba(sampled_pixel);
        apply_mask_alpha_to_pixel(&mut pixel, alpha);
        pixel
      },
    );
  } else if let Some(inverse) = inverse {
    let combined_inverse = logical_to_source * inverse;
    overlay_area(
      pixmap,
      Point {
        x: placement.left as f32,
        y: placement.top as f32,
      },
      Size {
        width: placement.width,
        height: placement.height,
      },
      mode,
      combined_mask,
      |x, y| {
        let alpha = mask[mask_index_from_coord(x, y, placement.width)];
        if alpha == 0 {
          return Color::transparent().into();
        }
        let Some(sampled_pixel) = sample_transformed_pixel(
          image,
          combined_inverse,
          algorithm,
          (x as i32 + placement.left) as f32 + 0.5,
          (y as i32 + placement.top) as f32 + 0.5,
          Point::ZERO,
        ) else {
          return Color::transparent().into();
        };
        let mut pixel = premultiplied_to_rgba(sampled_pixel);
        apply_mask_alpha_to_pixel(&mut pixel, alpha);
        pixel
      },
    );
  }

  buffer_pool.release(mask);
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub(crate) fn mask_index_from_coord(x: u32, y: u32, width: u32) -> usize {
  (y * width + x) as usize
}

pub(crate) fn overlay_gradient_tile<T>(
  pixmap: &mut PixmapMut<'_>,
  gradient: &T,
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<&TinyMask>,
) where
  T: GradientOverlayTile,
{
  let bottom_width = pixmap.width();
  let bottom_height = pixmap.height();
  let top_size = Size {
    width: gradient.width(),
    height: gradient.height(),
  };

  if mode != BlendMode::Normal || combined_mask.is_some() {
    return overlay_area(pixmap, offset, top_size, mode, combined_mask, |x, y| {
      let color = gradient.sample_pixel(x, y).demultiply();
      Rgba([color.red(), color.green(), color.blue(), color.alpha()])
    });
  }

  let bottom_data: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  overlay_gradient_tile_fast_normal_unconstrained(
    bottom_data,
    bottom_width,
    bottom_height,
    gradient,
    offset,
  );
}

#[cfg(test)]
mod tests {
  use image::RgbaImage;
  use tiny_skia::PixmapMut;

  use crate::{
    GlobalContext,
    layout::{
      Viewport,
      style::{
        Angle, BlendMode, Color, ColorInterpolationMethod, ConicGradient, ConicGradientTile,
        FromCss, GradientStop, Length, LinearGradient, LinearGradientTile, ObjectPosition,
        RadialGradient, RadialGradientTile, StopPosition,
      },
    },
    rendering::{RenderContext, blend_pixel},
  };

  use super::*;

  fn with_pixmap(image: &mut RgbaImage, f: impl FnOnce(&mut PixmapMut<'_>)) {
    let width = image.width();
    let height = image.height();
    let Some(mut pixmap) = PixmapMut::from_bytes(image.as_mut(), width, height) else {
      return;
    };
    f(&mut pixmap);
  }

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

    with_pixmap(&mut fast, |pixmap| {
      overlay_area(pixmap, offset, top_size, BlendMode::Normal, None, |x, y| {
        let alpha = ((x + y * 2) * 40).min(255) as u8;
        Rgba([200, 80, 30, alpha])
      });
    });

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
    let render_context = RenderContext::new_test(&global_context, Viewport::new((32, 16)));
    let tile = LinearGradientTile::new(&gradient, 32, 16, &render_context);

    let mut fast = RgbaImage::from_pixel(40, 24, Rgba([0, 0, 0, 0]));
    let mut reference = fast.clone();
    let offset = Point { x: 3.0, y: 4.0 };

    with_pixmap(&mut fast, |pixmap| {
      overlay_gradient_tile(pixmap, &tile, offset, BlendMode::Normal, None);
    });

    let top_size = Size {
      width: tile.width,
      height: tile.height,
    };
    overlay_area_reference(&mut reference, offset, top_size, |x, y| {
      let color = tile.sample_pixel(x, y).demultiply();
      Rgba([color.red(), color.green(), color.blue(), color.alpha()])
    });

    assert_eq!(fast.as_raw(), reference.as_raw());
  }

  #[test]
  fn test_overlay_radial_gradient_matches_reference() {
    let Ok(gradient) = RadialGradient::from_str("radial-gradient(circle, red, blue)") else {
      unreachable!()
    };
    let global_context = GlobalContext::default();
    let render_context = RenderContext::new_test(&global_context, Viewport::new((32, 24)));
    let tile = RadialGradientTile::new(&gradient, 32, 24, &render_context);

    let mut fast = RgbaImage::from_pixel(40, 30, Rgba([0, 0, 0, 0]));
    let mut reference = fast.clone();
    let offset = Point { x: 4.0, y: 3.0 };

    with_pixmap(&mut fast, |pixmap| {
      overlay_gradient_tile(pixmap, &tile, offset, BlendMode::Normal, None);
    });

    let top_size = Size {
      width: tile.width,
      height: tile.height,
    };
    overlay_area_reference(&mut reference, offset, top_size, |x, y| {
      let color = tile.sample_pixel(x, y).demultiply();
      Rgba([color.red(), color.green(), color.blue(), color.alpha()])
    });

    assert_eq!(fast.as_raw(), reference.as_raw());
  }

  #[test]
  fn test_overlay_conic_gradient_matches_reference() {
    let Ok(gradient) = ConicGradient::from_str("conic-gradient(red, blue)") else {
      unreachable!()
    };

    let global_context = GlobalContext::default();
    let render_context = RenderContext::new_test(&global_context, Viewport::new((32, 24)));
    let tile = ConicGradientTile::new(&gradient, 32, 24, &render_context);

    let mut fast = RgbaImage::from_pixel(40, 30, Rgba([0, 0, 0, 0]));
    let mut reference = fast.clone();
    let offset = Point { x: 4.0, y: 3.0 };

    with_pixmap(&mut fast, |pixmap| {
      overlay_gradient_tile(pixmap, &tile, offset, BlendMode::Normal, None);
    });

    let top_size = Size {
      width: tile.width,
      height: tile.height,
    };
    overlay_area_reference(&mut reference, offset, top_size, |x, y| {
      let color = tile.sample_pixel(x, y).demultiply();
      Rgba([color.red(), color.green(), color.blue(), color.alpha()])
    });

    assert_eq!(fast.as_raw(), reference.as_raw());
  }

  #[test]
  fn test_overlay_linear_gradient_clustered_stops_matches_reference() {
    let Ok(gradient) =
      LinearGradient::from_str("linear-gradient(to right, red 0px, lime 0.5px, blue 32px)")
    else {
      unreachable!()
    };
    let global_context = GlobalContext::default();
    let render_context = RenderContext::new_test(&global_context, Viewport::new((32, 16)));
    let tile = LinearGradientTile::new(&gradient, 32, 16, &render_context);

    let mut fast = RgbaImage::from_pixel(40, 24, Rgba([0, 0, 0, 0]));
    let mut reference = fast.clone();
    let offset = Point { x: 3.0, y: 4.0 };

    with_pixmap(&mut fast, |pixmap| {
      overlay_gradient_tile(pixmap, &tile, offset, BlendMode::Normal, None);
    });

    let top_size = Size {
      width: tile.width,
      height: tile.height,
    };
    overlay_area_reference(&mut reference, offset, top_size, |x, y| {
      let color = tile.sample_pixel(x, y).demultiply();
      Rgba([color.red(), color.green(), color.blue(), color.alpha()])
    });

    assert_eq!(fast.as_raw(), reference.as_raw());
  }

  #[test]
  fn test_subcanvas_overlay_sampled_image_matches_direct_render() {
    let source = RgbaImage::from_fn(2, 1, |x, _| {
      if x == 0 {
        Rgba([255, 0, 0, 255])
      } else {
        Rgba([0, 0, 255, 255])
      }
    });

    let mut direct = Canvas::new(Size {
      width: 8,
      height: 6,
    });
    direct.overlay_sampled_image(
      &source,
      4,
      2,
      BorderProperties::default(),
      Affine::translation(2.0, 2.0),
      Affine::scale(0.5, 0.5),
      ImageScalingAlgorithm::Pixelated,
      BlendMode::Normal,
    );

    let mut isolated = Canvas::new(Size {
      width: 8,
      height: 6,
    });
    let subcanvas = isolated
      .begin_subcanvas(Placement {
        left: 2,
        top: 2,
        width: 4,
        height: 2,
      })
      .unwrap_or_else(|_| unreachable!());
    isolated.overlay_sampled_image(
      &source,
      4,
      2,
      BorderProperties::default(),
      Affine::translation(2.0, 2.0),
      Affine::scale(0.5, 0.5),
      ImageScalingAlgorithm::Pixelated,
      BlendMode::Normal,
    );
    isolated.composite_subcanvas(subcanvas, BlendMode::Normal);

    assert_eq!(direct.into_inner().as_raw(), isolated.into_inner().as_raw());
  }

  #[test]
  fn test_overlay_conic_gradient_hard_stops_matches_reference() {
    let gradient = ConicGradient {
      repeating: false,
      from_angle: Angle::zero(),
      center: ObjectPosition::default(),
      interpolation: ColorInterpolationMethod::default(),
      stops: [
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(25.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(25.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(100.0))),
        },
      ]
      .into(),
    };

    let global_context = GlobalContext::default();
    let render_context = RenderContext::new_test(&global_context, Viewport::new((48, 48)));
    let tile = ConicGradientTile::new(&gradient, 48, 48, &render_context);

    let mut fast = RgbaImage::from_pixel(56, 56, Rgba([0, 0, 0, 0]));
    let mut reference = fast.clone();
    let offset = Point { x: 4.0, y: 4.0 };

    with_pixmap(&mut fast, |pixmap| {
      overlay_gradient_tile(pixmap, &tile, offset, BlendMode::Normal, None);
    });

    let top_size = Size {
      width: tile.width,
      height: tile.height,
    };
    overlay_area_reference(&mut reference, offset, top_size, |x, y| {
      let color = tile.sample_pixel(x, y).demultiply();
      Rgba([color.red(), color.green(), color.blue(), color.alpha()])
    });

    assert_eq!(fast.as_raw(), reference.as_raw());
  }

  #[test]
  fn test_overlay_radial_gradient_clustered_stops_matches_reference() {
    let Ok(gradient) =
      RadialGradient::from_str("radial-gradient(circle, red 0%, lime 1%, blue 100%)")
    else {
      unreachable!()
    };
    let global_context = GlobalContext::default();
    let render_context = RenderContext::new_test(&global_context, Viewport::new((32, 24)));
    let tile = RadialGradientTile::new(&gradient, 32, 24, &render_context);

    let mut fast = RgbaImage::from_pixel(40, 30, Rgba([0, 0, 0, 0]));
    let mut reference = fast.clone();
    let offset = Point { x: 4.0, y: 3.0 };

    with_pixmap(&mut fast, |pixmap| {
      overlay_gradient_tile(pixmap, &tile, offset, BlendMode::Normal, None);
    });

    let top_size = Size {
      width: tile.width,
      height: tile.height,
    };
    overlay_area_reference(&mut reference, offset, top_size, |x, y| {
      let color = tile.sample_pixel(x, y).demultiply();
      Rgba([color.red(), color.green(), color.blue(), color.alpha()])
    });

    assert_eq!(fast.as_raw(), reference.as_raw());
  }
}
