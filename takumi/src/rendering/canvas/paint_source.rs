use tiny_skia::{PixmapRef, PremultipliedColorU8};

use crate::{
  layout::style::ImageScalingAlgorithm,
  rendering::{BackgroundTile, ColorTile},
};

use super::BufferPool;
use crate::rendering::blend::premultiplied_from_pixel;

#[derive(Clone, Copy)]
pub(crate) enum PaintSource<'a> {
  Pixmap(PixmapRef<'a>),
  BackgroundTile(&'a BackgroundTile),
  ColorTile(&'a ColorTile),
}

impl<'a> PaintSource<'a> {
  pub(crate) fn width(self) -> u32 {
    match self {
      Self::Pixmap(pixmap) => pixmap.width(),
      Self::BackgroundTile(tile) => tile.width(),
      Self::ColorTile(tile) => tile.width(),
    }
  }

  pub(crate) fn height(self) -> u32 {
    match self {
      Self::Pixmap(pixmap) => pixmap.height(),
      Self::BackgroundTile(tile) => tile.height(),
      Self::ColorTile(tile) => tile.height(),
    }
  }

  pub(crate) fn get_pixel(self, x: u32, y: u32) -> PremultipliedColorU8 {
    match self {
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
      Self::Pixmap(source) => Some(source),
      Self::BackgroundTile(BackgroundTile::Pixmap(source)) => Some(source.as_ref().as_ref()),
      _ => None,
    }
  }

  pub(crate) fn premultiplied_constant(self) -> Option<[u8; 4]> {
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

  pub(crate) fn with_pixmap_ref<R>(
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
    let mut premultiplied = buffer_pool.acquire_dirty(source_len);
    self.write_premultiplied(&mut premultiplied);
    let result = PixmapRef::from_bytes(&premultiplied, width, height).map(f);
    buffer_pool.release(premultiplied);
    result
  }

  pub(crate) fn supports_rounded_fill_fast_path(self) -> bool {
    matches!(self, Self::Pixmap(_))
  }
}

impl<'a> From<PixmapRef<'a>> for PaintSource<'a> {
  fn from(value: PixmapRef<'a>) -> Self {
    Self::Pixmap(value)
  }
}

impl<'a> From<&'a tiny_skia::Pixmap> for PaintSource<'a> {
  fn from(value: &'a tiny_skia::Pixmap) -> Self {
    Self::Pixmap(value.as_ref())
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

#[derive(Clone, Copy)]
pub(super) enum MaskCompositeColor {
  SourceOnly,
  SourceOverColor([u8; 4]),
  ColorOverSource([u8; 4]),
}

#[inline(always)]
pub(super) fn apply_mask_color_mode(src: [u8; 4], color_mode: MaskCompositeColor) -> [u8; 4] {
  match color_mode {
    MaskCompositeColor::SourceOnly => src,
    MaskCompositeColor::SourceOverColor(color) => {
      let mut out = color;
      super::composite_premultiplied_over(&mut out, src);
      out
    }
    MaskCompositeColor::ColorOverSource(color) => {
      let mut out = src;
      super::composite_premultiplied_over(&mut out, color);
      out
    }
  }
}

#[inline(always)]
pub(super) fn sample_paint_source(
  source: PaintSource<'_>,
  algorithm: ImageScalingAlgorithm,
  x: f32,
  y: f32,
) -> Option<[u8; 4]> {
  match source {
    PaintSource::Pixmap(pixmap) => {
      if matches!(algorithm, ImageScalingAlgorithm::Pixelated) {
        super::sample_pixmap_nearest(pixmap, x, y)
      } else {
        super::sample_pixmap_bilinear(pixmap, x, y)
      }
    }
    _ if matches!(algorithm, ImageScalingAlgorithm::Pixelated) => {
      interpolate_nearest(source, x, y).map(premultiplied_from_pixel)
    }
    _ => interpolate_bilinear(source, x, y).map(premultiplied_from_pixel),
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

  let px = x.floor().max(0.0) as u32;
  let px = px.min(w.saturating_sub(1));
  let py = y.floor().max(0.0) as u32;
  let py = py.min(h.saturating_sub(1));

  Some(image.get_pixel(px, py))
}

#[inline(always)]
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

  let x = (x - 0.5).clamp(0.0, w.saturating_sub(1) as f32);
  let y = (y - 0.5).clamp(0.0, h.saturating_sub(1) as f32);

  let uf = x.floor() as u32;
  let vf = y.floor() as u32;
  let uc = (uf + 1).min(w.saturating_sub(1));
  let vc = (vf + 1).min(h.saturating_sub(1));

  let p00 = image.get_pixel(uf, vf);
  if uf == uc && vf == vc {
    return Some(p00);
  }

  let u_ratio = ((x - uf as f32) * 256.0) as u32;
  let v_ratio = ((y - vf as f32) * 256.0) as u32;
  if u_ratio == 0 && v_ratio == 0 {
    return Some(p00);
  }

  let p01 = image.get_pixel(uf, vc);
  let p10 = image.get_pixel(uc, vf);
  let p11 = image.get_pixel(uc, vc);

  let u_opposite = 256 - u_ratio;
  let v_opposite = 256 - v_ratio;

  let w00 = u_opposite * v_opposite;
  let w01 = u_opposite * v_ratio;
  let w10 = u_ratio * v_opposite;
  let w11 = u_ratio * v_ratio;

  PremultipliedColorU8::from_rgba(
    ((p00.red() as u32 * w00
      + p10.red() as u32 * w10
      + p01.red() as u32 * w01
      + p11.red() as u32 * w11)
      >> 16) as u8,
    ((p00.green() as u32 * w00
      + p10.green() as u32 * w10
      + p01.green() as u32 * w01
      + p11.green() as u32 * w11)
      >> 16) as u8,
    ((p00.blue() as u32 * w00
      + p10.blue() as u32 * w10
      + p01.blue() as u32 * w01
      + p11.blue() as u32 * w11)
      >> 16) as u8,
    ((p00.alpha() as u32 * w00
      + p10.alpha() as u32 * w10
      + p01.alpha() as u32 * w01
      + p11.alpha() as u32 * w11)
      >> 16) as u8,
  )
}
