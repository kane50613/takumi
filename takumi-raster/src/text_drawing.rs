use skrifa::color::ColorPalette;
use takumi_core::geometry::{Point, Size};
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint};
use xxhash_rust::xxh3::Xxh3;

use crate::{
  Canvas, CanvasViewport, Command, Placement, Result, SamplingOptions, SizedFontStyle, Stroke,
  checked_area, cull_bounds, pixmap_ref_from_buffer, render_mask,
  resources::{
    glyph::{ResolvedBitmapGlyph, ResolvedColorLayer, ResolvedGlyph},
    glyph_cache::glyph_mask,
  },
  style::{Affine, BlendMode, Color},
};

/// A glyph transform split as Skia's subpixel positioning splits it: the mask is rasterized
/// under the linear part at a quarter-pixel origin, then blitted at whole pixels.
#[derive(Clone, Copy)]
struct GlyphBucket {
  mask_transform: Affine,
  offset: (i32, i32),
}

impl GlyphBucket {
  /// Snaps the origin to a quarter pixel along the baseline and a whole pixel across it, or to
  /// a quarter pixel on both axes when the baseline is neither horizontal nor vertical
  /// (`SkScalerContextRec::computeAxisAlignmentForHText`).
  fn of(transform: Affine) -> Self {
    let (x_steps, y_steps) = if transform.b == 0.0 {
      (4, 1)
    } else if transform.a == 0.0 {
      (1, 4)
    } else {
      (4, 4)
    };
    let (int_x, x) = split_subpixel(transform.x, x_steps);
    let (int_y, y) = split_subpixel(transform.y, y_steps);

    Self {
      mask_transform: Affine { x, y, ..transform },
      offset: (int_x, int_y),
    }
  }

  /// `transform` with its origin snapped.
  fn snapped(self) -> Affine {
    Affine {
      x: self.mask_transform.x + self.offset.0 as f32,
      y: self.mask_transform.y + self.offset.1 as f32,
      ..self.mask_transform
    }
  }

  /// Identifies the mask by everything that changes its pixels: the outline, the transform it
  /// is rasterized under, and the stroke applied to it.
  fn mask_key(self, glyph_signature: u64, stroke: Option<Stroke>) -> u64 {
    let Affine { a, b, c, d, x, y } = self.mask_transform;
    let mut hasher = Xxh3::new();
    hasher.update(&glyph_signature.to_le_bytes());

    for value in [a, b, c, d, x, y] {
      hasher.update(&value.to_bits().to_le_bytes());
    }

    match stroke {
      Some(stroke) => {
        hasher.update(&[1, stroke.join as u8, stroke.cap as u8]);
        hasher.update(&stroke.width.to_le_bytes());
      }
      None => hasher.update(&[0]),
    }

    hasher.digest()
  }

  fn render_mask(self, paths: &[Command], stroke: Option<Stroke>) -> (Vec<u8>, Placement) {
    render_mask(
      paths,
      Some(self.mask_transform),
      stroke.map(Into::into),
      None,
    )
  }
}

/// `value` rounded to `1 / steps` of a pixel, as whole pixels and the remaining fraction.
fn split_subpixel(value: f32, steps: i64) -> (i32, f32) {
  let scaled = (value * steps as f32).round() as i64;

  (
    scaled.div_euclid(steps) as i32,
    scaled.rem_euclid(steps) as f32 / steps as f32,
  )
}

/// Paints `paths` through the mask cache, falling back to a direct rasterization
/// for a dashed stroke, which the cache does not key on.
fn draw_mask_with_cache(
  canvas: &mut Canvas,
  glyph_signature: u64,
  paths: &[Command],
  transform: Affine,
  stroke: Option<Stroke>,
  color: Color,
) {
  let bucket = GlyphBucket::of(transform);

  if stroke.is_some_and(|stroke| stroke.dash.is_some()) {
    let (mask, placement) = render_mask(
      paths,
      Some(bucket.snapped()),
      stroke.map(Into::into),
      Some(canvas.viewport()),
    );
    canvas.draw_mask(&mask, placement, color, BlendMode::Normal);
    return;
  }

  let (mask, placement) = glyph_mask(bucket.mask_key(glyph_signature, stroke), || {
    bucket.render_mask(paths, stroke)
  });
  canvas.draw_mask(
    &mask,
    placement.translate(bucket.offset.0, bucket.offset.1),
    color,
    BlendMode::Normal,
  );
}

