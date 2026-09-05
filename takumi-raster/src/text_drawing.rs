use std::{convert::Into, sync::Arc};

use skrifa::color::ColorPalette;
use takumi_core::geometry::{ComputedLayout as Layout, Point, Size};
use tiny_skia::Pixmap;
use xxhash_rust::xxh3::Xxh3;

use crate::{
  BorderProperties, Canvas, ColorTile, Command, MaskCompositeColor, MaskSamplingOptions,
  PaintSource, Placement, Result, SamplingOptions, SizedFontStyle, Stroke, checked_area,
  composite_mask_source_to_pixmap, draw_outset_shadow,
  layout::inline::ShapedRun,
  pixmap_ref_from_buffer, render_mask,
  resources::{
    glyph::{ResolvedColorLayer, ResolvedGlyph},
    glyph_cache::glyph_mask,
  },
  style::{Affine, BlendMode, Color, ImageScalingAlgorithm},
};

/// Identifies a mask by everything that changes its pixels: the outline, the
/// subpixel bucket it is rasterized at, and the stroke applied to it.
fn mask_key(glyph_signature: u64, bucket_x: u64, stroke: Option<Stroke>) -> u64 {
  let mut hasher = Xxh3::new();
  hasher.update(&glyph_signature.to_le_bytes());
  hasher.update(&bucket_x.to_le_bytes());

  match stroke {
    Some(stroke) => {
      hasher.update(&[1, stroke.join as u8, stroke.cap as u8]);
      hasher.update(&stroke.width.to_le_bytes());
    }
    None => hasher.update(&[0]),
  }

  hasher.digest()
}

fn render_bucket_mask(
  bucket_x: u64,
  paths: &[Command],
  stroke: Option<Stroke>,
) -> (Vec<u8>, Placement) {
  let offset = bucket_x as f32 * 0.25;

  render_mask(
    paths,
    Some(Affine::translation(offset, 0.0)),
    stroke.map(Into::into),
    None,
  )
}

/// Fetches the cached mask, rasterizing it on a miss; concurrent misses for the
/// same mask rasterize once.
fn cached_mask(
  glyph_signature: u64,
  bucket_x: u64,
  paths: &[Command],
  stroke: Option<Stroke>,
) -> (Arc<Vec<u8>>, Placement) {
  glyph_mask(mask_key(glyph_signature, bucket_x, stroke), || {
    render_bucket_mask(bucket_x, paths, stroke)
  })
}

/// Splits a translation into the subpixel bucket baked into the mask and the
/// whole pixels the blit offsets by. `None` when the transform is not a pure
/// translation, which the cache cannot represent.
fn glyph_cache_bucket_and_offset(transform: Affine) -> Option<(u64, i32, i32)> {
  if !transform.only_translation() {
    return None;
  }
  let scaled_x = (transform.x * 4.0).round() as i64;
  let int_x = scaled_x.div_euclid(4) as i32;
  let bucket_x = scaled_x.rem_euclid(4) as u64;
  let int_y = transform.y.round() as i32;
  Some((bucket_x, int_x, int_y))
}

/// Paints `paths` through the mask cache, falling back to a direct rasterization
/// when the transform or the stroke is outside what the cache keys on.
fn draw_mask_with_cache(
  paths: &[Command],
  glyph_signature: u64,
  transform: Affine,
  stroke: Option<Stroke>,
  color: Color,
  canvas: &mut Canvas,
) {
  let cacheable = stroke.is_none_or(|stroke| stroke.dash.is_none());
  let bucket = cacheable
    .then(|| glyph_cache_bucket_and_offset(transform))
    .flatten();

  let Some((bucket_x, int_x, int_y)) = bucket else {
    let (mask, placement) = render_mask(
      paths,
      Some(transform),
      stroke.map(Into::into),
      Some(canvas.viewport()),
    );
    canvas.draw_mask(&mask, placement, color, BlendMode::Normal);
    return;
  };

  let (mask, cached_placement) = cached_mask(glyph_signature, bucket_x, paths, stroke);
  canvas.draw_mask(
    &mask,
    cached_placement.translate(int_x, int_y),
    color,
    BlendMode::Normal,
  );
}

