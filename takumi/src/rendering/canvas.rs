//! Canvas operations and image blending for the takumi rendering system.
//!
//! This module provides performance-optimized canvas operations including
//! fast image blending and pixel manipulation operations.

mod buffer_pool;
mod mask;

use std::mem::replace;

use image::{
  ImageError, Rgba, RgbaImage,
  error::{ParameterError, ParameterErrorKind},
};
use taffy::{Point, Size};
use tiny_skia::{
  FillRule as TinyFillRule, FilterQuality as TinyFilterQuality, Mask as TinyMask,
  Paint as TinyPaint, Path as TinyPath, Pattern as TinyPattern, Pixmap, PixmapMut, PixmapPaint,
  PixmapRef, PremultipliedColorU8, SpreadMode as TinySpreadMode, Transform as TinyTransform,
};

use super::stacking_context::blend_pixmap_software;
use crate::{Result, layout::style::BlendMode};
use crate::{
  layout::style::{
    Affine, GradientOverlayTile, ImageScalingAlgorithm,
    overlay_gradient_tile_fast_normal_unconstrained,
  },
  rendering::{
    BackgroundTile, BorderProperties, ColorTile, Placement, blend_pixel, build_path, fast_div_255,
  },
};

pub(crate) use buffer_pool::BufferPool;
pub(crate) use mask::{CanvasViewport, NodeMaskAction, prepare_node_mask, render_mask};

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
      Self::BackgroundTile(BackgroundTile::Pixmap(source)) => Some(source.as_ref().as_ref()),
      _ => None,
    }
  }

  fn premultiplied_constant(self) -> Option<[u8; 4]> {
    match self {
      Self::ColorTile(tile) => Some(premultiplied_from_pixel(tile.get_pixel(0, 0))),
      Self::BackgroundTile(BackgroundTile::Color(tile)) => {
        Some(premultiplied_from_pixel(tile.get_pixel(0, 0)))
      }
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

#[derive(Clone, Copy)]
enum MaskCompositeColor {
  SourceOnly,
  SourceOverColor([u8; 4]),
  ColorOverSource([u8; 4]),
}

#[inline(always)]
fn apply_mask_color_mode(src: [u8; 4], color_mode: MaskCompositeColor) -> [u8; 4] {
  match color_mode {
    MaskCompositeColor::SourceOnly => src,
    MaskCompositeColor::SourceOverColor(color) => {
      let mut out = color;
      composite_premultiplied_over(&mut out, src);
      out
    }
    MaskCompositeColor::ColorOverSource(color) => {
      let mut out = src;
      composite_premultiplied_over(&mut out, color);
      out
    }
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
  pub(crate) fn composite_mask_source(
    &mut self,
    mask: &[u8],
    placement: Placement,
    source: PaintSource<'_>,
    canvas_to_source: Affine,
    sample_bias: Point<f32>,
    algorithm: ImageScalingAlgorithm,
    mode: BlendMode,
  ) {
    let placement = self.localize_placement(placement);
    let canvas_to_source =
      canvas_to_source * Affine::translation(self.origin.x as f32, self.origin.y as f32);
    self.with_overlay_state(|pixmap, combined_mask, _| {
      composite_masked_source(
        pixmap,
        mask,
        placement,
        source,
        canvas_to_source,
        sample_bias,
        algorithm,
        MaskCompositeColor::SourceOnly,
        mode,
        combined_mask,
      );
    });
  }

  #[allow(clippy::too_many_arguments)]
  pub(crate) fn composite_mask_source_over_color<C: Into<Rgba<u8>>>(
    &mut self,
    mask: &[u8],
    placement: Placement,
    source: PaintSource<'_>,
    color: C,
    canvas_to_source: Affine,
    sample_bias: Point<f32>,
    algorithm: ImageScalingAlgorithm,
    mode: BlendMode,
  ) {
    let placement = self.localize_placement(placement);
    let canvas_to_source =
      canvas_to_source * Affine::translation(self.origin.x as f32, self.origin.y as f32);
    self.with_overlay_state(|pixmap, combined_mask, _| {
      composite_masked_source(
        pixmap,
        mask,
        placement,
        source,
        canvas_to_source,
        sample_bias,
        algorithm,
        MaskCompositeColor::SourceOverColor(premultiply_rgba(color.into())),
        mode,
        combined_mask,
      );
    });
  }

  #[allow(clippy::too_many_arguments)]
  pub(crate) fn composite_mask_color_over_source<C: Into<Rgba<u8>>>(
    &mut self,
    mask: &[u8],
    placement: Placement,
    source: PaintSource<'_>,
    color: C,
    canvas_to_source: Affine,
    sample_bias: Point<f32>,
    algorithm: ImageScalingAlgorithm,
    mode: BlendMode,
  ) {
    let placement = self.localize_placement(placement);
    let canvas_to_source =
      canvas_to_source * Affine::translation(self.origin.x as f32, self.origin.y as f32);
    self.with_overlay_state(|pixmap, combined_mask, _| {
      composite_masked_source(
        pixmap,
        mask,
        placement,
        source,
        canvas_to_source,
        sample_bias,
        algorithm,
        MaskCompositeColor::ColorOverSource(premultiply_rgba(color.into())),
        mode,
        combined_mask,
      );
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
fn premultiply_rgba_pixel(red: u8, green: u8, blue: u8, alpha: u8) -> [u8; 4] {
  [
    fast_div_255(red as u32 * alpha as u32),
    fast_div_255(green as u32 * alpha as u32),
    fast_div_255(blue as u32 * alpha as u32),
    alpha,
  ]
}

#[inline(always)]
fn premultiply_rgba(color: Rgba<u8>) -> [u8; 4] {
  let [red, green, blue, alpha] = color.0;
  premultiply_rgba_pixel(red, green, blue, alpha)
}

#[inline(always)]
fn scale_premultiplied_pixel(pixel: [u8; 4], alpha: u8) -> [u8; 4] {
  if alpha == u8::MAX {
    return pixel;
  }

  [
    fast_div_255(pixel[0] as u32 * alpha as u32),
    fast_div_255(pixel[1] as u32 * alpha as u32),
    fast_div_255(pixel[2] as u32 * alpha as u32),
    fast_div_255(pixel[3] as u32 * alpha as u32),
  ]
}

#[inline(always)]
fn composite_premultiplied_over(dst: &mut [u8; 4], src: [u8; 4]) {
  let src_alpha = src[3];
  if src_alpha == 0 {
    return;
  }

  let dst_alpha = dst[3];
  if src_alpha == u8::MAX || dst_alpha == 0 {
    *dst = src;
    return;
  }

  let inverse_alpha = u8::MAX - src_alpha;
  dst[0] = src[0].saturating_add(fast_div_255(dst[0] as u32 * inverse_alpha as u32));
  dst[1] = src[1].saturating_add(fast_div_255(dst[1] as u32 * inverse_alpha as u32));
  dst[2] = src[2].saturating_add(fast_div_255(dst[2] as u32 * inverse_alpha as u32));
  dst[3] = src_alpha.saturating_add(fast_div_255(dst_alpha as u32 * inverse_alpha as u32));
}

#[inline(always)]
fn blend_premultiplied_pixel(dst: &mut [u8; 4], src: [u8; 4], mode: BlendMode) {
  if src[3] == 0 {
    return;
  }

  if mode == BlendMode::Normal {
    composite_premultiplied_over(dst, src);
    return;
  }

  let mut current = premultiplied_to_rgba(
    PremultipliedColorU8::from_rgba(
      dst[0].min(dst[3]),
      dst[1].min(dst[3]),
      dst[2].min(dst[3]),
      dst[3],
    )
    .unwrap_or(PremultipliedColorU8::TRANSPARENT),
  );
  let color = premultiplied_to_rgba(
    PremultipliedColorU8::from_rgba(src[0], src[1], src[2], src[3])
      .unwrap_or(PremultipliedColorU8::TRANSPARENT),
  );
  blend_pixel(&mut current, color, mode);
  *dst = premultiply_rgba(current);
}

#[inline(always)]
fn premultiplied_from_pixel(pixel: PremultipliedColorU8) -> [u8; 4] {
  [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
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
fn sample_pixmap_nearest(source: PixmapRef<'_>, x: f32, y: f32) -> Option<[u8; 4]> {
  let width = source.width();
  let height = source.height();
  if width == 0 || height == 0 {
    return None;
  }

  let px = x.floor().max(0.0) as u32;
  let py = y.floor().max(0.0) as u32;
  let px = px.min(width.saturating_sub(1));
  let py = py.min(height.saturating_sub(1));
  let pixel = source.pixels()[(py * width + px) as usize];
  Some([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()])
}

#[inline(always)]
fn sample_pixmap_bilinear(source: PixmapRef<'_>, x: f32, y: f32) -> Option<[u8; 4]> {
  let width = source.width();
  let height = source.height();
  if width == 0 || height == 0 {
    return None;
  }

  let x = (x - 0.5).clamp(0.0, width.saturating_sub(1) as f32);
  let y = (y - 0.5).clamp(0.0, height.saturating_sub(1) as f32);
  let uf = x.floor() as u32;
  let vf = y.floor() as u32;
  let uc = (uf + 1).min(width - 1);
  let vc = (vf + 1).min(height - 1);
  let pixels = source.pixels();
  let p00 = pixels[(vf * width + uf) as usize];
  let p01 = pixels[(vc * width + uf) as usize];
  let p10 = pixels[(vf * width + uc) as usize];
  let p11 = pixels[(vc * width + uc) as usize];

  let u_ratio = ((x - uf as f32) * 256.0) as u32;
  let v_ratio = ((y - vf as f32) * 256.0) as u32;
  let u_opposite = 256 - u_ratio;
  let v_opposite = 256 - v_ratio;
  let w00 = u_opposite * v_opposite;
  let w01 = u_opposite * v_ratio;
  let w10 = u_ratio * v_opposite;
  let w11 = u_ratio * v_ratio;

  let mut out = [0u8; 4];
  for (index, channel) in out.iter_mut().enumerate() {
    let p00_i = match index {
      0 => p00.red(),
      1 => p00.green(),
      2 => p00.blue(),
      _ => p00.alpha(),
    };
    let p01_i = match index {
      0 => p01.red(),
      1 => p01.green(),
      2 => p01.blue(),
      _ => p01.alpha(),
    };
    let p10_i = match index {
      0 => p10.red(),
      1 => p10.green(),
      2 => p10.blue(),
      _ => p10.alpha(),
    };
    let p11_i = match index {
      0 => p11.red(),
      1 => p11.green(),
      2 => p11.blue(),
      _ => p11.alpha(),
    };
    *channel = ((p00_i as u32 * w00 + p10_i as u32 * w10 + p01_i as u32 * w01 + p11_i as u32 * w11)
      >> 16) as u8;
  }

  Some(out)
}

#[inline(always)]
fn sample_rgba_nearest(source: &RgbaImage, x: f32, y: f32) -> Option<[u8; 4]> {
  let width = source.width();
  let height = source.height();
  if width == 0 || height == 0 {
    return None;
  }

  let px = x.floor().max(0.0) as u32;
  let py = y.floor().max(0.0) as u32;
  let px = px.min(width.saturating_sub(1));
  let py = py.min(height.saturating_sub(1));
  let raw = source.as_raw();
  let offset = ((py * width + px) * 4) as usize;
  Some(premultiply_rgba_pixel(
    raw[offset],
    raw[offset + 1],
    raw[offset + 2],
    raw[offset + 3],
  ))
}

#[inline(always)]
fn sample_rgba_bilinear(source: &RgbaImage, x: f32, y: f32) -> Option<[u8; 4]> {
  let width = source.width();
  let height = source.height();
  if width == 0 || height == 0 {
    return None;
  }

  let x = (x - 0.5).clamp(0.0, width.saturating_sub(1) as f32);
  let y = (y - 0.5).clamp(0.0, height.saturating_sub(1) as f32);
  let uf = x.floor() as u32;
  let vf = y.floor() as u32;
  let uc = (uf + 1).min(width - 1);
  let vc = (vf + 1).min(height - 1);
  let raw = source.as_raw();

  let get_pixel = |x: u32, y: u32| {
    let offset = ((y * width + x) * 4) as usize;
    premultiply_rgba_pixel(
      raw[offset],
      raw[offset + 1],
      raw[offset + 2],
      raw[offset + 3],
    )
  };

  let p00 = get_pixel(uf, vf);
  let p01 = get_pixel(uf, vc);
  let p10 = get_pixel(uc, vf);
  let p11 = get_pixel(uc, vc);

  let u_ratio = ((x - uf as f32) * 256.0) as u32;
  let v_ratio = ((y - vf as f32) * 256.0) as u32;
  let u_opposite = 256 - u_ratio;
  let v_opposite = 256 - v_ratio;
  let w00 = u_opposite * v_opposite;
  let w01 = u_opposite * v_ratio;
  let w10 = u_ratio * v_opposite;
  let w11 = u_ratio * v_ratio;

  let mut out = [0u8; 4];
  for index in 0..4 {
    out[index] = ((p00[index] as u32 * w00
      + p10[index] as u32 * w10
      + p01[index] as u32 * w01
      + p11[index] as u32 * w11)
      >> 16) as u8;
  }

  Some(out)
}

#[inline(always)]
fn sample_paint_source(
  source: PaintSource<'_>,
  algorithm: ImageScalingAlgorithm,
  x: f32,
  y: f32,
) -> Option<[u8; 4]> {
  match source {
    PaintSource::RgbaImage(image) => {
      if matches!(algorithm, ImageScalingAlgorithm::Pixelated) {
        sample_rgba_nearest(image, x, y)
      } else {
        sample_rgba_bilinear(image, x, y)
      }
    }
    PaintSource::Pixmap(pixmap) => {
      if matches!(algorithm, ImageScalingAlgorithm::Pixelated) {
        sample_pixmap_nearest(pixmap.as_ref(), x, y)
      } else {
        sample_pixmap_bilinear(pixmap.as_ref(), x, y)
      }
    }
    _ if matches!(algorithm, ImageScalingAlgorithm::Pixelated) => {
      interpolate_nearest(source, x, y).map(premultiplied_from_pixel)
    }
    _ => interpolate_bilinear(source, x, y).map(premultiplied_from_pixel),
  }
}

fn blit_sampled_rgba_translation(
  pixmap: &mut PixmapMut<'_>,
  source: &RgbaImage,
  size: Size<u32>,
  offset: Point<f32>,
  logical_to_source: Affine,
  algorithm: ImageScalingAlgorithm,
  mode: BlendMode,
  combined_mask: Option<&TinyMask>,
) {
  let canvas_width = pixmap.width();
  let canvas_height = pixmap.height();
  let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds_for_canvas(canvas_width, canvas_height, offset, size.width, size.height)
  else {
    return;
  };

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let mask_data = combined_mask.map(TinyMask::data);
  for dest_y in dest_y_min..dest_y_max {
    let src_y = (dest_y - offset_y) as f32;
    let mut sample_point = logical_to_source.transform_point(Point {
      x: (dest_x_min - offset_x) as f32 + 0.5,
      y: src_y + 0.5,
    });
    for dest_x in dest_x_min..dest_x_max {
      let mut src = if matches!(algorithm, ImageScalingAlgorithm::Pixelated) {
        sample_rgba_nearest(source, sample_point.x, sample_point.y)
      } else {
        sample_rgba_bilinear(source, sample_point.x, sample_point.y)
      }
      .unwrap_or([0, 0, 0, 0]);
      sample_point.x += logical_to_source.a;
      sample_point.y += logical_to_source.b;
      if src[3] == 0 {
        continue;
      }

      let dest_x = dest_x as u32;
      let dest_y = dest_y as u32;
      if let Some(mask_data) = mask_data {
        let alpha = mask_data[mask_index_from_coord(dest_x, dest_y, canvas_width)];
        if alpha == 0 {
          continue;
        }
        src = scale_premultiplied_pixel(src, alpha);
        if src[3] == 0 {
          continue;
        }
      }

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
  combined_mask: Option<&TinyMask>,
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
  let mask_data = combined_mask.map(TinyMask::data);
  match source {
    PaintSource::RgbaImage(source) => {
      let raw = source.as_raw();
      let source_width = source.width();
      for dest_y in dest_y_min..dest_y_max {
        let src_y = (dest_y - offset_y) as u32;
        for dest_x in dest_x_min..dest_x_max {
          let src_x = (dest_x - offset_x) as u32;
          let raw_offset = ((src_y * source_width + src_x) * 4) as usize;
          let mut src = premultiply_rgba_pixel(
            raw[raw_offset],
            raw[raw_offset + 1],
            raw[raw_offset + 2],
            raw[raw_offset + 3],
          );
          if src[3] == 0 {
            continue;
          }

          let dest_x = dest_x as u32;
          let dest_y = dest_y as u32;
          if let Some(mask_data) = mask_data {
            let alpha = mask_data[mask_index_from_coord(dest_x, dest_y, canvas_width)];
            if alpha == 0 {
              continue;
            }
            src = scale_premultiplied_pixel(src, alpha);
            if src[3] == 0 {
              continue;
            }
          }

          let index = (dest_y * canvas_width + dest_x) as usize;
          blend_premultiplied_pixel(&mut pixels[index], src, mode);
        }
      }
    }
    PaintSource::Pixmap(source) => {
      let source_pixels = source.pixels();
      let source_width = source.width();
      for dest_y in dest_y_min..dest_y_max {
        let src_y = (dest_y - offset_y) as u32;
        for dest_x in dest_x_min..dest_x_max {
          let src_x = (dest_x - offset_x) as u32;
          let mut src =
            premultiplied_from_pixel(source_pixels[(src_y * source_width + src_x) as usize]);
          if src[3] == 0 {
            continue;
          }

          let dest_x = dest_x as u32;
          let dest_y = dest_y as u32;
          if let Some(mask_data) = mask_data {
            let alpha = mask_data[mask_index_from_coord(dest_x, dest_y, canvas_width)];
            if alpha == 0 {
              continue;
            }
            src = scale_premultiplied_pixel(src, alpha);
            if src[3] == 0 {
              continue;
            }
          }

          let index = (dest_y * canvas_width + dest_x) as usize;
          blend_premultiplied_pixel(&mut pixels[index], src, mode);
        }
      }
    }
    _ => {
      for dest_y in dest_y_min..dest_y_max {
        let src_y = (dest_y - offset_y) as f32;
        for dest_x in dest_x_min..dest_x_max {
          let src_x = (dest_x - offset_x) as f32;
          let mut src = sample_paint_source(source, ImageScalingAlgorithm::Pixelated, src_x, src_y)
            .unwrap_or([0; 4]);
          if src[3] == 0 {
            continue;
          }

          let dest_x = dest_x as u32;
          let dest_y = dest_y as u32;
          if let Some(mask_data) = mask_data {
            let alpha = mask_data[mask_index_from_coord(dest_x, dest_y, canvas_width)];
            if alpha == 0 {
              continue;
            }
            src = scale_premultiplied_pixel(src, alpha);
            if src[3] == 0 {
              continue;
            }
          }

          let index = (dest_y * canvas_width + dest_x) as usize;
          blend_premultiplied_pixel(&mut pixels[index], src, mode);
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
  combined_mask: Option<&TinyMask>,
) {
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

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let mask_data = combined_mask.map(TinyMask::data);
  for dest_y in dest_y_min..dest_y_max {
    for dest_x in dest_x_min..dest_x_max {
      let mut src = color;
      if src[3] == 0 {
        continue;
      }

      let dest_x = dest_x as u32;
      let dest_y = dest_y as u32;
      if let Some(mask_data) = mask_data {
        let alpha = mask_data[mask_index_from_coord(dest_x, dest_y, canvas_width)];
        if alpha == 0 {
          continue;
        }
        src = scale_premultiplied_pixel(src, alpha);
        if src[3] == 0 {
          continue;
        }
      }

      let index = (dest_y * canvas_width + dest_x) as usize;
      blend_premultiplied_pixel(&mut pixels[index], src, mode);
    }
  }
}

fn composite_masked_constant(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  placement: Placement,
  color: [u8; 4],
  mode: BlendMode,
  combined_mask: Option<&TinyMask>,
) {
  let canvas_width = pixmap.width();
  let canvas_height = pixmap.height();
  let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds_for_canvas(
      canvas_width,
      canvas_height,
      Point {
        x: placement.left as f32,
        y: placement.top as f32,
      },
      placement.width,
      placement.height,
    )
  else {
    return;
  };

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let mask_data = combined_mask.map(TinyMask::data);
  for dest_y in dest_y_min..dest_y_max {
    let mask_y = (dest_y - offset_y) as u32;
    for dest_x in dest_x_min..dest_x_max {
      let mask_x = (dest_x - offset_x) as u32;
      let mut src = scale_premultiplied_pixel(
        color,
        mask[mask_index_from_coord(mask_x, mask_y, placement.width)],
      );
      if src[3] == 0 {
        continue;
      }

      let dest_x = dest_x as u32;
      let dest_y = dest_y as u32;
      if let Some(mask_data) = mask_data {
        let alpha = mask_data[mask_index_from_coord(dest_x, dest_y, canvas_width)];
        if alpha == 0 {
          continue;
        }
        src = scale_premultiplied_pixel(src, alpha);
        if src[3] == 0 {
          continue;
        }
      }

      let index = (dest_y * canvas_width + dest_x) as usize;
      blend_premultiplied_pixel(&mut pixels[index], src, mode);
    }
  }
}

#[allow(clippy::too_many_arguments)]
fn composite_masked_source(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  placement: Placement,
  source: PaintSource<'_>,
  canvas_to_source: Affine,
  sample_bias: Point<f32>,
  algorithm: ImageScalingAlgorithm,
  color_mode: MaskCompositeColor,
  mode: BlendMode,
  combined_mask: Option<&TinyMask>,
) {
  if mask.is_empty() {
    return;
  }

  if let Some(color) = source.premultiplied_constant() {
    composite_masked_constant(
      pixmap,
      mask,
      placement,
      apply_mask_color_mode(color, color_mode),
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
      Point {
        x: placement.left as f32,
        y: placement.top as f32,
      },
      placement.width,
      placement.height,
    )
  else {
    return;
  };

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let mask_data = combined_mask.map(TinyMask::data);
  for dest_y in dest_y_min..dest_y_max {
    let mask_y = (dest_y - offset_y) as u32;
    let mut sample_point = canvas_to_source.transform_point(Point {
      x: dest_x_min as f32 + sample_bias.x,
      y: dest_y as f32 + sample_bias.y,
    });
    for dest_x in dest_x_min..dest_x_max {
      let mask_x = (dest_x - offset_x) as u32;
      let mask_alpha = mask[mask_index_from_coord(mask_x, mask_y, placement.width)];
      let sampled = if mask_alpha == 0 {
        None
      } else {
        sample_paint_source(source, algorithm, sample_point.x, sample_point.y)
      };
      sample_point.x += canvas_to_source.a;
      sample_point.y += canvas_to_source.b;

      let Some(mut src) = sampled else {
        continue;
      };

      src = apply_mask_color_mode(src, color_mode);

      src = scale_premultiplied_pixel(src, mask_alpha);
      if src[3] == 0 {
        continue;
      }

      let dest_x = dest_x as u32;
      let dest_y = dest_y as u32;
      if let Some(mask_data) = mask_data {
        let alpha = mask_data[mask_index_from_coord(dest_x, dest_y, canvas_width)];
        if alpha == 0 {
          continue;
        }
        src = scale_premultiplied_pixel(src, alpha);
        if src[3] == 0 {
          continue;
        }
      }

      let index = (dest_y * canvas_width + dest_x) as usize;
      blend_premultiplied_pixel(&mut pixels[index], src, mode);
    }
  }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn composite_mask_source_to_pixmap(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  placement: Placement,
  source: PaintSource<'_>,
  canvas_to_source: Affine,
  sample_bias: Point<f32>,
  algorithm: ImageScalingAlgorithm,
  mode: BlendMode,
  combined_mask: Option<&TinyMask>,
) {
  composite_masked_source(
    pixmap,
    mask,
    placement,
    source,
    canvas_to_source,
    sample_bias,
    algorithm,
    MaskCompositeColor::SourceOnly,
    mode,
    combined_mask,
  );
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

  composite_masked_constant(
    pixmap,
    mask,
    placement,
    premultiply_rgba(color),
    mode,
    combined_mask,
  );
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

  if border.is_zero() && transform.only_translation() {
    let offset = transform.decompose_translation();
    blit_paint_source_translation(pixmap, image, offset, mode, combined_mask);
    return;
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

  let mut paths = Vec::new();
  border.append_mask_commands(&mut paths, size.map(|v| v as f32), Point::ZERO);

  let (mask, placement) = render_mask(&paths, Some(transform), None, buffer_pool);
  let inverse = transform.invert();
  if transform.is_identity() && placement.left >= 0 && placement.top >= 0 {
    composite_masked_source(
      pixmap,
      &mask,
      placement,
      image,
      Affine::IDENTITY,
      Point { x: 0.5, y: 0.5 },
      algorithm,
      MaskCompositeColor::SourceOnly,
      mode,
      combined_mask,
    );
  } else if let Some(inverse) = inverse {
    composite_masked_source(
      pixmap,
      &mask,
      placement,
      image,
      inverse,
      Point { x: 0.5, y: 0.5 },
      algorithm,
      MaskCompositeColor::SourceOnly,
      mode,
      combined_mask,
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
  if border.is_zero() && transform.only_translation() {
    blit_sampled_rgba_translation(
      pixmap,
      source,
      Size { width, height },
      transform.decompose_translation(),
      logical_to_source,
      algorithm,
      mode,
      combined_mask,
    );
    return;
  }

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
  let mut paths = Vec::new();
  border.append_mask_commands(&mut paths, size.map(|v| v as f32), Point::ZERO);
  let (mask, placement) = render_mask(&paths, Some(transform), None, buffer_pool);

  let inverse = transform.invert();
  if transform.is_identity() && placement.left >= 0 && placement.top >= 0 {
    composite_masked_source(
      pixmap,
      &mask,
      placement,
      image,
      logical_to_source,
      Point { x: 0.5, y: 0.5 },
      algorithm,
      MaskCompositeColor::SourceOnly,
      mode,
      combined_mask,
    );
  } else if let Some(inverse) = inverse {
    let combined_inverse = logical_to_source * inverse;
    composite_masked_source(
      pixmap,
      &mask,
      placement,
      image,
      combined_inverse,
      Point { x: 0.5, y: 0.5 },
      algorithm,
      MaskCompositeColor::SourceOnly,
      mode,
      combined_mask,
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
  let mask_data = combined_mask.map(TinyMask::data);
  for dest_y in dest_y_min..dest_y_max {
    let src_y = (dest_y - offset_y) as u32;
    for dest_x in dest_x_min..dest_x_max {
      let src_x = (dest_x - offset_x) as u32;
      let mut src = premultiplied_from_pixel(gradient.sample_pixel(src_x, src_y));
      if src[3] == 0 {
        continue;
      }

      let dest_x = dest_x as u32;
      let dest_y = dest_y as u32;
      if let Some(mask_data) = mask_data {
        let alpha = mask_data[mask_index_from_coord(dest_x, dest_y, bottom_width)];
        if alpha == 0 {
          continue;
        }
        src = scale_premultiplied_pixel(src, alpha);
        if src[3] == 0 {
          continue;
        }
      }

      let index = (dest_y * bottom_width + dest_x) as usize;
      blend_premultiplied_pixel(&mut pixels[index], src, mode);
    }
  }
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

  fn assert_gradient_overlay_matches_reference<T>(
    tile: &T,
    canvas_size: Size<u32>,
    offset: Point<f32>,
  ) where
    T: GradientOverlayTile,
  {
    let mut fast = RgbaImage::from_pixel(canvas_size.width, canvas_size.height, Rgba([0, 0, 0, 0]));
    let mut reference = fast.clone();

    with_pixmap(&mut fast, |pixmap| {
      overlay_gradient_tile(pixmap, tile, offset, BlendMode::Normal, None);
    });

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
    assert_gradient_overlay_matches_reference(
      &tile,
      Size {
        width: 40,
        height: 24,
      },
      Point { x: 3.0, y: 4.0 },
    );
  }

  #[test]
  fn test_overlay_radial_gradient_matches_reference() {
    let Ok(gradient) = RadialGradient::from_str("radial-gradient(circle, red, blue)") else {
      unreachable!()
    };
    let global_context = GlobalContext::default();
    let render_context = RenderContext::new_test(&global_context, Viewport::new((32, 24)));
    let tile = RadialGradientTile::new(&gradient, 32, 24, &render_context);
    assert_gradient_overlay_matches_reference(
      &tile,
      Size {
        width: 40,
        height: 30,
      },
      Point { x: 4.0, y: 3.0 },
    );
  }

  #[test]
  fn test_overlay_conic_gradient_matches_reference() {
    let Ok(gradient) = ConicGradient::from_str("conic-gradient(red, blue)") else {
      unreachable!()
    };

    let global_context = GlobalContext::default();
    let render_context = RenderContext::new_test(&global_context, Viewport::new((32, 24)));
    let tile = ConicGradientTile::new(&gradient, 32, 24, &render_context);
    assert_gradient_overlay_matches_reference(
      &tile,
      Size {
        width: 40,
        height: 30,
      },
      Point { x: 4.0, y: 3.0 },
    );
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
    assert_gradient_overlay_matches_reference(
      &tile,
      Size {
        width: 40,
        height: 24,
      },
      Point { x: 3.0, y: 4.0 },
    );
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
      unreachable!()
    };
    let global_context = GlobalContext::default();
    let render_context = RenderContext::new_test(&global_context, Viewport::new((32, 24)));
    let tile = RadialGradientTile::new(&gradient, 32, 24, &render_context);
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
