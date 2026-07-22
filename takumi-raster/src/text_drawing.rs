use std::{
  convert::Into,
  sync::{Arc, LazyLock},
};

use skrifa::color::ColorPalette;
use takumi_core::geometry::{ComputedLayout as Layout, Point, Size};
use tiny_skia::Pixmap;

use crate::{
  BorderProperties, Canvas, ColorTile, Command, MaskCompositeColor, MaskSamplingOptions,
  PaintSource, Placement, Result, SamplingOptions, SizedFontStyle, Stroke,
  composite_mask_source_to_pixmap, draw_outset_shadow,
  layout::inline::ShapedRun,
  pixmap_ref_from_buffer, render_mask,
  resources::{
    glyph::{ResolvedColorLayer, ResolvedGlyph},
    glyph_cache::{GlyphCache, glyph_cache_share_bytes},
  },
  style::{Affine, BlendMode, Color, ImageScalingAlgorithm},
  uninit_buffer,
};

type GlyphMaskCache = GlyphCache<(Arc<Vec<u8>>, Placement)>;

static SHARED_GLYPH_MASK_CACHE: LazyLock<GlyphMaskCache> =
  LazyLock::new(|| GlyphCache::new(glyph_cache_share_bytes()));

/// Rasterizes `paths` at the subpixel bucket encoded in the low bits of `key`.
fn render_bucket_mask(key: u64, paths: &[Command]) -> (Vec<u8>, Placement) {
  let bucket_x = (key & 3) as f32 * 0.25;

  render_mask(paths, Some(Affine::translation(bucket_x, 0.0)), None)
}

/// Fetches the cached mask for `key`, rasterizing it on a miss; concurrent
/// misses for the same key rasterize once. Charges the capacity the cache
/// retains, not just the mask length.
fn get_or_render_cached_mask(key: u64, paths: &[Command]) -> (Arc<Vec<u8>>, Placement) {
  let cached = SHARED_GLYPH_MASK_CACHE.get_or_insert_with(key, || {
    let (mask, placement) = render_bucket_mask(key, paths);
    let bytes = mask.capacity() + 64;

    Some(((Arc::new(mask), placement), bytes))
  });

  match cached {
    Some(entry) => entry,
    None => {
      let (mask, placement) = render_bucket_mask(key, paths);

      (Arc::new(mask), placement)
    }
  }
}

fn glyph_cache_key_and_offset(transform: Affine, glyph_signature: u64) -> Option<(u64, i32, i32)> {
  if !transform.only_translation() {
    return None;
  }
  let scaled_x = (transform.x * 4.0).round() as i64;
  let int_x = scaled_x.div_euclid(4) as i32;
  let bucket_x = scaled_x.rem_euclid(4) as u64;
  let int_y = transform.y.round() as i32;
  let key = (glyph_signature << 2) | bucket_x;
  Some((key, int_x, int_y))
}

fn draw_outline_with_cache(
  paths: &[Command],
  glyph_signature: u64,
  transform: Affine,
  color: Color,
  canvas: &mut Canvas,
) {
  let Some((key, int_x, int_y)) = glyph_cache_key_and_offset(transform, glyph_signature) else {
    let (mask, placement) = render_mask(paths, Some(transform), None);
    canvas.draw_mask(&mask, placement, color, BlendMode::Normal);
    return;
  };
  let (mask, cached_placement) = get_or_render_cached_mask(key, paths);
  canvas.draw_mask(
    &mask,
    cached_placement.translate(int_x, int_y),
    color,
    BlendMode::Normal,
  );
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
  let end_x = start_x + glyph_run.advance;
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
  transform: Affine,
  inline_offset: Point<f32>,
  paths: &'b [Command],
}

