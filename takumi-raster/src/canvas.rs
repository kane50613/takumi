//! Canvas operations and image blending for the takumi rendering system.
//!
//! This module provides performance-optimized canvas operations including
//! fast image blending and pixel manipulation operations.

mod buffer_pool;
mod composite;
mod mask;
mod paint_source;

use std::{mem::replace, sync::Arc};

use image::{
  ImageError, Rgba, RgbaImage,
  error::{ParameterError, ParameterErrorKind},
};
use taffy::{Point, Size};
use tiny_skia::{
  FillRule as TinyFillRule, FilterQuality as TinyFilterQuality, IntSize, Mask as TinyMask,
  Paint as TinyPaint, Path as TinyPath, Pattern as TinyPattern, Pixmap, PixmapMut, PixmapPaint,
  PixmapRef, SpreadMode as TinySpreadMode, Transform as TinyTransform,
};

use self::{
  composite::sampling_footprint,
  paint_source::{MaskCompositeColor, sample_paint_source},
};
use crate::{
  BackgroundTile, BorderProperties, ColorTile, Placement, Result,
  blend::*,
  build_path,
  error::Error,
  layout::style::{
    Affine, BlendMode, GradientOverlayTile, ImageScalingAlgorithm, LinearGradientFastPathKind,
    LinearGradientTile, RadialGradientTile, overlay_gradient_tile_fast_normal_unconstrained,
  },
  stacking_context::blend_pixmap_software,
};

pub(crate) use buffer_pool::BufferPool;
pub(crate) use mask::{
  CanvasViewport, NodeMaskAction, attenuate_alpha_by_mask, intersect_alpha_masks,
  prepare_node_mask, render_mask,
};
pub(crate) use paint_source::{PaintSource, SamplingFootprint, interpolate_with_footprint};

#[derive(Clone, Copy)]
pub(crate) struct SamplingOptions {
  pub logical_to_source: Affine,
  pub algorithm: ImageScalingAlgorithm,
}

#[derive(Clone, Copy)]
pub(crate) struct MaskSamplingOptions {
  pub canvas_to_source: Affine,
  pub sample_bias: Point<f32>,
  pub algorithm: ImageScalingAlgorithm,
}

#[derive(Clone, Copy)]
pub(crate) struct OverlayOptions<'a> {
  pub border: BorderProperties,
  pub transform: Affine,
  pub algorithm: ImageScalingAlgorithm,
  pub mode: BlendMode,
  pub combined_mask: Option<MaskView<'a>>,
}

#[derive(Clone, Copy)]
pub(crate) struct MaskSourceToPixmapOptions<'a> {
  pub placement: Placement,
  pub sampling: MaskSamplingOptions,
  pub mode: BlendMode,
  pub combined_mask: Option<MaskView<'a>>,
}

#[derive(Clone, Copy)]
struct ImagePathFillOptions<'a> {
  size: Size<u32>,
  border: BorderProperties,
  transform: Affine,
  source_to_canvas: Affine,
  algorithm: ImageScalingAlgorithm,
  mode: BlendMode,
  combined_mask: Option<MaskView<'a>>,
}

#[derive(Clone, Copy)]
struct FillColorOptions<'a> {
  color: &'a ColorTile,
  size: Size<u32>,
  border: BorderProperties,
  transform: Affine,
  mode: BlendMode,
  combined_mask: Option<MaskView<'a>>,
}

#[derive(Clone)]
struct MaskStackEntry {
  mask: Arc<TinyMask>,
  origin: Point<u32>,
}

#[derive(Clone, Copy)]
pub(crate) struct MaskView<'a> {
  mask: &'a TinyMask,
  origin: Point<u32>,
  canvas_origin: Point<u32>,
}

impl<'a> MaskView<'a> {
  #[inline]
  fn alpha_at(self, x: u32, y: u32) -> u8 {
    let local_x = x as i32 + self.canvas_origin.x as i32 - self.origin.x as i32;
    let local_y = y as i32 + self.canvas_origin.y as i32 - self.origin.y as i32;
    if local_x < 0
      || local_y < 0
      || local_x >= self.mask.width() as i32
      || local_y >= self.mask.height() as i32
    {
      return 0;
    }

    self.mask.data()[mask_index_from_coord(local_x as u32, local_y as u32, self.mask.width())]
  }

  #[inline]
  pub(crate) fn row(&self, canvas_y: i32, canvas_x_start: i32) -> MaskRow<'a> {
    let local_y = canvas_y + self.canvas_origin.y as i32 - self.origin.y as i32;
    let mask_width = self.mask.width() as i32;
    let mask_height = self.mask.height() as i32;
    if local_y < 0 || local_y >= mask_height {
      return MaskRow::EMPTY;
    }
    let local_x_start = canvas_x_start + self.canvas_origin.x as i32 - self.origin.x as i32;
    let row_offset = local_y as usize * self.mask.width() as usize;
    MaskRow {
      data: self.mask.data(),
      row_offset,
      local_x_start,
      mask_width,
    }
  }
}

#[derive(Clone, Copy)]
pub(crate) struct MaskRow<'a> {
  data: &'a [u8],
  row_offset: usize,
  local_x_start: i32,
  mask_width: i32,
}

impl<'a> MaskRow<'a> {
  const EMPTY: Self = Self {
    data: &[],
    row_offset: 0,
    local_x_start: 0,
    mask_width: 0,
  };

  #[inline]
  pub(crate) fn alpha_at_offset(&self, offset: usize) -> u8 {
    let local_x = self.local_x_start + offset as i32;
    if local_x < 0 || local_x >= self.mask_width {
      return 0;
    }
    self.data[self.row_offset + local_x as usize]
  }
}

/// A canvas that can be used to draw images onto.
pub(crate) struct Canvas {
  image: Pixmap,
  origin: Point<u32>,
  offscreen_pool: Vec<Pixmap>,
  constraint_mask_stack: Vec<Option<MaskStackEntry>>,
  pub(crate) buffer_pool: BufferPool,
}

pub(crate) struct CanvasSubcanvas {
  image: Pixmap,
  origin: Option<Point<u32>>,
  constraint_mask_stack: Option<Vec<Option<MaskStackEntry>>>,
  offset: Point<i32>,
}