fn draw_outline_with_cache(
  paths: &[Command],
  glyph_signature: u64,
  transform: Affine,
  color: Color,
  canvas: &mut Canvas,
) {
  draw_mask_with_cache(paths, glyph_signature, transform, None, color, canvas);
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DecorationSegmentParams {
  pub(crate) offset: f32,
  pub(crate) size: f32,
  pub(crate) start_x: f32,
  pub(crate) end_x: f32,
  pub(crate) layout: Layout,
  pub(crate) transform: Affine,
}

pub(crate) fn draw_decoration(
  canvas: &mut Canvas,
  glyph_run: &ShapedRun,
  color: Color,
  offset: f32,
  size: f32,
  layout: Layout,
  transform: Affine,
) {
  let start_x = layout.border.left + layout.padding.left + glyph_run.offset;
  let end_x = start_x + glyph_run.decorated_advance();
  draw_decoration_segment(
    canvas,
    color,
    DecorationSegmentParams {
      offset,
      size,
      start_x,
      end_x,
      layout,
      transform,
    },
  );
}

pub(crate) fn draw_decoration_segment(
  canvas: &mut Canvas,
  color: Color,
  params: DecorationSegmentParams,
) {
  if params.end_x <= params.start_x {
    return;
  }

  let snapped_start_x = params.start_x.floor();
  let width = (params.end_x.ceil() - snapped_start_x) as u32;

  let tile = ColorTile::new(color, width, params.size as u32);

  canvas.overlay_image(
    &tile,
    BorderProperties::default(),
    params.transform
      * Affine::translation(
        snapped_start_x,
        params.layout.border.top + params.layout.padding.top + params.offset,
      ),
    ImageScalingAlgorithm::Auto,
    BlendMode::Normal,
  );
}

struct GlyphPaintCtx<'a, 'b> {
  canvas: &'a mut Canvas,
  style: &'a SizedFontStyle<'a>,
  /// The run's own `-webkit-text-stroke`, which a span may set for itself.
  stroke: (f32, Color),
  transform: Affine,
  inline_offset: Point<f32>,
  paths: &'b [Command],
  glyph_signature: u64,
}

pub(crate) fn draw_glyph_clip_image(
  glyph: &ResolvedGlyph,
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  stroke: (f32, Color),
  mut transform: Affine,
  inline_offset: Point<f32>,
  clip_image: PaintSource<'_>,
) -> Result<()> {
  transform *= Affine::translation(inline_offset.x, inline_offset.y);

  match glyph {
    ResolvedGlyph::Bitmap(bitmap) => {
      transform *= Affine::translation(bitmap.placement.left as f32, -bitmap.placement.top as f32);

      let Some(mask_capacity) = checked_area(bitmap.placement.width, bitmap.placement.height, 1)
      else {
        return Ok(());
      };
      let mut mask = vec![0; mask_capacity];
      bitmap.write_alpha_mask(&mut mask);

      let Some(mut bottom) = Pixmap::new(bitmap.placement.width, bitmap.placement.height) else {
        return Ok(());
      };
      let mut bottom_pixmap = bottom.as_mut();
      composite_mask_source_to_pixmap(
        &mut bottom_pixmap,
        &mask,
        clip_image,
        Placement {
          left: 0,
          top: 0,
          width: bitmap.placement.width,
          height: bitmap.placement.height,
        },
        MaskSamplingOptions {
          canvas_to_source: Affine::translation(
            inline_offset.x + bitmap.placement.left as f32,
            inline_offset.y - bitmap.placement.top as f32,
          ),
          sample_bias: Point { x: 0.5, y: 0.5 },
          algorithm: ImageScalingAlgorithm::Pixelated,
        },
        BlendMode::Normal,
        None,
      );

      canvas.overlay_sampled_pixmap(
        bottom.as_ref(),
        Size {
          width: bottom.width(),
          height: bottom.height(),
        },
        BorderProperties::default(),
        transform,
        SamplingOptions {
          logical_to_source: Affine::IDENTITY,
          algorithm: ImageScalingAlgorithm::Auto,
        },
        BlendMode::Normal,
      );
    }
    ResolvedGlyph::Outline(outline) => {
      // If the transform is not invertible, we can't draw the glyph
      let Some(inverse) = transform.invert() else {
        return Ok(());
      };

      let sampling = MaskSamplingOptions {
        canvas_to_source: Affine::translation(inline_offset.x, inline_offset.y) * inverse,
        sample_bias: Point { x: 0.5, y: 0.5 },
        algorithm: style.parent.image_rendering,
      };

      if let Some((bucket_x, int_x, int_y)) = glyph_cache_bucket_and_offset(transform) {
        let (mask, cached_placement) =
          cached_mask(outline.cache_signature(), bucket_x, outline.paths(), None);
        canvas.composite_mask_source(
          &mask,
          cached_placement.translate(int_x, int_y),
          clip_image,
          MaskCompositeColor::SourceOnly,
          sampling,
          BlendMode::Normal,
        );
      } else {
        let (mask, placement) = render_mask(
          outline.paths(),
          Some(transform),
          None,
          Some(canvas.viewport()),
        );
        canvas.composite_mask_source(
          &mask,
          placement,
          clip_image,
          MaskCompositeColor::SourceOnly,
          sampling,
          BlendMode::Normal,
        );
      }

      let mut ctx = GlyphPaintCtx {
        canvas,
        style,
        stroke,
        transform,
        inline_offset,
        paths: outline.paths(),
        glyph_signature: outline.cache_signature(),
      };

      if let Some(embolden) = outline.embolden() {
        draw_text_embolden_clip_image(&mut ctx, embolden, clip_image);
      }

      draw_text_stroke_clip_image(&mut ctx, clip_image);
    }
  }

  Ok(())
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
      transform *= Affine::translation(bitmap.placement.left as f32, -bitmap.placement.top as f32);
      transform *= Affine::scale(bitmap.scale_x, bitmap.scale_y);
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
        draw_outline_with_cache(
          outline.paths(),
          outline.cache_signature(),
          transform,
          color,
          canvas,
        );
      }

      let mut ctx = GlyphPaintCtx {
        canvas,
        style,
        stroke,
        transform,
        inline_offset,
        paths: outline.paths(),
        glyph_signature: outline.cache_signature(),
      };

      if let Some(embolden) = outline.embolden() {
        draw_text_embolden(&mut ctx, color, embolden);
      }

      draw_text_stroke(&mut ctx);
    }
  }

  Ok(())
}

