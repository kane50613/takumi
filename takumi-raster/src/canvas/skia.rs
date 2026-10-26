//! tiny-skia fast paths for image draws and border-clipped fills.

use takumi_core::geometry::{Point, Size};
use tiny_skia::{
  BlendMode as TinyBlendMode, FillRule as TinyFillRule, Paint as TinyPaint, Path as TinyPath,
  Pattern as TinyPattern, PixmapPaint, SpreadMode as TinySpreadMode,
};

use super::{DrawTarget, PaintSource};
use crate::{
  BorderProperties, ColorTile, Command,
  style::{Affine, BlendMode, ImageScalingAlgorithm},
};

#[derive(Clone, Copy)]
pub(crate) struct ImagePathFillOptions {
  pub content_size: Size<u32>,
  pub border: BorderProperties,
  pub transform: Affine,
  pub source_to_canvas: Affine,
  pub algorithm: ImageScalingAlgorithm,
  pub mode: BlendMode,
}

#[derive(Clone, Copy)]
pub(crate) struct FillColorOptions<'a> {
  pub color: &'a ColorTile,
  pub content_size: Size<u32>,
  pub border: BorderProperties,
  pub transform: Affine,
  pub mode: BlendMode,
}

pub(crate) fn try_draw_image_with_tiny_skia(
  target: &mut DrawTarget,
  image: PaintSource<'_>,
  transform: Affine,
  algorithm: ImageScalingAlgorithm,
  mode: BlendMode,
) -> bool {
  let Ok(blend_mode) = TinyBlendMode::try_from(mode) else {
    return false;
  };

  let paint = PixmapPaint {
    opacity: 1.0,
    blend_mode,
    quality: algorithm.into(),
  };
  let resolved_mask = target.resolve_combined_mask();
  let combined_mask = resolved_mask.as_deref();

  image
    .with_pixmap_ref(|source_pixmap| {
      target
        .pixmap
        .draw_pixmap(0, 0, source_pixmap, &paint, transform.into(), combined_mask);
      true
    })
    .unwrap_or(false)
}

pub(crate) fn try_fill_color_with_tiny_skia(
  target: &mut DrawTarget,
  options: FillColorOptions<'_>,
) -> bool {
  let Ok(blend_mode) = TinyBlendMode::try_from(options.mode) else {
    return false;
  };
  let Some(path) = build_border_path(options.border, options.content_size) else {
    return false;
  };

  let mut paint = TinyPaint::default();
  let [red, green, blue, alpha] = options.color.color().0;
  paint.set_color_rgba8(red, green, blue, alpha);
  paint.blend_mode = blend_mode;
  paint.anti_alias = true;
  let resolved_mask = target.resolve_combined_mask();
  let combined_mask = resolved_mask.as_deref();
  target.pixmap.fill_path(
    &path,
    &paint,
    TinyFillRule::Winding,
    options.transform.into(),
    combined_mask,
  );

  true
}

pub(crate) fn try_fill_image_path_with_tiny_skia(
  target: &mut DrawTarget,
  image: PaintSource<'_>,
  options: ImagePathFillOptions,
) -> bool {
  let Ok(blend_mode) = TinyBlendMode::try_from(options.mode) else {
    return false;
  };
  let Some(path) = build_border_path(options.border, options.content_size) else {
    return false;
  };
  let resolved_mask = target.resolve_combined_mask();
  let combined_mask = resolved_mask.as_deref();

  image
    .with_pixmap_ref(|source_pixmap| {
      let paint = TinyPaint {
        shader: TinyPattern::new(
          source_pixmap,
          TinySpreadMode::Pad,
          options.algorithm.into(),
          1.0,
          options.source_to_canvas.into(),
        ),
        blend_mode,
        anti_alias: true,
        ..Default::default()
      };

      target.pixmap.fill_path(
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
  Command::build_path(&commands)
}
