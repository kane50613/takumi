//! Canvas operations and image blending for the takumi rendering system.
//!
//! This module provides performance-optimized canvas operations including
//! fast image blending and pixel manipulation operations.

mod blit;
mod buffer_pool;
mod composite;
mod gradient;
mod mask;
mod paint_source;
mod skia;

use std::{borrow::Cow, mem::replace, sync::Arc};

pub(crate) use blit::{
  composite_mask_source_to_pixmap, overlay_image, overlay_sampled_paint_source,
};
pub(crate) use buffer_pool::BufferPool;
pub(crate) use gradient::{
  overlay_gradient_tile, overlay_linear_gradient_tile, overlay_radial_gradient_tile,
};
use image::{
  ImageError, Rgba, RgbaImage,
  error::{ParameterError, ParameterErrorKind},
};
pub(crate) use mask::{
  CanvasViewport, MaskView, NodeMaskAction, attenuate_alpha_by_mask, intersect_alpha_masks,
  mask_index_from_coord, prepare_node_mask, render_mask,
};
use mask::{MaskStackEntry, resolve_mask};
pub(crate) use paint_source::{
  MaskCompositeColor, PaintSource, SamplingFootprint, interpolate_with_footprint,
};
use takumi_core::geometry::{Point, Size};
use tiny_skia::{
  FilterQuality as TinyFilterQuality, Mask as TinyMask, Pixmap, PixmapMut, PixmapPaint, PixmapRef,
  Transform as TinyTransform,
};

use self::skia::to_tiny_blend_mode;
use crate::{
  BackgroundTile, BorderProperties, Placement, Result,
  blend::*,
  error::Error,
  stacking_context::blend_pixmap_software,
  style::{Affine, BlendMode, Color, ImageScalingAlgorithm},
};

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
pub(crate) struct OverlayOptions {
  pub border: BorderProperties,
  pub transform: Affine,
  pub algorithm: ImageScalingAlgorithm,
  pub mode: BlendMode,
}

/// Borrowed view of the active paint destination: the pixmap, the canvas's
/// combined constraint mask, and the scratch pool. Primitives take this instead
/// of threading `(pixmap, mask, pool, size)` separately, so a materialized mask
/// always matches the pixmap it clips.
pub(crate) struct DrawTarget<'a, 'p> {
  pub pixmap: &'a mut PixmapMut<'p>,
  pub combined_mask: Option<MaskView<'a>>,
  pub buffer_pool: &'a mut BufferPool,
}

impl<'a> DrawTarget<'a, '_> {
  pub(crate) fn size(&self) -> Size<u32> {
    Size {
      width: self.pixmap.width(),
      height: self.pixmap.height(),
    }
  }

  pub(crate) fn resolve_combined_mask(&mut self) -> Option<Cow<'a, TinyMask>> {
    let size = self.size();
    self
      .combined_mask
      .and_then(|mask| resolve_mask(mask, size, self.buffer_pool))
  }

  pub(crate) fn release_resolved_mask(&mut self, resolved: Option<Cow<'_, TinyMask>>) {
    if let Some(Cow::Owned(mask)) = resolved {
      self.buffer_pool.release(mask.take());
    }
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
      Error::encode(ImageError::Parameter(ParameterError::from_kind(
        ParameterErrorKind::DimensionMismatch,
      )))
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
      Error::encode(ImageError::Parameter(ParameterError::from_kind(
        ParameterErrorKind::DimensionMismatch,
      )))
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

  pub(crate) fn draw_mask(
    &mut self,
    mask: &[u8],
    placement: Placement,
    color: Color,
    mode: BlendMode,
  ) {
    let placement = self.localize_placement(placement);
    self.with_overlay_state(|target| {
      blit::draw_mask(
        target.pixmap,
        mask,
        placement,
        Rgba(color.0),
        mode,
        target.combined_mask,
      );
    });
  }
  pub(crate) fn composite_mask_source(
    &mut self,
    mask: &[u8],
    placement: Placement,
    source: PaintSource<'_>,
    color_mode: MaskCompositeColor,
    sampling: MaskSamplingOptions,
    mode: BlendMode,
  ) {
    let placement = self.localize_placement(placement);
    let sampling = self.localize_mask_sampling(sampling);
    self.with_overlay_state(|target| {
      composite::source(
        target.pixmap,
        mask,
        source,
        composite::Options {
          placement,
          sampling,
          color_mode,
          mode,
          combined_mask: target.combined_mask,
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
    self.with_overlay_state(|target| {
      overlay_sampled_paint_source(
        target,
        PaintSource::from(source),
        size,
        OverlayOptions {
          border,
          transform,
          algorithm: sampling.algorithm,
          mode,
        },
        sampling,
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
    self.with_overlay_state(|target| {
      overlay_image(
        target,
        image,
        OverlayOptions {
          border,
          transform,
          algorithm,
          mode,
        },
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

    self.with_overlay_state(|target| match tile {
      BackgroundTile::Linear(gradient) => {
        overlay_linear_gradient_tile(
          target.pixmap,
          gradient,
          localized_translation,
          mode,
          target.combined_mask,
        );
        true
      }
      BackgroundTile::Radial(gradient) => {
        overlay_radial_gradient_tile(
          target.pixmap,
          gradient,
          localized_translation,
          mode,
          target.combined_mask,
        );
        true
      }
      BackgroundTile::Conic(gradient) => {
        overlay_gradient_tile(
          target.pixmap,
          gradient,
          localized_translation,
          mode,
          target.combined_mask,
        );
        true
      }
      _ => false,
    })
  }

  fn with_overlay_state<R>(&mut self, f: impl FnOnce(&mut DrawTarget<'_, '_>) -> R) -> R {
    let combined_mask = self.constraint_mask_stack.last().and_then(Option::as_ref);
    let combined_mask = combined_mask.map(|entry| MaskView {
      mask: entry.mask.as_ref(),
      origin: entry.origin,
      canvas_origin: self.origin,
    });
    let mut pixmap = self.image.as_mut();
    let mut target = DrawTarget {
      pixmap: &mut pixmap,
      combined_mask,
      buffer_pool: &mut self.buffer_pool,
    };
    f(&mut target)
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