fn draw_text_stroke_clip_image(ctx: &mut GlyphPaintCtx<'_, '_>, clip_image: PaintSource<'_>) {
  if ctx.stroke.0 <= 0.0 {
    return;
  }

  let Some(inverse) = ctx.transform.invert() else {
    return;
  };

  let scale = ctx.transform.uniform_scale().max(f32::EPSILON);
  let mut stroke = Stroke::new(ctx.stroke.0 / scale);
  stroke.join = ctx.style.parent.stroke_linejoin.into();

  let (stroke_mask, stroke_placement) = render_mask(
    ctx.paths,
    Some(ctx.transform),
    Some(stroke.into()),
    Some(ctx.canvas.viewport()),
  );

  ctx.canvas.composite_mask_source(
    &stroke_mask,
    stroke_placement,
    clip_image,
    MaskCompositeColor::color_over_source(ctx.stroke.1),
    MaskSamplingOptions {
      canvas_to_source: Affine::translation(ctx.inline_offset.x, ctx.inline_offset.y) * inverse,
      sample_bias: Point { x: 0.5, y: 0.5 },
      algorithm: ctx.style.parent.image_rendering,
    },
    BlendMode::Normal,
  );
}

fn draw_text_embolden_clip_image(
  ctx: &mut GlyphPaintCtx<'_, '_>,
  embolden: f32,
  clip_image: PaintSource<'_>,
) {
  if embolden <= 0.0 {
    return;
  }

  let Some(inverse) = ctx.transform.invert() else {
    return;
  };

  let mut stroke = Stroke::new(embolden);
  stroke.join = ctx.style.parent.stroke_linejoin.into();

  let (stroke_mask, stroke_placement) = render_mask(
    ctx.paths,
    Some(ctx.transform),
    Some(stroke.into()),
    Some(ctx.canvas.viewport()),
  );

  ctx.canvas.composite_mask_source(
    &stroke_mask,
    stroke_placement,
    clip_image,
    MaskCompositeColor::SourceOnly,
    MaskSamplingOptions {
      canvas_to_source: Affine::translation(ctx.inline_offset.x, ctx.inline_offset.y) * inverse,
      sample_bias: Point { x: 0.5, y: 0.5 },
      algorithm: ctx.style.parent.image_rendering,
    },
    BlendMode::Normal,
  );
}

fn draw_text_stroke(ctx: &mut GlyphPaintCtx<'_, '_>) {
  if ctx.stroke.0 <= 0.0 {
    return;
  }

  let scale = ctx.transform.uniform_scale().max(f32::EPSILON);
  let mut stroke = Stroke::new(ctx.stroke.0 / scale);
  stroke.join = ctx.style.parent.stroke_linejoin.into();

  draw_mask_with_cache(
    ctx.paths,
    ctx.glyph_signature,
    ctx.transform,
    Some(stroke),
    ctx.stroke.1,
    ctx.canvas,
  );
}

fn draw_text_embolden(ctx: &mut GlyphPaintCtx<'_, '_>, color: Color, embolden: f32) {
  if embolden <= 0.0 {
    return;
  }

  let mut stroke = Stroke::new(embolden);
  stroke.join = ctx.style.parent.stroke_linejoin.into();

  draw_mask_with_cache(
    ctx.paths,
    ctx.glyph_signature,
    ctx.transform,
    Some(stroke),
    color,
    ctx.canvas,
  );
}

fn draw_text_shadow(
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  transform: Affine,
  paths: &[Command],
) -> Result<()> {
  if style.text_shadow.is_empty() {
    return Ok(());
  }

  for shadow in style.painted_text_shadows() {
    draw_outset_shadow(shadow, canvas, paths, transform, Default::default(), None)?;
  }

  Ok(())
}

pub(crate) fn draw_glyph_text_shadow(
  glyph: &ResolvedGlyph,
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  mut transform: Affine,
  inline_offset: Point<f32>,
) -> Result<()> {
  transform *= Affine::translation(inline_offset.x, inline_offset.y);

  if let ResolvedGlyph::Outline(outline) = glyph {
    draw_text_shadow(canvas, style, transform, outline.paths())?;
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
    let (bucket_zero, bucket_zero_placement) = render_bucket_mask(0, &paths, None);

    assert_eq!(untransformed, bucket_zero);
    assert_eq!(untransformed_placement, bucket_zero_placement);
  }
}