struct GlyphPaintCtx<'a, 'b> {
  canvas: &'a mut Canvas,
  style: &'a SizedFontStyle<'a>,
  /// The run's own `-webkit-text-stroke`, which a span may set for itself.
  stroke: (f32, Color),
  transform: Affine,
  paths: &'b [Command],
  glyph_signature: u64,
}

impl GlyphPaintCtx<'_, '_> {
  /// A stroke of `width` joined the way the text's `stroke-linejoin` asks.
  fn stroke_of(&self, width: f32) -> Stroke {
    let mut stroke = Stroke::new(width);
    stroke.join = self.style.parent.stroke_linejoin.into();
    stroke
  }

  /// The `-webkit-text-stroke`, in glyph space so transforms and `text-fit` scale it.
  fn text_stroke(&self) -> Stroke {
    self.stroke_of(self.stroke.0)
  }

  fn draw_text_stroke(&mut self) {
    if self.stroke.0 <= 0.0 {
      return;
    }

    self.draw_stroke(self.text_stroke(), self.stroke.1);
  }

  fn draw_embolden(&mut self, embolden: f32, color: Color) {
    if embolden <= 0.0 {
      return;
    }

    self.draw_stroke(self.stroke_of(embolden), color);
  }

  fn draw_stroke(&mut self, stroke: Stroke, color: Color) {
    draw_mask_with_cache(
      self.canvas,
      self.glyph_signature,
      self.paths,
      self.transform,
      Some(stroke),
      color,
    );
  }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_glyph(
  glyph: &ResolvedGlyph,
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  stroke: (f32, Color),
  mut transform: Affine,
  inline_offset: Point<f32>,
  color: Color,
  palette: Option<&ColorPalette>,
) -> Result<()> {
  transform *= Affine::translation(inline_offset.x, inline_offset.y);

  match glyph {
    ResolvedGlyph::Bitmap(bitmap) => {
      let Some(source) = pixmap_ref_from_buffer(&bitmap.image) else {
        return Ok(());
      };
      transform *= bitmap.image_transform();
      canvas.overlay_sampled_pixmap(
        source,
        Size {
          width: source.width(),
          height: source.height(),
        },
        Default::default(),
        transform,
        SamplingOptions {
          logical_to_source: Affine::IDENTITY,
          algorithm: Default::default(),
        },
        BlendMode::Normal,
      );
    }
    ResolvedGlyph::Outline(outline) => {
      if let Some(color_layers) = outline.color_layers()
        && let Some(palette) = palette
      {
        draw_color_outline_image(canvas, color_layers, palette, color, transform);
      } else {
        draw_mask_with_cache(
          canvas,
          outline.cache_signature(),
          outline.paths(),
          transform,
          None,
          color,
        );
      }

      let mut ctx = GlyphPaintCtx {
        canvas,
        style,
        stroke,
        transform,
        paths: outline.paths(),
        glyph_signature: outline.cache_signature(),
      };

      if let Some(embolden) = outline.embolden() {
        ctx.draw_embolden(embolden, color);
      }

      ctx.draw_text_stroke();
    }
  }

  Ok(())
}

fn draw_color_outline_image(
  canvas: &mut Canvas,
  color_layers: &[ResolvedColorLayer],
  palette: &ColorPalette,
  foreground_color: Color,
  transform: Affine,
) {
  let foreground_opacity = foreground_color.0[3] as f32 / 255.0;
  if foreground_opacity <= 0.0 {
    return;
  }

  for layer in color_layers {
    let color = if layer.palette_index == u16::MAX {
      let alpha = (foreground_opacity * layer.alpha * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
      Color([
        foreground_color.0[0],
        foreground_color.0[1],
        foreground_color.0[2],
        alpha,
      ])
    } else {
      let Some(record) = palette.colors().get(usize::from(layer.palette_index)) else {
        continue;
      };
      let alpha = ((record.alpha() as f32 / 255.0) * layer.alpha * foreground_opacity * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
      Color([record.red(), record.green(), record.blue(), alpha])
    };

    let (mask, placement) =
      render_mask(&layer.paths, Some(transform), None, Some(canvas.viewport()));
    canvas.draw_mask(&mask, placement, color, BlendMode::Normal);
  }
}

/// The alpha coverage `bitmap` leaves on the canvas drawn under `transform`, within `cull`.
pub(crate) fn bitmap_coverage(
  bitmap: &ResolvedBitmapGlyph,
  transform: Affine,
  cull: CanvasViewport,
) -> Option<(Vec<u8>, Placement)> {
  let source = pixmap_ref_from_buffer(&bitmap.image)?;
  let transform = transform * bitmap.image_transform();
  let (width, height) = (source.width() as f32, source.height() as f32);
  let (mut min_x, mut min_y) = (f32::INFINITY, f32::INFINITY);
  let (mut max_x, mut max_y) = (f32::NEG_INFINITY, f32::NEG_INFINITY);

  for (x, y) in [(0.0, 0.0), (width, 0.0), (0.0, height), (width, height)] {
    let (x, y) = transform.transform_point(x, y);

    min_x = min_x.min(x);
    min_y = min_y.min(y);
    max_x = max_x.max(x);
    max_y = max_y.max(y);
  }

  let placement = cull_bounds(
    [
      min_x.floor() as i32,
      min_y.floor() as i32,
      max_x.ceil() as i32,
      max_y.ceil() as i32,
    ],
    Some(cull),
  )?;

  checked_area(placement.width, placement.height, 4)?;

  let mut pixmap = Pixmap::new(placement.width, placement.height)?;

  pixmap.draw_pixmap(
    0,
    0,
    source,
    &PixmapPaint {
      quality: FilterQuality::Bilinear,
      ..PixmapPaint::default()
    },
    (Affine::translation(-placement.left as f32, -placement.top as f32) * transform).into(),
    None,
  );

  Some((
    pixmap.data().iter().skip(3).step_by(4).copied().collect(),
    placement,
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn bucket_zero_mask_matches_untransformed_render() {
    let paths = vec![
      Command::MoveTo(Point::new(1.5, 1.0)),
      Command::LineTo(Point::new(9.0, 2.25)),
      Command::QuadTo(Point::new(11.0, 7.0), Point::new(4.0, 10.5)),
      Command::Close,
    ];

    let (untransformed, untransformed_placement) = render_mask(&paths, None, None, None);
    let (bucket_zero, bucket_zero_placement) =
      GlyphBucket::of(Affine::IDENTITY).render_mask(&paths, None);

    assert_eq!(untransformed, bucket_zero);
    assert_eq!(untransformed_placement, bucket_zero_placement);
  }

  #[test]
  fn a_bucket_snaps_the_origin_along_the_baseline() {
    let split = |transform: Affine| {
      let bucket = GlyphBucket::of(transform);

      (
        bucket.offset,
        (bucket.mask_transform.x, bucket.mask_transform.y),
      )
    };
    let rotated = Affine {
      a: 0.96,
      b: 0.26,
      c: -0.26,
      d: 0.96,
      x: -0.2,
      y: 5.6,
    };
    let vertical = Affine {
      a: 0.0,
      b: 1.0,
      c: -1.0,
      d: 0.0,
      x: 3.3,
      y: 7.3,
    };

    assert_eq!(
      split(Affine::translation(10.3, 5.6)),
      ((10, 6), (0.25, 0.0))
    );
    assert_eq!(split(rotated), ((-1, 5), (0.75, 0.5)));
    assert_eq!(split(vertical), ((3, 7), (0.0, 0.25)));
  }
}