pub(crate) fn draw_glyph_clip_image(
  glyph: &ResolvedGlyph,
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  mut transform: Affine,
  inline_offset: Point<f32>,
  clip_image: PaintSource<'_>,
) -> Result<()> {
  transform *= Affine::translation(inline_offset.x, inline_offset.y);

  match glyph {
    ResolvedGlyph::Bitmap(bitmap) => {
      transform *= Affine::translation(bitmap.placement.left as f32, -bitmap.placement.top as f32);

      let mask_capacity = (bitmap.placement.width * bitmap.placement.height) as usize;
      let mut mask = uninit_buffer(mask_capacity);
      if mask_capacity > 0 {
        let mask_len = mask.len();
        let write_len = mask_capacity.min(mask_len);
        bitmap.write_alpha_mask(&mut mask[..write_len]);
      }

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
          sample_bias: Point::ZERO,
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
        sample_bias: Point::ZERO,
        algorithm: style.parent.image_rendering,
      };

      if let Some((key, int_x, int_y)) =
        glyph_cache_key_and_offset(transform, outline.cache_signature())
      {
        let (mask, cached_placement) = get_or_render_cached_mask(key, outline.paths());
        canvas.composite_mask_source(
          &mask,
          cached_placement.translate(int_x, int_y),
          clip_image,
          MaskCompositeColor::SourceOnly,
          sampling,
          BlendMode::Normal,
        );
      } else {
        let (mask, placement) = render_mask(outline.paths(), Some(transform), None);
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
        transform,
        inline_offset,
        paths: outline.paths(),
      };

      if let Some(embolden) = outline.embolden() {
        draw_text_embolden_clip_image(&mut ctx, embolden, clip_image);
      }

      draw_text_stroke_clip_image(&mut ctx, clip_image);
    }
  }

  Ok(())
}
pub(crate) fn draw_glyph(
  glyph: &ResolvedGlyph,
  canvas: &mut Canvas,
  style: &SizedFontStyle,
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
        transform,
        inline_offset,
        paths: outline.paths(),
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
  if ctx.style.stroke_width <= 0.0 {
    return;
  }

  let Some(inverse) = ctx.transform.invert() else {
    return;
  };

  let scale = ctx.transform.uniform_scale().max(f32::EPSILON);
  let mut stroke = Stroke::new(ctx.style.stroke_width / scale);
  stroke.join = ctx.style.parent.stroke_linejoin.into();

  let (stroke_mask, stroke_placement) =
    render_mask(ctx.paths, Some(ctx.transform), Some(stroke.into()));

  ctx.canvas.composite_mask_source(
    &stroke_mask,
    stroke_placement,
    clip_image,
    MaskCompositeColor::color_over_source(ctx.style.text_stroke_color),
    MaskSamplingOptions {
      canvas_to_source: Affine::translation(ctx.inline_offset.x, ctx.inline_offset.y) * inverse,
      sample_bias: Point::ZERO,
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

  let (stroke_mask, stroke_placement) =
    render_mask(ctx.paths, Some(ctx.transform), Some(stroke.into()));

  ctx.canvas.composite_mask_source(
    &stroke_mask,
    stroke_placement,
    clip_image,
    MaskCompositeColor::SourceOnly,
    MaskSamplingOptions {
      canvas_to_source: Affine::translation(ctx.inline_offset.x, ctx.inline_offset.y) * inverse,
      sample_bias: Point::ZERO,
      algorithm: ctx.style.parent.image_rendering,
    },
    BlendMode::Normal,
  );
}

fn draw_text_stroke(ctx: &mut GlyphPaintCtx<'_, '_>) {
  if ctx.style.stroke_width <= 0.0 {
    return;
  }

  let scale = ctx.transform.uniform_scale().max(f32::EPSILON);
  let mut stroke = Stroke::new(ctx.style.stroke_width / scale);
  stroke.join = ctx.style.parent.stroke_linejoin.into();

  let (stroke_mask, stroke_placement) =
    render_mask(ctx.paths, Some(ctx.transform), Some(stroke.into()));

  ctx.canvas.draw_mask(
    &stroke_mask,
    stroke_placement,
    ctx.style.text_stroke_color,
    BlendMode::Normal,
  );
}

fn draw_text_embolden(ctx: &mut GlyphPaintCtx<'_, '_>, color: Color, embolden: f32) {
  if embolden <= 0.0 {
    return;
  }

  let mut stroke = Stroke::new(embolden);
  stroke.join = ctx.style.parent.stroke_linejoin.into();

  let (stroke_mask, stroke_placement) =
    render_mask(ctx.paths, Some(ctx.transform), Some(stroke.into()));

  ctx
    .canvas
    .draw_mask(&stroke_mask, stroke_placement, color, BlendMode::Normal);
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

  for shadow in style.text_shadow.iter() {
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

    let (mask, placement) = render_mask(&layer.paths, Some(transform), None);
    canvas.draw_mask(&mask, placement, color, BlendMode::Normal);
  }
}