impl Canvas {
  /// Creates a new canvas handle from a draw command sender.
  pub(crate) fn new(size: Size<u32>) -> Self {
    let Some(image) = Pixmap::new(size.width, size.height) else {
      return Self::new(Size {
        width: 1,
        height: 1,
      });
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
      Error::ImageError(
        ImageError::Parameter(ParameterError::from_kind(
          ParameterErrorKind::DimensionMismatch,
        ))
        .to_string(),
      )
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
    let constraint_mask_stack = self.constraint_mask_stack.clone();

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

  pub(crate) fn composite_subcanvas(
    &mut self,
    subcanvas: CanvasSubcanvas,
    mode: BlendMode,
    opacity: f32,
  ) {
    let CanvasSubcanvas {
      image,
      origin,
      constraint_mask_stack,
      offset,
    } = subcanvas;

    if opacity <= 0.0 {
      self.recycle_offscreen_image(image);
      self.restore_subcanvas_state(origin, constraint_mask_stack);
      return;
    }

    let isolated_image = replace(&mut self.image, image);
    self.restore_subcanvas_state(origin, constraint_mask_stack);

    if let Some(blend_mode) = to_tiny_blend_mode(mode) {
      let paint = PixmapPaint {
        opacity,
        blend_mode,
        quality: TinyFilterQuality::Nearest,
      };
      self.image.draw_pixmap(
        offset.x,
        offset.y,
        isolated_image.as_ref(),
        &paint,
        TinyTransform::identity(),
        None,
      );
    } else {
      blend_pixmap_software(&mut self.image, &isolated_image, mode, offset, opacity);
    }

    self.recycle_offscreen_image(isolated_image);
  }

  pub(crate) fn has_no_constraint_mask(&self) -> bool {
    self.constraint_mask_stack.iter().all(Option::is_none)
  }

  pub(crate) fn push_mask(&mut self, mask: TinyMask) {
    self.constraint_mask_stack.push(
      self
        .build_constraint_mask(&mask)
        .map(|mask| MaskStackEntry {
          mask: Arc::new(mask),
          origin: self.origin,
        }),
    );
  }

  pub(crate) fn pop_mask(&mut self) {
    self.constraint_mask_stack.pop();
  }

  pub(crate) fn into_inner(self) -> Result<RgbaImage> {
    RgbaImage::from_raw(
      self.image.width(),
      self.image.height(),
      self.image.take_demultiplied(),
    )
    .ok_or_else(|| {
      Error::ImageError(
        ImageError::Parameter(ParameterError::from_kind(
          ParameterErrorKind::DimensionMismatch,
        ))
        .to_string(),
      )
    })
  }

  pub(crate) fn recycle_offscreen_image(&mut self, image: Pixmap) {
    const MAX_OFFSCREEN_POOL: usize = 8;
    if self.offscreen_pool.len() >= MAX_OFFSCREEN_POOL {
      return;
    }
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
  pub(crate) fn composite_mask_source(
    &mut self,
    mask: &[u8],
    placement: Placement,
    source: PaintSource<'_>,
    sampling: MaskSamplingOptions,
    mode: BlendMode,
  ) {
    let placement = self.localize_placement(placement);
    let sampling = self.localize_mask_sampling(sampling);
    self.with_overlay_state(|pixmap, combined_mask, _| {
      composite::source(
        pixmap,
        mask,
        source,
        composite::Options {
          placement,
          sampling,
          color_mode: MaskCompositeColor::SourceOnly,
          mode,
          combined_mask,
        },
      );
    });
  }
  pub(crate) fn composite_mask_source_over_color<C: Into<Rgba<u8>>>(
    &mut self,
    mask: &[u8],
    placement: Placement,
    source: PaintSource<'_>,
    color: C,
    sampling: MaskSamplingOptions,
    mode: BlendMode,
  ) {
    let placement = self.localize_placement(placement);
    let sampling = self.localize_mask_sampling(sampling);
    self.with_overlay_state(|pixmap, combined_mask, _| {
      composite::source(
        pixmap,
        mask,
        source,
        composite::Options {
          placement,
          sampling,
          color_mode: MaskCompositeColor::SourceOverColor(premultiply_rgba(color.into())),
          mode,
          combined_mask,
        },
      );
    });
  }
  pub(crate) fn composite_mask_color_over_source<C: Into<Rgba<u8>>>(
    &mut self,
    mask: &[u8],
    placement: Placement,
    source: PaintSource<'_>,
    color: C,
    sampling: MaskSamplingOptions,
    mode: BlendMode,
  ) {
    let placement = self.localize_placement(placement);
    let sampling = self.localize_mask_sampling(sampling);
    self.with_overlay_state(|pixmap, combined_mask, _| {
      composite::source(
        pixmap,
        mask,
        source,
        composite::Options {
          placement,
          sampling,
          color_mode: MaskCompositeColor::ColorOverSource(premultiply_rgba(color.into())),
          mode,
          combined_mask,
        },
      );
    });
  }
  pub(crate) fn overlay_sampled_pixmap(
    &mut self,
    source: PixmapRef<'_>,
    size: Size<u32>,
    border: BorderProperties,
    transform: Affine,
    sampling: SamplingOptions,
    mode: BlendMode,
  ) {
    let transform = self.localize_transform(transform);
    self.with_overlay_state(|pixmap, combined_mask, buffer_pool| {
      overlay_sampled_paint_source(
        pixmap,
        PaintSource::from(source),
        size,
        OverlayOptions {
          border,
          transform,
          algorithm: sampling.algorithm,
          mode,
          combined_mask,
        },
        sampling,
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
        OverlayOptions {
          border,
          transform,
          algorithm,
          mode,
          combined_mask,
        },
        buffer_pool,
      );
    });
  }

  pub(crate) fn overlay_background_tile_direct(
    &mut self,
    tile: &BackgroundTile,
    translation: Point<f32>,
    mode: BlendMode,
  ) -> bool {
    let localized_translation = Point {
      x: translation.x - self.origin.x as f32,
      y: translation.y - self.origin.y as f32,
    };

    match tile {
      BackgroundTile::Linear(gradient) => {
        self.with_overlay_state(|pixmap, combined_mask, _| {
          overlay_linear_gradient_tile(
            pixmap,
            gradient,
            localized_translation,
            mode,
            combined_mask,
          );
        });
        true
      }
      BackgroundTile::Radial(gradient) => {
        self.with_overlay_state(|pixmap, combined_mask, _| {
          overlay_radial_gradient_tile(
            pixmap,
            gradient,
            localized_translation,
            mode,
            combined_mask,
          );
        });
        true
      }
      BackgroundTile::Conic(gradient) => {
        self.with_overlay_state(|pixmap, combined_mask, _| {
          overlay_gradient_tile(pixmap, gradient, localized_translation, mode, combined_mask);
        });
        true
      }
      _ => false,
    }
  }

  fn with_overlay_state<R>(
    &mut self,
    f: impl FnOnce(&mut PixmapMut<'_>, Option<MaskView<'_>>, &mut BufferPool) -> R,
  ) -> R {
    let combined_mask = self.constraint_mask_stack.last().and_then(Option::as_ref);
    let combined_mask = combined_mask.map(|entry| MaskView {
      mask: entry.mask.as_ref(),
      origin: entry.origin,
      canvas_origin: self.origin,
    });
    let mut pixmap = self.image.as_mut();
    f(&mut pixmap, combined_mask, &mut self.buffer_pool)
  }

  fn build_constraint_mask(&self, mask: &TinyMask) -> Option<TinyMask> {
    let mut combined = TinyMask::new(mask.width(), mask.height())?;
    let Some(previous) = self.constraint_mask_stack.last().and_then(Option::as_ref) else {
      combined.data_mut().copy_from_slice(mask.data());
      return Some(combined);
    };

    let previous = MaskView {
      mask: previous.mask.as_ref(),
      origin: previous.origin,
      canvas_origin: self.origin,
    };
    let mask_data = mask.data();
    let mask_width = mask.width();
    let combined_data = combined.data_mut();
    for y in 0..mask.height() {
      let row = previous.row(y as i32, 0);
      let row_start = y as usize * mask_width as usize;
      let dst = &mut combined_data[row_start..row_start + mask_width as usize];
      let new_row = &mask_data[row_start..row_start + mask_width as usize];
      for (x, (out, &right)) in dst.iter_mut().zip(new_row).enumerate() {
        if right == 0 {
          continue;
        }
        let left = row.alpha_at_offset(x);
        if left == 0 {
          continue;
        }
        *out = if left == u8::MAX {
          right
        } else if right == u8::MAX {
          left
        } else {
          ((left as u16 * right as u16 + 128) >> 8) as u8
        };
      }
    }
    Some(combined)
  }

  fn localize_transform(&self, transform: Affine) -> Affine {
    Affine::translation(-(self.origin.x as f32), -(self.origin.y as f32)) * transform
  }

  fn localize_mask_sampling(&self, sampling: MaskSamplingOptions) -> MaskSamplingOptions {
    MaskSamplingOptions {
      canvas_to_source: sampling.canvas_to_source
        * Affine::translation(self.origin.x as f32, self.origin.y as f32),
      ..sampling
    }
  }

  fn localize_placement(&self, placement: Placement) -> Placement {
    Placement {
      left: placement.left - self.origin.x as i32,
      top: placement.top - self.origin.y as i32,
      ..placement
    }
  }

  fn restore_subcanvas_state(
    &mut self,
    origin: Option<Point<u32>>,
    constraint_mask_stack: Option<Vec<Option<MaskStackEntry>>>,
  ) {
    if let Some(origin) = origin {
      self.origin = origin;
    }
    if let Some(constraint_mask_stack) = constraint_mask_stack {
      self.constraint_mask_stack = constraint_mask_stack;
    }
  }
}

fn materialize_mask(
  mask: MaskView<'_>,
  size: Size<u32>,
  buffer_pool: &mut BufferPool,
) -> Option<TinyMask> {
  let mut cropped = TinyMask::from_vec(
    buffer_pool.acquire((size.width as usize) * (size.height as usize)),
    IntSize::from_wh(size.width, size.height)?,
  )?;

  let offset = Point {
    x: mask.canvas_origin.x as i32 - mask.origin.x as i32,
    y: mask.canvas_origin.y as i32 - mask.origin.y as i32,
  };
  let src_width = mask.mask.width() as i32;
  let src_height = mask.mask.height() as i32;
  let start_x = offset.x.max(0);
  let start_y = offset.y.max(0);
  let end_x = (offset.x + size.width as i32).min(src_width);
  let end_y = (offset.y + size.height as i32).min(src_height);
  if start_x >= end_x || start_y >= end_y {
    return Some(cropped);
  }

  let src = mask.mask.data();
  let dst = cropped.data_mut();
  if start_x == 0
    && start_y == 0
    && end_x == src_width
    && end_y == src_height
    && src_width as u32 == size.width
    && src_height as u32 == size.height
  {
    dst.copy_from_slice(src);
    return Some(cropped);
  }

  let dst_width = size.width as usize;
  let src_width = src_width as usize;
  let copy_width = (end_x - start_x) as usize;
  let dst_x_start = (start_x - offset.x) as usize;
  for src_y in start_y..end_y {
    let dst_y = (src_y - offset.y) as usize;
    let src_row = src_y as usize * src_width + start_x as usize;
    let dst_row = dst_y * dst_width + dst_x_start;
    dst[dst_row..dst_row + copy_width].copy_from_slice(&src[src_row..src_row + copy_width]);
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

#[inline(always)]
fn apply_combined_mask(
  src: [u8; 4],
  combined_mask: Option<MaskView<'_>>,
  dest_x: u32,
  dest_y: u32,
) -> Option<[u8; 4]> {
  let Some(mask) = combined_mask else {
    return Some(src);
  };
  let alpha = mask.alpha_at(dest_x, dest_y);
  if alpha == 0 {
    return None;
  }
  let src = scale_premultiplied_pixel(src, alpha);
  if src[3] == 0 {
    return None;
  }
  Some(src)
}

fn blit_sampled_paint_source_translation(
  pixmap: &mut PixmapMut<'_>,
  source: PaintSource<'_>,
  size: Size<u32>,
  offset: Point<f32>,
  sampling: SamplingOptions,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) {
  if sampling.logical_to_source.is_identity()
    && size.width == source.width()
    && size.height == source.height()
  {
    blit_paint_source_translation(pixmap, source, offset, mode, combined_mask);
    return;
  }

  let canvas_width = pixmap.width();
  let canvas_height = pixmap.height();
  let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds_for_canvas(canvas_width, canvas_height, offset, size.width, size.height)
  else {
    return;
  };

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let footprint = sampling_footprint(sampling.logical_to_source);
  for dest_y in dest_y_min..dest_y_max {
    let src_y = (dest_y - offset_y) as f32;
    let mut sample_point = sampling.logical_to_source.transform_point(Point {
      x: (dest_x_min - offset_x) as f32 + 0.5,
      y: src_y + 0.5,
    });
    for dest_x in dest_x_min..dest_x_max {
      let src = sample_paint_source(
        source,
        sampling.algorithm,
        sample_point.x,
        sample_point.y,
        footprint,
      )
      .unwrap_or([0, 0, 0, 0]);
      sample_point.x += sampling.logical_to_source.a;
      sample_point.y += sampling.logical_to_source.b;
      if src[3] == 0 {
        continue;
      }

      let dest_x = dest_x as u32;
      let dest_y = dest_y as u32;
      let Some(src) = apply_combined_mask(src, combined_mask, dest_x, dest_y) else {
        continue;
      };

      let index = (dest_y * canvas_width + dest_x) as usize;
      blend_premultiplied_pixel(&mut pixels[index], src, mode);
    }
  }
}

fn blit_paint_source_translation(
  pixmap: &mut PixmapMut<'_>,
  source: PaintSource<'_>,
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) {
  if let Some(color) = source.premultiplied_constant() {
    blit_solid_translation(
      pixmap,
      source.width(),
      source.height(),
      color,
      offset,
      mode,
      combined_mask,
    );
    return;
  }

  let canvas_width = pixmap.width();
  let canvas_height = pixmap.height();
  let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds_for_canvas(
      canvas_width,
      canvas_height,
      offset,
      source.width(),
      source.height(),
    )
  else {
    return;
  };

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  match source {
    PaintSource::Pixmap(source) => {
      let source_pixels = source.pixels();
      let source_width = source.width();
      if mode == BlendMode::Normal && combined_mask.is_none() {
        let copy_width = (dest_x_max - dest_x_min) as usize;
        let src_x_start = (dest_x_min - offset_x) as usize;
        for dest_y in dest_y_min..dest_y_max {
          let src_y = (dest_y - offset_y) as usize;
          let src_start = src_y * source_width as usize + src_x_start;
          let src_end = src_start + copy_width;
          let dst_start = (dest_y as u32 * canvas_width + dest_x_min as u32) as usize;
          let dst_end = dst_start + copy_width;
          let dst = bytemuck::cast_slice_mut(&mut pixels[dst_start..dst_end]);
          composite_premultiplied_over_span(dst, &source_pixels[src_start..src_end]);
        }
        return;
      }

      for dest_y in dest_y_min..dest_y_max {
        let src_y = (dest_y - offset_y) as u32;
        let dst_row = dest_y as usize * canvas_width as usize;
        let src_row = src_y as usize * source_width as usize;
        for dest_x in dest_x_min..dest_x_max {
          let src_x = (dest_x - offset_x) as u32;
          let src = premultiplied_from_pixel(source_pixels[src_row + src_x as usize]);
          if src[3] == 0 {
            continue;
          }

          let dest_x = dest_x as u32;
          let Some(src) = apply_combined_mask(src, combined_mask, dest_x, dest_y as u32) else {
            continue;
          };

          blend_premultiplied_pixel(&mut pixels[dst_row + dest_x as usize], src, mode);
        }
      }
    }
    _ => {
      for dest_y in dest_y_min..dest_y_max {
        let src_y = (dest_y - offset_y) as f32;
        let dst_row = dest_y as usize * canvas_width as usize;
        for dest_x in dest_x_min..dest_x_max {
          let src_x = (dest_x - offset_x) as f32;
          let src = sample_paint_source(
            source,
            ImageScalingAlgorithm::Pixelated,
            src_x,
            src_y,
            SamplingFootprint::PIXEL,
          )
          .unwrap_or([0; 4]);
          if src[3] == 0 {
            continue;
          }

          let dest_x = dest_x as u32;
          let Some(src) = apply_combined_mask(src, combined_mask, dest_x, dest_y as u32) else {
            continue;
          };

          blend_premultiplied_pixel(&mut pixels[dst_row + dest_x as usize], src, mode);
        }
      }
    }
  }
}

fn blit_solid_translation(
  pixmap: &mut PixmapMut<'_>,
  source_width: u32,
  source_height: u32,
  color: [u8; 4],
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) {
  if color[3] == 0 {
    return;
  }

  let canvas_width = pixmap.width();
  let canvas_height = pixmap.height();
  let Some((_offset_x, _offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds_for_canvas(
      canvas_width,
      canvas_height,
      offset,
      source_width,
      source_height,
    )
  else {
    return;
  };

  let data: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());

  if mode == BlendMode::Normal && combined_mask.is_none() {
    let row_stride = canvas_width as usize * 4;
    let x_byte_start = dest_x_min as usize * 4;
    let x_byte_end = dest_x_max as usize * 4;
    for dest_y in dest_y_min..dest_y_max {
      let row_start = dest_y as usize * row_stride;
      let row = &mut data[row_start + x_byte_start..row_start + x_byte_end];
      if color[3] == u8::MAX {
        fill_repeated_premultiplied_pixel(row, color);
      } else {
        blend_repeated_premultiplied_pixel(
          row,
          tiny_skia::PremultipliedColorU8::from_rgba(color[0], color[1], color[2], color[3])
            .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT),
        );
      }
    }
    return;
  }

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(data);
  for dest_y in dest_y_min..dest_y_max {
    let dst_row = dest_y as usize * canvas_width as usize;
    for dest_x in dest_x_min..dest_x_max {
      let dest_x = dest_x as u32;
      let Some(src) = apply_combined_mask(color, combined_mask, dest_x, dest_y as u32) else {
        continue;
      };

      blend_premultiplied_pixel(&mut pixels[dst_row + dest_x as usize], src, mode);
    }
  }
}

pub(crate) fn composite_mask_source_to_pixmap(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  source: PaintSource<'_>,
  options: MaskSourceToPixmapOptions<'_>,
) {
  composite::source(
    pixmap,
    mask,
    source,
    composite::Options {
      placement: options.placement,
      sampling: options.sampling,
      color_mode: MaskCompositeColor::SourceOnly,
      mode: options.mode,
      combined_mask: options.combined_mask,
    },
  );
}

pub(crate) fn draw_mask(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  placement: Placement,
  color: Rgba<u8>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) {
  if mask.is_empty() {
    return;
  }

  assert_eq!(
    mask.len(),
    placement.width as usize * placement.height as usize,
  );

  composite::constant(
    pixmap,
    mask,
    placement,
    premultiply_rgba(color),
    mode,
    combined_mask,
  );
}

fn try_draw_image_with_tiny_skia(
  pixmap: &mut PixmapMut<'_>,
  image: PaintSource<'_>,
  transform: Affine,
  algorithm: ImageScalingAlgorithm,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
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
  let materialized_mask = combined_mask.and_then(|mask| {
    materialize_mask(
      mask,
      Size {
        width: pixmap.width(),
        height: pixmap.height(),
      },
      buffer_pool,
    )
  });
  let combined_mask = materialized_mask.as_ref();

  image
    .with_pixmap_ref(buffer_pool, |source_pixmap| {
      pixmap.draw_pixmap(0, 0, source_pixmap, &paint, transform.into(), combined_mask);
      true
    })
    .unwrap_or(false)
}
fn try_fill_color_with_tiny_skia(
  pixmap: &mut PixmapMut<'_>,
  options: FillColorOptions<'_>,
  buffer_pool: &mut BufferPool,
) -> bool {
  let Some(blend_mode) = to_tiny_blend_mode(options.mode) else {
    return false;
  };
  let Some(path) = build_border_path(options.border, options.size) else {
    return false;
  };

  let mut paint = TinyPaint::default();
  let [red, green, blue, alpha] = options.color.color().0;
  paint.set_color_rgba8(red, green, blue, alpha);
  paint.blend_mode = blend_mode;
  paint.anti_alias = true;
  let materialized_mask = options
    .combined_mask
    .and_then(|mask| materialize_mask(mask, options.size, buffer_pool));
  let combined_mask = materialized_mask.as_ref();
  pixmap.fill_path(
    &path,
    &paint,
    TinyFillRule::Winding,
    options.transform.into(),
    combined_mask,
  );
  true
}
fn try_fill_image_path_with_tiny_skia(
  pixmap: &mut PixmapMut<'_>,
  image: PaintSource<'_>,
  options: ImagePathFillOptions<'_>,
  buffer_pool: &mut BufferPool,
) -> bool {
  let Some(blend_mode) = to_tiny_blend_mode(options.mode) else {
    return false;
  };
  let Some(path) = build_border_path(options.border, options.size) else {
    return false;
  };
  let materialized_mask = options
    .combined_mask
    .and_then(|mask| materialize_mask(mask, options.size, buffer_pool));
  let combined_mask = materialized_mask.as_ref();

  image
    .with_pixmap_ref(buffer_pool, |source_pixmap| {
      let paint = TinyPaint {
        shader: TinyPattern::new(
          source_pixmap,
          TinySpreadMode::Pad,
          to_tiny_filter_quality(options.algorithm),
          1.0,
          options.source_to_canvas.into(),
        ),
        blend_mode,
        anti_alias: true,
        ..Default::default()
      };

      pixmap.fill_path(
        &path,
        &paint,
        TinyFillRule::Winding,
        options.transform.into(),
        combined_mask,
      );

      true
    })
    .unwrap_or(false)
}

fn build_border_path(border: BorderProperties, size: Size<u32>) -> Option<TinyPath> {
  let mut commands = Vec::new();
  border.append_mask_commands(&mut commands, size.map(|v| v as f32), Point::ZERO);
  build_path(&commands)
}

pub(crate) fn overlay_image<'a, I: Into<PaintSource<'a>>>(
  pixmap: &mut PixmapMut<'_>,
  image: I,
  options: OverlayOptions<'_>,
  buffer_pool: &mut BufferPool,
) {
  let image = image.into();
  let width = image.width();
  let height = image.height();
  let size = Size { width, height };

  if let PaintSource::ColorTile(color) = image
    && try_fill_color_with_tiny_skia(
      pixmap,
      FillColorOptions {
        color,
        size,
        border: options.border,
        transform: options.transform,
        mode: options.mode,
        combined_mask: options.combined_mask,
      },
      buffer_pool,
    )
  {
    return;
  }

  if options.border.is_zero() && options.transform.only_translation() {
    let offset = options.transform.decompose_translation();
    blit_paint_source_translation(pixmap, image, offset, options.mode, options.combined_mask);
    return;
  }

  if options.border.is_zero()
    && try_draw_image_with_tiny_skia(
      pixmap,
      image,
      options.transform,
      options.algorithm,
      options.mode,
      options.combined_mask,
      buffer_pool,
    )
  {
    return;
  }

  if !options.border.is_zero()
    && image.supports_rounded_fill_fast_path()
    && try_fill_image_path_with_tiny_skia(
      pixmap,
      image,
      ImagePathFillOptions {
        size,
        border: options.border,
        transform: options.transform,
        source_to_canvas: Affine {
          x: 0.0,
          y: 0.0,
          ..options.transform
        },
        algorithm: options.algorithm,
        mode: options.mode,
        combined_mask: options.combined_mask,
      },
      buffer_pool,
    )
  {
    return;
  }

  let mut paths = Vec::new();
  options
    .border
    .append_mask_commands(&mut paths, size.map(|v| v as f32), Point::ZERO);

  let (mask, placement) = render_mask(&paths, Some(options.transform), None, buffer_pool);
  let inverse = options.transform.invert();
  if options.transform.is_identity() && placement.left >= 0 && placement.top >= 0 {
    composite::source(
      pixmap,
      &mask,
      image,
      composite::Options {
        placement,
        sampling: MaskSamplingOptions {
          canvas_to_source: Affine::IDENTITY,
          sample_bias: Point { x: 0.5, y: 0.5 },
          algorithm: options.algorithm,
        },
        color_mode: MaskCompositeColor::SourceOnly,
        mode: options.mode,
        combined_mask: options.combined_mask,
      },
    );
  } else if let Some(inverse) = inverse {
    composite::source(
      pixmap,
      &mask,
      image,
      composite::Options {
        placement,
        sampling: MaskSamplingOptions {
          canvas_to_source: inverse,
          sample_bias: Point { x: 0.5, y: 0.5 },
          algorithm: options.algorithm,
        },
        color_mode: MaskCompositeColor::SourceOnly,
        mode: options.mode,
        combined_mask: options.combined_mask,
      },
    );
  }

  buffer_pool.release(mask);
}
fn overlay_sampled_paint_source(
  pixmap: &mut PixmapMut<'_>,
  source: PaintSource<'_>,
  size: Size<u32>,
  options: OverlayOptions<'_>,
  sampling: SamplingOptions,
  buffer_pool: &mut BufferPool,
) {
  let direct_identity_mapping = options.border.is_zero()
    && sampling.logical_to_source.is_identity()
    && size.width == source.width()
    && size.height == source.height();

  if direct_identity_mapping
    && try_draw_image_with_tiny_skia(
      pixmap,
      source,
      options.transform,
      options.algorithm,
      options.mode,
      options.combined_mask,
      buffer_pool,
    )
  {
    return;
  }

  if options.border.is_zero() && options.transform.only_translation() {
    blit_sampled_paint_source_translation(
      pixmap,
      source,
      size,
      options.transform.decompose_translation(),
      sampling,
      options.mode,
      options.combined_mask,
    );
    return;
  }

  let mut paths = Vec::new();
  options
    .border
    .append_mask_commands(&mut paths, size.map(|v| v as f32), Point::ZERO);
  let (mask, placement) = render_mask(&paths, Some(options.transform), None, buffer_pool);

  let inverse = options.transform.invert();
  if options.transform.is_identity() && placement.left >= 0 && placement.top >= 0 {
    composite::source(
      pixmap,
      &mask,
      source,
      composite::Options {
        placement,
        sampling: MaskSamplingOptions {
          canvas_to_source: sampling.logical_to_source,
          sample_bias: Point { x: 0.5, y: 0.5 },
          algorithm: sampling.algorithm,
        },
        color_mode: MaskCompositeColor::SourceOnly,
        mode: options.mode,
        combined_mask: options.combined_mask,
      },
    );
  } else if let Some(inverse) = inverse {
    let combined_inverse = sampling.logical_to_source * inverse;
    composite::source(
      pixmap,
      &mask,
      source,
      composite::Options {
        placement,
        sampling: MaskSamplingOptions {
          canvas_to_source: combined_inverse,
          sample_bias: Point { x: 0.5, y: 0.5 },
          algorithm: sampling.algorithm,
        },
        color_mode: MaskCompositeColor::SourceOnly,
        mode: options.mode,
        combined_mask: options.combined_mask,
      },
    );
  }

  buffer_pool.release(mask);
}
#[inline(always)]
pub(crate) fn mask_index_from_coord(x: u32, y: u32, width: u32) -> usize {
  (y * width + x) as usize
}

pub(crate) fn overlay_gradient_tile<T>(
  pixmap: &mut PixmapMut<'_>,
  gradient: &T,
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) where
  T: GradientOverlayTile,
{
  let bottom_width = pixmap.width();
  let bottom_height = pixmap.height();
  let top_size = Size {
    width: gradient.width(),
    height: gradient.height(),
  };

  if mode == BlendMode::Normal && combined_mask.is_none() {
    let bottom_data: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
    overlay_gradient_tile_fast_normal_unconstrained(
      bottom_data,
      bottom_width,
      bottom_height,
      gradient,
      offset,
    );
    return;
  }

  let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds_for_canvas(
      bottom_width,
      bottom_height,
      offset,
      top_size.width,
      top_size.height,
    )
  else {
    return;
  };

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  for dest_y in dest_y_min..dest_y_max {
    let src_y = (dest_y - offset_y) as u32;
    let dst_row = dest_y as usize * bottom_width as usize;
    for dest_x in dest_x_min..dest_x_max {
      let src_x = (dest_x - offset_x) as u32;
      let src = premultiplied_from_pixel(gradient.sample_pixel(src_x, src_y));
      if src[3] == 0 {
        continue;
      }

      let dest_x = dest_x as u32;
      let Some(src) = apply_combined_mask(src, combined_mask, dest_x, dest_y as u32) else {
        continue;
      };

      blend_premultiplied_pixel(&mut pixels[dst_row + dest_x as usize], src, mode);
    }
  }
}

fn try_overlay_linear_gradient_tile_fast_normal_unconstrained(
  data: &mut [u8],
  bottom_width: u32,
  bottom_height: u32,
  gradient: &LinearGradientTile,
  offset: Point<f32>,
) -> bool {
  let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds_for_canvas(
      bottom_width,
      bottom_height,
      offset,
      gradient.width(),
      gradient.height(),
    )
  else {
    return true;
  };

  let Some(fast_path) = gradient.fast_path() else {
    return false;
  };

  let row_stride = bottom_width as usize * 4;
  let row_count = (dest_y_max - dest_y_min) as usize;
  let segment_pixel_count = (dest_x_max - dest_x_min) as usize;
  let dest_byte_start = dest_x_min as usize * 4;
  let dest_byte_end = dest_byte_start + segment_pixel_count * 4;
  let rows = &mut data[dest_y_min as usize * row_stride..dest_y_max as usize * row_stride];

  match fast_path.kind {
    LinearGradientFastPathKind::Horizontal => {
      let src_x_start = (dest_x_min - offset_x) as usize;
      let src_x_end = src_x_start + segment_pixel_count;
      let src_pixels = &fast_path.axis_samples[src_x_start..src_x_end];

      if fast_path.fully_opaque {
        let scanline: &[u8] = bytemuck::cast_slice(src_pixels);

        for row in rows.chunks_mut(row_stride) {
          row[dest_byte_start..dest_byte_end].copy_from_slice(scanline);
        }
      } else {
        for row in rows.chunks_mut(row_stride) {
          composite_premultiplied_over_span(&mut row[dest_byte_start..dest_byte_end], src_pixels);
        }
      }
    }
    LinearGradientFastPathKind::Vertical => {
      let src_y_start = (dest_y_min - offset_y) as usize;
      let src_pixels = &fast_path.axis_samples[src_y_start..src_y_start + row_count];

      for (row_offset, row) in rows.chunks_mut(row_stride).enumerate() {
        let row_segment = &mut row[dest_byte_start..dest_byte_end];
        let pixel = src_pixels[row_offset];
        if fast_path.fully_opaque {
          fill_repeated_premultiplied_pixel(
            row_segment,
            [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()],
          );
        } else {
          blend_repeated_premultiplied_pixel(row_segment, pixel);
        }
      }
    }
  }

  true
}

pub(crate) fn overlay_linear_gradient_tile(
  pixmap: &mut PixmapMut<'_>,
  gradient: &LinearGradientTile,
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) {
  overlay_gradient_tile_with_fast_path(
    pixmap,
    gradient,
    offset,
    mode,
    combined_mask,
    try_overlay_linear_gradient_tile_fast_normal_unconstrained,
  );
}

fn overlay_gradient_tile_with_fast_path<T>(
  pixmap: &mut PixmapMut<'_>,
  gradient: &T,
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
  try_fast_path: impl FnOnce(&mut [u8], u32, u32, &T, Point<f32>) -> bool,
) where
  T: GradientOverlayTile,
{
  let bottom_width = pixmap.width();
  let bottom_height = pixmap.height();

  if mode == BlendMode::Normal && combined_mask.is_none() {
    let bottom_data: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
    if try_fast_path(bottom_data, bottom_width, bottom_height, gradient, offset) {
      return;
    }

    overlay_gradient_tile_fast_normal_unconstrained(
      bottom_data,
      bottom_width,
      bottom_height,
      gradient,
      offset,
    );
    return;
  }

  overlay_gradient_tile(pixmap, gradient, offset, mode, combined_mask);
}

fn try_overlay_radial_gradient_tile_fast_normal_unconstrained(
  data: &mut [u8],
  bottom_width: u32,
  bottom_height: u32,
  gradient: &RadialGradientTile,
  offset: Point<f32>,
) -> bool {
  let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds_for_canvas(
      bottom_width,
      bottom_height,
      offset,
      gradient.width(),
      gradient.height(),
    )
  else {
    return true;
  };

  if gradient.repeating {
    return false;
  }

  let Some(outer_pixel) = gradient.outer_sample() else {
    return true;
  };

  let lut_len = gradient.lut_len();
  if lut_len == 0 {
    return true;
  }

  let row_stride = bottom_width as usize * 4;
  for dest_y in dest_y_min..dest_y_max {
    let src_y = (dest_y - offset_y) as u32;
    let src_x_start = (dest_x_min - offset_x) as u32;
    let src_x_end = (dest_x_max - offset_x) as u32;
    let Some((active_x_start, active_x_end)) =
      gradient.non_repeating_active_span(src_x_start, src_x_end, src_y)
    else {
      return false;
    };

    let row_start = dest_y as usize * row_stride + dest_x_min as usize * 4;
    let row_end = row_start + (dest_x_max - dest_x_min) as usize * 4;
    let row = &mut data[row_start..row_end];

    let left_pixels = (active_x_start - src_x_start) as usize;
    if left_pixels > 0 {
      composite_repeated_premultiplied_pixel_normal(&mut row[..left_pixels * 4], outer_pixel);
    }

    let center_pixels = (active_x_end - active_x_start) as usize;
    if center_pixels > 0 {
      let center_byte_start = left_pixels * 4;
      let center_byte_end = center_byte_start + center_pixels * 4;
      let center_row = &mut row[center_byte_start..center_byte_end];
      let mut row_state = gradient.begin_row(active_x_start, src_y, lut_len);
      for pixel in center_row.chunks_exact_mut(4) {
        let lut_idx = gradient.next_lut_index(&mut row_state);
        let src = gradient.sample_at(lut_idx);
        blend_premultiplied_pixel_normal(pixel, src);
      }
    }

    let right_pixels = (src_x_end - active_x_end) as usize;
    if right_pixels > 0 {
      let right_byte_start = row.len() - right_pixels * 4;
      composite_repeated_premultiplied_pixel_normal(&mut row[right_byte_start..], outer_pixel);
    }
  }

  true
}

pub(crate) fn overlay_radial_gradient_tile(
  pixmap: &mut PixmapMut<'_>,
  gradient: &RadialGradientTile,
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) {
  overlay_gradient_tile_with_fast_path(
    pixmap,
    gradient,
    offset,
    mode,
    combined_mask,
    try_overlay_radial_gradient_tile_fast_normal_unconstrained,
  );
}

#[cfg(test)]
mod tests {
  use image::RgbaImage;
  use tiny_skia::PixmapMut;

  use crate::{
    Fonts, RenderContext, blend_pixel,
    layout::{
      Viewport,
      style::{
        Angle, BlendMode, Color, ColorInterpolationMethod, ConicGradient, ConicGradientTile,
        FromCss, GradientStop, Length, LinearGradient, LinearGradientTile, PositionValue,
        RadialGradient, RadialGradientTile, SizingContext, StopPosition,
      },
    },
    pixmap_from_buffer,
    resources::image_buffer::ImageBuffer,
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

  fn assert_gradient_overlay_matches_reference_with<T>(
    tile: &T,
    canvas_size: Size<u32>,
    offset: Point<f32>,
    overlay: impl FnOnce(&mut PixmapMut<'_>, &T, Point<f32>),
  ) where
    T: GradientOverlayTile,
  {
    let mut canvas = Canvas::new(canvas_size);
    let mut reference =
      RgbaImage::from_pixel(canvas_size.width, canvas_size.height, Rgba([0, 0, 0, 0]));

    {
      let mut pixmap = canvas.image.as_mut();
      overlay(&mut pixmap, tile, offset);
    }

    overlay_area_reference(
      &mut reference,
      offset,
      Size {
        width: tile.width(),
        height: tile.height(),
      },
      |x, y| {
        let color = tile.sample_pixel(x, y).demultiply();
        Rgba([color.red(), color.green(), color.blue(), color.alpha()])
      },
    );

    let Ok(fast) = canvas.into_inner() else {
      return;
    };
    assert_eq!(fast.as_raw(), reference.as_raw());
  }

  fn assert_gradient_overlay_matches_reference<T>(
    tile: &T,
    canvas_size: Size<u32>,
    offset: Point<f32>,
  ) where
    T: GradientOverlayTile,
  {
    assert_gradient_overlay_matches_reference_with(
      tile,
      canvas_size,
      offset,
      |pixmap, tile, offset| {
        overlay_gradient_tile(pixmap, tile, offset, BlendMode::Normal, None);
      },
    );
  }

  #[test]
  fn test_overlay_linear_gradient_matches_reference() {
    let Ok(gradient) = LinearGradient::from_str("linear-gradient(to right, red, blue)") else {
      return;
    };
    let global_context = Fonts::default();
    let render_context = RenderContext::builder()
      .fonts(global_context.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((32, 16)))
          .build(),
      )
      .build();
    let tile = LinearGradientTile::new(
      &gradient,
      32,
      16,
      &render_context.sizing,
      render_context.current_color,
    );
    assert_gradient_overlay_matches_reference_with(
      &tile,
      Size {
        width: 40,
        height: 24,
      },
      Point { x: 3.0, y: 4.0 },
      |pixmap, tile, offset| {
        overlay_linear_gradient_tile(pixmap, tile, offset, BlendMode::Normal, None);
      },
    );
  }

  #[test]
  fn test_overlay_radial_gradient_fast_paths_match_reference() {
    let cases = [
      (
        "radial-gradient(circle, red, blue)",
        Size {
          width: 32,
          height: 24,
        },
        Size {
          width: 40,
          height: 30,
        },
        Point { x: 4.0, y: 3.0 },
      ),
      (
        "radial-gradient(circle at 20% 30%, red, rgba(0,0,255,0.25))",
        Size {
          width: 40,
          height: 28,
        },
        Size {
          width: 52,
          height: 36,
        },
        Point { x: 5.0, y: 4.0 },
      ),
    ];

    let global_context = Fonts::default();
    for (gradient_css, tile_size, canvas_size, offset) in cases {
      let Ok(gradient) = RadialGradient::from_str(gradient_css) else {
        continue;
      };
      let render_context = RenderContext::builder()
        .fonts(global_context.snapshot())
        .sizing(
          SizingContext::builder()
            .viewport(Viewport::new((tile_size.width, tile_size.height)))
            .build(),
        )
        .build();
      let tile = RadialGradientTile::new(
        &gradient,
        tile_size.width,
        tile_size.height,
        &render_context.sizing,
        render_context.current_color,
      );
      assert_gradient_overlay_matches_reference_with(
        &tile,
        canvas_size,
        offset,
        |pixmap, tile, offset| {
          overlay_radial_gradient_tile(pixmap, tile, offset, BlendMode::Normal, None);
        },
      );
    }
  }

  #[test]
  fn test_overlay_conic_gradient_matches_reference() {
    let Ok(gradient) = ConicGradient::from_str("conic-gradient(red, blue)") else {
      return;
    };

    let global_context = Fonts::default();
    let render_context = RenderContext::builder()
      .fonts(global_context.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((32, 24)))
          .build(),
      )
      .build();
    let tile = ConicGradientTile::new(
      &gradient,
      32,
      24,
      &render_context.sizing,
      render_context.current_color,
    );
    assert_gradient_overlay_matches_reference_with(
      &tile,
      Size {
        width: 40,
        height: 30,
      },
      Point { x: 4.0, y: 3.0 },
      |pixmap, tile, offset| {
        overlay_gradient_tile(pixmap, tile, offset, BlendMode::Normal, None);
      },
    );
  }

  #[test]
  fn test_overlay_linear_gradient_fast_paths_match_reference() {
    let cases = [
      (
        "linear-gradient(to right, red 0px, lime 0.5px, blue 32px)",
        Size {
          width: 32,
          height: 16,
        },
        Size {
          width: 40,
          height: 24,
        },
        Point { x: 3.0, y: 4.0 },
      ),
      (
        "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
        Size {
          width: 48,
          height: 12,
        },
        Size {
          width: 56,
          height: 24,
        },
        Point { x: 4.0, y: 6.0 },
      ),
      (
        "linear-gradient(180deg, rgba(0,128,255,0.9), rgba(0,128,255,0))",
        Size {
          width: 24,
          height: 48,
        },
        Size {
          width: 36,
          height: 64,
        },
        Point { x: 6.0, y: 5.0 },
      ),
      (
        "linear-gradient(to right, grey 1px, transparent 1px)",
        Size {
          width: 40,
          height: 8,
        },
        Size {
          width: 48,
          height: 16,
        },
        Point { x: 4.0, y: 3.0 },
      ),
      (
        "repeating-linear-gradient(90deg, red 0px 5px, blue 5px 10px)",
        Size {
          width: 40,
          height: 8,
        },
        Size {
          width: 52,
          height: 16,
        },
        Point { x: 5.0, y: 4.0 },
      ),
    ];

    let global_context = Fonts::default();
    for (gradient_css, tile_size, canvas_size, offset) in cases {
      let Ok(gradient) = LinearGradient::from_str(gradient_css) else {
        continue;
      };
      let render_context = RenderContext::builder()
        .fonts(global_context.snapshot())
        .sizing(
          SizingContext::builder()
            .viewport(Viewport::new((tile_size.width, tile_size.height)))
            .build(),
        )
        .build();
      let tile = LinearGradientTile::new(
        &gradient,
        tile_size.width,
        tile_size.height,
        &render_context.sizing,
        render_context.current_color,
      );
      assert_gradient_overlay_matches_reference_with(
        &tile,
        canvas_size,
        offset,
        |pixmap, tile, offset| {
          overlay_linear_gradient_tile(pixmap, tile, offset, BlendMode::Normal, None);
        },
      );
    }
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
    let source_pixmap = ImageBuffer::from_rgba(std::borrow::Cow::Borrowed(&source))
      .as_ref()
      .and_then(pixmap_from_buffer)
      .expect("fixture pixmap conversion");

    let mut direct = Canvas::new(Size {
      width: 8,
      height: 6,
    });
    direct.overlay_sampled_pixmap(
      source_pixmap.as_ref(),
      Size {
        width: 4,
        height: 2,
      },
      BorderProperties::default(),
      Affine::translation(2.0, 2.0),
      SamplingOptions {
        logical_to_source: Affine::scale(0.5, 0.5),
        algorithm: ImageScalingAlgorithm::Pixelated,
      },
      BlendMode::Normal,
    );

    let mut isolated = Canvas::new(Size {
      width: 8,
      height: 6,
    });
    let Ok(subcanvas) = isolated.begin_subcanvas(Placement {
      left: 2,
      top: 2,
      width: 4,
      height: 2,
    }) else {
      return;
    };
    isolated.overlay_sampled_pixmap(
      source_pixmap.as_ref(),
      Size {
        width: 4,
        height: 2,
      },
      BorderProperties::default(),
      Affine::translation(2.0, 2.0),
      SamplingOptions {
        logical_to_source: Affine::scale(0.5, 0.5),
        algorithm: ImageScalingAlgorithm::Pixelated,
      },
      BlendMode::Normal,
    );
    isolated.composite_subcanvas(subcanvas, BlendMode::Normal, 1.0);

    assert_eq!(
      direct.into_inner().map(RgbaImage::into_raw).ok(),
      isolated.into_inner().map(RgbaImage::into_raw).ok()
    );
  }

  #[test]
  fn test_overlay_conic_gradient_hard_stops_matches_reference() {
    let gradient = ConicGradient {
      repeating: false,
      from_angle: Angle::zero(),
      center: PositionValue::center(),
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

    let global_context = Fonts::default();
    let render_context = RenderContext::builder()
      .fonts(global_context.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((48, 48)))
          .build(),
      )
      .build();
    let tile = ConicGradientTile::new(
      &gradient,
      48,
      48,
      &render_context.sizing,
      render_context.current_color,
    );
    assert_gradient_overlay_matches_reference(
      &tile,
      Size {
        width: 56,
        height: 56,
      },
      Point { x: 4.0, y: 4.0 },
    );
  }

  #[test]
  fn test_overlay_radial_gradient_clustered_stops_matches_reference() {
    let Ok(gradient) =
      RadialGradient::from_str("radial-gradient(circle, red 0%, lime 1%, blue 100%)")
    else {
      return;
    };
    let global_context = Fonts::default();
    let render_context = RenderContext::builder()
      .fonts(global_context.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((32, 24)))
          .build(),
      )
      .build();
    let tile = RadialGradientTile::new(
      &gradient,
      32,
      24,
      &render_context.sizing,
      render_context.current_color,
    );
    assert_gradient_overlay_matches_reference(
      &tile,
      Size {
        width: 40,
        height: 30,
      },
      Point { x: 4.0, y: 3.0 },
    );
  }
}
