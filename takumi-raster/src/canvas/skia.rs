//! tiny-skia fast paths for image draws and border-clipped fills.

use takumi_core::geometry::{Point, Size};
use tiny_skia::{
  FillRule as TinyFillRule, FilterQuality as TinyFilterQuality, Paint as TinyPaint,
  Path as TinyPath, Pattern as TinyPattern, PixmapPaint, SpreadMode as TinySpreadMode,
};

use super::{DrawTarget, PaintSource};
use crate::{
  BorderProperties, ColorTile, build_path,
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

pub(crate) fn to_tiny_blend_mode(mode: BlendMode) -> Option<tiny_skia::BlendMode> {
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
    _ => return None,
  })
}

pub(crate) fn to_tiny_filter_quality(algorithm: ImageScalingAlgorithm) -> TinyFilterQuality {
  match algorithm {
    ImageScalingAlgorithm::Pixelated => TinyFilterQuality::Nearest,
    _ => TinyFilterQuality::Bilinear,
  }
}

pub(crate) fn try_draw_image_with_tiny_skia(
  target: &mut DrawTarget,
  image: PaintSource<'_>,
  transform: Affine,
  algorithm: ImageScalingAlgorithm,
  mode: BlendMode,
) -> bool {
  let Some(blend_mode) = to_tiny_blend_mode(mode) else {
    return false;
  };

  let paint = PixmapPaint {
    opacity: 1.0,
    blend_mode,
    quality: to_tiny_filter_quality(algorithm),
  };
  let materialized_mask = target.materialize_combined_mask();
  let combined_mask = materialized_mask.as_ref();

  image
    .with_pixmap_ref(target.buffer_pool, |source_pixmap| {
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
  let Some(blend_mode) = to_tiny_blend_mode(options.mode) else {
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
  let materialized_mask = target.materialize_combined_mask();
  let combined_mask = materialized_mask.as_ref();
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
  let Some(blend_mode) = to_tiny_blend_mode(options.mode) else {
    return false;
  };
  let Some(path) = build_border_path(options.border, options.content_size) else {
    return false;
  };
  let materialized_mask = target.materialize_combined_mask();
  let combined_mask = materialized_mask.as_ref();

  image
    .with_pixmap_ref(target.buffer_pool, |source_pixmap| {
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
  build_path(&commands)
}
